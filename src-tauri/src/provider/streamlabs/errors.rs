//! Traducción de las respuestas de error de Streamlabs a [`AppError`].
//!
//! Distingue token inválido, permisos insuficientes, restricciones de la
//! cuenta y fallos del proveedor. Los códigos de TikTok proceden de
//! `ETikTokErrorTypes` en Streamlabs Desktop.

use crate::error::AppError;
use crate::security::redact;
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Exchange,
    Info,
    Categories,
    Start,
    End,
}

const MAX_MESSAGE_CHARS: usize = 300;

const BLOCKED_DETAIL: &str = "La respuesta no es JSON: puede tratarse de una protección anti-bots o de una restricción del proveedor para este cliente o plataforma.";

const PERMISSION_HINT: &str = "Solicita el acceso a LIVE de Streamlabs para tu cuenta de TikTok o revisa su estado en TikTok LIVE Center.";

/// Limpia un texto del proveedor antes de mostrarlo: sin caracteres de
/// control, sin secretos y con longitud acotada.
pub fn sanitize_message(msg: &str) -> String {
    let cleaned: String = msg
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    let collapsed = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut out: String = redact(&collapsed, &[]).chars().take(MAX_MESSAGE_CHARS).collect();
    if collapsed.chars().count() > MAX_MESSAGE_CHARS {
        out.push('…');
    }
    out
}

fn non_empty_str(v: Option<&Value>) -> Option<String> {
    v.and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

/// Mensaje legible de una respuesta de error JSON.
pub fn extract_message(v: &Value) -> Option<String> {
    non_empty_str(v.get("message"))
        .or_else(|| non_empty_str(v.get("error")))
        .or_else(|| non_empty_str(v.get("error").and_then(|e| e.get("message"))))
        .or_else(|| non_empty_str(v.get("data").and_then(|d| d.get("message"))))
        .or_else(|| {
            // Errores de validación tipo Laravel: { errors: { campo: ["…"] } }
            v.get("errors")?.as_object()?.values().find_map(|e| {
                e.as_array()
                    .and_then(|a| a.first())
                    .and_then(Value::as_str)
                    .or_else(|| e.as_str())
                    .map(str::to_string)
            })
        })
        .map(|m| sanitize_message(&m))
}

/// Código de error de TikTok/Streamlabs si existe.
pub fn extract_code(v: &Value) -> Option<String> {
    non_empty_str(v.get("data").and_then(|d| d.get("code")))
        .or_else(|| non_empty_str(v.get("error").and_then(|e| e.get("code"))))
        .or_else(|| non_empty_str(v.get("code")))
        .map(|c| c.to_ascii_lowercase())
}

fn platform_hint(message: &str) -> &'static str {
    let m = message.to_ascii_lowercase();
    if ["platform", "device", "linux", "plataforma", " os "].iter().any(|k| m.contains(k)) {
        " Streamlabs podría no admitir la plataforma «linux» (device_platform). Esta aplicación no simula Windows."
    } else {
        ""
    }
}

fn from_code(code: &str, message: Option<&str>) -> Option<AppError> {
    let detail = message.map(|m| format!("{PERMISSION_HINT} Detalle: {m}")).unwrap_or_else(|| PERMISSION_HINT.to_string());
    match code {
        "access_token_invalid" => Some(AppError::TokenInvalid),
        "scope_not_authorized" | "scope_permission_missed" | "user_has_no_live_auth" => {
            Some(AppError::PermissionDenied(detail))
        }
        "user_already_live" => Some(AppError::AlreadyLive),
        "rate_limit_exceeded" => Some(AppError::RateLimited),
        _ => None,
    }
}

/// Error para una respuesta HTTP no exitosa.
pub fn map_http_error(op: Op, status: u16, body: &[u8]) -> AppError {
    let json: Option<Value> = serde_json::from_slice(body).ok().filter(|v: &Value| v.is_object());
    let message = json.as_ref().and_then(extract_message);
    if let Some(v) = &json {
        if let Some(code) = extract_code(v) {
            if let Some(e) = from_code(&code, message.as_deref()) {
                return e;
            }
        }
    }

    if op == Op::Exchange {
        return match status {
            400 | 401 | 404 | 410 | 422 => AppError::AuthExchangeRejected,
            403 if json.is_some() => AppError::AuthExchangeRejected,
            403 => AppError::ProviderBlocked { status, detail: BLOCKED_DETAIL.into() },
            429 => AppError::RateLimited,
            500..=599 => AppError::ProviderUnavailable(status),
            _ => AppError::ProviderUnexpected(format!("HTTP {status} en el intercambio del código")),
        };
    }

    match status {
        401 => AppError::TokenInvalid,
        403 => match (&json, message) {
            (Some(_), Some(m)) => AppError::PermissionDenied(format!("{PERMISSION_HINT} Detalle: {m}{}", platform_hint(&m))),
            (Some(_), None) => AppError::PermissionDenied(PERMISSION_HINT.into()),
            (None, _) => AppError::ProviderBlocked { status, detail: BLOCKED_DETAIL.into() },
        },
        // Streamlabs Desktop interpreta 422 al iniciar como cuenta sancionada.
        422 if op == Op::Start => AppError::AccountRestricted,
        429 => AppError::RateLimited,
        400..=499 => {
            let m = message.unwrap_or_else(|| "sin detalle".to_string());
            let hint = platform_hint(&m);
            AppError::ProviderRejected { status, message: format!("{m}{hint}") }
        }
        500..=599 => AppError::ProviderUnavailable(status),
        _ => AppError::ProviderUnexpected(format!("HTTP {status}")),
    }
}

