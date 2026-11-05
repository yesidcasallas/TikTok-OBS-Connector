//! Persistencia opcional del token OAuth en el almacén de credenciales del
//! sistema (Secret Service / GNOME Keyring o KWallet en Linux, Keychain en
//! macOS, Administrador de credenciales en Windows).
//!
//! Las operaciones son bloqueantes (D-Bus): llámelas desde `spawn_blocking`.

use super::Secret;
use std::sync::Mutex;

pub const KEYRING_SERVICE: &str = "TikTokOBSConnector";
pub const KEYRING_ACCOUNT: &str = "streamlabs-oauth-token";

pub trait TokenStore: Send + Sync {
    fn load(&self) -> Result<Option<Secret>, String>;
    fn save(&self, token: &Secret) -> Result<(), String>;
    fn delete(&self) -> Result<(), String>;
}

/// Almacén del sistema mediante `keyring`.
#[derive(Default)]
pub struct KeyringStore;

impl KeyringStore {
    fn entry() -> Result<keyring::Entry, String> {
        keyring::Entry::new(KEYRING_SERVICE, KEYRING_ACCOUNT).map_err(describe)
    }
}

fn describe(e: keyring::Error) -> String {
    match e {
        keyring::Error::NoDefaultStore => {
            "no hay un almacén de credenciales disponible (¿falta GNOME Keyring/KWallet o una sesión D-Bus?)".to_string()
        }
        keyring::Error::NoStorageAccess(err) => format!("acceso denegado o llavero bloqueado ({err})"),
        keyring::Error::PlatformFailure(err) => format!("fallo del sistema ({err})"),
        other => other.to_string(),
    }
}

impl TokenStore for KeyringStore {
    fn load(&self) -> Result<Option<Secret>, String> {
        match Self::entry()?.get_password() {
            Ok(v) if v.is_empty() => Ok(None),
            Ok(v) => Ok(Some(Secret::new(v))),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(describe(e)),
        }
    }

    fn save(&self, token: &Secret) -> Result<(), String> {
        Self::entry()?.set_password(token.expose()).map_err(describe)
    }

    fn delete(&self) -> Result<(), String> {
        match Self::entry()?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(describe(e)),
        }
    }
}

/// Almacén en memoria para pruebas.
#[derive(Default)]
pub struct MemoryStore {
    value: Mutex<Option<Secret>>,
    pub fail: bool,
}

impl MemoryStore {
    pub fn failing() -> Self {
        MemoryStore { value: Mutex::new(None), fail: true }
    }
    pub fn peek(&self) -> Option<Secret> {
        self.value.lock().unwrap().clone()
    }
}

impl TokenStore for MemoryStore {
    fn load(&self) -> Result<Option<Secret>, String> {
        if self.fail {
            return Err("no disponible".into());
        }
        Ok(self.value.lock().unwrap().clone())
    }
    fn save(&self, token: &Secret) -> Result<(), String> {
        if self.fail {
            return Err("no disponible".into());
        }
        *self.value.lock().unwrap() = Some(token.clone());
        Ok(())
    }
    fn delete(&self) -> Result<(), String> {
        *self.value.lock().unwrap() = None;
        Ok(())
    }
}
