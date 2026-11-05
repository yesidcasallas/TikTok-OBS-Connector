//! TikTok OBS Connector: inicia sesión con TikTok mediante Streamlabs y obtiene
//! el servidor RTMP y la clave de transmisión para OBS.

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
                "TikTok OBS Connector {} iniciado ({} {}).",
                env!("CARGO_PKG_VERSION"),
                std::env::consts::OS,
                std::env::consts::ARCH
            ));
            let provider = Arc::new(StreamlabsProvider::production()?.with_diagnostics(log.clone()));
            let core = Arc::new(AppCore::new(
                provider,
                Arc::new(KeyringStore),
                Arc::new(opener::SystemOpener),
                PreferencesStore::new(app.path().app_config_dir().ok()),
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
        .expect("error al iniciar TikTok OBS Connector");
}
