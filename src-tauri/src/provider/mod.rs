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

/// Estado del acceso a LIVE según el proveedor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum LiveAccess {
    /// Aprobada para generar servidor y clave mediante Streamlabs.
    Approved,
    /// Acceso "legacy": la clave se obtiene en TikTok LIVE Producer, no aquí.
    Legacy,
    /// No aprobada (incluye cuentas denegadas).
    NotApproved,
    /// Nunca solicitó acceso.
    NeverApplied,
    /// Solicitud rechazada.
    Rejected,
    /// El proveedor pide volver a iniciar sesión (permisos desactualizados).
    Relog,
    /// El proveedor no indicó el estado.
    Unknown,
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

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountInfo {
    pub username: String,
    pub display_name: Option<String>,
    pub live_access: LiveAccess,
    pub can_be_live: Option<bool>,
    pub application_status: Option<String>,
    pub application_timestamp: Option<String>,
    pub audience: AudienceControls,
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

    async fn account_info(&self, token: &Secret) -> AppResult<AccountInfo>;

    async fn search_categories(&self, token: &Secret, query: &str) -> AppResult<Vec<Category>>;

    async fn start_live(&self, token: &Secret, req: &StartLiveRequest) -> AppResult<LiveCredentials>;

    async fn end_live(&self, token: &Secret, id: &str) -> AppResult<()>;
}
