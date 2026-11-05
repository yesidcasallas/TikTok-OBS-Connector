//! Virelo: inicia sesión con TikTok mediante la integración de Streamlabs y
//! obtiene el servidor RTMP y la clave de transmisión para OBS.
//!
//! Virelo no tiene (todavía) una integración propia aprobada por TikTok: la
//! autorización la concede Streamlabs. Ver `docs/integracion-tiktok/`.

pub mod app_core;
pub mod auth;
pub mod commands;
pub mod error;
pub mod event_log;
pub mod opener;
pub mod prefs;
pub mod provider;
pub mod security;

use app_core::{AppCore, Snapshot, StateSink};
use event_log::EventLog;
use prefs::PreferencesStore;
use provider::streamlabs::StreamlabsProvider;
use security::KeyringStore;
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

pub const EVENT_STATE: &str = "app-state";
/// Nombre visible de la aplicación.
pub const APP_NAME: &str = "Virelo";
/// Identificadores de versiones anteriores (carpetas de datos a migrar).
pub const LEGACY_IDENTIFIERS: &[&str] = &[
    "com.tiktokobsconnector.desktop",
    "com.tiktokobsconnector.app",
];
/// Tiempo máximo para completar el login en el navegador.
pub const LOGIN_TIMEOUT: Duration = Duration::from_secs(300);

struct TauriSink(AppHandle);

impl StateSink for TauriSink {
    fn on_state(&self, snapshot: &Snapshot) {
        let _ = self.0.emit(EVENT_STATE, snapshot);
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    provider::streamlabs::client::ensure_crypto_provider();
    tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .setup(|app| {
            let log = Arc::new(EventLog::default());
            if let Ok(dir) = app.path().app_log_dir() {
                log.set_file(dir.join("diagnostico.log"));
            }
            log.info(format!(
                "{APP_NAME} {} iniciado ({} {}).",
                env!("CARGO_PKG_VERSION"),
                std::env::consts::OS,
                std::env::consts::ARCH
            ));
            let provider =
                Arc::new(StreamlabsProvider::production()?.with_diagnostics(log.clone()));
            // Migración desde versiones con otro identificador (mismo usuario).
            let config_dir = app.path().app_config_dir().ok();
            let prefs = PreferencesStore::new(config_dir.clone());
            if let Some(parent) = config_dir.as_deref().and_then(|d| d.parent()) {
                let legacy: Vec<_> = LEGACY_IDENTIFIERS
                    .iter()
                    .map(|id| parent.join(id))
                    .collect();
                if let Some(from) = prefs.migrate_from(&legacy) {
                    log.info(format!("Preferencias migradas desde {}.", from.display()));
                }
            }
            let core = Arc::new(AppCore::new(
                provider,
                Arc::new(KeyringStore::default()),
                Arc::new(opener::SystemOpener),
                prefs,
                log,
                LOGIN_TIMEOUT,
            ));
            core.set_sink(Arc::new(TauriSink(app.handle().clone())));
            tauri::async_runtime::spawn(core.clone().restore());
            app.manage(core);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_state,
            commands::auth_login,
            commands::auth_cancel,
            commands::auth_logout,
            commands::set_remember_session,
            commands::account_refresh,
            commands::open_access_page,
            commands::copy_diagnostics,
            commands::categories_search,
            commands::stream_create,
            commands::stream_end,
            commands::stream_reveal_key,
            commands::copy_value,
        ])
        .run(tauri::generate_context!())
        .expect("error al iniciar Virelo");
}
