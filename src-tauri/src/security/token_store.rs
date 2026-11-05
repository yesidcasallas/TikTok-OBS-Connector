//! Persistencia opcional del token de sesión de Streamlabs en el almacén de
//! credenciales del sistema (Secret Service / GNOME Keyring o KWallet en Linux,
//! Keychain en macOS, Administrador de credenciales en Windows).
//!
//! Solo guarda el **token de sesión del proveedor (Streamlabs)**. No guarda la
//! clave de transmisión ni ningún token de TikTok.
//!
//! Migración: hasta la 0.1.0-beta.1 el servicio se llamaba `TikTokOBSConnector`.
//! Al leer, si el servicio actual está vacío y existe una entrada antigua, se
//! copia al nuevo y se borra la antigua; al cerrar sesión se borran ambas.
//!
//! Las operaciones son bloqueantes (D-Bus): llámelas desde `spawn_blocking`.

use super::Secret;
use std::collections::HashMap;
use std::sync::Mutex;

pub const KEYRING_SERVICE: &str = "Virelo";
pub const KEYRING_ACCOUNT: &str = "streamlabs-oauth-token";
/// Servicios de versiones anteriores (para migrar sin perder la sesión).
pub const LEGACY_KEYRING_SERVICES: &[&str] = &["TikTokOBSConnector"];

pub trait TokenStore: Send + Sync {
    fn load(&self) -> Result<Option<Secret>, String>;
    fn save(&self, token: &Secret) -> Result<(), String>;
    fn delete(&self) -> Result<(), String>;
}

/// Acceso básico a un almacén de credenciales (servicio + cuenta).
pub trait CredentialBackend: Send + Sync {
    fn get(&self, service: &str, account: &str) -> Result<Option<String>, String>;
    fn set(&self, service: &str, account: &str, value: &str) -> Result<(), String>;
    fn remove(&self, service: &str, account: &str) -> Result<(), String>;
}

/// Almacén del sistema mediante `keyring`.
#[derive(Default)]
pub struct OsCredentials;

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

impl CredentialBackend for OsCredentials {
    fn get(&self, service: &str, account: &str) -> Result<Option<String>, String> {
        let entry = keyring::Entry::new(service, account).map_err(describe)?;
        match entry.get_password() {
            Ok(v) if v.is_empty() => Ok(None),
            Ok(v) => Ok(Some(v)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(describe(e)),
        }
    }

    fn set(&self, service: &str, account: &str, value: &str) -> Result<(), String> {
        keyring::Entry::new(service, account)
            .map_err(describe)?
            .set_password(value)
            .map_err(describe)
    }

    fn remove(&self, service: &str, account: &str) -> Result<(), String> {
        match keyring::Entry::new(service, account)
            .map_err(describe)?
            .delete_credential()
        {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(describe(e)),
        }
    }
}

/// Almacén de tokens con migración desde los servicios antiguos.
pub struct MigratingStore<B: CredentialBackend> {
    backend: B,
    service: &'static str,
    legacy: &'static [&'static str],
}

impl<B: CredentialBackend> MigratingStore<B> {
    pub fn new(backend: B, service: &'static str, legacy: &'static [&'static str]) -> Self {
        MigratingStore {
            backend,
            service,
            legacy,
        }
    }
}

/// Almacén del sistema que usa la aplicación.
pub type KeyringStore = MigratingStore<OsCredentials>;

impl Default for KeyringStore {
    fn default() -> Self {
        MigratingStore::new(OsCredentials, KEYRING_SERVICE, LEGACY_KEYRING_SERVICES)
    }
}

impl<B: CredentialBackend> TokenStore for MigratingStore<B> {
    fn load(&self) -> Result<Option<Secret>, String> {
        if let Some(v) = self.backend.get(self.service, KEYRING_ACCOUNT)? {
            return Ok(Some(Secret::new(v)));
        }
        for old in self.legacy {
            if let Some(v) = self.backend.get(old, KEYRING_ACCOUNT)? {
                let token = Secret::new(v);
                // Solo se borra la entrada antigua si la copia nueva se guardó.
                if self
                    .backend
                    .set(self.service, KEYRING_ACCOUNT, token.expose())
                    .is_ok()
                {
                    let _ = self.backend.remove(old, KEYRING_ACCOUNT);
                }
                return Ok(Some(token));
            }
        }
        Ok(None)
    }

