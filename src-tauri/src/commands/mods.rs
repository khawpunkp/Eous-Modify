use std::path::PathBuf;

use rusqlite::params;
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

use crate::keybinds;
use crate::models::{KeybindInfo, ModInput, ModWithState};
use crate::mods;
use crate::persisted_vars;
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

/// Flips a mod on or off, preserving the in-game toggle choices it holds.
///
/// The order here is the whole point, and it is easy to get wrong:
///
/// *Disabling* asks 3DMigoto to flush first, because the variables the user just changed in-game live
/// in its memory — `d3dx_user.ini` still holds the previous values until a reload or game exit, so
/// reading the file before flushing would snapshot stale state. We then snapshot, then rename. The
/// caller's reload afterwards is what makes 3DMigoto drop the now-unrecognised keys, which is fine
/// because they are saved by then.
///
/// *Enabling* renames first so the mod's ini is back at the path its keys are derived from, then
/// writes the values back, so the caller's reload finds them and applies them.
#[tauri::command]
pub fn toggle_mod_enabled(mod_id: i64, state: State<DbState>) -> Result<bool, String> {
    let mods_path = get_mods_folder(&state)?;

    let (folder_name, auto_reload, game_exe) = {
        let conn = state.0.lock().map_err(|e| e.to_string())?;
        let folder_name: String = conn
            .query_row("SELECT folder_name FROM mods WHERE id = ?1", params![mod_id], |row| {
                row.get(0)
            })
            .map_err(|e| e.to_string())?;
        let auto_reload = crate::commands::reload::auto_reload_enabled(&conn);
        let game_exe = crate::commands::reload::game_executable_from(&conn)?;
        (folder_name, auto_reload, game_exe)
    };

    let was_enabled = mods::is_mod_enabled(&mods_path, &folder_name).unwrap_or(false);

    // Snapshot before the rename, while the keys still match the enabled path.
    let saved = if was_enabled {
        // Only ask for a flush if the user opted into us sending keypresses at all. Without one the
        // snapshot is whatever 3DMigoto last wrote, which is still better than losing everything.
        if auto_reload {
            crate::commands::reload::flush_persisted_vars(&mods_path, game_exe.as_deref());
        }
        persisted_vars::snapshot(&mods_path, &folder_name)
    } else {
        Vec::new()
    };

    let is_enabled = mods::toggle_mod(&mods_path, &folder_name)?;

    if was_enabled {
        let conn = state.0.lock().map_err(|e| e.to_string())?;
        persisted_vars::store(&conn, mod_id, &saved)?;
    } else {
        let stored = {
            let conn = state.0.lock().map_err(|e| e.to_string())?;
            persisted_vars::load(&conn, mod_id)?
        };
        // Never fail the toggle over this: the rename already succeeded, and the mod works — it just
        // comes back with default toggles.
        if let Err(e) = persisted_vars::restore(&mods_path, &stored) {
            eprintln!("[toggle] could not restore persisted variables for mod {mod_id}: {e}");
        }
    }

    Ok(is_enabled)
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
