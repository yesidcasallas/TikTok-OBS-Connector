//! Pruebas del flujo completo con un proveedor HTTP simulado (wiremock) y un
//! "navegador" simulado que llama al callback local real.

use super::*;
use crate::provider::streamlabs::endpoints::{AUTH_DATA_PATH, STREAM_START_PATH, TIKTOK_INFO_PATH};
use crate::provider::streamlabs::StreamlabsProvider;
use crate::security::MemoryStore;
use serde_json::json;
use std::io::{Read, Write};
use std::path::PathBuf;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

const TOKEN: &str = "OAUTH_TOKEN_abcdefghijklmnop";
const KEY: &str = "stream-key-SECRET-0123456789";

/// Simula el navegador: tras "abrir" la URL, llama al callback con `query`.
struct FakeBrowser {
    query: Option<String>,
    opened: bool,
}

impl UrlOpener for FakeBrowser {
    fn open(&self, url: &str) -> bool {
        if let Some(q) = self.query.clone() {
            let port: u16 = url::Url::parse(url)
                .unwrap()
                .query_pairs()
                .find(|(k, _)| k == "port")
                .unwrap()
                .1
                .parse()
                .unwrap();
            std::thread::spawn(move || {
                let mut s = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
                write!(s, "GET /?{q} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\r\n").unwrap();
                let mut out = String::new();
                let _ = s.read_to_string(&mut out);
            });
        }
        self.opened
    }
}

fn temp_dir() -> PathBuf {
    use std::sync::atomic::{AtomicU32, Ordering};
    static N: AtomicU32 = AtomicU32::new(0);
    let d = std::env::temp_dir().join(format!(
        "tlk-core-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&d);
    d
}

struct Env {
    server: MockServer,
    store: Arc<MemoryStore>,
    dir: PathBuf,
}

impl Env {
    async fn new() -> Self {
        Env { server: MockServer::start().await, store: Arc::new(MemoryStore::default()), dir: temp_dir() }
    }

    fn core(&self, query: Option<&str>, timeout: Duration) -> Arc<AppCore> {
        let provider = Arc::new(StreamlabsProvider::with_base_url(&self.server.uri()).unwrap());
        Arc::new(AppCore::new(
            provider,
            self.store.clone(),
            Arc::new(FakeBrowser { query: query.map(str::to_string), opened: true }),
            PreferencesStore::new(Some(self.dir.clone())),
            Arc::new(EventLog::default()),
            timeout,
        ))
    }

    async fn mock_exchange_ok(&self) {
        Mock::given(method("GET"))
            .and(path(AUTH_DATA_PATH))
            .and(query_param("code", "abc"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "success": true,
                "data": { "oauth_token": TOKEN, "platform": "tiktok", "platform_username": "creadora" }
            })))
            .mount(&self.server)
            .await;
    }

    async fn mock_info(&self, reason: i64) {
        Mock::given(method("GET"))
            .and(path(TIKTOK_INFO_PATH))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "reason": reason, "can_be_live": reason == 1,
                "user": { "username": "creadora" },
                "application_status": { "status": "approved" }
            })))
            .mount(&self.server)
            .await;
    }

    async fn mock_start_ok(&self) {
        Mock::given(method("POST"))
            .and(path(STREAM_START_PATH))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": "555", "rtmp": "rtmp://push.example/game/", "key": KEY
            })))
            .mount(&self.server)
            .await;
    }

    async fn mock_end_ok(&self) {
        Mock::given(method("POST"))
            .and(path("/api/v5/slobs/tiktok/stream/555/end"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"success": true})))
            .mount(&self.server)
            .await;
    }
}

