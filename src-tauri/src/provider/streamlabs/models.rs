//! Interpretación de las respuestas de Streamlabs.
//!
//! Tipos de referencia: `app/services/platforms/tiktok/api.ts` de Streamlabs
//! Desktop (`ITikTokLiveScopeResponse`, `ETikTokLiveScopeReason`,
//! `ITikTokAudienceControlsInfo`, `ITikTokStartStreamResponse`).
//!
//! Los campos de estado se leen de forma **estricta**: `reason` debe ser un
//! entero, `can_be_live` un booleano y `application_status.status` una cadena.
//! Un campo ausente o `null` es [`Field::Missing`]; un tipo distinto es
//! [`Field::Invalid`]. Ninguno de los dos se interpreta como «no aprobada».

use crate::provider::{
    AccessCheck, AccountInfo, AudienceControls, AudienceOption, Availability, Category, Field,
    LiveScope, UnavailableCause,
};
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
        self.id
            .as_ref()
            .and_then(as_string)
            .filter(|s| !s.is_empty())
    }
}

/// Valores de `reason` (enum `ETikTokLiveScopeReason`).
pub mod reason {
    pub const RELOG: i64 = -1;
    pub const NOT_APPROVED: i64 = 0;
    pub const APPROVED: i64 = 1;
    pub const APPROVED_OBS: i64 = 2;
}

pub fn as_string(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.trim().to_string()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

fn json_type(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "booleano",
        Value::Number(_) => "número",
        Value::String(_) => "cadena",
        Value::Array(_) => "lista",
        Value::Object(_) => "objeto",
    }
}

pub fn field_i64(v: Option<&Value>) -> Field<i64> {
    match v {
        None | Some(Value::Null) => Field::Missing,
        Some(Value::Number(n)) => match n.as_i64() {
            Some(value) => Field::Present { value },
            None => Field::Invalid {
                found: "número no entero".into(),
            },
        },
        Some(other) => Field::Invalid {
            found: json_type(other).into(),
        },
    }
}

pub fn field_bool(v: Option<&Value>) -> Field<bool> {
    match v {
        None | Some(Value::Null) => Field::Missing,
        Some(Value::Bool(value)) => Field::Present { value: *value },
        Some(other) => Field::Invalid {
            found: json_type(other).into(),
        },
    }
}

/// `application_status.status` (objeto anidado).
pub fn field_application_status(v: Option<&Value>) -> Field<String> {
    match v {
        None | Some(Value::Null) => Field::Missing,
        Some(Value::Object(o)) => match o.get("status") {
            None | Some(Value::Null) => Field::Missing,
            Some(Value::String(s)) if s.trim().is_empty() => Field::Missing,
            Some(Value::String(s)) => Field::Present {
                value: s.trim().chars().take(40).collect(),
            },
            Some(other) => Field::Invalid {
                found: json_type(other).into(),
            },
        },
        Some(other) => Field::Invalid {
            found: format!("{} (se esperaba objeto)", json_type(other)),
        },
    }
}

/// Texto breve de un campo para registros y diagnóstico (sin valores sensibles).
pub fn describe<T: std::fmt::Display>(f: &Field<T>) -> String {
    match f {
        Field::Present { value } => value.to_string(),
        Field::Missing => "ausente".into(),
        Field::Invalid { found } => format!("tipo inesperado ({found})"),
    }
}

/// Resumen de los campos de estado para el registro de diagnóstico.
pub fn summary(a: &AccessCheck) -> String {
    format!(
        "reason={} · can_be_live={} · application_status={} · user={}",
        describe(&a.reason),
        describe(&a.can_be_live),
        describe(&a.application_status),
        if a.user_present {
            "presente"
        } else {
            "ausente"
        }
    )
}

/// Ámbito según `reason` y `application_status` (réplica de `convertScope`
/// de Streamlabs Desktop, sin convertir valores desconocidos en «denegada»).
pub fn scope_of(
    reason_f: &Field<i64>,
    app: &Field<String>,
    user_present: bool,
    info_present: bool,
) -> LiveScope {
    let app_status = app.value().map(String::as_str);
    let reason_v = reason_f.value().copied();
    if app_status == Some("never_applied") && reason_v != Some(reason::APPROVED_OBS) {
        return LiveScope::NeverApplied;
    }
    match reason_f {
        Field::Present { value } => match *value {
            reason::APPROVED => LiveScope::Approved,
            reason::APPROVED_OBS => LiveScope::Legacy,
            reason::RELOG => LiveScope::Relog,
            reason::NOT_APPROVED if app_status == Some("rejected") => LiveScope::Rejected,
            reason::NOT_APPROVED => LiveScope::NotApproved,
            _ => LiveScope::Unknown,
        },
        // Desktop: sin `user` pero con `info` (y sin reason) => volver a iniciar sesión.
        Field::Missing if !user_present && info_present => LiveScope::Relog,
        Field::Missing | Field::Invalid { .. } => LiveScope::Unknown,
    }
}

