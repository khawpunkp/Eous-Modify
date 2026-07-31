use rusqlite::{params, OptionalExtension};
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

use crate::DbState;

#[tauri::command]
pub fn get_setting(key: String, state: State<DbState>) -> Result<Option<String>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    conn.query_row("SELECT value FROM settings WHERE key = ?1", params![key], |row| row.get(0))
        .optional()
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_setting(key: String, value: String, state: State<DbState>) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn open_mods_folder(state: State<DbState>, app_handle: AppHandle) -> Result<(), String> {
    let path: String = {
        let conn = state.0.lock().map_err(|e| e.to_string())?;
        conn.query_row("SELECT value FROM settings WHERE key = 'mods_folder_path'", [], |row| row.get(0))
            .map_err(|_| "Mods folder path is not configured. Set it in Settings first.".to_string())?
    };

    app_handle.opener().open_path(path, None::<String>).map_err(|e| e.to_string())
}
