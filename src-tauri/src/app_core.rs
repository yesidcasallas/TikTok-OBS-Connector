//! Orquestación: sesión, comprobación de acceso, creación y fin del LIVE.
//!
//! El token OAuth y la clave de transmisión viven solo aquí, dentro de
//! [`Secret`]. La interfaz recibe instantáneas ([`Snapshot`]) sin secretos;
//! la clave solo sale por petición explícita (mostrar / copiar).

use crate::auth::{LoopbackServer, Pkce};
use crate::error::{AppError, AppResult, ErrorInfo};
use crate::event_log::{now_ms, EventLog};
use crate::prefs::{ActiveStreamRecord, Preferences, PreferencesStore};
use crate::provider::streamlabs::endpoints::{device_platform, TITLE_MAX_CHARS};
use crate::provider::streamlabs::models::summary;
use crate::provider::{
    AccountInfo, Availability, Category, LiveProvider, StartLiveRequest, UnavailableCause,
};
use crate::security::{Secret, TokenStore};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio_util::sync::CancellationToken;

pub trait StateSink: Send + Sync {
    fn on_state(&self, snapshot: &Snapshot);
}

pub trait UrlOpener: Send + Sync {
    /// Abre la URL en el navegador del sistema. Bloqueante.
    fn open(&self, url: &str) -> bool;
}

// ---------------------------------------------------------------- vistas --

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum AuthStatus {
    SignedOut,
    Pending,
    SignedIn,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingView {
    pub login_url: String,
    pub browser_opened: bool,
    pub started_at_ms: u64,
    pub expires_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "mode", rename_all = "camelCase")]
pub enum Persistence {
    /// La sesión solo existe en memoria.
    Memory,
    /// Guardada en el almacén de credenciales del sistema.
    Keyring,
    /// Se intentó usar el almacén y no está disponible.
    Unavailable { detail: String },
}

