//! Interpretación tolerante de las respuestas de Streamlabs.
//!
//! Los tipos de referencia están en `app/services/platforms/tiktok/api.ts` de
//! Streamlabs Desktop (`ITikTokLiveScopeResponse`, `ETikTokLiveScopeReason`,
//! `ITikTokAudienceControlsInfo`, `ITikTokStartStreamResponse`).

use crate::provider::{AccountInfo, AudienceControls, AudienceOption, Category, LiveAccess};
use serde::Deserialize;
use serde_json::Value;
use zeroize::{Zeroize, ZeroizeOnDrop};

/// `GET /api/v5/slobs/auth/data` (solo los campos necesarios; el resto, como
/// `platform_token` o `token`, se descarta sin copiarse).
#[derive(Deserialize, Zeroize, ZeroizeOnDrop)]
pub struct AuthDataResponse {
    #[serde(default)]
    #[zeroize(skip)]
    pub success: Option<bool>,
    #[serde(default)]
    pub data: Option<AuthData>,
}

#[derive(Deserialize, Zeroize, ZeroizeOnDrop)]
pub struct AuthData {
    #[serde(default)]
    pub oauth_token: Option<String>,
    #[serde(default)]
    pub platform: Option<String>,
    #[serde(default)]
    pub platform_username: Option<String>,
}

/// `POST /api/v5/slobs/tiktok/stream/start`.
#[derive(Deserialize, Zeroize, ZeroizeOnDrop)]
pub struct StartResponse {
    #[serde(default)]
    #[zeroize(skip)]
    pub id: Option<Value>,
    #[serde(default)]
    pub rtmp: Option<String>,
    #[serde(default)]
    pub key: Option<String>,
}

impl StartResponse {
    pub fn id_string(&self) -> Option<String> {
        self.id.as_ref().and_then(as_string).filter(|s| !s.is_empty())
    }
}

/// Valores de `reason` (enum `ETikTokLiveScopeReason`).
pub mod reason {
    pub const RELOG: i64 = -1;
    pub const NOT_APPROVED: i64 = 0;
    pub const APPROVED: i64 = 1;
    pub const APPROVED_OBS: i64 = 2;
}

pub fn as_i64(v: &Value) -> Option<i64> {
    v.as_i64().or_else(|| v.as_str()?.trim().parse().ok())
}

pub fn as_bool(v: &Value) -> Option<bool> {
    v.as_bool()
        .or_else(|| v.as_i64().map(|n| n != 0))
        .or_else(|| match v.as_str()?.trim() {
            "true" | "1" => Some(true),
            "false" | "0" => Some(false),
            _ => None,
        })
}

