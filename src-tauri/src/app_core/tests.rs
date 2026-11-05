//! Pruebas del flujo completo con un proveedor HTTP simulado (wiremock) y un
//! "navegador" simulado que llama al callback local real. Ninguna prueba
//! contacta con Streamlabs ni crea sesiones LIVE reales.

use super::*;
use crate::provider::streamlabs::endpoints::{AUTH_DATA_PATH, STREAM_START_PATH, TIKTOK_INFO_PATH};
use crate::provider::streamlabs::StreamlabsProvider;
use crate::provider::{Availability, UnavailableCause};
use crate::security::MemoryStore;
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

const TOKEN: &str = "OAUTH_TOKEN_abcdefghijklmnop";
const KEY: &str = "stream-key-SECRET-0123456789";
const GENERIC: &str = "Unable to go live. Please try again later";

/// Simula el navegador: tras "abrir" la URL de login, llama al callback.
struct FakeBrowser {
    query: Option<String>,
    opened: Mutex<Vec<String>>,
}

impl UrlOpener for FakeBrowser {
    fn open(&self, url: &str) -> bool {
        self.opened.lock().unwrap().push(url.to_string());
        let port = url::Url::parse(url).ok().and_then(|u| {
            u.query_pairs()
                .find(|(k, _)| k == "port")
                .map(|(_, v)| v.to_string())
        });
        if let (Some(q), Some(port)) = (self.query.clone(), port) {
            let port: u16 = port.parse().unwrap();
            std::thread::spawn(move || {
                let mut s = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
                write!(s, "GET /?{q} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\r\n").unwrap();
                let mut out = String::new();
                let _ = s.read_to_string(&mut out);
            });
        }
        true
    }
}