    fn save(&self, token: &Secret) -> Result<(), String> {
        self.backend
            .set(self.service, KEYRING_ACCOUNT, token.expose())
    }

    fn delete(&self) -> Result<(), String> {
        let result = self.backend.remove(self.service, KEYRING_ACCOUNT);
        for old in self.legacy {
            let _ = self.backend.remove(old, KEYRING_ACCOUNT);
        }
        result
    }
}

/// Almacén de credenciales en memoria (pruebas).
#[derive(Default)]
pub struct MemoryCredentials {
    entries: Mutex<HashMap<(String, String), String>>,
}

impl MemoryCredentials {
    pub fn insert(&self, service: &str, account: &str, value: &str) {
        self.entries
            .lock()
            .unwrap()
            .insert((service.into(), account.into()), value.into());
    }
    pub fn contains(&self, service: &str, account: &str) -> bool {
        self.entries
            .lock()
            .unwrap()
            .contains_key(&(service.into(), account.into()))
    }
}

impl CredentialBackend for MemoryCredentials {
    fn get(&self, service: &str, account: &str) -> Result<Option<String>, String> {
        Ok(self
            .entries
            .lock()
            .unwrap()
            .get(&(service.into(), account.into()))
            .cloned())
    }
    fn set(&self, service: &str, account: &str, value: &str) -> Result<(), String> {
        self.insert(service, account, value);
        Ok(())
    }
    fn remove(&self, service: &str, account: &str) -> Result<(), String> {
        self.entries
            .lock()
            .unwrap()
            .remove(&(service.into(), account.into()));
        Ok(())
    }
}

/// Almacén de tokens en memoria para pruebas del núcleo.
#[derive(Default)]
pub struct MemoryStore {
    value: Mutex<Option<Secret>>,
    pub fail: bool,
}

impl MemoryStore {
    pub fn failing() -> Self {
        MemoryStore {
            value: Mutex::new(None),
            fail: true,
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> MigratingStore<MemoryCredentials> {
        MigratingStore::new(
            MemoryCredentials::default(),
            KEYRING_SERVICE,
            LEGACY_KEYRING_SERVICES,
        )
    }

    #[test]
    fn migrates_legacy_entry_once() {
        let s = store();
        s.backend
            .insert("TikTokOBSConnector", KEYRING_ACCOUNT, "tok_legacy_123");
        assert_eq!(s.load().unwrap().unwrap().expose(), "tok_legacy_123");
        assert!(
            s.backend.contains(KEYRING_SERVICE, KEYRING_ACCOUNT),
            "se copia al servicio nuevo"
        );
        assert!(
            !s.backend.contains("TikTokOBSConnector", KEYRING_ACCOUNT),
            "y se borra el antiguo"
        );
        assert_eq!(s.load().unwrap().unwrap().expose(), "tok_legacy_123");
    }

    #[test]
    fn current_entry_takes_precedence() {
        let s = store();
        s.backend
            .insert(KEYRING_SERVICE, KEYRING_ACCOUNT, "tok_new");
        s.backend
            .insert("TikTokOBSConnector", KEYRING_ACCOUNT, "tok_old");
        assert_eq!(s.load().unwrap().unwrap().expose(), "tok_new");
    }

    #[test]
    fn logout_removes_current_and_legacy_entries() {
        let s = store();
        s.save(&Secret::new("tok_new")).unwrap();
        s.backend
            .insert("TikTokOBSConnector", KEYRING_ACCOUNT, "tok_old");
        s.delete().unwrap();
        assert!(s.load().unwrap().is_none());
    }

    #[test]
    fn empty_store_returns_none() {
        assert!(store().load().unwrap().is_none());
    }
}
