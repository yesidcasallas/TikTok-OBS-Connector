//! Servidor HTTP temporal para recibir el callback OAuth.
//!
//! - Escucha **solo** en `127.0.0.1`, en un puerto libre que asigna el sistema.
//! - Solo acepta `GET` con cabecera `Host` igual a `127.0.0.1:<puerto>` o
//!   `localhost:<puerto>` (mitiga DNS rebinding) y cabeceras de tamaño acotado.
//! - Acepta **una sola** respuesta válida por intento: la primera que el
//!   proveedor interpreta como código o como fallo cierra el intento.
//! - Se cierra al completar, cancelar o agotar el tiempo; las conexiones
//!   pendientes se abortan y el puerto queda liberado al volver de [`wait`].
//!
//! [`wait`]: LoopbackServer::wait

use crate::error::{AppError, AppResult};
use crate::security::Secret;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;
use zeroize::{Zeroize, ZeroizeOnDrop};

const MAX_HEAD_BYTES: usize = 8 * 1024;
const READ_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_CONNECTIONS: usize = 16;

/// Resultado de interpretar una petición al callback.
pub enum CallbackVerdict {
    /// Código de autorización válido: completa el intento.
    Code(Secret),
    /// El proveedor informa de un fallo: completa el intento con error.
    Failed(AppError),
    /// Petición ajena al flujo (favicon, sondeos…): se ignora.
    Ignore,
}

/// Lógica del proveedor para interpretar los parámetros del callback.
pub trait CallbackInterpreter: Send + Sync {
    fn interpret(&self, path: &str, params: &[(String, String)]) -> CallbackVerdict;
}

pub struct LoopbackServer {
    listener: TcpListener,
    port: u16,
}

