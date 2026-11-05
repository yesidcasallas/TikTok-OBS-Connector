//! Preparación de la integración **propia** de Virelo con TikTok Login Kit
//! para escritorio. **No está activa**: la aplicación sigue usando Streamlabs.
//!
//! Qué falta para activarla (ver `docs/integracion-tiktok/oauth-y-credenciales.md`):
//! 1. Una app de Virelo aprobada en el portal de TikTok for Developers, con su
//!    `client_key` (público) y el redirect URI `http://127.0.0.1:*/callback/`.
//! 2. Un servidor intermediario de Virelo que guarde el `client_secret` y haga
//!    el intercambio del código: TikTok exige `client_secret` en
//!    `POST https://open.tiktokapis.com/v2/oauth/token/` y pide llamar a ese
//!    endpoint «desde tu servidor, no desde código cliente». El secreto **nunca**
//!    se incluye en el ejecutable.
//!
//! Alcance: Login Kit autentica e identifica al usuario (`user.info.basic`).
//! **No** permite crear emisiones ni obtener servidor RTMP o clave: ninguno de
//! los scopes públicos de TikTok cubre LIVE (comprobado el 2026-11-05).
//!
//! Referencias: <https://developers.tiktok.com/docs/en/login-kit-desktop> y
//! <https://developers.tiktok.com/docs/en/oauth-get-a-user-access-token>.

use crate::auth::{CallbackInterpreter, CallbackVerdict};
use crate::error::{AppError, AppResult};
use crate::security::Secret;
use sha2::{Digest, Sha256};
use url::Url;

/// Página de autorización (solo HTTPS).
pub const AUTHORIZE_URL: &str = "https://www.tiktok.com/v2/auth/authorize/";
/// Endpoint de intercambio del código. Lo usa **solo** el servidor intermediario.
pub const TOKEN_URL: &str = "https://open.tiktokapis.com/v2/oauth/token/";
/// Ruta fija del callback (TikTok exige URIs estáticas, sin parámetros).
pub const REDIRECT_PATH: &str = "/callback/";
/// URI a registrar en el portal (comodín de puerto permitido en escritorio).
pub const REGISTERED_REDIRECT_URI: &str = "http://127.0.0.1:*/callback/";
/// Único scope previsto: identificar al usuario. Solicitar solo lo necesario.
pub const DEFAULT_SCOPES: &[&str] = &["user.info.basic"];

/// Configuración pública de Login Kit. No contiene ningún secreto.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoginKitConfig {
    pub client_key: String,
    /// Servidor de Virelo que hace el intercambio con `client_secret`.
    pub token_broker_url: Url,
    pub scopes: Vec<String>,
}

impl LoginKitConfig {
    /// Configuración de compilación: `VIRELO_TIKTOK_CLIENT_KEY` y
    /// `VIRELO_TOKEN_BROKER_URL`. Devuelve `None` si falta alguna (estado actual).
    pub fn from_build_env() -> Option<Self> {
        Self::from_values(
            option_env!("VIRELO_TIKTOK_CLIENT_KEY"),
            option_env!("VIRELO_TOKEN_BROKER_URL"),
        )
        .ok()
    }

    pub fn from_values(client_key: Option<&str>, broker: Option<&str>) -> AppResult<Self> {
        let client_key = client_key
            .map(str::trim)
            .filter(|k| !k.is_empty())
            .ok_or_else(|| {
                AppError::Validation("Falta el client_key de Virelo en TikTok.".into())
            })?;
        let broker = broker
            .map(str::trim)
            .filter(|b| !b.is_empty())
            .ok_or_else(|| {
                AppError::Validation("Falta la URL del servidor intermediario.".into())
            })?;
        let token_broker_url = Url::parse(broker).map_err(|e| {
            AppError::Validation(format!("URL del servidor intermediario no válida: {e}"))
        })?;
        if token_broker_url.scheme() != "https" {
            return Err(AppError::Validation(
                "El servidor intermediario debe usar HTTPS.".into(),
            ));
        }
        Ok(LoginKitConfig {
            client_key: client_key.to_string(),
            token_broker_url,
            scopes: DEFAULT_SCOPES.iter().map(|s| s.to_string()).collect(),
        })
    }
}