fn temp_dir() -> PathBuf {
    static N: AtomicUsize = AtomicUsize::new(0);
    let d = std::env::temp_dir().join(format!(
        "toc-core-{}-{}",
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
    browser: Arc<FakeBrowser>,
}

fn info_body(reason: i64) -> Value {
    json!({
        "reason": reason, "can_be_live": reason == 1,
        "user": { "username": "creadora" },
        "application_status": { "status": if reason == 1 { "approved" } else { "never_applied" } }
    })
}

impl Env {
    async fn new() -> Self {
        Env {
            server: MockServer::start().await,
            store: Arc::new(MemoryStore::default()),
            dir: temp_dir(),
            browser: Arc::new(FakeBrowser {
                query: Some("success=true&code=abc".into()),
                opened: Mutex::new(vec![]),
            }),
        }
    }

    fn core_with(&self, browser: Arc<FakeBrowser>, timeout: Duration) -> Arc<AppCore> {
        let provider = Arc::new(StreamlabsProvider::with_base_url(&self.server.uri()).unwrap());
        Arc::new(AppCore::new(
            provider,
            self.store.clone(),
            browser,
            PreferencesStore::new(Some(self.dir.clone())),
            Arc::new(EventLog::default()),
            timeout,
        ))
    }

    fn core(&self) -> Arc<AppCore> {
        self.core_with(self.browser.clone(), Duration::from_secs(10))
    }

    fn core_callback(&self, query: Option<&str>, timeout: Duration) -> Arc<AppCore> {
        self.core_with(
            Arc::new(FakeBrowser {
                query: query.map(str::to_string),
                opened: Mutex::new(vec![]),
            }),
            timeout,
        )
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

    async fn mock_info_json(&self, body: Value) {
        Mock::given(method("GET"))
            .and(path(TIKTOK_INFO_PATH))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .mount(&self.server)
            .await;
    }

    async fn mock_info(&self, reason: i64) {
        self.mock_info_json(info_body(reason)).await;
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

    /// El inicio de LIVE no debe llamarse nunca en esta prueba.
    async fn forbid_start(&self) {
        Mock::given(path(STREAM_START_PATH))
            .respond_with(ResponseTemplate::new(200))
            .expect(0)
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
        category: Category {
            id: "1".into(),
            name: "Juego".into(),
        },
        audience_type: "0".into(),
    }
}

fn availability(core: &AppCore) -> Availability {
    match core.snapshot().auth.access {
        AccessView::Done { account } => account.access.availability,
        other => panic!("se esperaba una comprobación terminada: {other:?}"),
    }
}

fn assert_no_secrets(core: &AppCore) {
    let snapshot = serde_json::to_string(&core.snapshot()).unwrap();
    assert!(
        !snapshot.contains(TOKEN),
        "el token no debe salir en la instantánea"
    );
    assert!(
        !snapshot.contains(KEY),
        "la clave no debe salir en la instantánea"
    );
    let diag = core.diagnostics_text();
    assert!(
        !diag.contains(TOKEN) && !diag.contains(KEY),
        "diagnóstico con secretos"
    );
    for e in core.log().entries() {
        assert!(
            !e.message.contains(TOKEN),
            "token en el registro: {}",
            e.message
        );
        assert!(
            !e.message.contains(KEY),
            "clave en el registro: {}",
            e.message
        );
        assert!(
            !e.message.contains("code=abc"),
            "código en el registro: {}",
            e.message
        );
    }
}

/// Respuestas de `/tiktok/info` por orden de llamada; la llamada `slow`
/// tarda `delay` y devuelve `slow_body`, las demás responden al instante.
struct Sequenced {
    calls: AtomicUsize,
    slow: usize,
    delay: Duration,
    slow_body: Value,
    fast_body: Value,
}

impl Respond for Sequenced {
    fn respond(&self, _: &Request) -> ResponseTemplate {
        let n = self.calls.fetch_add(1, Ordering::SeqCst);
        if n == self.slow {
            ResponseTemplate::new(200)
                .set_body_json(self.slow_body.clone())
                .set_delay(self.delay)
        } else {
            ResponseTemplate::new(200).set_body_json(self.fast_body.clone())
        }
    }
}

// --------------------------------------------------------------- flujo base --

#[tokio::test]
async fn confirmed_permission_full_flow() {
    let env = Env::new().await;
    env.mock_exchange_ok().await;
    env.mock_info(1).await;
    env.mock_start_ok().await;
    env.mock_end_ok().await;
    let core = env.core();

    let snap = core.login().await.unwrap();
    assert_eq!(snap.auth.status, AuthStatus::SignedIn);
    assert_eq!(snap.auth.username.as_deref(), Some("creadora"));
    assert_eq!(availability(&core), Availability::Confirmed);
    assert!(
        env.store.peek().is_none(),
        "sin autorización no se guarda nada"
    );

    let snap = core.start_stream(request()).await.unwrap();
    let stream = snap.stream.unwrap();
    assert_eq!(stream.server.as_deref(), Some("rtmp://push.example/game/"));
    assert_eq!(stream.key_hint.as_deref(), Some("••••6789"));
    assert_eq!(stream.title, "Mi directo");
    assert_eq!(core.reveal_key().unwrap().expose(), KEY);
    assert_eq!(core.copy_text(CopyField::Key).unwrap().expose(), KEY);
    assert_no_secrets(&core);

    let raw = std::fs::read_to_string(env.dir.join("preferences.json")).unwrap();
    assert!(raw.contains("555") && !raw.contains(KEY));
    assert_eq!(
        core.start_stream(request()).await.unwrap_err(),
        AppError::StreamAlreadyCreated
    );
    assert!(core.end_stream().await.unwrap().stream.is_none());
    assert_no_secrets(&core);
}

#[tokio::test]
async fn login_can_be_cancelled() {
    let env = Env::new().await;
    let core = env.core_callback(None, Duration::from_secs(30));
    let c = core.clone();
    let task = tokio::spawn(async move { c.login().await });
    for _ in 0..100 {
        if core.snapshot().auth.status == AuthStatus::Pending {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(core
        .snapshot()
        .auth
        .pending
        .unwrap()
        .login_url
        .contains("code_challenge="));
    assert_eq!(core.login().await.unwrap_err(), AppError::AuthInProgress);
    core.cancel_login();
    assert_eq!(task.await.unwrap().unwrap_err(), AppError::AuthCancelled);
    assert_eq!(core.snapshot().auth.status, AuthStatus::SignedOut);
}

#[tokio::test]
async fn login_times_out() {
    let env = Env::new().await;
    let core = env.core_callback(None, Duration::from_millis(300));
    assert!(matches!(
        core.login().await.unwrap_err(),
        AppError::AuthTimeout(_)
    ));
}

#[tokio::test]
async fn provider_denial_and_rejected_code() {
    let env = Env::new().await;
    let core = env.core_callback(
        Some("success=false&reason=connected_with_another_account"),
        Duration::from_secs(10),
    );
    assert!(matches!(
        core.login().await.unwrap_err(),
        AppError::AuthDenied(_)
    ));
    Mock::given(path(AUTH_DATA_PATH))
        .respond_with(ResponseTemplate::new(401).set_body_json(json!({"message": ""})))
        .mount(&env.server)
        .await;
    let core = env.core_callback(Some("success=true&code=zzz"), Duration::from_secs(10));
    assert_eq!(
        core.login().await.unwrap_err(),
        AppError::AuthExchangeRejected
    );
}

// ------------------------------------------------ permiso de emisión y botón --

#[tokio::test]
async fn emission_not_available_blocks_start_in_rust() {
    let env = Env::new().await;
    env.mock_exchange_ok().await;
    env.mock_info(0).await;
    env.forbid_start().await;
    let core = env.core();
    let snap = core.login().await.unwrap();
    assert_eq!(
        snap.auth.status,
        AuthStatus::SignedIn,
        "autenticado aunque sin permiso"
    );
    assert_eq!(
        availability(&core),
        Availability::NotAvailable {
            cause: UnavailableCause::NeverApplied
        }
    );
    let err = core.start_stream(request()).await.unwrap_err();
    assert!(matches!(err, AppError::LiveNotAvailable(_)), "{err:?}");
}

#[tokio::test]
async fn missing_null_or_unexpected_fields_are_unknown_and_block_start() {
    for body in [
        json!({"user": {"username": "creadora"}}),
        json!({"reason": null, "can_be_live": null, "user": {}}),
        json!({"reason": "1", "can_be_live": "true", "user": {}}),
    ] {
        let env = Env::new().await;
        env.mock_exchange_ok().await;
        env.mock_info_json(body.clone()).await;
        env.forbid_start().await;
        let core = env.core();
        core.login().await.unwrap();
        assert!(
            matches!(availability(&core), Availability::Unknown { .. }),
            "{body}"
        );
        let err = core.start_stream(request()).await.unwrap_err();
        assert!(
            matches!(err, AppError::AccessUnconfirmed(_)),
            "{body}: {err:?}"
        );
    }
}

#[tokio::test]
async fn contradictory_data_requires_refresh() {
    let env = Env::new().await;
    env.mock_exchange_ok().await;
    env.mock_info_json(json!({"reason": 1, "can_be_live": false, "user": {}}))
        .await;
    env.forbid_start().await;
    let core = env.core();
    core.login().await.unwrap();
    assert!(matches!(
        availability(&core),
        Availability::Contradictory { .. }
    ));
    assert!(matches!(
        core.start_stream(request()).await.unwrap_err(),
        AppError::AccessUnconfirmed(_)
    ));
}

#[tokio::test]
async fn failed_check_is_unknown_not_rejected() {
    let env = Env::new().await;
    env.mock_exchange_ok().await;
    Mock::given(path(TIKTOK_INFO_PATH))
        .respond_with(ResponseTemplate::new(503).set_body_json(json!({"message": "maintenance"})))
        .mount(&env.server)
        .await;
    env.forbid_start().await;
    let core = env.core();
    core.login().await.unwrap();
    let snap = core.snapshot();
    assert_eq!(
        snap.auth.status,
        AuthStatus::SignedIn,
        "un fallo de comprobación no cierra la sesión"
    );
    match snap.auth.access {
        AccessView::Failed { error } => {
            assert_eq!(error.code, "PROVIDER_UNAVAILABLE");
            assert_eq!(error.detail.as_deref(), Some("maintenance"));
        }
        other => panic!("{other:?}"),
    }
    assert!(matches!(
        core.start_stream(request()).await.unwrap_err(),
        AppError::AccessUnconfirmed(_)
    ));
}

#[tokio::test]
async fn refreshing_status_never_creates_a_live() {
    let env = Env::new().await;
    env.mock_exchange_ok().await;
    env.mock_info(1).await;
    env.forbid_start().await;
    let core = env.core();
    core.login().await.unwrap();
    core.refresh_account().await.unwrap();
    core.refresh_account().await.unwrap();
    assert_eq!(availability(&core), Availability::Confirmed);
}

#[tokio::test]
async fn start_is_blocked_while_checking() {
    let env = Env::new().await;
    env.mock_exchange_ok().await;
    Mock::given(path(TIKTOK_INFO_PATH))
        .respond_with(Sequenced {
            calls: AtomicUsize::new(0),
            slow: 1,
            delay: Duration::from_millis(500),
            slow_body: info_body(1),
            fast_body: info_body(1),
        })
        .mount(&env.server)
        .await;
    env.forbid_start().await;
    let core = env.core();
    core.login().await.unwrap();
    let c = core.clone();
    let refresh = tokio::spawn(async move { c.refresh_account().await });
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(matches!(core.snapshot().auth.access, AccessView::Checking));
    assert_eq!(
        core.start_stream(request()).await.unwrap_err(),
        AppError::AccessCheckInProgress
    );
    refresh.await.unwrap().unwrap();
}

// ---------------------------------------------------------------- errores --

#[tokio::test]
async fn generic_provider_error_keeps_detail_and_is_not_permission() {
    let env = Env::new().await;
    env.mock_exchange_ok().await;
    env.mock_info(1).await;
    Mock::given(path(STREAM_START_PATH))
        .respond_with(ResponseTemplate::new(403).set_body_json(json!({ "message": GENERIC })))
        .expect(1)
        .mount(&env.server)
        .await;
    let core = env.core();
    core.login().await.unwrap();
    let err = core.start_stream(request()).await.unwrap_err();
    assert!(
        matches!(err, AppError::ProviderForbidden { status: 403, .. }),
        "{err:?}"
    );
    assert!(!matches!(err, AppError::PermissionDenied { .. }));
    let info = ErrorInfo::from(&err);
    assert_eq!(info.detail.as_deref(), Some(GENERIC));
    assert_eq!(info.http_status, Some(403));
    assert_eq!(core.snapshot().auth.status, AuthStatus::SignedIn);
    assert!(core.snapshot().stream.is_none());
    assert!(core.diagnostics_text().contains(GENERIC));
}

#[tokio::test]
async fn invalid_token_on_check_closes_session() {
    let env = Env::new().await;
    env.mock_exchange_ok().await;
    Mock::given(path(TIKTOK_INFO_PATH))
        .respond_with(ResponseTemplate::new(401).set_body_raw("Unauthorized.", "text/html"))
        .mount(&env.server)
        .await;
    let core = env.core();
    core.set_remember(true).await.unwrap();
    core.login().await.unwrap();
    let snap = core.snapshot();
    assert_eq!(snap.auth.status, AuthStatus::SignedOut);
    assert_eq!(snap.auth.session_error.unwrap().code, "TOKEN_INVALID");
    assert!(
        env.store.peek().is_none(),
        "el token inválido se borra del almacén"
    );
}

#[tokio::test]
async fn invalid_token_on_start_closes_session() {
    let env = Env::new().await;
    env.mock_exchange_ok().await;
    env.mock_info(1).await;
    Mock::given(path(STREAM_START_PATH))
        .respond_with(ResponseTemplate::new(401).set_body_raw("Unauthorized.", "text/html"))
        .mount(&env.server)
        .await;
    let core = env.core();
    core.login().await.unwrap();
    assert_eq!(
        core.start_stream(request()).await.unwrap_err(),
        AppError::TokenInvalid
    );
    assert_eq!(core.snapshot().auth.status, AuthStatus::SignedOut);
}

// ------------------------------------------- concurrencia y datos antiguos --

#[tokio::test]
async fn late_response_from_previous_session_is_discarded() {
    let env = Env::new().await;
    env.mock_exchange_ok().await;
    // Llamada 0: login (aprobada). Llamada 1: comprobación lenta que devuelve
    // «no aprobada». Llamada 2: login de la nueva sesión (aprobada).
    Mock::given(path(TIKTOK_INFO_PATH))
        .respond_with(Sequenced {
            calls: AtomicUsize::new(0),
            slow: 1,
            delay: Duration::from_millis(700),
            slow_body: info_body(0),
            fast_body: info_body(1),
        })
        .mount(&env.server)
        .await;
    let core = env.core();
    core.login().await.unwrap();
    let c = core.clone();
    let late = tokio::spawn(async move { c.refresh_account().await });
    tokio::time::sleep(Duration::from_millis(100)).await;
    core.logout(false).await.unwrap();
    core.login().await.unwrap();
    assert_eq!(availability(&core), Availability::Confirmed);
    late.await.unwrap().unwrap();
    assert_eq!(
        availability(&core),
        Availability::Confirmed,
        "la respuesta tardía no debe aplicarse"
    );
}

#[tokio::test]
async fn out_of_order_checks_keep_the_latest() {
    let env = Env::new().await;
    env.mock_exchange_ok().await;
    Mock::given(path(TIKTOK_INFO_PATH))
        .respond_with(Sequenced {
            calls: AtomicUsize::new(0),
            slow: 1,
            delay: Duration::from_millis(600),
            slow_body: info_body(0),
            fast_body: info_body(1),
        })
        .mount(&env.server)
        .await;
    let core = env.core();
    core.login().await.unwrap();
    let c = core.clone();
    let older = tokio::spawn(async move { c.refresh_account().await });
    tokio::time::sleep(Duration::from_millis(100)).await;
    core.refresh_account().await.unwrap();
    older.await.unwrap().unwrap();
    assert_eq!(availability(&core), Availability::Confirmed);
}

#[tokio::test]
async fn double_click_creates_a_single_live() {
    let env = Env::new().await;
    env.mock_exchange_ok().await;
    env.mock_info(1).await;
    Mock::given(method("POST"))
        .and(path(STREAM_START_PATH))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(
                    json!({"id": "555", "rtmp": "rtmp://push.example/game/", "key": KEY}),
                )
                .set_delay(Duration::from_millis(300)),
        )
        .expect(1)
        .mount(&env.server)
        .await;
    let core = env.core();
    core.login().await.unwrap();
    let (a, b) = tokio::join!(core.start_stream(request()), core.start_stream(request()));
    let results = [a, b];
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    assert!(results.iter().any(|r| matches!(r, Err(AppError::Busy))));
}

// ------------------------------------------------------------------- otros --

#[tokio::test]
async fn access_pages_open_verified_urls() {
    let env = Env::new().await;
    let core = env.core();
    core.open_access_page(AccessPage::Apply).await.unwrap();
    core.open_access_page(AccessPage::Status).await.unwrap();
    let opened = env.browser.opened.lock().unwrap().clone();
    assert_eq!(
        opened,
        vec![ACCESS_APPLY_URL.to_string(), ACCESS_STATUS_URL.to_string()]
    );
    assert!(
        ACCESS_APPLY_URL.contains("live_access_pc_apply/intro")
            && ACCESS_APPLY_URL.contains("GL6399433079641606942")
    );
}

#[tokio::test]
async fn remembered_session_is_restored() {
    let env = Env::new().await;
    env.mock_exchange_ok().await;
    env.mock_info(1).await;
    let core = env.core();
    core.set_remember(true).await.unwrap();
    assert_eq!(
        core.login().await.unwrap().auth.persistence,
        Persistence::Keyring
    );
    assert_eq!(env.store.peek().unwrap().expose(), TOKEN);

    let core2 = env.core_callback(None, Duration::from_secs(10));
    core2.clone().restore().await;
    assert_eq!(core2.snapshot().auth.status, AuthStatus::SignedIn);
    assert_eq!(availability(&core2), Availability::Confirmed);
    core2.set_remember(false).await.unwrap();
    assert!(env.store.peek().is_none());
}

#[tokio::test]
async fn keyring_unavailable_falls_back_to_memory() {
    let mut env = Env::new().await;
    env.store = Arc::new(MemoryStore::failing());
    env.mock_exchange_ok().await;
    env.mock_info(1).await;
    let core = env.core();
    core.login().await.unwrap();
    assert!(matches!(
        core.set_remember(true).await.unwrap_err(),
        AppError::Keyring(_)
    ));
    let snap = core.snapshot();
    assert!(!snap.auth.remember);
    assert!(matches!(
        snap.auth.persistence,
        Persistence::Unavailable { .. }
    ));
    assert_eq!(snap.auth.status, AuthStatus::SignedIn);
}

#[tokio::test]
async fn logout_requires_ending_live_unless_forced() {
    let env = Env::new().await;
    env.mock_exchange_ok().await;
    env.mock_info(1).await;
    env.mock_start_ok().await;
    let core = env.core();
    core.login().await.unwrap();
    assert_eq!(core.login().await.unwrap_err(), AppError::AlreadyLoggedIn);
    core.start_stream(request()).await.unwrap();
    assert_eq!(
        core.logout(false).await.unwrap_err(),
        AppError::StreamBlocksLogout
    );
    let snap = core.logout(true).await.unwrap();
    assert_eq!(snap.auth.status, AuthStatus::SignedOut);
    assert!(
        matches!(snap.auth.access, AccessView::Idle),
        "sin datos de la sesión anterior"
    );
    assert!(snap.stream.is_some());
}

#[tokio::test]
async fn restored_stream_can_be_ended_after_restart() {
    let env = Env::new().await;
    env.mock_exchange_ok().await;
    env.mock_info(1).await;
    env.mock_start_ok().await;
    env.mock_end_ok().await;
    let core = env.core();
    core.login().await.unwrap();
    core.start_stream(request()).await.unwrap();

    let core2 = env.core();
    let s = core2.snapshot().stream.unwrap();
    assert!(s.restored && !s.key_available && s.server.is_none());
    assert_eq!(core2.end_stream().await.unwrap_err(), AppError::NotLoggedIn);
    core2.login().await.unwrap();
    assert!(core2.end_stream().await.unwrap().stream.is_none());
}

#[tokio::test]
async fn validation_happens_before_calling_provider() {
    let env = Env::new().await;
    env.mock_exchange_ok().await;
    env.mock_info(1).await;
    env.forbid_start().await;
    let core = env.core();
    core.login().await.unwrap();
    for (title, audience) in [
        ("   ".to_string(), "0"),
        ("x".repeat(TITLE_MAX_CHARS + 1), "0"),
        ("ok".to_string(), "9"),
    ] {
        let mut r = request();
        r.title = title;
        r.audience_type = audience.into();
        assert!(matches!(
            core.start_stream(r).await.unwrap_err(),
            AppError::Validation(_)
        ));
    }
}

#[tokio::test]
async fn operations_require_login() {
    let env = Env::new().await;
    let core = env.core_callback(None, Duration::from_secs(10));
    assert_eq!(
        core.start_stream(request()).await.unwrap_err(),
        AppError::NotLoggedIn
    );
    assert_eq!(
        core.refresh_account().await.unwrap_err(),
        AppError::NotLoggedIn
    );
    assert_eq!(
        core.search_categories("x").await.unwrap_err(),
        AppError::NotLoggedIn
    );
    assert_eq!(
        core.end_stream().await.unwrap_err(),
        AppError::NoActiveStream
    );
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
    let core = env.core();
    core.login().await.unwrap();
    core.start_stream(request()).await.unwrap();
    assert!(core.end_stream().await.unwrap().stream.is_none());
}
