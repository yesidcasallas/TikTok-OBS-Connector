//! Preferencias no sensibles en `<config>/preferences.json`.
//!
//! Nunca contienen tokens ni claves. El identificador del LIVE activo se
//! guarda para poder finalizarlo tras reiniciar la aplicación.

use crate::provider::Category;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Preferences {
    pub remember_session: bool,
    pub last_title: String,
    pub last_category: Option<Category>,
    pub audience_type: String,
    pub active_stream: Option<ActiveStreamRecord>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveStreamRecord {
    pub id: String,
    pub title: String,
    pub category_name: String,
    pub started_at_ms: u64,
}

pub struct PreferencesStore {
    path: Option<PathBuf>,
}

impl PreferencesStore {
    pub fn new(dir: Option<PathBuf>) -> Self {
        PreferencesStore {
            path: dir.map(|d| d.join("preferences.json")),
        }
    }

    pub fn load(&self) -> Preferences {
        self.path
            .as_ref()
            .and_then(|p| std::fs::read(p).ok())
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    /// Copia `preferences.json` desde la carpeta de una versión anterior si la
    /// actual aún no lo tiene. Devuelve la carpeta de origen si migró.
    ///
    /// Las carpetas dependen del identificador de la aplicación; hasta la
    /// 0.1.0-beta.1 eran `com.tiktokobsconnector.desktop` y `com.tiktokobsconnector.app`.
    pub fn migrate_from(&self, legacy_dirs: &[PathBuf]) -> Option<PathBuf> {
        let path = self.path.as_ref()?;
        if path.exists() {
            return None;
        }
        for dir in legacy_dirs {
            let old = dir.join("preferences.json");
            let Ok(bytes) = std::fs::read(&old) else {
                continue;
            };
            // Solo se migra si el contenido es válido.
            let Ok(prefs) = serde_json::from_slice::<Preferences>(&bytes) else {
                continue;
            };
            if self.save(&prefs).is_ok() {
                return Some(dir.clone());
            }
        }
        None
    }

    pub fn save(&self, prefs: &Preferences) -> std::io::Result<()> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(prefs)?)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600));
        }
        std::fs::rename(&tmp, path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("tlk-prefs-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn roundtrip_and_no_secret_fields() {
        let dir = temp_dir("rt");
        let store = PreferencesStore::new(Some(dir.clone()));
        let prefs = Preferences {
            remember_session: true,
            last_title: "Título".into(),
            last_category: Some(Category {
                id: "1".into(),
                name: "Juego".into(),
            }),
            audience_type: "0".into(),
            active_stream: Some(ActiveStreamRecord {
                id: "42".into(),
                title: "Título".into(),
                category_name: "Juego".into(),
                started_at_ms: 1,
            }),
        };
        store.save(&prefs).unwrap();
        assert_eq!(store.load(), prefs);
        let raw = std::fs::read_to_string(dir.join("preferences.json")).unwrap();
        for forbidden in [
            "token",
            "\"key\"",
            "streamKey",
            "rtmp",
            "verifier",
            "\"code\"",
        ] {
            assert!(!raw.contains(forbidden), "no debe contener {forbidden}");
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn migrates_preferences_from_legacy_folder_once() {
        let base = temp_dir("migr");
        let legacy = base.join("com.tiktokobsconnector.desktop");
        let current = base.join("com.virelo.desktop");
        std::fs::create_dir_all(&legacy).unwrap();
        let old = Preferences {
            remember_session: true,
            last_title: "Mi directo".into(),
            ..Default::default()
        };
        PreferencesStore::new(Some(legacy.clone()))
            .save(&old)
            .unwrap();
        let store = PreferencesStore::new(Some(current.clone()));
        let missing = base.join("com.tiktokobsconnector.app");
        assert_eq!(
            store.migrate_from(&[missing, legacy.clone()]),
            Some(legacy.clone())
        );
        assert_eq!(store.load(), old);
        // Una vez migrado no se vuelve a sobrescribir.
        let newer = Preferences {
            last_title: "Nuevo".into(),
            ..old.clone()
        };
        store.save(&newer).unwrap();
        assert_eq!(store.migrate_from(&[legacy]), None);
        assert_eq!(store.load(), newer);
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn invalid_file_yields_defaults() {
        let dir = temp_dir("bad");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("preferences.json"), b"{no json").unwrap();
        assert_eq!(
            PreferencesStore::new(Some(dir.clone())).load(),
            Preferences::default()
        );
        let _ = std::fs::remove_dir_all(dir);
    }
}
