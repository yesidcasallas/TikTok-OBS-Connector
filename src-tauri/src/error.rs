//! Errores de la aplicación.
//!
//! Los mensajes están en español y **nunca** incluyen tokens, códigos de
//! autorización ni claves de transmisión. Hacia el frontend se serializan como
//! `{ code, category, message, detail, httpStatus }`:
//!
//! - `message`: explicación propia que no afirma causas sin confirmar.
//! - `detail`: mensaje original del proveedor (saneado), si lo hay.
//! - `category`: `auth`, `authorization`, `validation`, `rate_limit`,
//!   `server`, `unexpected`, `network`, `access`, `provider` o `app`.

use serde::{Serialize, Serializer};

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum AppError {
    #[error("Inicia sesión con TikTok antes de continuar.")]
    NotLoggedIn,

    #[error("Ya hay un inicio de sesión en curso. Complétalo en el navegador o cancélalo.")]
    AuthInProgress,

    #[error("Ya has iniciado sesión. Cierra la sesión para cambiar de cuenta.")]
    AlreadyLoggedIn,

    #[error("Inicio de sesión cancelado.")]
    AuthCancelled,

    #[error("Se agotó el tiempo para completar el inicio de sesión en el navegador ({0} min). Inténtalo de nuevo.")]
    AuthTimeout(u64),

    #[error("Streamlabs no completó el inicio de sesión: {0}")]
    AuthDenied(String),

    #[error("Streamlabs rechazó el código de autorización (caducado, ya usado o no válido). Vuelve a iniciar sesión.")]
    AuthExchangeRejected,

    #[error("No se pudo preparar el retorno local del inicio de sesión: {0}")]
    AuthServer(String),

    #[error("La sesión de Streamlabs no es válida o ha caducado (HTTP 401). Vuelve a iniciar sesión.")]
    TokenInvalid,

    /// Solo cuando el proveedor envía un código explícito de falta de permiso.
    #[error("Streamlabs indica expresamente que la cuenta no tiene permiso para emitir en TikTok LIVE desde PC.")]
    PermissionDenied { message: Option<String> },

    /// HTTP 403 con JSON pero sin código explícito: no se sabe la causa.
    #[error("Streamlabs no autorizó la solicitud (HTTP {status}). No indica el motivo: puede deberse a que la cuenta no tiene acceso a LIVE desde PC, a una suspensión o a una restricción temporal.")]
    ProviderForbidden { status: u16, message: Option<String> },

    #[error("Streamlabs respondió HTTP 422 al crear el LIVE. Streamlabs Desktop asocia este código a una cuenta con restricciones para emitir; confírmalo en TikTok LIVE Center.")]
    AccountRestricted { message: Option<String> },

    #[error("TikTok indica que ya hay un LIVE activo en esta cuenta. Finalízalo antes de crear otro.")]
    AlreadyLive,

    #[error("Streamlabs limita la frecuencia de solicitudes (HTTP 429). Espera unos minutos.")]
    RateLimited,

    #[error("Streamlabs bloqueó la solicitud (HTTP {status}). {detail}")]
    ProviderBlocked { status: u16, detail: String },

    #[error("Streamlabs rechazó la solicitud (HTTP {status}).")]
    ProviderRejected { status: u16, message: Option<String> },

    #[error("Streamlabs devolvió un error de servidor (HTTP {status}). Inténtalo más tarde.")]
    ProviderUnavailable { status: u16, message: Option<String> },

    #[error("Respuesta inesperada de Streamlabs: {0}.")]
    ProviderUnexpected(String),

    #[error("No se pudo conectar con Streamlabs: {0}. Comprueba tu conexión a Internet.")]
    Network(String),

    #[error("Se está comprobando el acceso a LIVE. Espera a que termine.")]
    AccessCheckInProgress,

    #[error("La emisión no está disponible para esta cuenta: {0}")]
    LiveNotAvailable(String),

    #[error("No se ha confirmado el permiso de emisión: {0} Pulsa «Actualizar estado».")]
    AccessUnconfirmed(String),

    #[error("No hay ningún LIVE activo creado desde esta aplicación.")]
    NoActiveStream,

    #[error("Ya hay un LIVE activo creado desde esta aplicación. Finalízalo antes de solicitar otro.")]
    StreamAlreadyCreated,

    #[error("Hay un LIVE activo. Finalízalo antes de cerrar la sesión; sin sesión no podrás finalizarlo desde aquí.")]
    StreamBlocksLogout,

    #[error("{0}")]
    Validation(String),

    #[error("Ya hay una operación en curso. Espera a que termine.")]
    Busy,

    #[error("Almacén de credenciales del sistema: {0}")]
    Keyring(String),

    #[error("No se pudo copiar al portapapeles: {0}")]
    Clipboard(String),

    #[error("Error interno: {0}")]
    Internal(String),
}

