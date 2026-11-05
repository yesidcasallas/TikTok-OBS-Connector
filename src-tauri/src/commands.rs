//! Comandos Tauri: única puerta de entrada del frontend. Ninguno devuelve el
//! token; la clave de transmisión solo sale con `stream_reveal_key`.

use crate::app_core::{AppCore, CopyField, Snapshot};
use crate::error::{AppError, AppResult};
use crate::provider::{Category, StartLiveRequest};
use std::sync::Arc;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_clipboard_manager::ClipboardExt;

type Core<'a> = State<'a, Arc<AppCore>>;

fn focus_main_window(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

#[tauri::command]
pub async fn get_state(core: Core<'_>) -> AppResult<Snapshot> {
    Ok(core.snapshot())
}

#[tauri::command]
pub async fn auth_login(app: AppHandle, core: Core<'_>) -> AppResult<Snapshot> {
    let result = core.login().await;
    if !matches!(
        result,
        Err(AppError::AuthCancelled) | Err(AppError::AuthInProgress) | Err(AppError::AlreadyLoggedIn)
    ) {
        focus_main_window(&app);
    }
    result
}

#[tauri::command]
pub async fn auth_cancel(core: Core<'_>) -> AppResult<()> {
    core.cancel_login();
    Ok(())
}

#[tauri::command]
pub async fn auth_logout(core: Core<'_>) -> AppResult<Snapshot> {
    core.logout(false).await
}

#[tauri::command]
pub async fn set_remember_session(core: Core<'_>, remember: bool) -> AppResult<Snapshot> {
    core.set_remember(remember).await
}

#[tauri::command]
pub async fn categories_search(core: Core<'_>, query: String) -> AppResult<Vec<Category>> {
    core.search_categories(&query).await
}

#[tauri::command]
pub async fn stream_create(core: Core<'_>, request: StartLiveRequest) -> AppResult<Snapshot> {
    core.start_stream(request).await
}

#[tauri::command]
pub async fn stream_end(core: Core<'_>) -> AppResult<Snapshot> {
    core.end_stream().await
}

#[tauri::command]
pub async fn stream_reveal_key(core: Core<'_>) -> AppResult<String> {
    Ok(core.reveal_key()?.expose().to_string())
}

/// Copia al portapapeles desde Rust: el valor no pasa por el frontend.
#[tauri::command]
pub async fn copy_value(app: AppHandle, core: Core<'_>, field: CopyField) -> AppResult<()> {
    let value = core.copy_text(field)?;
    app.clipboard()
        .write_text(value.expose().to_string())
        .map_err(|e| AppError::Clipboard(e.to_string()))
}
