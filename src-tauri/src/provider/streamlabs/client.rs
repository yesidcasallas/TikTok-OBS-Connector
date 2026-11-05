//! Cliente HTTP del proveedor Streamlabs.

use super::endpoints::*;
use super::errors::{body_error, map_http_error, sanitize_message, Op};
use super::models::{parse_account, parse_categories, summary, AuthDataResponse, StartResponse};
use crate::auth::{CallbackInterpreter, CallbackVerdict};
use crate::error::{AppError, AppResult};
use crate::event_log::{now_ms, EventLog};
use crate::provider::{
    AccountInfo, AuthGrant, Category, LiveCredentials, LiveProvider, StartLiveRequest,
};
use crate::security::Secret;
use async_trait::async_trait;
use reqwest::header::{ACCEPT, CONTENT_TYPE};
use reqwest::multipart::Form;
use serde_json::Value;
use std::sync::Arc;
use std::time::{Duration, Instant};
use url::Url;
use zeroize::Zeroize;

const MAX_BODY_BYTES: usize = 2 * 1024 * 1024;

/// Instala `ring` como proveedor criptográfico de rustls (una sola vez).
pub fn ensure_crypto_provider() {
    let _ = rustls::crypto::ring::default_provider().install_default();
}

/// Respuesta HTTP ya leída, con los metadatos necesarios para validar
/// antes de deserializar.
struct HttpResponse {
    status: u16,
    content_type: Option<String>,
    body: Vec<u8>,
    duration_ms: u64,
}

impl HttpResponse {
    /// `true` si el tipo de contenido es JSON o no se indicó.
    fn json_or_unspecified(&self) -> bool {
        self.content_type
            .as_deref()
            .map(|c| c.contains("json"))
            .unwrap_or(true)
    }
}

pub struct StreamlabsProvider {
    http: reqwest::Client,
    base: Url,
    diag: Option<Arc<EventLog>>,
}

impl StreamlabsProvider {
    pub fn production() -> AppResult<Self> {
        Self::with_base_url(DEFAULT_BASE_URL)
    }

    /// Permite apuntar a otro servidor (pruebas automáticas).
    pub fn with_base_url(base: &str) -> AppResult<Self> {
        ensure_crypto_provider();
        let base =
            Url::parse(base).map_err(|e| AppError::Internal(format!("URL base no válida: {e}")))?;
        let http = reqwest::Client::builder()
            .user_agent(user_agent())
            .timeout(Duration::from_secs(25))
            .connect_timeout(Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::none())
            .https_only(base.scheme() == "https")
            .build()
            .map_err(|e| AppError::Internal(format!("cliente HTTP: {}", e.without_url())))?;
        Ok(StreamlabsProvider {
            http,
            base,
            diag: None,
        })
    }

    /// Registra cada petición (operación, código, tipo, duración) en `log`.
    pub fn with_diagnostics(mut self, log: Arc<EventLog>) -> Self {
        self.diag = Some(log);
        self
    }

    fn diag(&self, msg: String) {
        if let Some(log) = &self.diag {
            log.info(msg);
        }
    }

    fn url(&self, path: &str) -> Url {
        let mut u = self.base.clone();
        u.set_path(path);
        u
    }

