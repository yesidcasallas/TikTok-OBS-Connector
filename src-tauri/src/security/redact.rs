//! Redacción de textos antes de registrarlos o mostrarlos.
//!
//! Elimina los secretos conocidos y, por precaución, cualquier valor asociado
//! a nombres sensibles (`code`, `code_verifier`, `oauth_token`, `key`…) tanto
//! en parámetros de URL como en JSON, además de cabeceras `Bearer`.

const MASK: &str = "••••";

/// Nombres de parámetros/campos cuyo valor se oculta siempre.
const SENSITIVE_NAMES: &[&str] = &[
    "code",
    "code_verifier",
    "oauth_token",
    "access_token",
    "refresh_token",
    "platform_token",
    "token",
    "key",
    "stream_key",
    "streamKey",
];

/// Devuelve `text` sin los secretos indicados ni valores sensibles.
pub fn redact(text: &str, secrets: &[&str]) -> String {
    let mut out = text.to_string();
    for s in secrets {
        if s.len() >= 4 {
            out = out.replace(s, MASK);
        }
    }
    out = redact_bearer(&out);
    for name in SENSITIVE_NAMES {
        out = redact_query_param(&out, name);
        out = redact_json_field(&out, name);
    }
    out
}

fn is_token_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || "-._~+/=%".contains(c)
}

fn redact_bearer(text: &str) -> String {
    let lower = text.to_ascii_lowercase();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while let Some(pos) = lower[i..].find("bearer ") {
        let start = i + pos + "bearer ".len();
        out.push_str(&text[i..start]);
        let rest = &text[start..];
        let len = rest.find(|c: char| !is_token_char(c)).unwrap_or(rest.len());
        if len > 0 {
            out.push_str(MASK);
        }
        i = start + len;
    }
    out.push_str(&text[i..]);
    out
}

/// Oculta `name=valor` cuando `name` empieza tras un delimitador típico.
fn redact_query_param(text: &str, name: &str) -> String {
    let pattern = format!("{name}=");
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while let Some(pos) = text[i..].find(&pattern) {
        let abs = i + pos;
        let boundary = abs == 0
            || text[..abs]
                .chars()
                .last()
                .map(|c| "?&;, \t\n(\"'".contains(c))
                .unwrap_or(true);
        let value_start = abs + pattern.len();
        out.push_str(&text[i..value_start]);
        if boundary {
            let rest = &text[value_start..];
            let len = rest
                .find(|c: char| c == '&' || c == '"' || c == '\'' || c == ')' || c.is_whitespace())
                .unwrap_or(rest.len());
            if len > 0 {
                out.push_str(MASK);
            }
            i = value_start + len;
        } else {
            i = value_start;
        }
    }
    out.push_str(&text[i..]);
    out
}

/// Oculta el valor de cadena de `"name": "valor"` en JSON.
fn redact_json_field(text: &str, name: &str) -> String {
    let pattern = format!("\"{name}\"");
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while let Some(pos) = text[i..].find(&pattern) {
        let after_name = i + pos + pattern.len();
        out.push_str(&text[i..after_name]);
        let rest = &text[after_name..];
        let trimmed = rest.trim_start();
        let ws = rest.len() - trimmed.len();
        if let Some(after_colon) = trimmed.strip_prefix(':') {
            let value = after_colon.trim_start();
            let ws2 = after_colon.len() - value.len();
            if let Some(body) = value.strip_prefix('"') {
                // Busca la comilla de cierre no escapada.
                let mut end = None;
                let mut escaped = false;
                for (idx, c) in body.char_indices() {
                    if escaped {
                        escaped = false;
                    } else if c == '\\' {
                        escaped = true;
                    } else if c == '"' {
                        end = Some(idx);
                        break;
                    }
                }
                let consumed = ws + 1 + ws2 + 1;
                out.push_str(&rest[..consumed]);
                match end {
                    Some(e) => {
                        out.push_str(MASK);
                        i = after_name + consumed + e;
                    }
                    None => {
                        out.push_str(MASK);
                        i = text.len();
                    }
                }
                continue;
            }
        }
        i = after_name;
    }
    if i < text.len() {
        out.push_str(&text[i..]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_known_secrets() {
        let out = redact("token abcdef123 visto", &["abcdef123"]);
        assert!(!out.contains("abcdef123"));
    }

    #[test]
    fn removes_query_params() {
        let out = redact(
            "GET /api?code_verifier=AAA111&code=BBB222&port=5 HTTP/1.1",
            &[],
        );
        assert!(!out.contains("AAA111"));
        assert!(!out.contains("BBB222"));
        assert!(out.contains("port=5"));
    }

    #[test]
    fn keeps_unrelated_words_containing_names() {
        let out = redact("barcode=1 monkey=2", &[]);
        assert_eq!(out, "barcode=1 monkey=2");
    }

    #[test]
    fn removes_json_fields_and_bearer() {
        let out = redact(
            r#"{"oauth_token": "tok-123", "rtmp":"rtmp://x/app", "key":"live_999"} Authorization: Bearer abc.def-ghi"#,
            &[],
        );
        assert!(!out.contains("tok-123"));
        assert!(!out.contains("live_999"));
        assert!(!out.contains("abc.def-ghi"));
        assert!(out.contains("rtmp://x/app"));
    }

    #[test]
    fn handles_escaped_quotes_and_unicode() {
        let out = redact(r#"{"token":"a\"b","msg":"ñandú"}"#, &[]);
        assert!(!out.contains("a\\\"b"));
        assert!(out.contains("ñandú"));
    }
}
