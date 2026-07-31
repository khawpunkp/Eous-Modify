use std::path::PathBuf;

use rusqlite::params;
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

use crate::keybinds;
use crate::models::{KeybindInfo, ModInput, ModWithState};
use crate::mods;
use crate::DbState;

fn get_mods_folder(state: &State<DbState>) -> Result<PathBuf, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    let path_str: String = conn
        .query_row("SELECT value FROM settings WHERE key = 'mods_folder_path'", [], |row| row.get(0))
        .map_err(|_| "Mods folder path is not configured. Set it in Settings first.".to_string())?;
    Ok(PathBuf::from(path_str))
}

#[tauri::command]
pub fn list_mods(
    agent_id: Option<i64>,
    category_id: Option<i64>,
    category_item_id: Option<i64>,
    state: State<DbState>,
) -> Result<Vec<ModWithState>, String> {
    let mods_path = get_mods_folder(&state)?;
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    mods::list_mods(&conn, &mods_path, agent_id, category_id, category_item_id)
}

#[tauri::command]
pub fn list_uncategorized_mods(state: State<DbState>) -> Result<Vec<ModWithState>, String> {
    let mods_path = get_mods_folder(&state)?;
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    mods::list_uncategorized_mods(&conn, &mods_path)
}

#[tauri::command]
pub fn toggle_mod_enabled(mod_id: i64, state: State<DbState>) -> Result<bool, String> {
    let mods_path = get_mods_folder(&state)?;
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    let folder_name: String = conn
        .query_row("SELECT folder_name FROM mods WHERE id = ?1", params![mod_id], |row| row.get(0))
        .map_err(|e| e.to_string())?;
    mods::toggle_mod(&mods_path, &folder_name)
}

#[tauri::command]
pub fn update_mod_info(mod_id: i64, input: ModInput, state: State<DbState>) -> Result<ModWithState, String> {
    let mods_path = get_mods_folder(&state)?;
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    mods::update_mod(&conn, &mods_path, mod_id, &input)
}

#[tauri::command]
pub fn update_mod_category(
    mod_id: i64,
    agent_id: Option<i64>,
    category_id: Option<i64>,
    category_item_id: Option<i64>,
    state: State<DbState>,
) -> Result<ModWithState, String> {
    let mods_path = get_mods_folder(&state)?;
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    mods::update_mod_category(&conn, &mods_path, mod_id, agent_id, category_id, category_item_id)
}

#[tauri::command]
pub fn delete_mod(mod_id: i64, state: State<DbState>) -> Result<(), String> {
    let mods_path = get_mods_folder(&state)?;
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    mods::delete_mod(&conn, &mods_path, mod_id)
}

#[tauri::command]
pub fn open_mod_folder(mod_id: i64, state: State<DbState>, app_handle: AppHandle) -> Result<(), String> {
    let mods_path = get_mods_folder(&state)?;
    let folder_name: String = {
        let conn = state.0.lock().map_err(|e| e.to_string())?;
        conn.query_row("SELECT folder_name FROM mods WHERE id = ?1", params![mod_id], |row| row.get(0))
            .map_err(|e| e.to_string())?
    };
    let path = mods::current_mod_path(&mods_path, &folder_name)
        .ok_or_else(|| "Mod folder not found on disk.".to_string())?;

    app_handle
        .opener()
        .open_path(path.to_string_lossy().to_string(), None::<String>)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_mod_keybinds(mod_id: i64, state: State<DbState>) -> Result<Vec<KeybindInfo>, String> {
    let mods_path = get_mods_folder(&state)?;
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    let folder_name: String = conn
        .query_row("SELECT folder_name FROM mods WHERE id = ?1", params![mod_id], |row| row.get(0))
        .map_err(|e| e.to_string())?;
    Ok(keybinds::get_keybinds(&mods_path, &folder_name))
}