/// `code_challenge` según Login Kit para escritorio: **SHA-256 en hexadecimal**
/// («You must use hex encoding of SHA256…»). Ojo: difiere del RFC 7636
/// (base64url) y del PKCE de Streamlabs (`auth::pkce`), que sí usa base64url.
pub fn code_challenge_hex(verifier: &str) -> String {
    Sha256::digest(verifier.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Redirect URI concreto para el puerto local asignado.
pub fn redirect_uri(port: u16) -> String {
    format!("http://127.0.0.1:{port}{REDIRECT_PATH}")
}

/// URL de autorización con todos los parámetros documentados.
pub fn authorize_url(
    cfg: &LoginKitConfig,
    redirect_uri: &str,
    state: &OAuthState,
    code_challenge: &str,
) -> String {
    let mut url = Url::parse(AUTHORIZE_URL).expect("URL constante válida");
    url.query_pairs_mut()
        .append_pair("client_key", &cfg.client_key)
        .append_pair("response_type", "code")
        .append_pair("scope", &cfg.scopes.join(","))
        .append_pair("redirect_uri", redirect_uri)
        .append_pair("state", state.0.expose())
        .append_pair("code_challenge", code_challenge)
        .append_pair("code_challenge_method", "S256");
    url.into()
}

/// Valor `state` anti-CSRF: 32 bytes aleatorios en hexadecimal.
pub struct OAuthState(Secret);

impl OAuthState {
    pub fn generate() -> AppResult<Self> {
        let mut bytes = [0u8; 32];
        getrandom::fill(&mut bytes)
            .map_err(|e| AppError::Internal(format!("generador aleatorio no disponible: {e}")))?;
        Ok(OAuthState(Secret::new(
            bytes.iter().map(|b| format!("{b:02x}")).collect::<String>(),
        )))
    }

    /// Comparación en tiempo constante.
    pub fn matches(&self, received: &str) -> bool {
        let a = self.0.expose().as_bytes();
        let b = received.as_bytes();
        if a.len() != b.len() {
            return false;
        }
        a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
    }
}

/// Interpreta el callback de Login Kit: exige la ruta registrada y el `state`
/// esperado. Una petición con `state` ausente o distinto se ignora (no cierra el
/// intento, para que un tercero no pueda cancelarlo).
pub struct LoginKitCallback {
    pub expected_state: OAuthState,
}

impl CallbackInterpreter for LoginKitCallback {
    fn interpret(&self, path: &str, params: &[(String, String)]) -> CallbackVerdict {
        if path != REDIRECT_PATH {
            return CallbackVerdict::Ignore;
        }
        let get = |k: &str| params.iter().find(|(n, _)| n == k).map(|(_, v)| v.as_str());
        match get("state") {
            Some(s) if self.expected_state.matches(s) => {}
            _ => return CallbackVerdict::Ignore,
        }
        if let Some(error) = get("error") {
            let detail = get("error_description").unwrap_or(error);
            return CallbackVerdict::Failed(AppError::AuthDenied(format!(
                "TikTok devolvió «{}».",
                crate::provider::streamlabs::errors::sanitize_message(detail)
            )));
        }
        match get("code") {
            Some(c) if !c.is_empty() && c.len() <= 2048 => CallbackVerdict::Code(Secret::new(c)),
            _ => CallbackVerdict::Failed(AppError::AuthDenied(
                "TikTok no devolvió un código de autorización.".into(),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> LoginKitConfig {
        LoginKitConfig::from_values(
            Some("awvirelotest"),
            Some("https://api.virelo.example/oauth"),
        )
        .unwrap()
    }

    #[test]
    fn current_build_has_no_own_credentials() {
        // Sin VIRELO_TIKTOK_CLIENT_KEY la integración propia queda desactivada.
        if option_env!("VIRELO_TIKTOK_CLIENT_KEY").is_none() {
            assert!(LoginKitConfig::from_build_env().is_none());
        }
    }

    #[test]
    fn config_requires_key_and_https_broker() {
        assert!(LoginKitConfig::from_values(None, Some("https://x.example")).is_err());
        assert!(LoginKitConfig::from_values(Some("k"), None).is_err());
        assert!(LoginKitConfig::from_values(Some("k"), Some("http://x.example")).is_err());
        assert_eq!(cfg().scopes, vec!["user.info.basic".to_string()]);
    }

    #[test]
    fn challenge_is_hex_sha256() {
        // SHA-256("abc"), vector de FIPS 180-2.
        assert_eq!(
            code_challenge_hex("abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(code_challenge_hex("x").len(), 64);
    }

    #[test]
    fn authorize_url_has_documented_parameters() {
        let state = OAuthState::generate().unwrap();
        let u = authorize_url(&cfg(), &redirect_uri(54321), &state, "abc123");
        let parsed = Url::parse(&u).unwrap();
        assert_eq!(parsed.as_str().split('?').next().unwrap(), AUTHORIZE_URL);
        let q: std::collections::HashMap<_, _> = parsed.query_pairs().into_owned().collect();
        assert_eq!(q["client_key"], "awvirelotest");
        assert_eq!(q["response_type"], "code");
        assert_eq!(q["scope"], "user.info.basic");
        assert_eq!(q["redirect_uri"], "http://127.0.0.1:54321/callback/");
        assert_eq!(q["code_challenge"], "abc123");
        assert_eq!(q["code_challenge_method"], "S256");
        assert!(state.matches(&q["state"]));
        assert!(
            !u.contains("client_secret"),
            "el secreto nunca va al cliente"
        );
    }

    #[test]
    fn state_is_random_and_compared_safely() {
        let a = OAuthState::generate().unwrap();
        let b = OAuthState::generate().unwrap();
        assert_eq!(a.0.expose().len(), 64);
        assert!(!a.matches(b.0.expose()));
        assert!(!a.matches(""));
    }

    #[test]
    fn callback_validates_path_and_state() {
        let state = OAuthState::generate().unwrap();
        let good = state.0.expose().to_string();
        let cb = LoginKitCallback {
            expected_state: state,
        };
        let p = |v: &[(&str, &str)]| -> Vec<(String, String)> {
            v.iter()
                .map(|(a, b)| (a.to_string(), b.to_string()))
                .collect()
        };
        assert!(
            matches!(cb.interpret("/callback/", &p(&[("code", "c1"), ("state", &good)])), CallbackVerdict::Code(c) if c.expose() == "c1")
        );
        assert!(matches!(
            cb.interpret("/otra/", &p(&[("code", "c1"), ("state", &good)])),
            CallbackVerdict::Ignore
        ));
        assert!(matches!(
            cb.interpret("/callback/", &p(&[("code", "c1"), ("state", "falso")])),
            CallbackVerdict::Ignore
        ));
        assert!(matches!(
            cb.interpret("/callback/", &p(&[("code", "c1")])),
            CallbackVerdict::Ignore
        ));
        assert!(matches!(
            cb.interpret("/callback/", &p(&[("error", "access_denied"), ("error_description", "User denied"), ("state", &good)])),
            CallbackVerdict::Failed(AppError::AuthDenied(m)) if m.contains("User denied")
        ));
    }
}