/// Conclusión sobre el permiso de emisión.
///
/// | ámbito \ can_be_live | true | false | ausente/inválido |
/// |---|---|---|---|
/// | aprobada | confirmado | contradictorio | confirmado (Desktop solo usa `reason`) |
/// | no aprobada / nunca solicitada / rechazada | contradictorio | no disponible | no disponible |
/// | legacy / volver a iniciar sesión | no disponible | no disponible | no disponible |
/// | desconocido | confirmado (la referencia usa `can_be_live`) | no disponible | **desconocido** |
pub fn availability_of(scope: LiveScope, can: &Field<bool>, reason_f: &Field<i64>) -> Availability {
    let can_v = can.value().copied();
    match scope {
        LiveScope::Approved => match can_v {
            Some(false) => Availability::Contradictory {
                detail: "reason=1 (aprobada) pero can_be_live=false".into(),
            },
            _ => Availability::Confirmed,
        },
        LiveScope::NotApproved | LiveScope::NeverApplied | LiveScope::Rejected => match can_v {
            Some(true) => Availability::Contradictory {
                detail: "la solicitud figura como no aprobada pero can_be_live=true".into(),
            },
            _ => Availability::NotAvailable {
                cause: match scope {
                    LiveScope::NeverApplied => UnavailableCause::NeverApplied,
                    LiveScope::Rejected => UnavailableCause::Rejected,
                    _ => UnavailableCause::NotApproved,
                },
            },
        },
        LiveScope::Legacy => Availability::NotAvailable {
            cause: UnavailableCause::Legacy,
        },
        LiveScope::Relog => Availability::NotAvailable {
            cause: UnavailableCause::Relog,
        },
        LiveScope::Unknown => match can_v {
            Some(true) => Availability::Confirmed,
            Some(false) => Availability::NotAvailable {
                cause: UnavailableCause::CannotGoLive,
            },
            None => Availability::Unknown {
                detail: format!(
                    "reason {} y can_be_live {}",
                    describe(reason_f),
                    describe(can)
                ),
            },
        },
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
            .map(|v| AudienceOption {
                value: v.to_string(),
                label: default_audience_label(v),
            })
            .collect(),
    }
}

pub fn parse_audience(v: Option<&Value>) -> AudienceControls {
    let Some(obj) = v.filter(|v| v.is_object()) else {
        return default_audience();
    };
    let disabled = obj.get("disable").and_then(Value::as_bool).unwrap_or(false);
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
    AudienceControls {
        disabled,
        default_value,
        options,
    }
}