impl AppError {
    /// Código estable que usa el frontend para decidir cómo mostrar el error.
    pub fn code(&self) -> &'static str {
        match self {
            AppError::NotLoggedIn => "NOT_LOGGED_IN",
            AppError::AuthInProgress => "AUTH_IN_PROGRESS",
            AppError::AlreadyLoggedIn => "ALREADY_LOGGED_IN",
            AppError::AuthCancelled => "AUTH_CANCELLED",
            AppError::AuthTimeout(_) => "AUTH_TIMEOUT",
            AppError::AuthDenied(_) => "AUTH_DENIED",
            AppError::AuthExchangeRejected => "AUTH_EXCHANGE_REJECTED",
            AppError::AuthServer(_) => "AUTH_SERVER",
            AppError::TokenInvalid => "TOKEN_INVALID",
            AppError::PermissionDenied { .. } => "PERMISSION_DENIED",
            AppError::ProviderForbidden { .. } => "PROVIDER_FORBIDDEN",
            AppError::AccountRestricted { .. } => "ACCOUNT_RESTRICTED",
            AppError::AlreadyLive => "ALREADY_LIVE",
            AppError::RateLimited => "RATE_LIMITED",
            AppError::ProviderBlocked { .. } => "PROVIDER_BLOCKED",
            AppError::ProviderRejected { .. } => "PROVIDER_REJECTED",
            AppError::ProviderUnavailable { .. } => "PROVIDER_UNAVAILABLE",
            AppError::ProviderUnexpected(_) => "PROVIDER_UNEXPECTED",
            AppError::Network(_) => "NETWORK",
            AppError::AccessCheckInProgress => "ACCESS_CHECK_IN_PROGRESS",
            AppError::LiveNotAvailable(_) => "LIVE_NOT_AVAILABLE",
            AppError::AccessUnconfirmed(_) => "ACCESS_UNCONFIRMED",
            AppError::NoActiveStream => "NO_ACTIVE_STREAM",
            AppError::StreamAlreadyCreated => "STREAM_ALREADY_CREATED",
            AppError::StreamBlocksLogout => "STREAM_BLOCKS_LOGOUT",
            AppError::Validation(_) => "VALIDATION",
            AppError::Busy => "BUSY",
            AppError::Keyring(_) => "KEYRING",
            AppError::Clipboard(_) => "CLIPBOARD",
            AppError::Internal(_) => "INTERNAL",
        }
    }

    /// Clasificación del error.
    pub fn category(&self) -> &'static str {
        match self {
            AppError::TokenInvalid
            | AppError::AuthExchangeRejected
            | AppError::AuthDenied(_)
            | AppError::NotLoggedIn => "auth",
            AppError::PermissionDenied { .. }
            | AppError::ProviderForbidden { .. }
            | AppError::AccountRestricted { .. }
            | AppError::ProviderBlocked { .. } => "authorization",
            AppError::ProviderRejected { status, .. } if (400..500).contains(status) => "validation",
            AppError::ProviderRejected { .. } | AppError::AlreadyLive => "provider",
            AppError::RateLimited => "rate_limit",
            AppError::ProviderUnavailable { .. } => "server",
            AppError::ProviderUnexpected(_) => "unexpected",
            AppError::Network(_) => "network",
            AppError::AccessCheckInProgress | AppError::LiveNotAvailable(_) | AppError::AccessUnconfirmed(_) => {
                "access"
            }
            _ => "app",
        }
    }

    /// Mensaje original del proveedor (ya saneado), si existe.
    pub fn provider_detail(&self) -> Option<&str> {
        match self {
            AppError::PermissionDenied { message }
            | AppError::ProviderForbidden { message, .. }
            | AppError::AccountRestricted { message }
            | AppError::ProviderRejected { message, .. }
            | AppError::ProviderUnavailable { message, .. } => message.as_deref(),
            _ => None,
        }
    }

    /// Código HTTP de la respuesta del proveedor, si se conoce.
    pub fn http_status(&self) -> Option<u16> {
        match self {
            AppError::TokenInvalid => Some(401),
            AppError::RateLimited => Some(429),
            AppError::AccountRestricted { .. } => Some(422),
            AppError::ProviderForbidden { status, .. }
            | AppError::ProviderBlocked { status, .. }
            | AppError::ProviderRejected { status, .. }
            | AppError::ProviderUnavailable { status, .. } => Some(*status),
            _ => None,
        }
    }

    /// Texto completo para el registro (mensaje + detalle del proveedor).
    pub fn for_log(&self) -> String {
        match self.provider_detail() {
            Some(d) => format!("{} [{}] Detalle del proveedor: «{d}»", self, self.code()),
            None => format!("{} [{}]", self, self.code()),
        }
    }
}

impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        ErrorInfo::from(self).serialize(serializer)
    }
}

/// Versión serializable y clonable de un error para guardarla en el estado.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorInfo {
    pub code: String,
    pub category: String,
    pub message: String,
    pub detail: Option<String>,
    pub http_status: Option<u16>,
}

impl From<&AppError> for ErrorInfo {
    fn from(e: &AppError) -> Self {
        ErrorInfo {
            code: e.code().to_string(),
            category: e.category().to_string(),
            message: e.to_string(),
            detail: e.provider_detail().map(str::to_string),
            http_status: e.http_status(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_all_fields() {
        let e = AppError::ProviderForbidden {
            status: 403,
            message: Some("Unable to go live. Please try again later".into()),
        };
        let json = serde_json::to_value(&e).unwrap();
        assert_eq!(json["code"], "PROVIDER_FORBIDDEN");
        assert_eq!(json["category"], "authorization");
        assert_eq!(json["httpStatus"], 403);
        assert_eq!(json["detail"], "Unable to go live. Please try again later");
        // La explicación no afirma una causa concreta.
        assert!(json["message"].as_str().unwrap().contains("No indica el motivo"));
    }

    #[test]
    fn categories_distinguish_error_kinds() {
        assert_eq!(AppError::TokenInvalid.category(), "auth");
        assert_eq!(AppError::PermissionDenied { message: None }.category(), "authorization");
        assert_eq!(AppError::ProviderRejected { status: 400, message: None }.category(), "validation");
        assert_eq!(AppError::ProviderRejected { status: 200, message: None }.category(), "provider");
        assert_eq!(AppError::RateLimited.category(), "rate_limit");
        assert_eq!(AppError::ProviderUnavailable { status: 502, message: None }.category(), "server");
        assert_eq!(AppError::ProviderUnexpected("x".into()).category(), "unexpected");
        assert_eq!(AppError::Network("x".into()).category(), "network");
        assert_eq!(AppError::AccessUnconfirmed("x".into()).category(), "access");
    }
}