/// Estado de la comprobación del permiso de emisión. Separado de la sesión:
/// estar autenticado no implica permiso.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum AccessView {
    /// Aún no se ha comprobado en esta sesión.
    Idle,
    /// Comprobación en curso.
    Checking,
    /// Respuesta interpretada (ver `account.access.availability`).
    Done { account: AccountInfo },
    /// La comprobación falló (red, servidor, respuesta inesperada…): estado desconocido.
    Failed { error: ErrorInfo },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthView {
    pub status: AuthStatus,
    pub pending: Option<PendingView>,
    pub username: Option<String>,
    pub access: AccessView,
    /// Motivo por el que terminó la sesión anterior (p. ej., token inválido).
    pub session_error: Option<ErrorInfo>,
    pub remember: bool,
    pub persistence: Persistence,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamView {
    pub id: String,
    pub server: Option<String>,
    pub key_hint: Option<String>,
    pub key_available: bool,
    pub title: String,
    pub category_name: String,
    pub started_at_ms: u64,
    /// Creado en una ejecución anterior (servidor y clave ya no disponibles).
    pub restored: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FormDefaults {
    pub title: String,
    pub category: Option<Category>,
    pub audience_type: String,
    pub title_max_chars: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub version: String,
    pub os: String,
    pub device_platform: String,
    pub provider: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub auth: AuthView,
    pub stream: Option<StreamView>,
    pub busy: Option<String>,
    pub form: FormDefaults,
    pub app: AppInfo,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CopyField {
    Server,
    Key,
    LoginUrl,
}

/// Páginas oficiales de TikTok sobre el acceso a LIVE con Streamlabs.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AccessPage {
    /// Solicitar el acceso.
    Apply,
    /// Consultar el resultado de la solicitud.
    Status,
}

// ---------------------------------------------------------------- estado --

struct ActiveStream {
    id: String,
    server: Option<String>,
    key: Option<Secret>,
    title: String,
    category_name: String,
    started_at_ms: u64,
    restored: bool,
}

impl ActiveStream {
    fn view(&self) -> StreamView {
        StreamView {
            id: self.id.clone(),
            server: self.server.clone(),
            key_hint: self.key.as_ref().map(Secret::hint),
            key_available: self.key.is_some(),
            title: self.title.clone(),
            category_name: self.category_name.clone(),
            started_at_ms: self.started_at_ms,
            restored: self.restored,
        }
    }
}

struct Inner {
    token: Option<Secret>,
    username_hint: Option<String>,
    access: AccessView,
    session_error: Option<ErrorInfo>,
    pending: Option<PendingView>,
    stream: Option<ActiveStream>,
    busy: Option<String>,
    persistence: Persistence,
    /// Cambia en cada inicio o cierre de sesión: invalida respuestas antiguas.
    session_gen: u64,
    /// Cambia en cada comprobación: solo se aplica la más reciente.
    check_seq: u64,
}

pub struct AppCore {
    provider: Arc<dyn LiveProvider>,
    store: Arc<dyn TokenStore>,
    opener: Arc<dyn UrlOpener>,
    prefs_store: PreferencesStore,
    prefs: Mutex<Preferences>,
    log: Arc<EventLog>,
    inner: Mutex<Inner>,
    op: tokio::sync::Mutex<()>,
    login_cancel: Mutex<Option<CancellationToken>>,
    sink: Mutex<Option<Arc<dyn StateSink>>>,
    login_timeout: Duration,
}

/// Explicación de por qué la emisión no está disponible (según el proveedor).
pub fn cause_text(c: UnavailableCause) -> &'static str {
    match c {
        UnavailableCause::NotApproved => "Streamlabs indica que la cuenta no tiene aprobado el acceso a LIVE desde PC (reason=0).",
        UnavailableCause::NeverApplied => "Streamlabs no registra un acceso aprobado (application_status=never_applied). Si ya enviaste la solicitud, puede estar en revisión: Streamlabs no muestra ese estado.",
        UnavailableCause::Rejected => "Streamlabs indica que la solicitud de acceso fue rechazada.",
        UnavailableCause::Legacy => "la cuenta tiene acceso «legacy»: Streamlabs no genera la clave; se obtiene en TikTok LIVE Producer.",
        UnavailableCause::Relog => "Streamlabs pide volver a iniciar sesión para actualizar los permisos.",
        UnavailableCause::CannotGoLive => "Streamlabs indica can_be_live=false: la cuenta no puede emitir ahora.",
        UnavailableCause::Pending => "Streamlabs indica que la solicitud de acceso está en revisión.",
    }
}

fn availability_label(a: &Availability) -> String {
    match a {
        Availability::Confirmed => "permiso de emisión confirmado".into(),
        Availability::NotAvailable { cause } => {
            format!("emisión no disponible ({})", cause_text(*cause))
        }
        Availability::Contradictory { detail } => format!("datos contradictorios ({detail})"),
        Availability::Unknown { detail } => format!("estado desconocido ({detail})"),
    }
}

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> AppResult<T> {
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| AppError::Internal(format!("tarea interrumpida: {e}")))
}

impl AppCore {
    pub fn new(
        provider: Arc<dyn LiveProvider>,
        store: Arc<dyn TokenStore>,
        opener: Arc<dyn UrlOpener>,
        prefs_store: PreferencesStore,
        log: Arc<EventLog>,
        login_timeout: Duration,
    ) -> Self {
        let prefs = prefs_store.load();
        let stream = prefs.active_stream.as_ref().map(|r| ActiveStream {
            id: r.id.clone(),
            server: None,
            key: None,
            title: r.title.clone(),
            category_name: r.category_name.clone(),
            started_at_ms: r.started_at_ms,
            restored: true,
        });
        AppCore {
            provider,
            store,
            opener,
            prefs_store,
            prefs: Mutex::new(prefs),
            log,
            inner: Mutex::new(Inner {
                token: None,
                username_hint: None,
                access: AccessView::Idle,
                session_error: None,
                pending: None,
                stream,
                busy: None,
                persistence: Persistence::Memory,
                session_gen: 0,
                check_seq: 0,
            }),
            op: tokio::sync::Mutex::new(()),
            login_cancel: Mutex::new(None),
            sink: Mutex::new(None),
            login_timeout,
        }
    }