impl LoopbackServer {
    /// Abre el servidor en `127.0.0.1` con un puerto libre.
    pub async fn bind() -> AppResult<Self> {
        let listener = TcpListener::bind(("127.0.0.1", 0))
            .await
            .map_err(|e| AppError::AuthServer(e.to_string()))?;
        let port = listener
            .local_addr()
            .map_err(|e| AppError::AuthServer(e.to_string()))?
            .port();
        Ok(LoopbackServer { listener, port })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    /// Espera el callback. Consume el servidor: al volver, el puerto está
    /// cerrado y no queda ninguna conexión abierta.
    pub async fn wait<I>(
        self,
        interpreter: Arc<I>,
        cancel: CancellationToken,
        timeout: Duration,
    ) -> AppResult<Secret>
    where
        I: CallbackInterpreter + ?Sized + 'static,
    {
        let LoopbackServer { listener, port } = self;
        let claimed = Arc::new(AtomicBool::new(false));
        let (tx, mut rx) = mpsc::channel::<CallbackVerdict>(4);
        let mut tasks: JoinSet<()> = JoinSet::new();
        let deadline = tokio::time::sleep(timeout);
        tokio::pin!(deadline);

        let result = loop {
            tokio::select! {
                biased;
                _ = cancel.cancelled() => break Err(AppError::AuthCancelled),
                _ = &mut deadline => break Err(AppError::AuthTimeout(timeout.as_secs().div_ceil(60).max(1))),
                Some(verdict) = rx.recv() => match verdict {
                    CallbackVerdict::Code(code) => break Ok(code),
                    CallbackVerdict::Failed(e) => break Err(e),
                    CallbackVerdict::Ignore => {}
                },
                Some(_) = tasks.join_next(), if !tasks.is_empty() => {}
                accepted = listener.accept() => {
                    if let Ok((stream, peer)) = accepted {
                        if peer.ip().is_loopback() && tasks.len() < MAX_CONNECTIONS {
                            let tx = tx.clone();
                            let interp = interpreter.clone();
                            let claimed = claimed.clone();
                            tasks.spawn(async move {
                                handle_connection(stream, port, interp.as_ref(), &claimed, &tx).await;
                            });
                        }
                    }
                }
            }
        };

        // Limpieza: cerrar el puerto y abortar conexiones pendientes.
        drop(listener);
        tasks.abort_all();
        while tasks.join_next().await.is_some() {}
        result
    }
}

#[derive(Zeroize, ZeroizeOnDrop)]
struct ParsedRequest {
    method: String,
    target: String,
    host: Option<String>,
}

enum ReadError {
    TooLarge,
    Closed,
}

async fn read_head(stream: &mut TcpStream) -> Result<Vec<u8>, ReadError> {
    let mut data = Vec::with_capacity(1024);
    let mut buf = [0u8; 1024];
    loop {
        let n = stream.read(&mut buf).await.map_err(|_| ReadError::Closed)?;
        if n == 0 {
            buf.zeroize();
            data.zeroize();
            return Err(ReadError::Closed);
        }
        let search_from = data.len().saturating_sub(3);
        data.extend_from_slice(&buf[..n]);
        // El límite se comprueba antes que el terminador: una cabecera
        // grande que llegue completa en un solo bloque también se rechaza.
        if data.len() > MAX_HEAD_BYTES {
            buf.zeroize();
            data.zeroize();
            return Err(ReadError::TooLarge);
        }
        if data[search_from..].windows(4).any(|w| w == b"\r\n\r\n") {
            buf.zeroize();
            return Ok(data);
        }
    }
}

fn parse_request(head: &[u8]) -> Option<ParsedRequest> {
    let text = std::str::from_utf8(head).ok()?;
    let mut lines = text.split("\r\n");
    let mut parts = lines.next()?.splitn(3, ' ');
    let method = parts.next()?.to_string();
    let target = parts.next()?.to_string();
    let version = parts.next()?;
    if !version.starts_with("HTTP/1.") || !target.starts_with('/') {
        return None;
    }
    let mut host = None;
    for line in lines {
        if line.is_empty() {
            break;
        }
        let (name, value) = line.split_once(':')?;
        if name.trim().eq_ignore_ascii_case("host") {
            if host.is_some() {
                return None; // varias cabeceras Host: petición ambigua
            }
            host = Some(value.trim().to_string());
        }
    }
    Some(ParsedRequest { method, target, host })
}

fn host_matches(host: Option<&str>, port: u16) -> bool {
    match host {
        Some(h) => {
            let h = h.to_ascii_lowercase();
            h == format!("127.0.0.1:{port}") || h == format!("localhost:{port}")
        }
        None => false,
    }
}

async fn handle_connection<I: CallbackInterpreter + ?Sized>(
    mut stream: TcpStream,
    port: u16,
    interp: &I,
    claimed: &AtomicBool,
    tx: &mpsc::Sender<CallbackVerdict>,
) {
    let mut head = match tokio::time::timeout(READ_TIMEOUT, read_head(&mut stream)).await {
        Ok(Ok(h)) => h,
        Ok(Err(ReadError::TooLarge)) => {
            respond(&mut stream, 431, "Request Header Fields Too Large", &page_simple("Petición demasiado grande.")).await;
            drain(&mut stream).await;
            return;
        }
        _ => return, // conexión vacía, cerrada o lenta: se descarta
    };
    let parsed = parse_request(&head);
    head.zeroize();
    let Some(req) = parsed else {
        respond(&mut stream, 400, "Bad Request", &page_simple("Petición no válida.")).await;
        return;
    };
    if req.method != "GET" {
        respond(&mut stream, 405, "Method Not Allowed", &page_simple("Método no permitido.")).await;
        return;
    }
    if !host_matches(req.host.as_deref(), port) {
        respond(&mut stream, 400, "Bad Request", &page_simple("Host no válido.")).await;
        return;
    }

    let (path, query) = match req.target.split_once('?') {
        Some((p, q)) => (p, q.split('#').next().unwrap_or("")),
        None => (req.target.as_str(), ""),
    };
    let mut params: Vec<(String, String)> =
        url::form_urlencoded::parse(query.as_bytes()).into_owned().collect();
    let verdict = interp.interpret(path, &params);
    for (k, v) in params.iter_mut() {
        k.zeroize();
        v.zeroize();
    }
    drop(req);

    match verdict {
        CallbackVerdict::Ignore => {
            respond(&mut stream, 404, "Not Found", &page_simple("No encontrado.")).await;
        }
        CallbackVerdict::Code(code) => {
            if claimed.swap(true, Ordering::SeqCst) {
                respond(&mut stream, 409, "Conflict", &page_done()).await;
            } else {
                respond(&mut stream, 200, "OK", &page_success()).await;
                let _ = tx.send(CallbackVerdict::Code(code)).await;
            }
        }
        CallbackVerdict::Failed(err) => {
            if claimed.swap(true, Ordering::SeqCst) {
                respond(&mut stream, 409, "Conflict", &page_done()).await;
            } else {
                respond(&mut stream, 400, "Bad Request", &page_failure(&err.to_string())).await;
                let _ = tx.send(CallbackVerdict::Failed(err)).await;
            }
        }
    }
}

async fn respond(stream: &mut TcpStream, status: u16, reason: &str, body: &str) {
    let head = format!(
        "HTTP/1.1 {status} {reason}\r\n\
         Content-Type: text/html; charset=utf-8\r\n\
         Content-Length: {}\r\n\
         Cache-Control: no-store\r\n\
         Referrer-Policy: no-referrer\r\n\
         X-Content-Type-Options: nosniff\r\n\
         Content-Security-Policy: default-src 'none'; style-src 'unsafe-inline'\r\n\
         Connection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(head.as_bytes()).await;
    let _ = stream.write_all(body.as_bytes()).await;
    let _ = stream.flush().await;
    let _ = stream.shutdown().await;
}

/// Descarta (acotadamente) lo que quede por leer para que el cierre no
/// provoque un RST que impida al cliente leer la respuesta.
async fn drain(stream: &mut TcpStream) {
    let mut buf = [0u8; 4096];
    let mut total = 0usize;
    let _ = tokio::time::timeout(Duration::from_millis(500), async {
        while total < 256 * 1024 {
            match stream.read(&mut buf).await {
                Ok(0) | Err(_) => break,
                Ok(n) => total += n,
            }
        }
    })
    .await;
    buf.zeroize();
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn page(title: &str, message: &str, ok: bool) -> String {
    let color = if ok { "#22c55e" } else { "#f87171" };
    format!(
        "<!doctype html><html lang=\"es\"><head><meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\
         <title>{t}</title><style>\
         body{{font-family:system-ui,sans-serif;background:#0f1117;color:#e6e8ee;display:flex;\
         min-height:100vh;align-items:center;justify-content:center;margin:0;padding:16px}}\
         main{{max-width:480px;background:#171a23;border:1px solid #2a2f3d;border-radius:12px;padding:28px}}\
         h1{{font-size:1.25rem;margin:0 0 12px;color:{color}}}p{{line-height:1.5;margin:0}}\
         </style></head><body><main><h1>{t}</h1><p>{m}</p></main></body></html>",
        t = html_escape(title),
        m = html_escape(message),
    )
}

fn page_simple(message: &str) -> String {
    page("TikTok OBS Connector", message, false)
}

fn page_success() -> String {
    page(
        "Autorización recibida",
        "Ya puedes cerrar esta pestaña y volver a TikTok OBS Connector para ver el estado de tu cuenta.",
        true,
    )
}

fn page_failure(message: &str) -> String {
    page("No se completó el inicio de sesión", message, false)
}

fn page_done() -> String {
    page(
        "Intento ya completado",
        "Este intento de inicio de sesión ya se procesó. Vuelve a la aplicación.",
        false,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    struct TestInterp;

    impl CallbackInterpreter for TestInterp {
        fn interpret(&self, _path: &str, params: &[(String, String)]) -> CallbackVerdict {
            let get = |k: &str| params.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone());
            match get("success").as_deref() {
                Some("true") => match get("code") {
                    Some(c) if !c.is_empty() => CallbackVerdict::Code(Secret::new(c)),
                    _ => CallbackVerdict::Failed(AppError::AuthDenied("sin código".into())),
                },
                Some(_) => CallbackVerdict::Failed(AppError::AuthDenied(
                    get("reason").unwrap_or_default(),
                )),
                None => CallbackVerdict::Ignore,
            }
        }
    }

    async fn raw(port: u16, request: &str) -> String {
        let mut s = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        s.write_all(request.as_bytes()).await.unwrap();
        let mut out = String::new();
        let _ = s.read_to_string(&mut out).await;
        out
    }

    async fn get(port: u16, target: &str) -> String {
        raw(port, &format!("GET {target} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\r\n")).await
    }

    async fn start_async(
        timeout: Duration,
    ) -> (u16, CancellationToken, tokio::task::JoinHandle<AppResult<Secret>>) {
        let server = LoopbackServer::bind().await.unwrap();
        let port = server.port();
        let cancel = CancellationToken::new();
        let c = cancel.clone();
        let handle = tokio::spawn(async move { server.wait(Arc::new(TestInterp), c, timeout).await });
        (port, cancel, handle)
    }

    async fn port_closed(port: u16) -> bool {
        TcpStream::connect(("127.0.0.1", port)).await.is_err()
    }

    #[tokio::test]
    async fn binds_only_on_loopback() {
        let server = LoopbackServer::bind().await.unwrap();
        let addr = server.listener.local_addr().unwrap();
        assert!(addr.ip().is_loopback());
        assert_eq!(addr.ip().to_string(), "127.0.0.1");
        assert_ne!(addr.port(), 0);
    }

    #[tokio::test]
    async fn valid_callback_returns_code_and_closes_port() {
        let (port, _c, handle) = start_async(Duration::from_secs(10)).await;
        let resp = get(port, "/?success=true&code=abc123").await;
        assert!(resp.starts_with("HTTP/1.1 200"));
        assert!(resp.contains("Autorización recibida"));
        assert!(resp.contains("Referrer-Policy: no-referrer"));
        let code = handle.await.unwrap().unwrap();
        assert_eq!(code.expose(), "abc123");
        assert!(port_closed(port).await, "el puerto debe quedar cerrado");
    }

    #[tokio::test]
    async fn accepts_localhost_host_header() {
        let (port, _c, handle) = start_async(Duration::from_secs(10)).await;
        let resp = raw(
            port,
            &format!("GET /?success=true&code=xyz HTTP/1.1\r\nHost: localhost:{port}\r\n\r\n"),
        )
        .await;
        assert!(resp.starts_with("HTTP/1.1 200"));
        assert_eq!(handle.await.unwrap().unwrap().expose(), "xyz");
    }

    #[tokio::test]
    async fn rejects_wrong_host_and_keeps_waiting() {
        let (port, _c, handle) = start_async(Duration::from_secs(10)).await;
        let bad = raw(
            port,
            "GET /?success=true&code=evil HTTP/1.1\r\nHost: attacker.example:80\r\n\r\n",
        )
        .await;
        assert!(bad.starts_with("HTTP/1.1 400"));
        let missing = raw(port, "GET /?success=true&code=evil HTTP/1.1\r\n\r\n").await;
        assert!(missing.starts_with("HTTP/1.1 400"));
        let ok = get(port, "/?success=true&code=good").await;
        assert!(ok.starts_with("HTTP/1.1 200"));
        assert_eq!(handle.await.unwrap().unwrap().expose(), "good");
    }

    #[tokio::test]
    async fn ignores_unrelated_requests() {
        let (port, _c, handle) = start_async(Duration::from_secs(10)).await;
        assert!(get(port, "/favicon.ico").await.starts_with("HTTP/1.1 404"));
        assert!(!handle.is_finished());
        get(port, "/?success=true&code=c1").await;
        assert_eq!(handle.await.unwrap().unwrap().expose(), "c1");
    }

    #[tokio::test]
    async fn rejects_non_get_and_malformed_requests() {
        let (port, _c, handle) = start_async(Duration::from_secs(10)).await;
        let post = raw(
            port,
            &format!("POST /?success=true&code=p HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\r\n"),
        )
        .await;
        assert!(post.starts_with("HTTP/1.1 405"));
        let junk = raw(port, "HELLO\r\n\r\n").await;
        assert!(junk.starts_with("HTTP/1.1 400"));
        let dup = raw(
            port,
            &format!(
                "GET /?success=true&code=d HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nHost: localhost:{port}\r\n\r\n"
            ),
        )
        .await;
        assert!(dup.starts_with("HTTP/1.1 400"));
        assert!(!handle.is_finished());
        get(port, "/?success=true&code=fin").await;
        assert_eq!(handle.await.unwrap().unwrap().expose(), "fin");
    }

    #[tokio::test]
    async fn rejects_oversized_headers() {
        let (port, cancel, handle) = start_async(Duration::from_secs(10)).await;
        let big = format!(
            "GET / HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nX-Pad: {}\r\n\r\n",
            "a".repeat(MAX_HEAD_BYTES + 10)
        );
        let resp = raw(port, &big).await;
        assert!(resp.starts_with("HTTP/1.1 431"), "respuesta: {:?}", &resp[..resp.len().min(200)]);
        cancel.cancel();
        assert_eq!(handle.await.unwrap().unwrap_err(), AppError::AuthCancelled);
    }

    #[tokio::test]
    async fn provider_failure_ends_attempt() {
        let (port, _c, handle) = start_async(Duration::from_secs(10)).await;
        let resp = get(port, "/?success=false&reason=connected_with_another_account").await;
        assert!(resp.starts_with("HTTP/1.1 400"));
        let err = handle.await.unwrap().unwrap_err();
        assert_eq!(err, AppError::AuthDenied("connected_with_another_account".into()));
        assert!(port_closed(port).await);
    }

    #[tokio::test]
    async fn only_one_valid_response_per_attempt() {
        let (port, _c, handle) = start_async(Duration::from_secs(10)).await;
        get(port, "/?success=true&code=first").await;
        let code = handle.await.unwrap().unwrap();
        assert_eq!(code.expose(), "first");
        // Una segunda respuesta ya no puede entregarse: el puerto está cerrado.
        assert!(port_closed(port).await);
    }

    #[tokio::test]
    async fn timeout_cleans_up() {
        let (port, _c, handle) = start_async(Duration::from_millis(200)).await;
        let err = handle.await.unwrap().unwrap_err();
        assert!(matches!(err, AppError::AuthTimeout(_)));
        assert!(port_closed(port).await);
    }

    #[tokio::test]
    async fn cancel_cleans_up_even_with_idle_connections() {
        let (port, cancel, handle) = start_async(Duration::from_secs(10)).await;
        // Conexión abierta sin enviar nada (como un "preconnect" del navegador).
        let idle = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;
        cancel.cancel();
        let err = handle.await.unwrap().unwrap_err();
        assert_eq!(err, AppError::AuthCancelled);
        assert!(port_closed(port).await);
        drop(idle);
    }

    #[tokio::test]
    async fn idle_connection_does_not_block_valid_callback() {
        let (port, _c, handle) = start_async(Duration::from_secs(10)).await;
        let _idle = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        let resp = get(port, "/?success=true&code=fast").await;
        assert!(resp.starts_with("HTTP/1.1 200"));
        let code = tokio::time::timeout(Duration::from_secs(2), handle)
            .await
            .expect("no debe esperar a la conexión inactiva")
            .unwrap()
            .unwrap();
        assert_eq!(code.expose(), "fast");
    }

    #[test]
    fn host_validation() {
        assert!(host_matches(Some("127.0.0.1:5000"), 5000));
        assert!(host_matches(Some("LOCALHOST:5000"), 5000));
        assert!(!host_matches(Some("127.0.0.1:5001"), 5000));
        assert!(!host_matches(Some("evil.com"), 5000));
        assert!(!host_matches(None, 5000));
    }
}