    /// Envía la petición. Nunca incluye la URL (que puede contener el código
    /// o el verifier) ni el cuerpo en errores o registros.
    async fn send(&self, op: Op, request: reqwest::RequestBuilder) -> AppResult<HttpResponse> {
        let started = Instant::now();
        let elapsed = |s: Instant| s.elapsed().as_millis() as u64;
        let resp = match request.header(ACCEPT, "application/json").send().await {
            Ok(r) => r,
            Err(e) => {
                let err = network_error(e);
                self.diag(format!(
                    "HTTP {}: sin respuesta ({}) · {} ms",
                    op.name(),
                    err,
                    elapsed(started)
                ));
                return Err(err);
            }
        };
        let status = resp.status().as_u16();
        let content_type = resp
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(|v| {
                v.split(';')
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_ascii_lowercase()
            });
        if resp
            .content_length()
            .map(|l| l as usize > MAX_BODY_BYTES)
            .unwrap_or(false)
        {
            return Err(AppError::ProviderUnexpected(
                "respuesta demasiado grande".into(),
            ));
        }
        let bytes = resp.bytes().await.map_err(network_error)?;
        if bytes.len() > MAX_BODY_BYTES {
            return Err(AppError::ProviderUnexpected(
                "respuesta demasiado grande".into(),
            ));
        }
        let duration_ms = elapsed(started);
        self.diag(format!(
            "HTTP {}: {} · {} · {} ms",
            op.name(),
            status,
            content_type.as_deref().unwrap_or("sin tipo"),
            duration_ms
        ));
        Ok(HttpResponse {
            status,
            content_type,
            body: bytes.to_vec(),
            duration_ms,
        })
    }

    /// Objeto JSON de una respuesta 2xx; valida el tipo de contenido antes.
    fn parse_json(resp: &HttpResponse, what: &str) -> AppResult<Value> {
        if !resp.json_or_unspecified() {
            return Err(AppError::ProviderUnexpected(format!(
                "{what} devolvió «{}» en lugar de JSON (HTTP {})",
                resp.content_type.as_deref().unwrap_or("?"),
                resp.status
            )));
        }
        serde_json::from_slice::<Value>(&resp.body)
            .ok()
            .filter(Value::is_object)
            .ok_or_else(|| {
                AppError::ProviderUnexpected(format!("{what} no devolvió un objeto JSON válido"))
            })
    }

    /// Registra el error de una operación (saneado, sin cuerpo).
    fn diag_error(&self, op: Op, e: &AppError) {
        self.diag(format!("{}: {}", op.name(), e.for_log()));
    }
}

fn network_error(e: reqwest::Error) -> AppError {
    let e = e.without_url();
    if e.is_timeout() {
        AppError::Network("se agotó el tiempo de espera".into())
    } else if e.is_connect() {
        AppError::Network("no se pudo establecer la conexión".into())
    } else {
        AppError::Network(sanitize_message(&e.to_string()))
    }
}

fn valid_code(code: &str) -> bool {
    !code.is_empty() && code.len() <= 2048 && code.chars().all(|c| c.is_ascii_graphic())
}

fn describe_reason(reason: Option<&str>) -> String {
    match reason {
        Some("connected_with_another_account") => "esta cuenta de TikTok ya está vinculada a otra cuenta de Streamlabs. Revísalo en streamlabs.com → Ajustes → Plataformas.".into(),
        Some("unknown") | Some("") | None => "Streamlabs no indicó el motivo.".into(),
        Some(other) => format!("motivo «{}».", sanitize_message(other)),
    }
}

impl CallbackInterpreter for StreamlabsProvider {
    /// Streamlabs redirige a `/?success=true&code=…` o `/?success=false&reason=…`
    /// (ver `externalLogin` en `auth-module.ts`). No devuelve `state`.
    fn interpret(&self, _path: &str, params: &[(String, String)]) -> CallbackVerdict {
        let get = |k: &str| params.iter().find(|(n, _)| n == k).map(|(_, v)| v.as_str());
        match get(CALLBACK_SUCCESS) {
            None => CallbackVerdict::Ignore,
            Some("true") => {
                let reason = get(CALLBACK_REASON);
                if matches!(
                    reason,
                    Some("connected_with_another_account") | Some("unknown")
                ) {
                    return CallbackVerdict::Failed(AppError::AuthDenied(describe_reason(reason)));
                }
                match get(CALLBACK_CODE) {
                    Some(c) if valid_code(c) => CallbackVerdict::Code(Secret::new(c)),
                    _ => CallbackVerdict::Failed(AppError::AuthDenied(
                        "no se recibió un código de autorización válido.".into(),
                    )),
                }
            }
            Some(_) => {
                CallbackVerdict::Failed(AppError::AuthDenied(describe_reason(get(CALLBACK_REASON))))
            }
        }
    }
}