    pub fn set_sink(&self, sink: Arc<dyn StateSink>) {
        *self.sink.lock().unwrap() = Some(sink);
    }

    pub fn log(&self) -> &EventLog {
        &self.log
    }

    fn prefs(&self) -> Preferences {
        self.prefs.lock().unwrap().clone()
    }

    fn update_prefs(&self, f: impl FnOnce(&mut Preferences)) {
        let mut p = self.prefs.lock().unwrap();
        f(&mut p);
        if let Err(e) = self.prefs_store.save(&p) {
            self.log
                .warn(format!("No se pudieron guardar las preferencias: {e}"));
        }
    }

    pub fn snapshot(&self) -> Snapshot {
        let prefs = self.prefs();
        let i = self.inner.lock().unwrap();
        let status = if i.token.is_some() {
            AuthStatus::SignedIn
        } else if i.pending.is_some() {
            AuthStatus::Pending
        } else {
            AuthStatus::SignedOut
        };
        Snapshot {
            auth: AuthView {
                status,
                pending: i.pending.clone(),
                username: match &i.access {
                    AccessView::Done { account } => account.username.clone(),
                    _ => None,
                }
                .or_else(|| i.username_hint.clone()),
                access: if i.token.is_some() {
                    i.access.clone()
                } else {
                    AccessView::Idle
                },
                session_error: i.session_error.clone(),
                remember: prefs.remember_session,
                persistence: i.persistence.clone(),
            },
            stream: i.stream.as_ref().map(ActiveStream::view),
            busy: i.busy.clone(),
            form: FormDefaults {
                title: prefs.last_title.clone(),
                category: prefs.last_category.clone(),
                audience_type: if prefs.audience_type.is_empty() {
                    "0".into()
                } else {
                    prefs.audience_type.clone()
                },
                title_max_chars: TITLE_MAX_CHARS,
            },
            app: AppInfo {
                version: env!("CARGO_PKG_VERSION").into(),
                os: std::env::consts::OS.into(),
                device_platform: device_platform().into(),
                provider: self.provider.display_name().into(),
            },
        }
    }

    fn emit(&self) {
        let snapshot = self.snapshot();
        let sink = self.sink.lock().unwrap().clone();
        if let Some(sink) = sink {
            sink.on_state(&snapshot);
        }
    }

    fn with_inner<T>(&self, f: impl FnOnce(&mut Inner) -> T) -> T {
        f(&mut self.inner.lock().unwrap())
    }

    fn set_busy(&self, label: Option<&str>) {
        self.with_inner(|i| i.busy = label.map(str::to_string));
        self.emit();
    }

    fn token(&self) -> AppResult<Secret> {
        self.with_inner(|i| i.token.clone())
            .ok_or(AppError::NotLoggedIn)
    }

    // ------------------------------------------------------------ arranque --

    /// Recupera la sesión guardada (si el usuario lo autorizó) y el LIVE
    /// pendiente de una ejecución anterior.
    pub async fn restore(self: Arc<Self>) {
        if let Some(s) = self.with_inner(|i| i.stream.as_ref().map(|s| s.id.clone())) {
            self.log.warn(format!(
                "Hay un LIVE creado en una ejecución anterior (id {s}). Finalízalo cuando termines."
            ));
        }
        if !self.prefs().remember_session {
            return;
        }
        let store = self.store.clone();
        match blocking(move || store.load()).await {
            Ok(Ok(Some(token))) => {
                self.log.protect(&token);
                self.with_inner(|i| {
                    i.token = Some(token);
                    i.persistence = Persistence::Keyring;
                    i.session_gen += 1;
                    i.access = AccessView::Idle;
                });
                self.log
                    .info("Sesión recuperada del almacén de credenciales del sistema.");
                self.emit();
                if let Err(e) = self.refresh_account().await {
                    self.log
                        .warn(format!("No se pudo comprobar la cuenta: {e}"));
                }
            }
            Ok(Ok(None)) => {
                self.with_inner(|i| i.persistence = Persistence::Memory);
                self.log.info("No hay ninguna sesión guardada.");
            }
            Ok(Err(e)) => {
                self.with_inner(|i| i.persistence = Persistence::Unavailable { detail: e.clone() });
                self.log.warn(format!(
                    "Almacén de credenciales no disponible: {e}. La sesión se mantendrá solo en memoria."
                ));
            }
            Err(e) => self.log.error(e.to_string()),
        }
        self.emit();
    }