pub fn as_string(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.trim().to_string()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

/// Réplica de `convertScope` de Streamlabs Desktop, sin convertir valores
/// desconocidos en "denegado".
pub fn convert_scope(
    reason: Option<i64>,
    application_status: Option<&str>,
    has_user: bool,
    has_info: bool,
) -> LiveAccess {
    if application_status == Some("never_applied") && reason != Some(reason::APPROVED_OBS) {
        return LiveAccess::NeverApplied;
    }
    match reason {
        Some(reason::APPROVED) => LiveAccess::Approved,
        Some(reason::APPROVED_OBS) => LiveAccess::Legacy,
        Some(reason::RELOG) => LiveAccess::Relog,
        Some(reason::NOT_APPROVED) => {
            if application_status == Some("rejected") {
                LiveAccess::Rejected
            } else {
                LiveAccess::NotApproved
            }
        }
        Some(_) => LiveAccess::Unknown,
        // Desktop: sin `user` pero con `info` (y sin reason) => volver a iniciar sesión.
        None if !has_user && has_info => LiveAccess::Relog,
        None => LiveAccess::Unknown,
    }
}

fn default_audience_label(value: &str) -> String {
    match value {
        "0" => "Todos los públicos".to_string(),
        "1" => "Solo adultos (18+)".to_string(),
        other => format!("Tipo {other}"),
    }
}

pub fn default_audience() -> AudienceControls {
    AudienceControls {
        disabled: false,
        default_value: "0".into(),
        options: ["0", "1"]
            .iter()
            .map(|v| AudienceOption { value: v.to_string(), label: default_audience_label(v) })
            .collect(),
    }
}

pub fn parse_audience(v: Option<&Value>) -> AudienceControls {
    let Some(obj) = v.filter(|v| v.is_object()) else {
        return default_audience();
    };
    let disabled = obj.get("disable").and_then(as_bool).unwrap_or(false);
    let mut options: Vec<AudienceOption> = obj
        .get("types")
        .and_then(Value::as_array)
        .map(|types| {
            types
                .iter()
                .filter_map(|t| {
                    let value = t.get("key").and_then(as_string)?;
                    let label = t
                        .get("label")
                        .and_then(Value::as_str)
                        .map(str::trim)
                        .filter(|l| !l.is_empty())
                        .map(str::to_string)
                        .unwrap_or_else(|| default_audience_label(&value));
                    Some(AudienceOption { value, label })
                })
                .collect()
        })
        .unwrap_or_default();
    if options.is_empty() {
        options = default_audience().options;
    }
    let default_value = obj
        .get("info_type")
        .and_then(as_string)
        .filter(|d| options.iter().any(|o| &o.value == d))
        .unwrap_or_else(|| options[0].value.clone());
    AudienceControls { disabled, default_value, options }
}

pub fn parse_account(v: &Value, now_ms: u64) -> AccountInfo {
    let reason = v.get("reason").and_then(as_i64);
    let app = v.get("application_status").filter(|a| a.is_object());
    let application_status = app
        .and_then(|a| a.get("status"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let application_timestamp = app.and_then(|a| a.get("timestamp")).and_then(as_string);
    let user = v.get("user").filter(|u| u.is_object());
    let has_info = v.get("info").map(|i| !i.is_null()).unwrap_or(false);
    let live_access = convert_scope(reason, application_status.as_deref(), user.is_some(), has_info);
    AccountInfo {
        username: user
            .and_then(|u| u.get("username"))
            .and_then(as_string)
            .unwrap_or_default(),
        display_name: user
            .and_then(|u| u.get("display_name"))
            .and_then(as_string)
            .filter(|s| !s.is_empty()),
        live_access,
        can_be_live: v.get("can_be_live").and_then(as_bool),
        application_status,
        application_timestamp,
        audience: parse_audience(v.get("audience_controls_info")),
        checked_at_ms: now_ms,
    }
}

pub fn parse_categories(v: &Value) -> Option<Vec<Category>> {
    let list = v.get("categories")?.as_array()?;
    Some(
        list.iter()
            .filter_map(|c| {
                let name = c.get("full_name").and_then(as_string).filter(|n| !n.is_empty())?;
                let id = c.get("game_mask_id").and_then(as_string).unwrap_or_default();
                Some(Category { id, name })
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn scope_matrix_matches_streamlabs_desktop() {
        use LiveAccess::*;
        assert_eq!(convert_scope(Some(1), Some("approved"), true, true), Approved);
        assert_eq!(convert_scope(Some(2), Some("never_applied"), true, true), Legacy);
        assert_eq!(convert_scope(Some(1), Some("never_applied"), true, true), NeverApplied);
        assert_eq!(convert_scope(Some(0), None, true, true), NotApproved);
        assert_eq!(convert_scope(Some(0), Some("rejected"), true, true), Rejected);
        assert_eq!(convert_scope(Some(-1), None, true, true), Relog);
        assert_eq!(convert_scope(None, None, false, true), Relog);
        assert_eq!(convert_scope(None, None, true, false), Unknown);
        assert_eq!(convert_scope(Some(7), None, true, true), Unknown);
    }

    #[test]
    fn parses_full_info_response() {
        let v = json!({
            "platform": "tiktok",
            "reason": 1,
            "can_be_live": true,
            "user": { "username": "creadora", "display_name": "Creadora", "open_id": "x" },
            "info": [],
            "application_status": { "status": "approved", "timestamp": null },
            "audience_controls_info": {
                "disable": false, "info_type": 0,
                "types": [ { "key": 0, "label": "Everyone" }, { "key": 1, "label": null } ]
            }
        });
        let a = parse_account(&v, 42);
        assert_eq!(a.username, "creadora");
        assert_eq!(a.display_name.as_deref(), Some("Creadora"));
        assert_eq!(a.live_access, LiveAccess::Approved);
        assert_eq!(a.can_be_live, Some(true));
        assert_eq!(a.application_status.as_deref(), Some("approved"));
        assert_eq!(a.audience.default_value, "0");
        assert_eq!(a.audience.options[0].label, "Everyone");
        assert_eq!(a.audience.options[1].label, "Solo adultos (18+)");
        assert_eq!(a.checked_at_ms, 42);
    }

    #[test]
    fn tolerates_strings_and_missing_fields() {
        let v = json!({ "reason": "0", "can_be_live": "false" });
        let a = parse_account(&v, 0);
        assert_eq!(a.live_access, LiveAccess::NotApproved);
        assert_eq!(a.can_be_live, Some(false));
        assert_eq!(a.audience, default_audience());
        assert_eq!(a.username, "");
    }

    #[test]
    fn parses_categories() {
        let v = json!({ "categories": [
            { "full_name": "Fortnite", "game_mask_id": "123" },
            { "full_name": "Minecraft", "game_mask_id": 456 },
            { "full_name": "" }
        ]});
        let c = parse_categories(&v).unwrap();
        assert_eq!(c.len(), 2);
        assert_eq!(c[1], Category { id: "456".into(), name: "Minecraft".into() });
        assert!(parse_categories(&json!({})).is_none());
    }

    #[test]
    fn start_response_accepts_numeric_id() {
        let r: StartResponse =
            serde_json::from_value(json!({ "id": 99, "rtmp": "rtmp://a/b", "key": "k" })).unwrap();
        assert_eq!(r.id_string().as_deref(), Some("99"));
    }
}
