//! Traducción de las respuestas de error de Streamlabs a [`AppError`].
//!
//! Principio: **un código HTTP aislado no prueba la causa**. Solo se afirma
//! «sin permiso» cuando el proveedor envía un código explícito
//! (`ETikTokErrorTypes` de Streamlabs Desktop). En los demás casos se
//! clasifica por tipo (autenticación, autorización, validación, límite,
//! servidor, inesperado) y se conserva el mensaje original como detalle.

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

impl Op {
    pub fn name(self) -> &'static str {
        match self {
            Op::Exchange => "auth_data",
            Op::Info => "tiktok_info",
            Op::Categories => "tiktok_categories",
            Op::Start => "stream_start",
            Op::End => "stream_end",
        }
    }
}

const MAX_MESSAGE_CHARS: usize = 300;

const BLOCKED_DETAIL: &str = "La respuesta no es JSON: puede tratarse de una protección anti-bots o de una restricción del proveedor.";

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

/// Códigos explícitos (`ETikTokErrorTypes`).
fn from_code(code: &str, status: u16, message: Option<String>) -> Option<AppError> {
    match code {
        "access_token_invalid" => Some(AppError::TokenInvalid),
        "scope_not_authorized" | "scope_permission_missed" | "user_has_no_live_auth" => {
            Some(AppError::PermissionDenied { message })
        }
        "user_already_live" => Some(AppError::AlreadyLive),
        "rate_limit_exceeded" => Some(AppError::RateLimited),
        "invalid_params" => Some(AppError::ProviderRejected { status, message }),
        _ => None,
    }
}

/// Error para una respuesta HTTP no exitosa.
pub fn map_http_error(op: Op, status: u16, body: &[u8]) -> AppError {
    let json: Option<Value> = serde_json::from_slice(body).ok().filter(|v: &Value| v.is_object());
    let message = json.as_ref().and_then(extract_message);
    if let Some(v) = &json {
        if let Some(code) = extract_code(v) {
            if let Some(e) = from_code(&code, status, message.clone()) {
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
            500..=599 => AppError::ProviderUnavailable { status, message },
            _ => AppError::ProviderUnexpected(format!("HTTP {status} en el intercambio del código")),
        };
    }

    match status {
        401 => AppError::TokenInvalid,
        403 if json.is_some() => AppError::ProviderForbidden { status, message },
        403 => AppError::ProviderBlocked { status, detail: BLOCKED_DETAIL.into() },
        // Streamlabs Desktop interpreta 422 al iniciar como cuenta sancionada.
        422 if op == Op::Start => AppError::AccountRestricted { message },
        429 => AppError::RateLimited,
        400..=499 => AppError::ProviderRejected { status, message },
        500..=599 => AppError::ProviderUnavailable { status, message },
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
        if let Some(e) = from_code(&code, status, message.clone()) {
            return Some(e);
        }
    }
    Some(AppError::ProviderRejected { status, message })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn body(v: Value) -> Vec<u8> {
        serde_json::to_vec(&v).unwrap()
    }

    const GENERIC: &str = "Unable to go live. Please try again later";

    #[test]
    fn generic_provider_error_is_not_presented_as_missing_permission() {
        for status in [400u16, 403, 500, 503] {
            let e = map_http_error(Op::Start, status, &body(json!({ "message": GENERIC })));
            assert!(!matches!(e, AppError::PermissionDenied { .. }), "HTTP {status}: {e:?}");
            assert_eq!(e.provider_detail(), Some(GENERIC), "HTTP {status} debe conservar el mensaje");
            assert_eq!(e.http_status(), Some(status));
        }
        let e = map_http_error(Op::Start, 403, &body(json!({ "message": GENERIC })));
        assert!(matches!(e, AppError::ProviderForbidden { status: 403, .. }));
        let e = body_error(200, &json!({"success": false, "message": GENERIC})).unwrap();
        assert!(matches!(e, AppError::ProviderRejected { status: 200, .. }));
        assert_eq!(e.provider_detail(), Some(GENERIC));
    }

    #[test]
    fn explicit_codes_are_permission_denied() {
        let e = map_http_error(
            Op::Start,
            400,
            &body(json!({"data": {"code": "user_has_no_live_auth", "message": "no auth"}})),
        );
        assert_eq!(e, AppError::PermissionDenied { message: Some("no auth".into()) });
        let e = body_error(200, &json!({"error": true, "data": {"code": "scope_not_authorized"}})).unwrap();
        assert!(matches!(e, AppError::PermissionDenied { .. }));
    }

    #[test]
    fn other_explicit_codes() {
        assert_eq!(map_http_error(Op::Start, 400, &body(json!({"data": {"code": "user_already_live"}}))), AppError::AlreadyLive);
        assert_eq!(map_http_error(Op::Info, 400, &body(json!({"error": {"code": "access_token_invalid"}}))), AppError::TokenInvalid);
        assert_eq!(map_http_error(Op::Start, 400, &body(json!({"code": "rate_limit_exceeded"}))), AppError::RateLimited);
    }

    #[test]
    fn status_classification() {
        assert_eq!(map_http_error(Op::Info, 401, b"Unauthorized."), AppError::TokenInvalid);
        assert!(matches!(map_http_error(Op::Start, 403, b"<html>cf</html>"), AppError::ProviderBlocked { status: 403, .. }));
        assert_eq!(map_http_error(Op::Start, 422, b"{}"), AppError::AccountRestricted { message: None });
        assert!(matches!(map_http_error(Op::Info, 422, b"{}"), AppError::ProviderRejected { status: 422, .. }));
        assert_eq!(map_http_error(Op::End, 429, b""), AppError::RateLimited);
        assert_eq!(map_http_error(Op::Categories, 500, b""), AppError::ProviderUnavailable { status: 500, message: None });
        assert!(matches!(map_http_error(Op::Info, 302, b""), AppError::ProviderUnexpected(_)));
    }

    #[test]
    fn exchange_failures() {
        assert_eq!(map_http_error(Op::Exchange, 401, br#"{"message":""}"#), AppError::AuthExchangeRejected);
        assert!(matches!(map_http_error(Op::Exchange, 503, b""), AppError::ProviderUnavailable { status: 503, .. }));
    }

    #[test]
    fn messages_are_sanitized() {
        let e = map_http_error(Op::Start, 400, &body(json!({"message": "bad key=live_123456 \n\t Bearer abc.def"})));
        let d = e.provider_detail().unwrap();
        assert!(!d.contains("live_123456") && !d.contains("abc.def"));
        assert!(sanitize_message(&"x".repeat(1000)).chars().count() <= MAX_MESSAGE_CHARS + 1);
    }

    #[test]
    fn success_bodies_are_not_errors() {
        assert!(body_error(200, &json!({"success": true})).is_none());
        assert!(body_error(200, &json!({"reason": 1, "error": null})).is_none());
    }
}