    // -------------------------------------------------------------- login --

    pub async fn login(&self) -> AppResult<Snapshot> {
        if self.with_inner(|i| i.token.is_some()) {
            return Err(AppError::AlreadyLoggedIn);
        }
        let cancel = {
            let mut slot = self.login_cancel.lock().unwrap();
            if slot.is_some() {
                return Err(AppError::AuthInProgress);
            }
            let c = CancellationToken::new();
            *slot = Some(c.clone());
            c
        };
        let result = self.login_flow(cancel).await;
        *self.login_cancel.lock().unwrap() = None;
        self.with_inner(|i| {
            i.pending = None;
            i.busy = None;
        });
        match &result {
            Ok(()) => {}
            Err(AppError::AuthCancelled) => self.log.info("Inicio de sesión cancelado."),
            Err(e) => self.log.error(format!("Inicio de sesión fallido: {e}")),
        }
        self.emit();
        result.map(|_| self.snapshot())
    }

    async fn login_flow(&self, cancel: CancellationToken) -> AppResult<()> {
        let pkce = Pkce::generate()?;
        self.log.protect(&pkce.verifier);
        let server = LoopbackServer::bind().await?;
        let port = server.port();
        let url = self.provider.login_url(port, &pkce.challenge);
        let started = now_ms();
        self.with_inner(|i| {
            i.session_error = None;
            i.pending = Some(PendingView {
                login_url: url.clone(),
                browser_opened: false,
                started_at_ms: started,
                expires_at_ms: started + self.login_timeout.as_millis() as u64,
            })
        });
        self.log.info(format!(
            "Esperando el inicio de sesión en el navegador (retorno local en 127.0.0.1:{port})."
        ));
        self.emit();

        let open_browser = async {
            let opener = self.opener.clone();
            let u = url.clone();
            let opened = blocking(move || opener.open(&u)).await.unwrap_or(false);
            self.with_inner(|i| {
                if let Some(p) = i.pending.as_mut() {
                    p.browser_opened = opened;
                }
            });
            if opened {
                self.log
                    .info("Se abrió el navegador del sistema con la página de Streamlabs.");
            } else {
                self.log.warn("No se pudo abrir el navegador automáticamente. Copia el enlace y ábrelo manualmente.");
            }
            self.emit();
        };
        let (code, ()) = tokio::join!(
            server.wait(self.provider.clone(), cancel, self.login_timeout),
            open_browser
        );
        let code = code?;
        self.log.protect(&code);

        self.with_inner(|i| {
            i.pending = None;
            i.busy = Some("Validando el inicio de sesión con Streamlabs…".into());
        });
        self.emit();
        let grant = self.provider.exchange_code(&code, &pkce.verifier).await;
        drop(code);
        let grant = grant?;
        self.log.protect(&grant.token);

        if let Some(p) = grant.platform.as_deref() {
            if !p.eq_ignore_ascii_case("tiktok") {
                self.log.warn(format!(
                    "Streamlabs devolvió una sesión de «{p}», no de TikTok."
                ));
            }
        }
        let who = grant
            .username
            .as_deref()
            .map(|u| format!(" como @{u}"))
            .unwrap_or_default();
        self.with_inner(|i| {
            i.token = Some(grant.token.clone());
            i.username_hint = grant.username.clone();
            i.access = AccessView::Idle;
            i.session_error = None;
            i.busy = None;
            i.session_gen += 1;
        });
        self.log.success(format!(
            "Autenticación correcta{who}. Esto no garantiza acceso a LIVE."
        ));
        self.emit();

        if self.prefs().remember_session {
            let _ = self.persist_token(grant.token.clone()).await;
        }
        if let Err(e) = self.refresh_account().await {
            self.log
                .warn(format!("No se pudo comprobar el acceso a LIVE: {e}"));
        }
        Ok(())
    }

