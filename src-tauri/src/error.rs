//! Errores de la aplicación.
//!
//! Los mensajes están en español y **nunca** incluyen tokens, códigos de
//! autorización ni claves de transmisión. Hacia el frontend se serializan como
//! `{ code, message }` para que la interfaz pueda reaccionar según el código.

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

    #[error("La sesión de Streamlabs no es válida o ha caducado. Vuelve a iniciar sesión.")]
    TokenInvalid,

    #[error("Tu cuenta no tiene permiso para emitir en TikTok LIVE mediante Streamlabs. {0}")]
    PermissionDenied(String),

    #[error("TikTok o Streamlabs indican que la cuenta tiene restricciones para emitir (HTTP 422). Revisa el estado de la cuenta en TikTok LIVE Center.")]
    AccountRestricted,

    #[error("TikTok indica que ya hay un LIVE activo en esta cuenta. Finalízalo antes de crear otro.")]
    AlreadyLive,

    #[error("Streamlabs limita la frecuencia de solicitudes. Espera unos minutos y vuelve a intentarlo.")]
    RateLimited,

    #[error("Streamlabs bloqueó la solicitud (HTTP {status}). {detail}")]
    ProviderBlocked { status: u16, detail: String },

    #[error("Streamlabs rechazó la solicitud (HTTP {status}): {message}")]
    ProviderRejected { status: u16, message: String },

    #[error("Streamlabs no está disponible o devolvió un error interno (HTTP {0}). Inténtalo más tarde.")]
    ProviderUnavailable(u16),

    #[error("Respuesta inesperada de Streamlabs: {0}. Es posible que su API interno haya cambiado.")]
    ProviderUnexpected(String),

    #[error("No se pudo conectar con Streamlabs: {0}. Comprueba tu conexión a Internet.")]
    Network(String),

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
            AppError::PermissionDenied(_) => "PERMISSION_DENIED",
            AppError::AccountRestricted => "ACCOUNT_RESTRICTED",
            AppError::AlreadyLive => "ALREADY_LIVE",
            AppError::RateLimited => "RATE_LIMITED",
            AppError::ProviderBlocked { .. } => "PROVIDER_BLOCKED",
            AppError::ProviderRejected { .. } => "PROVIDER_REJECTED",
            AppError::ProviderUnavailable(_) => "PROVIDER_UNAVAILABLE",
            AppError::ProviderUnexpected(_) => "PROVIDER_UNEXPECTED",
            AppError::Network(_) => "NETWORK",
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

    /// Clasificación para la interfaz: distingue autenticación, permisos y
    /// fallos del proveedor.
    pub fn category(&self) -> &'static str {
        match self {
            AppError::TokenInvalid
            | AppError::AuthExchangeRejected
            | AppError::AuthDenied(_)
            | AppError::NotLoggedIn => "auth",
            AppError::PermissionDenied(_) | AppError::AccountRestricted => "permission",
            AppError::ProviderBlocked { .. }
            | AppError::ProviderRejected { .. }
            | AppError::ProviderUnavailable(_)
            | AppError::ProviderUnexpected(_)
            | AppError::RateLimited
            | AppError::AlreadyLive => "provider",
            AppError::Network(_) => "network",
            _ => "app",
        }
    }
}

impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut s = serializer.serialize_struct("AppError", 3)?;
        s.serialize_field("code", self.code())?;
        s.serialize_field("category", self.category())?;
        s.serialize_field("message", &self.to_string())?;
        s.end()
    }
}

/// Versión serializable y clonable de un error para guardarla en el estado.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorInfo {
    pub code: String,
    pub category: String,
    pub message: String,
}

impl From<&AppError> for ErrorInfo {
    fn from(e: &AppError) -> Self {
        ErrorInfo {
            code: e.code().to_string(),
            category: e.category().to_string(),
            message: e.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_code_category_and_message() {
        let json = serde_json::to_value(AppError::TokenInvalid).unwrap();
        assert_eq!(json["code"], "TOKEN_INVALID");
        assert_eq!(json["category"], "auth");
        assert!(json["message"].as_str().unwrap().contains("sesión"));
    }

    #[test]
    fn distinguishes_auth_permission_and_provider() {
        assert_eq!(AppError::TokenInvalid.category(), "auth");
        assert_eq!(AppError::PermissionDenied(String::new()).category(), "permission");
        assert_eq!(AppError::ProviderUnavailable(502).category(), "provider");
        assert_eq!(AppError::Network("x".into()).category(), "network");
    }
}
