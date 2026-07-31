use std::path::PathBuf;

use tauri::State;

use crate::mod_groups;
use crate::models::ModGroupWithMembers;
use crate::DbState;

fn get_mods_folder(state: &State<DbState>) -> Result<PathBuf, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    let path_str: String = conn
        .query_row("SELECT value FROM settings WHERE key = 'mods_folder_path'", [], |row| row.get(0))
        .map_err(|_| "Mods folder path is not configured. Set it in Settings first.".to_string())?;
    Ok(PathBuf::from(path_str))
}

#[tauri::command]
pub fn list_mod_groups(state: State<DbState>) -> Result<Vec<ModGroupWithMembers>, String> {
    let mods_path = get_mods_folder(&state)?;
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    mod_groups::list_groups(&conn, &mods_path)
}

#[tauri::command]
pub fn create_mod_group(
    name: String,
    base_image: Option<String>,
    mod_ids: Vec<i64>,
    state: State<DbState>,
) -> Result<ModGroupWithMembers, String> {
    let mods_path = get_mods_folder(&state)?;
    let mut conn = state.0.lock().map_err(|e| e.to_string())?;
    mod_groups::create_group(&mut conn, &mods_path, &name, base_image.as_deref(), &mod_ids)
}

#[tauri::command]
pub fn add_mod_to_group(group_id: i64, mod_id: i64, state: State<DbState>) -> Result<ModGroupWithMembers, String> {
    let mods_path = get_mods_folder(&state)?;
    let mut conn = state.0.lock().map_err(|e| e.to_string())?;
    mod_groups::add_member(&mut conn, &mods_path, group_id, mod_id)
}

#[tauri::command]
pub fn remove_mod_from_group(
    group_id: i64,
    mod_id: i64,
    state: State<DbState>,
) -> Result<Option<ModGroupWithMembers>, String> {
    let mods_path = get_mods_folder(&state)?;
    let mut conn = state.0.lock().map_err(|e| e.to_string())?;
    mod_groups::remove_member(&mut conn, &mods_path, group_id, mod_id)
}

#[tauri::command]
pub fn update_mod_group(
    group_id: i64,
    name: String,
    base_image: Option<String>,
    state: State<DbState>,
) -> Result<ModGroupWithMembers, String> {
    let mods_path = get_mods_folder(&state)?;
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    mod_groups::update_group(&conn, &mods_path, group_id, &name, base_image.as_deref())
}

#[tauri::command]
pub fn delete_mod_group(group_id: i64, state: State<DbState>) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    mod_groups::delete_group(&conn, group_id)
}

#[tauri::command]
pub fn toggle_mod_group(group_id: i64, state: State<DbState>) -> Result<bool, String> {
    let mods_path = get_mods_folder(&state)?;
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    mod_groups::toggle_group(&conn, &mods_path, group_id)
}