    pub fn cancel_login(&self) {
        if let Some(c) = self.login_cancel.lock().unwrap().as_ref() {
            c.cancel();
        }
    }

    async fn persist_token(&self, token: Secret) -> AppResult<()> {
        let store = self.store.clone();
        match blocking(move || store.save(&token)).await? {
            Ok(()) => {
                self.with_inner(|i| i.persistence = Persistence::Keyring);
                self.log
                    .info("Sesión guardada en el almacén de credenciales del sistema.");
                self.emit();
                Ok(())
            }
            Err(e) => {
                self.with_inner(|i| i.persistence = Persistence::Unavailable { detail: e.clone() });
                self.log.warn(format!(
                    "No se pudo guardar la sesión en el almacén del sistema: {e}. Se mantiene solo en memoria."
                ));
                self.emit();
                Err(AppError::Keyring(e))
            }
        }
    }

    async fn clear_session(&self, reason: Option<&AppError>) {
        self.with_inner(|i| {
            i.token = None;
            i.access = AccessView::Idle;
            i.username_hint = None;
            i.session_error = reason.map(ErrorInfo::from);
            i.session_gen += 1;
            if i.persistence == Persistence::Keyring {
                i.persistence = Persistence::Memory;
            }
        });
        if self.prefs().remember_session {
            let store = self.store.clone();
            if let Ok(Err(e)) = blocking(move || store.delete()).await {
                self.log.warn(format!(
                    "No se pudo borrar la sesión del almacén del sistema: {e}"
                ));
            }
        }
        self.emit();
    }

    pub async fn logout(&self, force: bool) -> AppResult<Snapshot> {
        let _guard = self.op.try_lock().map_err(|_| AppError::Busy)?;
        if self.with_inner(|i| i.token.is_none()) {
            return Ok(self.snapshot());
        }
        if self.with_inner(|i| i.stream.is_some()) {
            if !force {
                return Err(AppError::StreamBlocksLogout);
            }
            self.log.warn(
                "Sesión cerrada con un LIVE activo: vuelve a iniciar sesión para finalizarlo, o hazlo desde TikTok LIVE Center.",
            );
        }
        self.clear_session(None).await;
        self.log
            .info("Sesión cerrada en este equipo (el token se ha borrado de la aplicación).");
        Ok(self.snapshot())
    }

    pub async fn set_remember(&self, remember: bool) -> AppResult<Snapshot> {
        self.update_prefs(|p| p.remember_session = remember);
        if remember {
            if let Some(token) = self.with_inner(|i| i.token.clone()) {
                if let Err(e) = self.persist_token(token).await {
                    self.update_prefs(|p| p.remember_session = false);
                    self.emit();
                    return Err(e);
                }
            } else {
                self.log
                    .info("La sesión se guardará en el almacén del sistema al iniciar sesión.");
            }
        } else {
            let store = self.store.clone();
            match blocking(move || store.delete()).await? {
                Ok(()) => self
                    .log
                    .info("La sesión ya no se guardará y se borró del almacén del sistema."),
                Err(e) => self.log.warn(format!(
                    "No se pudo borrar la sesión del almacén del sistema: {e}"
                )),
            }
            self.with_inner(|i| {
                if i.persistence == Persistence::Keyring {
                    i.persistence = Persistence::Memory;
                }
            });
        }
        self.emit();
        Ok(self.snapshot())
    }

    // ------------------------------------------------------------- cuenta --

    async fn handle_auth_error(&self, e: &AppError) {
        if *e == AppError::TokenInvalid {
            self.log
                .error("Streamlabs rechazó el token: la sesión se ha cerrado.");
            self.clear_session(Some(e)).await;
        }
    }