/// Error incluido en una respuesta 2xx (`success: false`, `error: …`).
pub fn body_error(status: u16, v: &Value) -> Option<AppError> {
    let failed = v.get("success").and_then(Value::as_bool) == Some(false)
        || match v.get("error") {
            Some(Value::Bool(b)) => *b,
            Some(Value::String(s)) => !s.trim().is_empty(),
            Some(Value::Object(_)) => true,
            _ => false,
        };
    if !failed {
        return None;
    }
    let message = extract_message(v);
    if let Some(code) = extract_code(v) {
        if let Some(e) = from_code(&code, message.as_deref()) {
            return Some(e);
        }
    }
    let m = message.unwrap_or_else(|| "el proveedor indicó un error sin detalle".into());
    let hint = platform_hint(&m);
    Some(AppError::ProviderRejected { status, message: format!("{m}{hint}") })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn body(v: Value) -> Vec<u8> {
        serde_json::to_vec(&v).unwrap()
    }

    #[test]
    fn unauthorized_is_token_invalid() {
        assert_eq!(map_http_error(Op::Info, 401, b"Unauthorized."), AppError::TokenInvalid);
        assert_eq!(map_http_error(Op::Start, 401, b""), AppError::TokenInvalid);
    }

    #[test]
    fn exchange_failures_are_rejections() {
        assert_eq!(map_http_error(Op::Exchange, 401, br#"{"message":""}"#), AppError::AuthExchangeRejected);
        assert_eq!(map_http_error(Op::Exchange, 503, b""), AppError::ProviderUnavailable(503));
    }

    #[test]
    fn forbidden_json_is_permission_but_html_is_block() {
        let e = map_http_error(Op::Start, 403, &body(json!({"message":"No LIVE access"})));
        assert!(matches!(e, AppError::PermissionDenied(ref m) if m.contains("No LIVE access")));
        let e = map_http_error(Op::Start, 403, b"<html>challenge</html>");
        assert!(matches!(e, AppError::ProviderBlocked { status: 403, .. }));
    }

    #[test]
    fn tiktok_codes_take_precedence() {
        let e = map_http_error(
            Op::Start,
            400,
            &body(json!({"data": {"code": "user_has_no_live_auth", "message": "no auth"}})),
        );
        assert!(matches!(e, AppError::PermissionDenied(_)));
        let e = map_http_error(Op::Start, 400, &body(json!({"data": {"code": "user_already_live"}})));
        assert_eq!(e, AppError::AlreadyLive);
        let e = map_http_error(Op::Info, 400, &body(json!({"error": {"code": "access_token_invalid"}})));
        assert_eq!(e, AppError::TokenInvalid);
    }

    #[test]
    fn status_specific_mappings() {
        assert_eq!(map_http_error(Op::Start, 422, b"{}"), AppError::AccountRestricted);
        assert!(matches!(map_http_error(Op::Info, 422, b"{}"), AppError::ProviderRejected { status: 422, .. }));
        assert_eq!(map_http_error(Op::End, 429, b""), AppError::RateLimited);
        assert_eq!(map_http_error(Op::Categories, 500, b""), AppError::ProviderUnavailable(500));
        assert!(matches!(map_http_error(Op::Info, 302, b""), AppError::ProviderUnexpected(_)));
    }

    #[test]
    fn platform_restriction_is_explained() {
        let e = map_http_error(
            Op::Start,
            400,
            &body(json!({"message": "Unsupported device platform"})),
        );
        let text = e.to_string();
        assert!(text.contains("Unsupported device platform"));
        assert!(text.contains("linux"));
    }

    #[test]
    fn messages_are_sanitized() {
        let e = map_http_error(
            Op::Start,
            400,
            &body(json!({"message": "bad key=live_123456 \n\t Bearer abc.def"})),
        );
        let text = e.to_string();
        assert!(!text.contains("live_123456"));
        assert!(!text.contains("abc.def"));
        let long = "x".repeat(1000);
        assert!(sanitize_message(&long).chars().count() <= MAX_MESSAGE_CHARS + 1);
    }

    #[test]
    fn body_errors_on_success_status() {
        assert!(body_error(200, &json!({"success": true})).is_none());
        assert!(body_error(200, &json!({"reason": 1, "error": null})).is_none());
        let e = body_error(200, &json!({"success": false, "message": "Stream limit"})).unwrap();
        assert!(matches!(e, AppError::ProviderRejected { status: 200, ref message } if message.contains("Stream limit")));
        let e = body_error(200, &json!({"error": true, "data": {"code": "scope_not_authorized"}})).unwrap();
        assert!(matches!(e, AppError::PermissionDenied(_)));
    }
}