/// Interpreta `/tiktok/info` (estado HTTP y tipo de contenido ya validados).
pub fn parse_account(v: &Value, http_status: u16, duration_ms: u64, now_ms: u64) -> AccountInfo {
    let user = v.get("user").filter(|u| u.is_object());
    let info_present = v.get("info").map(|i| !i.is_null()).unwrap_or(false);
    let reason_f = field_i64(v.get("reason"));
    let can = field_bool(v.get("can_be_live"));
    let app = field_application_status(v.get("application_status"));
    let scope = scope_of(&reason_f, &app, user.is_some(), info_present);
    let availability = availability_of(scope, &can, &reason_f);
    AccountInfo {
        username: user
            .and_then(|u| u.get("username"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string),
        display_name: user
            .and_then(|u| u.get("display_name"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string),
        access: AccessCheck {
            reason: reason_f,
            can_be_live: can,
            application_status: app,
            user_present: user.is_some(),
            scope,
            availability,
        },
        audience: parse_audience(v.get("audience_controls_info")),
        http_status,
        duration_ms,
        checked_at_ms: now_ms,
    }
}

pub fn parse_categories(v: &Value) -> Option<Vec<Category>> {
    let list = v.get("categories")?.as_array()?;
    Some(
        list.iter()
            .filter_map(|c| {
                let name = c
                    .get("full_name")
                    .and_then(as_string)
                    .filter(|n| !n.is_empty())?;
                let id = c
                    .get("game_mask_id")
                    .and_then(as_string)
                    .unwrap_or_default();
                Some(Category { id, name })
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn access(v: Value) -> AccessCheck {
        parse_account(&v, 200, 1, 0).access
    }

    #[test]
    fn confirmed_permission() {
        let a = access(
            json!({"reason": 1, "can_be_live": true, "user": {"username": "u"},
                              "application_status": {"status": "approved"}}),
        );
        assert_eq!(a.scope, LiveScope::Approved);
        assert_eq!(a.availability, Availability::Confirmed);
    }

    #[test]
    fn emission_not_available() {
        let a = access(json!({"reason": 0, "can_be_live": false, "user": {"username": "u"}}));
        assert_eq!(
            a.availability,
            Availability::NotAvailable {
                cause: UnavailableCause::NotApproved
            }
        );
        let a = access(
            json!({"reason": 0, "user": {}, "application_status": {"status": "never_applied"}}),
        );
        assert_eq!(
            a.availability,
            Availability::NotAvailable {
                cause: UnavailableCause::NeverApplied
            }
        );
        let a =
            access(json!({"reason": 0, "user": {}, "application_status": {"status": "rejected"}}));
        assert_eq!(
            a.availability,
            Availability::NotAvailable {
                cause: UnavailableCause::Rejected
            }
        );
        let a = access(json!({"reason": 2, "can_be_live": true, "user": {}}));
        assert_eq!(
            a.availability,
            Availability::NotAvailable {
                cause: UnavailableCause::Legacy
            }
        );
        let a = access(json!({"can_be_live": false, "user": {}}));
        assert_eq!(
            a.availability,
            Availability::NotAvailable {
                cause: UnavailableCause::CannotGoLive
            }
        );
    }

    #[test]
    fn missing_null_or_wrong_types_are_unknown_not_rejected() {
        for v in [
            json!({"user": {"username": "u"}}),
            json!({"reason": null, "can_be_live": null, "user": {}}),
            json!({"reason": "0", "can_be_live": "false", "user": {}}),
            json!({"reason": 1.5, "can_be_live": 0, "user": {}}),
            json!({"reason": [], "can_be_live": {}, "user": {}}),
        ] {
            let a = access(v.clone());
            assert!(
                matches!(a.availability, Availability::Unknown { .. }),
                "{v}: {:?}",
                a.availability
            );
            assert!(!matches!(a.availability, Availability::NotAvailable { .. }));
        }
        let a = access(json!({"reason": "0", "can_be_live": "true", "user": {}}));
        assert_eq!(
            a.reason,
            Field::Invalid {
                found: "cadena".into()
            }
        );
        assert_eq!(
            a.can_be_live,
            Field::Invalid {
                found: "cadena".into()
            }
        );
    }

    #[test]
    fn contradictory_data_is_reported() {
        let a = access(json!({"reason": 1, "can_be_live": false, "user": {}}));
        assert!(matches!(a.availability, Availability::Contradictory { .. }));
        let a = access(json!({"reason": 0, "can_be_live": true, "user": {}}));
        assert!(matches!(a.availability, Availability::Contradictory { .. }));
    }

    #[test]
    fn approved_without_can_be_live_follows_desktop() {
        let a = access(json!({"reason": 1, "user": {}}));
        assert_eq!(a.availability, Availability::Confirmed);
        let a = access(json!({"can_be_live": true, "user": {}}));
        assert_eq!(a.availability, Availability::Confirmed);
    }

    #[test]
    fn nested_application_status_is_validated() {
        assert_eq!(
            field_application_status(Some(&json!({"status": "approved"}))),
            Field::Present {
                value: "approved".into()
            }
        );
        assert_eq!(
            field_application_status(Some(&json!({"status": null}))),
            Field::Missing
        );
        assert_eq!(field_application_status(Some(&json!(null))), Field::Missing);
        assert!(matches!(
            field_application_status(Some(&json!("approved"))),
            Field::Invalid { .. }
        ));
        assert!(matches!(
            field_application_status(Some(&json!({"status": 1}))),
            Field::Invalid { .. }
        ));
    }

    #[test]
    fn relog_rules() {
        let a = access(json!({"reason": -1, "user": {}}));
        assert_eq!(
            a.availability,
            Availability::NotAvailable {
                cause: UnavailableCause::Relog
            }
        );
        let a = access(json!({"info": []}));
        assert_eq!(a.scope, LiveScope::Relog);
    }

    #[test]
    fn username_and_summary() {
        let info = parse_account(
            &json!({"reason": 0, "can_be_live": false, "user": {"username": " creadora "},
                    "application_status": {"status": "never_applied"}}),
            200,
            312,
            9,
        );
        assert_eq!(info.username.as_deref(), Some("creadora"));
        assert_eq!(info.http_status, 200);
        assert_eq!(info.duration_ms, 312);
        assert_eq!(
            summary(&info.access),
            "reason=0 · can_be_live=false · application_status=never_applied · user=presente"
        );
        let none = parse_account(&json!({"user": {"username": 5}}), 200, 0, 0);
        assert_eq!(none.username, None);
    }

    #[test]
    fn audience_parsing() {
        let a = parse_audience(Some(&json!({"disable": false, "info_type": 1,
            "types": [{"key": 0, "label": "Everyone"}, {"key": 1, "label": null}]})));
        assert_eq!(a.default_value, "1");
        assert_eq!(a.options[1].label, "Solo adultos (18+)");
        assert_eq!(parse_audience(None), default_audience());
    }

    #[test]
    fn parses_categories_with_mask_id() {
        let v = json!({ "categories": [
            { "full_name": "Fortnite", "game_mask_id": "123" },
            { "full_name": "Minecraft", "game_mask_id": 456 },
            { "full_name": "" }
        ]});
        let c = parse_categories(&v).unwrap();
        assert_eq!(c.len(), 2);
        assert_eq!(
            c[1],
            Category {
                id: "456".into(),
                name: "Minecraft".into()
            }
        );
        assert!(parse_categories(&json!({})).is_none());
    }

    #[test]
    fn start_response_accepts_numeric_id() {
        let r: StartResponse =
            serde_json::from_value(json!({ "id": 99, "rtmp": "rtmp://a/b", "key": "k" })).unwrap();
        assert_eq!(r.id_string().as_deref(), Some("99"));
    }
}