    /// Comprueba el permiso de emisión (solo lectura: nunca crea un LIVE).
    ///
    /// No retiene el candado de operaciones durante la petición; en su lugar,
    /// la respuesta solo se aplica si sigue siendo de la misma sesión y es la
    /// comprobación más reciente. Las respuestas tardías se descartan.
    pub async fn refresh_account(&self) -> AppResult<Snapshot> {
        if self.op.try_lock().is_err() {
            return Err(AppError::Busy);
        }
        let token = self.token()?;
        let (gen, seq) = self.with_inner(|i| {
            i.check_seq += 1;
            i.access = AccessView::Checking;
            (i.session_gen, i.check_seq)
        });
        self.emit();
        let result = self.provider.account_info(&token).await;
        drop(token);
        let current = self.with_inner(|i| i.session_gen == gen && i.check_seq == seq);
        if !current {
            self.log.info("Respuesta de comprobación descartada: pertenece a una sesión o comprobación anterior.");
            return Ok(self.snapshot());
        }
        match result {
            Ok(mut account) => {
                if account.username.is_none() {
                    account.username = self.with_inner(|i| i.username_hint.clone());
                }
                self.log.info(format!(
                    "Comprobación de acceso: HTTP {} en {} ms · {} → {}.",
                    account.http_status,
                    account.duration_ms,
                    summary(&account.access),
                    availability_label(&account.access.availability)
                ));
                self.with_inner(|i| i.access = AccessView::Done { account });
                self.emit();
                Ok(self.snapshot())
            }
            Err(e) => {
                self.log.warn(format!(
                    "No se pudo comprobar el acceso a LIVE: {}",
                    e.for_log()
                ));
                if e == AppError::TokenInvalid {
                    self.handle_auth_error(&e).await;
                } else {
                    self.with_inner(|i| {
                        i.access = AccessView::Failed {
                            error: ErrorInfo::from(&e),
                        }
                    });
                    self.emit();
                }
                Err(e)
            }
        }
    }

    /// Abre la página oficial de TikTok del proveedor activo para solicitar o
    /// consultar el acceso. Abrir el formulario no envía ni aprueba nada: el
    /// estado solo cambia con la respuesta del proveedor.
    pub async fn open_access_page(&self, page: AccessPage) -> AppResult<()> {
        let pages = self.provider.access_pages();
        let url = match page {
            AccessPage::Apply => pages.apply_url,
            AccessPage::Status => pages.status_url,
        };
        let opener = self.opener.clone();
        if blocking(move || opener.open(url)).await? {
            self.log.info(format!(
                "Abierta la página de TikTok para {} el acceso a LIVE con {} (id {}).",
                match page {
                    AccessPage::Apply => "solicitar",
                    AccessPage::Status => "consultar",
                },
                pages.provider,
                pages.application_id
            ));
            Ok(())
        } else {
            Err(AppError::Validation(format!(
                "No se pudo abrir el navegador. Visita {url}"
            )))
        }
    }

    /// Diagnóstico para copiar: versión, plataforma y últimas entradas
    /// (ya redactadas, sin tokens ni claves).
    pub fn diagnostics_text(&self) -> String {
        let s = self.snapshot();
        let access = match &s.auth.access {
            AccessView::Idle => "sin comprobar".to_string(),
            AccessView::Checking => "comprobando".to_string(),
            AccessView::Done { account } => format!(
                "HTTP {} · {} · {}",
                account.http_status,
                summary(&account.access),
                availability_label(&account.access.availability)
            ),
            AccessView::Failed { error } => format!("error {} ({})", error.code, error.message),
        };
        format!(
            "TikTok OBS Connector {} · {} · device_platform={}\nSesión: {:?} · Acceso: {}\nRegistro: {}\n---\n{}",
            s.app.version,
            s.app.os,
            s.app.device_platform,
            s.auth.status,
            access,
            self.log.file_path().map(|p| p.display().to_string()).unwrap_or_else(|| "solo en memoria".into()),
            self.log.recent_text(80)
        )
    }