#[async_trait]
impl LiveProvider for StreamlabsProvider {
    fn display_name(&self) -> &'static str {
        "Streamlabs"
    }

    fn login_url(&self, port: u16, code_challenge: &str) -> String {
        let base = self.base.as_str().trim_end_matches('/');
        let flags: Vec<String> = LOGIN_FLAGS
            .iter()
            .map(|(k, v)| match v {
                Some(v) => format!("{k}={v}"),
                None => k.to_string(),
            })
            .collect();
        format!(
            "{base}{LOGIN_PATH}?{}&port={port}&code_challenge={code_challenge}&code_flow=true",
            flags.join("&")
        )
    }

    async fn exchange_code(&self, code: &Secret, verifier: &Secret) -> AppResult<AuthGrant> {
        let mut url = self.url(AUTH_DATA_PATH);
        url.query_pairs_mut()
            .append_pair("code_verifier", verifier.expose())
            .append_pair("code", code.expose());
        let mut resp = self.send(Op::Exchange, self.http.get(url)).await?;
        let result = if resp.status == 200 {
            match serde_json::from_slice::<AuthDataResponse>(&resp.body) {
                Ok(mut r) => {
                    if r.success == Some(false) {
                        Err(AppError::AuthExchangeRejected)
                    } else {
                        match r.data.as_mut() {
                            Some(d) => match d.oauth_token.take().filter(|t| !t.is_empty()) {
                                Some(token) => Ok(AuthGrant {
                                    token: Secret::new(token),
                                    username: d.platform_username.take().filter(|u| !u.is_empty()),
                                    platform: d.platform.take(),
                                }),
                                None => Err(AppError::ProviderUnexpected(
                                    "falta data.oauth_token".into(),
                                )),
                            },
                            None => {
                                Err(AppError::ProviderUnexpected("falta el objeto data".into()))
                            }
                        }
                    }
                }
                Err(_) => Err(AppError::ProviderUnexpected(
                    "la respuesta del intercambio no es JSON válido".into(),
                )),
            }
        } else {
            Err(map_http_error(Op::Exchange, resp.status, &resp.body))
        };
        resp.body.zeroize();
        if let Err(e) = &result {
            self.diag_error(Op::Exchange, e);
        }
        result
    }

    async fn account_info(&self, token: &Secret) -> AppResult<AccountInfo> {
        let req = self
            .http
            .get(self.url(TIKTOK_INFO_PATH))
            .bearer_auth(token.expose());
        let resp = self.send(Op::Info, req).await?;
        let result = (|| {
            if resp.status != 200 {
                return Err(map_http_error(Op::Info, resp.status, &resp.body));
            }
            let v = Self::parse_json(&resp, "/tiktok/info")?;
            if let Some(e) = body_error(resp.status, &v) {
                return Err(e);
            }
            Ok(parse_account(&v, resp.status, resp.duration_ms, now_ms()))
        })();
        match &result {
            Ok(info) => self.diag(format!("{}: {}", Op::Info.name(), summary(&info.access))),
            Err(e) => self.diag_error(Op::Info, e),
        }
        result
    }

    async fn search_categories(&self, token: &Secret, query: &str) -> AppResult<Vec<Category>> {
        let q: String = query
            .trim()
            .chars()
            .take(CATEGORY_QUERY_MAX_CHARS)
            .collect();
        let mut out = Vec::new();
        if !q.is_empty() {
            let mut url = self.url(TIKTOK_INFO_PATH);
            url.query_pairs_mut().append_pair("category", &q);
            let resp = self
                .send(
                    Op::Categories,
                    self.http.get(url).bearer_auth(token.expose()),
                )
                .await?;
            if resp.status != 200 {
                let e = map_http_error(Op::Categories, resp.status, &resp.body);
                self.diag_error(Op::Categories, &e);
                return Err(e);
            }
            let v = Self::parse_json(&resp, "la búsqueda de categorías")?;
            if let Some(e) = body_error(resp.status, &v) {
                return Err(e);
            }
            out = parse_categories(&v).ok_or_else(|| {
                AppError::ProviderUnexpected("la búsqueda no devolvió el campo categories".into())
            })?;
        }
        out.retain(|c| !c.id.is_empty());
        out.push(Category {
            id: String::new(),
            name: OTHER_CATEGORY_NAME.into(),
        });
        Ok(out)
    }

    async fn start_live(
        &self,
        token: &Secret,
        req: &StartLiveRequest,
    ) -> AppResult<LiveCredentials> {
        // Mismos campos y orden que Streamlabs Desktop (`startStream`) y que la
        // referencia (`Stream.start`): title, device_platform, category, audience_type.
        let form = Form::new()
            .text("title", req.title.clone())
            .text("device_platform", device_platform())
            .text("category", req.category.id.clone())
            .text("audience_type", req.audience_type.clone());
        self.diag(format!(
            "{}: título de {} caracteres · category={} · device_platform={} · audience_type={}",
            Op::Start.name(),
            req.title.chars().count(),
            if req.category.id.is_empty() {
                "«» (Otra)".to_string()
            } else {
                format!("id {}", req.category.id)
            },
            device_platform(),
            req.audience_type
        ));
        let request = self
            .http
            .post(self.url(STREAM_START_PATH))
            .bearer_auth(token.expose())
            .multipart(form);
        let mut resp = self.send(Op::Start, request).await?;
        let result = if (200..300).contains(&resp.status) {
            match serde_json::from_slice::<StartResponse>(&resp.body) {
                Ok(mut r) => {
                    let id = r.id_string();
                    match (id, r.rtmp.take(), r.key.take()) {
                        (Some(id), Some(rtmp), Some(key))
                            if !rtmp.trim().is_empty() && !key.is_empty() =>
                        {
                            Ok(LiveCredentials {
                                id,
                                server: rtmp.trim().to_string(),
                                key: Secret::new(key),
                            })
                        }
                        (_, mut rtmp, mut key) => {
                            rtmp.zeroize();
                            key.zeroize();
                            let v: Value =
                                serde_json::from_slice(&resp.body).unwrap_or(Value::Null);
                            Err(body_error(resp.status, &v).unwrap_or_else(|| {
                                AppError::ProviderUnexpected(
                                    "la respuesta no incluye id, rtmp y key".into(),
                                )
                            }))
                        }
                    }
                }
                Err(_) => {
                    let v: Value = serde_json::from_slice(&resp.body).unwrap_or(Value::Null);
                    Err(body_error(resp.status, &v).unwrap_or_else(|| {
                        AppError::ProviderUnexpected(format!(
                            "respuesta de inicio de LIVE no reconocida ({})",
                            resp.content_type.as_deref().unwrap_or("sin tipo")
                        ))
                    }))
                }
            }
        } else {
            Err(map_http_error(Op::Start, resp.status, &resp.body))
        };
        resp.body.zeroize();
        match &result {
            Ok(c) => self.diag(format!("{}: LIVE creado (id {})", Op::Start.name(), c.id)),
            Err(e) => self.diag_error(Op::Start, e),
        }
        result
    }

    async fn end_live(&self, token: &Secret, id: &str) -> AppResult<()> {
        let mut url = self.url(STREAM_END_PREFIX);
        url.path_segments_mut()
            .map_err(|_| AppError::Internal("URL base no válida".into()))?
            .push(id)
            .push(STREAM_END_SUFFIX);
        let resp = self
            .send(Op::End, self.http.post(url).bearer_auth(token.expose()))
            .await?;
        let result = if !(200..300).contains(&resp.status) {
            Err(map_http_error(Op::End, resp.status, &resp.body))
        } else {
            let v: Value = serde_json::from_slice(&resp.body).unwrap_or(Value::Null);
            match body_error(resp.status, &v) {
                Some(e) => Err(e),
                None => Ok(()),
            }
        };
        if let Err(e) = &result {
            self.diag_error(Op::End, e);
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::{Availability, Field, UnavailableCause};
    use serde_json::json;
    use wiremock::matchers::{body_string_contains, header, method, path, query_param};
    use wiremock::{Mock, MockServer, Request, ResponseTemplate};

    async fn setup() -> (MockServer, StreamlabsProvider) {
        let server = MockServer::start().await;
        let p = StreamlabsProvider::with_base_url(&server.uri()).unwrap();
        (server, p)
    }

    fn token() -> Secret {
        Secret::new("tok_test_1234567890")
    }

    fn req() -> StartLiveRequest {
        StartLiveRequest {
            title: "Mi directo".into(),
            category: Category {
                id: "123".into(),
                name: "Juego".into(),
            },
            audience_type: "0".into(),
        }
    }

    #[test]
    fn login_url_matches_contract() {
        let p = StreamlabsProvider::production().unwrap();
        let u = p.login_url(51234, "CHALLENGE_abc-_");
        assert_eq!(
            u,
            "https://streamlabs.com/slobs/login?skip_splash=true&external=electron&tiktok&force_verify&origin=slobs&port=51234&code_challenge=CHALLENGE_abc-_&code_flow=true"
        );
    }

    #[test]
    fn interprets_callback_parameters() {
        let p = StreamlabsProvider::production().unwrap();
        let params = |v: &[(&str, &str)]| -> Vec<(String, String)> {
            v.iter()
                .map(|(a, b)| (a.to_string(), b.to_string()))
                .collect()
        };
        assert!(
            matches!(p.interpret("/", &params(&[("success", "true"), ("code", "abc")])), CallbackVerdict::Code(c) if c.expose() == "abc")
        );
        assert!(matches!(
            p.interpret("/favicon.ico", &params(&[])),
            CallbackVerdict::Ignore
        ));
        assert!(matches!(
            p.interpret("/", &params(&[("success", "false"), ("reason", "connected_with_another_account")])),
            CallbackVerdict::Failed(AppError::AuthDenied(m)) if m.contains("otra cuenta")
        ));
        assert!(matches!(
            p.interpret("/", &params(&[("success", "true")])),
            CallbackVerdict::Failed(_)
        ));
        assert!(matches!(
            p.interpret("/", &params(&[("success", "true"), ("code", "a b")])),
            CallbackVerdict::Failed(_)
        ));
    }

    #[tokio::test]
    async fn exchange_extracts_oauth_token() {
        let (server, p) = setup().await;
        Mock::given(method("GET"))
            .and(path(AUTH_DATA_PATH))
            .and(query_param("code", "the-code"))
            .and(query_param("code_verifier", "the-verifier"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "success": true,
                "data": { "oauth_token": "OAUTH123", "platform": "tiktok", "platform_username": "creadora",
                          "platform_token": "PLATFORM_SECRET", "token": "WIDGET_SECRET" }
            })))
            .expect(1)
            .mount(&server)
            .await;
        let g = p
            .exchange_code(&Secret::new("the-code"), &Secret::new("the-verifier"))
            .await
            .unwrap();
        assert_eq!(g.token.expose(), "OAUTH123");
        assert_eq!(g.username.as_deref(), Some("creadora"));
        assert_eq!(g.platform.as_deref(), Some("tiktok"));
    }

    #[tokio::test]
    async fn exchange_errors_are_classified() {
        let (server, p) = setup().await;
        Mock::given(path(AUTH_DATA_PATH))
            .and(query_param("code", "bad"))
            .respond_with(ResponseTemplate::new(401).set_body_json(json!({"message": ""})))
            .mount(&server)
            .await;
        Mock::given(path(AUTH_DATA_PATH))
            .and(query_param("code", "nofield"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(json!({"success": true, "data": {}})),
            )
            .mount(&server)
            .await;
        Mock::given(path(AUTH_DATA_PATH))
            .and(query_param("code", "fail"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"success": false})))
            .mount(&server)
            .await;
        Mock::given(path(AUTH_DATA_PATH))
            .and(query_param("code", "down"))
            .respond_with(ResponseTemplate::new(502))
            .mount(&server)
            .await;
        let v = Secret::new("v");
        assert_eq!(
            p.exchange_code(&Secret::new("bad"), &v).await.unwrap_err(),
            AppError::AuthExchangeRejected
        );
        assert!(matches!(
            p.exchange_code(&Secret::new("nofield"), &v)
                .await
                .unwrap_err(),
            AppError::ProviderUnexpected(_)
        ));
        assert_eq!(
            p.exchange_code(&Secret::new("fail"), &v).await.unwrap_err(),
            AppError::AuthExchangeRejected
        );
        assert_eq!(
            p.exchange_code(&Secret::new("down"), &v).await.unwrap_err(),
            AppError::ProviderUnavailable {
                status: 502,
                message: None
            }
        );
    }

    #[tokio::test]
    async fn info_uses_bearer_and_maps_access() {
        let (server, p) = setup().await;
        Mock::given(method("GET"))
            .and(path(TIKTOK_INFO_PATH))
            .and(header("authorization", "Bearer tok_test_1234567890"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "reason": 0, "can_be_live": false,
                "user": { "username": "creadora" },
                "application_status": { "status": "rejected", "timestamp": "2026-10-01" }
            })))
            .mount(&server)
            .await;
        let a = p.account_info(&token()).await.unwrap();
        assert_eq!(
            a.access.availability,
            Availability::NotAvailable {
                cause: UnavailableCause::Rejected
            }
        );
        assert_eq!(a.access.can_be_live, Field::Present { value: false });
        assert_eq!(a.http_status, 200);
    }

    #[tokio::test]
    async fn info_unauthorized_html_is_token_invalid() {
        let (server, p) = setup().await;
        Mock::given(path(TIKTOK_INFO_PATH))
            .respond_with(ResponseTemplate::new(401).set_body_raw("Unauthorized.", "text/html"))
            .mount(&server)
            .await;
        assert_eq!(
            p.account_info(&token()).await.unwrap_err(),
            AppError::TokenInvalid
        );
    }

    #[tokio::test]
    async fn redirects_are_not_followed() {
        let (server, p) = setup().await;
        Mock::given(path(TIKTOK_INFO_PATH))
            .respond_with(
                ResponseTemplate::new(302).insert_header("location", "https://evil.example/"),
            )
            .mount(&server)
            .await;
        assert!(matches!(
            p.account_info(&token()).await.unwrap_err(),
            AppError::ProviderUnexpected(_)
        ));
    }

    #[tokio::test]
    async fn categories_are_truncated_and_include_other() {
        let (server, p) = setup().await;
        Mock::given(path(TIKTOK_INFO_PATH))
            .and(query_param("category", "abcdefghijklmnopqrstuvwxy"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "categories": [ { "full_name": "Juego Uno", "game_mask_id": "1" } ]
            })))
            .expect(1)
            .mount(&server)
            .await;
        let list = p
            .search_categories(&token(), "abcdefghijklmnopqrstuvwxyz0123")
            .await
            .unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].name, "Juego Uno");
        assert_eq!(list[1].id, "");
        // Consulta vacía: solo "Otra", sin llamar al proveedor.
        let only_other = p.search_categories(&token(), "  ").await.unwrap();
        assert_eq!(
            only_other,
            vec![Category {
                id: String::new(),
                name: OTHER_CATEGORY_NAME.into()
            }]
        );
    }

    #[tokio::test]
    async fn start_sends_multipart_and_returns_credentials() {
        let (server, p) = setup().await;
        Mock::given(method("POST"))
            .and(path(STREAM_START_PATH))
            .and(header("authorization", "Bearer tok_test_1234567890"))
            .and(body_string_contains("name=\"title\"\r\n\r\nMi directo"))
            .and(body_string_contains(format!("name=\"device_platform\"\r\n\r\n{}", device_platform()).as_str()))
            .and(body_string_contains("name=\"category\"\r\n\r\n123"))
            .and(body_string_contains("name=\"audience_type\"\r\n\r\n0"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": "777", "rtmp": "rtmp://push.example/game/", "key": "stream-key-abcdef", "chat_url": null
            })))
            .expect(1)
            .mount(&server)
            .await;
        let c = p.start_live(&token(), &req()).await.unwrap();
        assert_eq!(c.id, "777");
        assert_eq!(c.server, "rtmp://push.example/game/");
        assert_eq!(c.key.expose(), "stream-key-abcdef");
    }

    #[test]
    fn device_platform_is_never_faked() {
        #[cfg(target_os = "linux")]
        assert_eq!(device_platform(), "linux");
        #[cfg(target_os = "windows")]
        assert_eq!(device_platform(), "win32");
        assert_ne!(device_platform(), "", "debe ser el sistema real");
    }

    type ErrorCheck = fn(&AppError) -> bool;

    #[tokio::test]
    async fn start_error_responses() {
        let cases: Vec<(ResponseTemplate, ErrorCheck)> = vec![
            (
                ResponseTemplate::new(401).set_body_raw("Unauthorized.", "text/html"),
                |e| *e == AppError::TokenInvalid,
            ),
            (
                ResponseTemplate::new(403)
                    .set_body_json(json!({"message": "Unable to go live. Please try again later"})),
                |e| matches!(e, AppError::ProviderForbidden { status: 403, message: Some(m) } if m.starts_with("Unable to go live")),
            ),
            (
                ResponseTemplate::new(400)
                    .set_body_json(json!({"data": {"code": "user_has_no_live_auth"}})),
                |e| matches!(e, AppError::PermissionDenied { .. }),
            ),
            (
                ResponseTemplate::new(403).set_body_raw("<html>cf</html>", "text/html"),
                |e| matches!(e, AppError::ProviderBlocked { .. }),
            ),
            (ResponseTemplate::new(422).set_body_json(json!({})), |e| {
                matches!(e, AppError::AccountRestricted { .. })
            }),
            (ResponseTemplate::new(429), |e| *e == AppError::RateLimited),
            (ResponseTemplate::new(500), |e| {
                *e == AppError::ProviderUnavailable {
                    status: 500,
                    message: None,
                }
            }),
            (
                ResponseTemplate::new(500)
                    .set_body_json(json!({"message": "Unable to go live. Please try again later"})),
                |e| e.provider_detail() == Some("Unable to go live. Please try again later"),
            ),
            (
                ResponseTemplate::new(200)
                    .set_body_json(json!({"success": false, "message": "Limit"})),
                |e| matches!(e, AppError::ProviderRejected { status: 200, .. }),
            ),
            (
                ResponseTemplate::new(200).set_body_json(json!({"id": "1"})),
                |e| matches!(e, AppError::ProviderUnexpected(_)),
            ),
            (
                ResponseTemplate::new(200).set_body_raw("<html>ok</html>", "text/html"),
                |e| matches!(e, AppError::ProviderUnexpected(_)),
            ),
            (
                ResponseTemplate::new(400)
                    .set_body_json(json!({"data": {"code": "user_already_live"}})),
                |e| *e == AppError::AlreadyLive,
            ),
        ];
        for (i, (resp, check)) in cases.into_iter().enumerate() {
            let (server, p) = setup().await;
            Mock::given(path(STREAM_START_PATH))
                .respond_with(resp)
                .mount(&server)
                .await;
            let err = p.start_live(&token(), &req()).await.unwrap_err();
            assert!(check(&err), "caso {i}: {err:?}");
            assert!(
                !err.to_string().contains("tok_test"),
                "caso {i}: el token no debe aparecer"
            );
        }
    }

    #[tokio::test]
    async fn end_live_posts_to_encoded_id() {
        let (server, p) = setup().await;
        Mock::given(method("POST"))
            .and(path("/api/v5/slobs/tiktok/stream/a%2Fb/end"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"success": true})))
            .expect(1)
            .mount(&server)
            .await;
        p.end_live(&token(), "a/b").await.unwrap();
    }

    #[tokio::test]
    async fn end_live_failure_is_reported() {
        let (server, p) = setup().await;
        Mock::given(path("/api/v5/slobs/tiktok/stream/9/end"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"success": false})))
            .mount(&server)
            .await;
        assert!(matches!(
            p.end_live(&token(), "9").await.unwrap_err(),
            AppError::ProviderRejected { .. }
        ));
    }

    #[tokio::test]
    async fn network_errors_do_not_leak_secrets() {
        // Puerto cerrado: error de conexión. La URL lleva el code y el verifier.
        let p = StreamlabsProvider::with_base_url("http://127.0.0.1:9").unwrap();
        let err = p
            .exchange_code(
                &Secret::new("SECRETCODE123"),
                &Secret::new("SECRETVERIFIER456"),
            )
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::Network(_)));
        let text = err.to_string();
        assert!(!text.contains("SECRETCODE123") && !text.contains("SECRETVERIFIER456"));
    }

    #[tokio::test]
    async fn sends_honest_user_agent() {
        let (server, p) = setup().await;
        Mock::given(path(TIKTOK_INFO_PATH))
            .respond_with(|r: &Request| {
                let ua = r
                    .headers
                    .get("user-agent")
                    .and_then(|h| h.to_str().ok())
                    .unwrap_or("");
                if ua.starts_with("TikTokOBSConnector/") && !ua.contains("Windows") {
                    ResponseTemplate::new(200)
                        .set_body_json(json!({"reason": 1, "user": {"username": "u"}}))
                } else {
                    ResponseTemplate::new(400)
                }
            })
            .mount(&server)
            .await;
        assert_eq!(
            p.account_info(&token()).await.unwrap().access.availability,
            Availability::Confirmed
        );
    }

    #[tokio::test]
    async fn info_checks_content_type_before_parsing() {
        let (server, p) = setup().await;
        Mock::given(path(TIKTOK_INFO_PATH))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw("<html>login</html>", "text/html"),
            )
            .mount(&server)
            .await;
        let e = p.account_info(&token()).await.unwrap_err();
        assert!(
            matches!(e, AppError::ProviderUnexpected(ref m) if m.contains("text/html")),
            "{e:?}"
        );
    }

    #[tokio::test]
    async fn info_failures_never_look_like_rejection() {
        for resp in [
            ResponseTemplate::new(500).set_body_json(json!({"message": "boom"})),
            ResponseTemplate::new(200).set_body_raw("{no json", "application/json"),
            ResponseTemplate::new(200).set_body_json(json!(["no", "objeto"])),
        ] {
            let (server, p) = setup().await;
            Mock::given(path(TIKTOK_INFO_PATH))
                .respond_with(resp)
                .mount(&server)
                .await;
            let e = p.account_info(&token()).await.unwrap_err();
            assert!(
                !matches!(
                    e,
                    AppError::PermissionDenied { .. } | AppError::LiveNotAvailable(_)
                ),
                "{e:?}"
            );
        }
    }

    #[tokio::test]
    async fn diagnostics_record_status_without_secrets() {
        let server = MockServer::start().await;
        let log = Arc::new(EventLog::default());
        let p = StreamlabsProvider::with_base_url(&server.uri())
            .unwrap()
            .with_diagnostics(log.clone());
        Mock::given(path(STREAM_START_PATH))
            .respond_with(ResponseTemplate::new(403).set_body_json(json!({"message": "Unable to go live. Please try again later", "key": "live_SECRET_999"})))
            .mount(&server)
            .await;
        let _ = p.start_live(&token(), &req()).await;
        let text: Vec<String> = log.entries().into_iter().map(|e| e.message).collect();
        let all = text.join("\n");
        assert!(
            all.contains("stream_start: 403 · application/json"),
            "{all}"
        );
        assert!(all.contains("category=id 123"), "{all}");
        assert!(all.contains("Unable to go live"), "{all}");
        assert!(
            !all.contains("tok_test_1234567890") && !all.contains("live_SECRET_999"),
            "{all}"
        );
    }
}