impl Drop for Env {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn request() -> StartLiveRequest {
    StartLiveRequest {
        title: "  Mi directo  ".into(),
        category: Category { id: "1".into(), name: "Juego".into() },
        audience_type: "0".into(),
    }
}

fn assert_no_secrets(core: &AppCore) {
    let snapshot = serde_json::to_string(&core.snapshot()).unwrap();
    assert!(!snapshot.contains(TOKEN), "el token no debe salir en la instantánea");
    assert!(!snapshot.contains(KEY), "la clave no debe salir en la instantánea");
    for e in core.log().entries() {
        assert!(!e.message.contains(TOKEN), "token en el registro: {}", e.message);
        assert!(!e.message.contains(KEY), "clave en el registro: {}", e.message);
        assert!(!e.message.contains("code=abc"), "código en el registro: {}", e.message);
    }
}

#[tokio::test]
async fn full_flow_login_create_and_end() {
    let env = Env::new().await;
    env.mock_exchange_ok().await;
    env.mock_info(1).await;
    env.mock_start_ok().await;
    env.mock_end_ok().await;
    let core = env.core(Some("success=true&code=abc"), Duration::from_secs(10));

    let snap = core.login().await.unwrap();
    assert_eq!(snap.auth.status, AuthStatus::SignedIn);
    let account = snap.auth.account.unwrap();
    assert_eq!(account.username, "creadora");
    assert_eq!(account.live_access, LiveAccess::Approved);
    assert!(snap.auth.pending.is_none());
    assert_eq!(snap.auth.persistence, Persistence::Memory);
    assert!(env.store.peek().is_none(), "sin autorización no se guarda nada");

    let snap = core.start_stream(request()).await.unwrap();
    let stream = snap.stream.unwrap();
    assert_eq!(stream.server.as_deref(), Some("rtmp://push.example/game/"));
    assert_eq!(stream.key_hint.as_deref(), Some("••••6789"));
    assert_eq!(stream.title, "Mi directo");
    assert!(stream.key_available);
    assert_eq!(core.reveal_key().unwrap().expose(), KEY);
    assert_eq!(core.copy_text(CopyField::Key).unwrap().expose(), KEY);
    assert_no_secrets(&core);

    // El id queda guardado (sin la clave) para poder finalizar tras reiniciar.
    let raw = std::fs::read_to_string(env.dir.join("preferences.json")).unwrap();
    assert!(raw.contains("555") && !raw.contains(KEY));

    assert_eq!(core.start_stream(request()).await.unwrap_err(), AppError::StreamAlreadyCreated);

    let snap = core.end_stream().await.unwrap();
    assert!(snap.stream.is_none());
    assert_no_secrets(&core);
}

#[tokio::test]
async fn login_can_be_cancelled() {
    let env = Env::new().await;
    let core = env.core(None, Duration::from_secs(30));
    let c = core.clone();
    let task = tokio::spawn(async move { c.login().await });
    for _ in 0..100 {
        if core.snapshot().auth.status == AuthStatus::Pending {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let pending = core.snapshot().auth.pending.expect("debe haber un login pendiente");
    assert!(pending.login_url.contains("code_challenge="));
    assert_eq!(core.login().await.unwrap_err(), AppError::AuthInProgress);
    core.cancel_login();
    assert_eq!(task.await.unwrap().unwrap_err(), AppError::AuthCancelled);
    let snap = core.snapshot();
    assert_eq!(snap.auth.status, AuthStatus::SignedOut);
    assert!(snap.auth.pending.is_none());
}

#[tokio::test]
async fn login_times_out() {
    let env = Env::new().await;
    let core = env.core(None, Duration::from_millis(300));
    assert!(matches!(core.login().await.unwrap_err(), AppError::AuthTimeout(_)));
    assert_eq!(core.snapshot().auth.status, AuthStatus::SignedOut);
}

#[tokio::test]
async fn provider_denial_and_rejected_code() {
    let env = Env::new().await;
    let core = env.core(Some("success=false&reason=connected_with_another_account"), Duration::from_secs(10));
    assert!(matches!(core.login().await.unwrap_err(), AppError::AuthDenied(_)));

    Mock::given(path(AUTH_DATA_PATH))
        .respond_with(ResponseTemplate::new(401).set_body_json(json!({"message": ""})))
        .mount(&env.server)
        .await;
    let core = env.core(Some("success=true&code=zzz"), Duration::from_secs(10));
    assert_eq!(core.login().await.unwrap_err(), AppError::AuthExchangeRejected);
    assert_eq!(core.snapshot().auth.status, AuthStatus::SignedOut);
}

#[tokio::test]
async fn authenticated_but_not_approved_is_distinct() {
    let env = Env::new().await;
    env.mock_exchange_ok().await;
    env.mock_info(0).await;
    let core = env.core(Some("success=true&code=abc"), Duration::from_secs(10));
    let snap = core.login().await.unwrap();
    assert_eq!(snap.auth.status, AuthStatus::SignedIn);
    assert_eq!(snap.auth.account.unwrap().live_access, LiveAccess::NotApproved);
}

#[tokio::test]
async fn invalid_token_closes_session() {
    let env = Env::new().await;
    env.mock_exchange_ok().await;
    env.mock_info(1).await;
    Mock::given(path(STREAM_START_PATH))
        .respond_with(ResponseTemplate::new(401).set_body_raw("Unauthorized.", "text/html"))
        .mount(&env.server)
        .await;
    let core = env.core(Some("success=true&code=abc"), Duration::from_secs(10));
    core.set_remember(true).await.unwrap();
    core.login().await.unwrap();
    assert!(env.store.peek().is_some());
    assert_eq!(core.start_stream(request()).await.unwrap_err(), AppError::TokenInvalid);
    let snap = core.snapshot();
    assert_eq!(snap.auth.status, AuthStatus::SignedOut);
    assert_eq!(snap.auth.account_error.unwrap().code, "TOKEN_INVALID");
    assert!(env.store.peek().is_none(), "el token inválido se borra del almacén");
}

#[tokio::test]
async fn remembered_session_is_restored() {
    let env = Env::new().await;
    env.mock_exchange_ok().await;
    env.mock_info(1).await;
    let core = env.core(Some("success=true&code=abc"), Duration::from_secs(10));
    core.set_remember(true).await.unwrap();
    let snap = core.login().await.unwrap();
    assert_eq!(snap.auth.persistence, Persistence::Keyring);
    assert_eq!(env.store.peek().unwrap().expose(), TOKEN);

    // "Reinicio": nueva instancia con las mismas preferencias y almacén.
    let core2 = env.core(None, Duration::from_secs(10));
    core2.clone().restore().await;
    let snap = core2.snapshot();
    assert_eq!(snap.auth.status, AuthStatus::SignedIn);
    assert_eq!(snap.auth.account.unwrap().username, "creadora");

    core2.set_remember(false).await.unwrap();
    assert!(env.store.peek().is_none());
}

#[tokio::test]
async fn keyring_unavailable_falls_back_to_memory() {
    let mut env = Env::new().await;
    env.store = Arc::new(MemoryStore::failing());
    env.mock_exchange_ok().await;
    env.mock_info(1).await;
    let core = env.core(Some("success=true&code=abc"), Duration::from_secs(10));
    core.login().await.unwrap();
    assert!(matches!(core.set_remember(true).await.unwrap_err(), AppError::Keyring(_)));
    let snap = core.snapshot();
    assert!(!snap.auth.remember);
    assert!(matches!(snap.auth.persistence, Persistence::Unavailable { .. }));
    assert_eq!(snap.auth.status, AuthStatus::SignedIn, "la sesión sigue en memoria");
}

#[tokio::test]
async fn logout_requires_ending_live_unless_forced() {
    let env = Env::new().await;
    env.mock_exchange_ok().await;
    env.mock_info(1).await;
    env.mock_start_ok().await;
    let core = env.core(Some("success=true&code=abc"), Duration::from_secs(10));
    core.login().await.unwrap();
    assert_eq!(core.login().await.unwrap_err(), AppError::AlreadyLoggedIn);
    core.start_stream(request()).await.unwrap();
    assert_eq!(core.logout(false).await.unwrap_err(), AppError::StreamBlocksLogout);
    let snap = core.logout(true).await.unwrap();
    assert_eq!(snap.auth.status, AuthStatus::SignedOut);
    assert!(snap.stream.is_some(), "el LIVE se conserva para finalizarlo tras volver a entrar");
}

#[tokio::test]
async fn restored_stream_can_be_ended_after_restart() {
    let env = Env::new().await;
    env.mock_exchange_ok().await;
    env.mock_info(1).await;
    env.mock_start_ok().await;
    env.mock_end_ok().await;
    let core = env.core(Some("success=true&code=abc"), Duration::from_secs(10));
    core.login().await.unwrap();
    core.start_stream(request()).await.unwrap();

    let core2 = env.core(Some("success=true&code=abc"), Duration::from_secs(10));
    let s = core2.snapshot().stream.unwrap();
    assert!(s.restored && !s.key_available && s.server.is_none());
    assert!(core2.reveal_key().is_err());
    assert_eq!(core2.end_stream().await.unwrap_err(), AppError::NotLoggedIn);
    core2.login().await.unwrap();
    assert!(core2.end_stream().await.unwrap().stream.is_none());
}

#[tokio::test]
async fn validation_happens_before_calling_provider() {
    let env = Env::new().await;
    env.mock_exchange_ok().await;
    env.mock_info(1).await;
    Mock::given(path(STREAM_START_PATH))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&env.server)
        .await;
    let core = env.core(Some("success=true&code=abc"), Duration::from_secs(10));
    core.login().await.unwrap();
    let mut r = request();
    r.title = "   ".into();
    assert!(matches!(core.start_stream(r).await.unwrap_err(), AppError::Validation(_)));
    let mut r = request();
    r.title = "x".repeat(TITLE_MAX_CHARS + 1);
    assert!(matches!(core.start_stream(r).await.unwrap_err(), AppError::Validation(_)));
    let mut r = request();
    r.audience_type = "9".into();
    assert!(matches!(core.start_stream(r).await.unwrap_err(), AppError::Validation(_)));
}

#[tokio::test]
async fn operations_require_login() {
    let env = Env::new().await;
    let core = env.core(None, Duration::from_secs(10));
    assert_eq!(core.start_stream(request()).await.unwrap_err(), AppError::NotLoggedIn);
    assert_eq!(core.refresh_account().await.unwrap_err(), AppError::NotLoggedIn);
    assert_eq!(core.search_categories("x").await.unwrap_err(), AppError::NotLoggedIn);
    assert_eq!(core.end_stream().await.unwrap_err(), AppError::NoActiveStream);
}

#[tokio::test]
async fn ending_a_live_unknown_to_the_provider_clears_it() {
    let env = Env::new().await;
    env.mock_exchange_ok().await;
    env.mock_info(1).await;
    env.mock_start_ok().await;
    Mock::given(method("POST"))
        .and(path("/api/v5/slobs/tiktok/stream/555/end"))
        .respond_with(ResponseTemplate::new(404).set_body_json(json!({"message": "Not found"})))
        .mount(&env.server)
        .await;
    let core = env.core(Some("success=true&code=abc"), Duration::from_secs(10));
    core.login().await.unwrap();
    core.start_stream(request()).await.unwrap();
    let snap = core.end_stream().await.unwrap();
    assert!(snap.stream.is_none(), "un 404 significa que el LIVE ya terminó");
    let raw = std::fs::read_to_string(env.dir.join("preferences.json")).unwrap();
    assert!(!raw.contains("555"));
}