    pub async fn search_categories(&self, query: &str) -> AppResult<Vec<Category>> {
        let token = self.token()?;
        let result = self.provider.search_categories(&token, query).await;
        if let Err(e) = &result {
            self.handle_auth_error(e).await;
        }
        result
    }

    // ---------------------------------------------------------------- LIVE --

    fn validate(&self, mut req: StartLiveRequest) -> AppResult<StartLiveRequest> {
        req.title = req.title.trim().to_string();
        if req.title.is_empty() {
            return Err(AppError::Validation(
                "Escribe un título para el LIVE.".into(),
            ));
        }
        if req.title.chars().count() > TITLE_MAX_CHARS {
            return Err(AppError::Validation(format!(
                "El título no puede superar {TITLE_MAX_CHARS} caracteres."
            )));
        }
        if req.title.chars().any(char::is_control) {
            return Err(AppError::Validation(
                "El título contiene caracteres no válidos.".into(),
            ));
        }
        if req.category.id.len() > 128 || req.category.name.chars().count() > 200 {
            return Err(AppError::Validation("Categoría no válida.".into()));
        }
        let audience = self.with_inner(|i| match &i.access {
            AccessView::Done { account } => Some(account.audience.clone()),
            _ => None,
        });
        match audience {
            Some(a) if a.disabled => req.audience_type = a.default_value,
            Some(a) => {
                if !a.options.iter().any(|o| o.value == req.audience_type) {
                    return Err(AppError::Validation("Tipo de audiencia no válido.".into()));
                }
            }
            None => {
                if req.audience_type != "0" && req.audience_type != "1" {
                    return Err(AppError::Validation("Tipo de audiencia no válido.".into()));
                }
            }
        }
        Ok(req)
    }

    /// Misma regla que el botón de la interfaz: solo con permiso confirmado y
    /// sin comprobaciones en curso (Streamlabs Desktop tampoco pide
    /// credenciales si la cuenta no está aprobada).
    fn ensure_live_permission(&self) -> AppResult<()> {
        self.with_inner(|i| match &i.access {
            AccessView::Checking => Err(AppError::AccessCheckInProgress),
            AccessView::Done { account } => match &account.access.availability {
                Availability::Confirmed => Ok(()),
                Availability::NotAvailable { cause } => {
                    Err(AppError::LiveNotAvailable(cause_text(*cause).into()))
                }
                Availability::Contradictory { detail } => Err(AppError::AccessUnconfirmed(
                    format!("Streamlabs devolvió datos contradictorios ({detail})."),
                )),
                Availability::Unknown { detail } => Err(AppError::AccessUnconfirmed(format!(
                    "la respuesta de Streamlabs no permite saberlo ({detail})."
                ))),
            },
            AccessView::Failed { .. } => Err(AppError::AccessUnconfirmed(
                "la última comprobación falló.".into(),
            )),
            AccessView::Idle => Err(AppError::AccessUnconfirmed(
                "aún no se ha comprobado.".into(),
            )),
        })
    }

