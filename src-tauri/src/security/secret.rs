//! Contenedor de datos sensibles (token OAuth, código de autorización,
//! `code_verifier`, clave de transmisión).
//!
//! - `Debug` y `Display` nunca muestran el valor.
//! - No implementa `Serialize`: no puede enviarse al frontend ni a un registro
//!   por accidente.
//! - La memoria se sobrescribe con ceros al liberarse (`zeroize`).

use std::fmt;
use zeroize::{Zeroize, ZeroizeOnDrop};

#[derive(Clone, Zeroize, ZeroizeOnDrop, PartialEq, Eq)]
pub struct Secret(String);

impl Secret {
    pub fn new(value: impl Into<String>) -> Self {
        Secret(value.into())
    }

    /// Acceso explícito al valor. Solo para entregarlo al proveedor, al
    /// almacén del sistema o al portapapeles; nunca para formatear mensajes.
    pub fn expose(&self) -> &str {
        &self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Pista segura para la interfaz: `••••` seguido de los 4 últimos
    /// caracteres si el valor es suficientemente largo.
    pub fn hint(&self) -> String {
        let chars: Vec<char> = self.0.chars().collect();
        if chars.len() >= 12 {
            let tail: String = chars[chars.len() - 4..].iter().collect();
            format!("••••{tail}")
        } else {
            "••••".to_string()
        }
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret(••••)")
    }
}

impl fmt::Display for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("••••")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn never_prints_value() {
        let s = Secret::new("super-secreto-123456");
        assert_eq!(format!("{s}"), "••••");
        assert_eq!(format!("{s:?}"), "Secret(••••)");
        assert_eq!(s.expose(), "super-secreto-123456");
    }

    #[test]
    fn hint_only_shows_tail_of_long_values() {
        assert_eq!(Secret::new("abcdefghijkl3456").hint(), "••••3456");
        assert_eq!(Secret::new("corto").hint(), "••••");
    }
}
