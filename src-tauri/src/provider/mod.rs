//! Abstracción del proveedor que autentica y emite credenciales de LIVE.
//!
//! Toda la lógica dependiente de Streamlabs (URLs, parámetros, formato de las
//! respuestas, significado de los códigos) vive en [`streamlabs`]. Si el API
//! interno cambia, solo hay que tocar ese módulo.

pub mod streamlabs;

use crate::auth::CallbackInterpreter;
use crate::error::AppResult;
use crate::security::Secret;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Resultado del intercambio del código de autorización.
/// (`Debug` es seguro: `Secret` nunca muestra su valor.)
#[derive(Debug)]
pub struct AuthGrant {
    pub token: Secret,
    pub username: Option<String>,
    pub platform: Option<String>,
}

/// Valor de un campo de la respuesta que conserva la diferencia entre
/// presente, ausente (o `null`) y presente con un tipo inesperado.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum Field<T> {
    Present { value: T },
    Missing,
    /// `found` es el tipo JSON recibido (nunca el valor).
    Invalid { found: String },
}

impl<T> Field<T> {
    pub fn value(&self) -> Option<&T> {
        match self {
            Field::Present { value } => Some(value),
            _ => None,
        }
    }
}

/// Ámbito del acceso según `reason` y `application_status`
/// (réplica de `convertScope` de Streamlabs Desktop).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum LiveScope {
    /// `reason = 1`: aprobada para generar servidor y clave mediante Streamlabs.
    Approved,
    /// `reason = 2`: acceso «legacy»; la clave se obtiene en TikTok LIVE Producer.
    Legacy,
    /// `reason = 0`.
    NotApproved,
    /// `application_status = never_applied`.
    NeverApplied,
    /// `reason = 0` y `application_status = rejected`.
    Rejected,
    /// `reason = -1`: Streamlabs pide volver a iniciar sesión.
    Relog,
    /// Falta `reason` o tiene un valor o tipo no reconocido.
    Unknown,
}

/// Motivo identificado por el proveedor por el que no se puede emitir.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum UnavailableCause {
    NotApproved,
    NeverApplied,
    Rejected,
    Legacy,
    Relog,
    /// `can_be_live = false` sin un `reason` reconocible.
    CannotGoLive,
}

/// Conclusión sobre el permiso de emisión.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Availability {
    /// Permiso de emisión confirmado por el proveedor.
    Confirmed,
    /// El proveedor identifica que la emisión no está disponible.
    NotAvailable { cause: UnavailableCause },
    /// Los campos se contradicen: hay que actualizar la comprobación.
    Contradictory { detail: String },
    /// La respuesta no permite saberlo (campos ausentes o con tipo inesperado).
    Unknown { detail: String },
}

/// Campos de estado de `/tiktok/info` y su interpretación.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccessCheck {
    pub reason: Field<i64>,
    pub can_be_live: Field<bool>,
    pub application_status: Field<String>,
    pub user_present: bool,
    pub scope: LiveScope,
    pub availability: Availability,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudienceOption {
    pub value: String,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudienceControls {
    pub disabled: bool,
    pub default_value: String,
    pub options: Vec<AudienceOption>,
}

/// Resultado completo de una comprobación de cuenta y acceso.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountInfo {
    pub username: Option<String>,
    pub display_name: Option<String>,
    pub access: AccessCheck,
    pub audience: AudienceControls,
    pub http_status: u16,
    pub duration_ms: u64,
    pub checked_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Category {
    /// Identificador del proveedor (`game_mask_id`); vacío = "Otra".
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartLiveRequest {
    pub title: String,
    pub category: Category,
    pub audience_type: String,
}

/// Credenciales devueltas al crear la sesión LIVE.
#[derive(Debug)]
pub struct LiveCredentials {
    pub id: String,
    pub server: String,
    pub key: Secret,
}

#[async_trait]
pub trait LiveProvider: CallbackInterpreter {
    fn display_name(&self) -> &'static str;

    /// URL de login que se abre en el navegador del sistema.
    fn login_url(&self, port: u16, code_challenge: &str) -> String;

    async fn exchange_code(&self, code: &Secret, verifier: &Secret) -> AppResult<AuthGrant>;

    /// Solo lectura: nunca crea sesiones LIVE.
    async fn account_info(&self, token: &Secret) -> AppResult<AccountInfo>;

    async fn search_categories(&self, token: &Secret, query: &str) -> AppResult<Vec<Category>>;

    async fn start_live(&self, token: &Secret, req: &StartLiveRequest) -> AppResult<LiveCredentials>;

    async fn end_live(&self, token: &Secret, id: &str) -> AppResult<()>;
}