    pub async fn start_stream(&self, req: StartLiveRequest) -> AppResult<Snapshot> {
        let _guard = self.op.try_lock().map_err(|_| AppError::Busy)?;
        let token = self.token()?;
        if self.with_inner(|i| i.stream.is_some()) {
            return Err(AppError::StreamAlreadyCreated);
        }
        self.ensure_live_permission()?;
        let req = self.validate(req)?;
        self.set_busy(Some("Solicitando servidor y clave a Streamlabs…"));
        self.log.info(format!(
            "Solicitando credenciales de LIVE (título «{}», categoría «{}», plataforma «{}»).",
            req.title,
            req.category.name,
            device_platform()
        ));
        let result = self.provider.start_live(&token, &req).await;
        self.with_inner(|i| i.busy = None);
        match result {
            Ok(creds) => {
                self.log.protect(&creds.key);
                if !(creds.server.starts_with("rtmp://") || creds.server.starts_with("rtmps://")) {
                    self.log
                        .warn("El servidor devuelto no empieza por rtmp:// ni rtmps://.");
                }
                let started = now_ms();
                let hint = creds.key.hint();
                let record = ActiveStreamRecord {
                    id: creds.id.clone(),
                    title: req.title.clone(),
                    category_name: req.category.name.clone(),
                    started_at_ms: started,
                };
                self.with_inner(|i| {
                    i.stream = Some(ActiveStream {
                        id: creds.id.clone(),
                        server: Some(creds.server.clone()),
                        key: Some(creds.key),
                        title: req.title.clone(),
                        category_name: req.category.name.clone(),
                        started_at_ms: started,
                        restored: false,
                    })
                });
                self.update_prefs(|p| {
                    p.active_stream = Some(record);
                    p.last_title = req.title.clone();
                    p.last_category = Some(req.category.clone());
                    p.audience_type = req.audience_type.clone();
                });
                self.log.success(format!(
                    "Sesión LIVE creada (id {}). Servidor {} y clave {hint} listos para OBS.",
                    creds.id, creds.server
                ));
                self.emit();
                Ok(self.snapshot())
            }
            Err(e) => {
                self.log.error(format!(
                    "No se pudieron obtener las credenciales: {}",
                    e.for_log()
                ));
                self.handle_auth_error(&e).await;
                self.emit();
                Err(e)
            }
        }
    }

    pub async fn end_stream(&self) -> AppResult<Snapshot> {
        let _guard = self.op.try_lock().map_err(|_| AppError::Busy)?;
        let id = self
            .with_inner(|i| i.stream.as_ref().map(|s| s.id.clone()))
            .ok_or(AppError::NoActiveStream)?;
        let token = self.token()?;
        self.set_busy(Some("Finalizando la sesión LIVE…"));
        let result = self.provider.end_live(&token, &id).await;
        self.with_inner(|i| i.busy = None);
        // 404: el proveedor ya no conoce ese LIVE (terminó por otra vía).
        let already_ended = matches!(result, Err(AppError::ProviderRejected { status: 404, .. }));
        match result {
            Ok(()) => {
                self.with_inner(|i| i.stream = None);
                self.update_prefs(|p| p.active_stream = None);
                self.log.success(format!("Sesión LIVE {id} finalizada."));
                self.emit();
                Ok(self.snapshot())
            }
            Err(_) if already_ended => {
                self.with_inner(|i| i.stream = None);
                self.update_prefs(|p| p.active_stream = None);
                self.log.warn(format!(
                    "El LIVE {id} ya no existía en Streamlabs; se da por finalizado."
                ));
                self.emit();
                Ok(self.snapshot())
            }
            Err(e) => {
                self.log
                    .error(format!("No se pudo finalizar el LIVE: {}", e.for_log()));
                self.handle_auth_error(&e).await;
                self.emit();
                Err(e)
            }
        }
    }

    pub fn reveal_key(&self) -> AppResult<Secret> {
        let key = self.with_inner(|i| i.stream.as_ref().and_then(|s| s.key.clone()));
        match key {
            Some(k) => {
                self.log.info("Clave de transmisión mostrada en pantalla.");
                Ok(k)
            }
            None => Err(AppError::Validation(
                "La clave no está disponible (el LIVE se creó en una ejecución anterior).".into(),
            )),
        }
    }

    pub fn copy_text(&self, field: CopyField) -> AppResult<Secret> {
        let value = self.with_inner(|i| match field {
            CopyField::Server => i
                .stream
                .as_ref()
                .and_then(|s| s.server.clone())
                .map(Secret::new),
            CopyField::Key => i.stream.as_ref().and_then(|s| s.key.clone()),
            CopyField::LoginUrl => i.pending.as_ref().map(|p| Secret::new(p.login_url.clone())),
        });
        value.ok_or_else(|| AppError::Validation("No hay nada que copiar.".into()))
    }
}

#[cfg(test)]
mod tests;
