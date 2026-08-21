use std::path::{Path, PathBuf};

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

/// Replaces a mod's files from a file the user picked, and hands back the mod as it now stands.
///
/// An archive is treated as a new version of the mod and replaces the folder's contents; any other
/// file is copied in over a file of the same name. The mod itself keeps its name, category, group and
/// enabled state either way — see `scanner::archive::update_files`.
#[tauri::command]
pub fn update_mod_files(
    mod_id: i64,
    source_path: String,
    state: State<DbState>,
) -> Result<ModWithState, String> {
    let mods_path = get_mods_folder(&state)?;
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    crate::scanner::archive::update_files(&conn, &mods_path, mod_id, Path::new(&source_path))?;
    mods::get_mod(&conn, &mods_path, mod_id)
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

/// The mod's preview image as a data URL, or `None` when it has none / the file is gone.
///
/// Resolved here rather than in the webview because the folder name in the database is the *enabled*
/// one: a disabled mod sits behind the `DISABLED_` prefix, so a path built from `folder_name` alone
/// misses and the card falls back to the placeholder. `current_mod_path` already picks whichever
/// variant exists.
#[tauri::command]
pub fn get_mod_preview(mod_id: i64, state: State<DbState>) -> Result<Option<String>, String> {
    let mods_path = get_mods_folder(&state)?;

    let (folder_name, image_filename): (String, Option<String>) = {
        let conn = state.0.lock().map_err(|e| e.to_string())?;
        conn.query_row(
            "SELECT folder_name, image_filename FROM mods WHERE id = ?1",
            params![mod_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|e| e.to_string())?
    };

    let Some(image_filename) = image_filename else {
        return Ok(None);
    };
    let Some(mod_dir) = mods::current_mod_path(&mods_path, &folder_name) else {
        return Ok(None);
    };

    let path = mod_dir.join(&image_filename);
    if !path.is_file() {
        return Ok(None);
    }

    crate::commands::images::read_image_as_data_url(path.to_string_lossy().to_string()).map(Some)
}

/// The image the mod itself ships with, ignoring any preview this app saved.
///
/// `find_preview_image` only matches the conventional names a mod author uses — preview, icon,
/// thumbnail — and never our `mod_preview.*`, so this is precisely what removing a custom image falls
/// back to. That lets the editor show the real result before the save happens, rather than a
/// placeholder that misrepresents it.
#[tauri::command]
pub fn get_mod_default_preview(mod_id: i64, state: State<DbState>) -> Result<Option<String>, String> {
    let mods_path = get_mods_folder(&state)?;

    let folder_name: String = {
        let conn = state.0.lock().map_err(|e| e.to_string())?;
        conn.query_row("SELECT folder_name FROM mods WHERE id = ?1", params![mod_id], |row| {
            row.get(0)
        })
        .map_err(|e| e.to_string())?
    };

    let Some(mod_dir) = mods::current_mod_path(&mods_path, &folder_name) else {
        return Ok(None);
    };
    let Some(filename) = crate::scanner::deduce::find_preview_image(&mod_dir) else {
        return Ok(None);
    };

    crate::commands::images::read_image_as_data_url(
        mod_dir.join(filename).to_string_lossy().to_string(),
    )
    .map(Some)
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
