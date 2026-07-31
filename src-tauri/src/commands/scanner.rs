use std::path::PathBuf;

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::scanner;
use crate::scanner::archive::{self, ArchiveAnalysis, ImportRequest};
use crate::scanner::deduce::fetch_deduction_maps;
use crate::DbState;

fn get_mods_folder(state: &State<DbState>) -> Result<PathBuf, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    let path_str: String = conn
        .query_row("SELECT value FROM settings WHERE key = 'mods_folder_path'", [], |row| row.get(0))
        .map_err(|_| "Mods folder path is not configured. Set it in Settings first.".to_string())?;
    Ok(PathBuf::from(path_str))
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ScanProgress {
    processed: usize,
    current_path: String,
}

#[tauri::command]
pub fn scan_mods_directory(state: State<DbState>, app_handle: AppHandle) -> Result<String, String> {
    let mods_path = get_mods_folder(&state)?;
    let mut conn = state.0.lock().map_err(|e| e.to_string())?;

    let result = scanner::run_scan(&mut conn, &mods_path, |processed, current_path| {
        app_handle
            .emit("scan-progress", ScanProgress { processed, current_path: current_path.to_string() })
            .ok();
    });

    match &result {
        Ok(summary) => {
            app_handle.emit("scan-complete", summary).ok();
        }
        Err(e) => {
            app_handle.emit("scan-error", e).ok();
        }
    }
    result
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportArchiveRequest {
    pub archive_path: String,
    pub agent_id: Option<i64>,
    pub category_id: Option<i64>,
    pub category_item_id: Option<i64>,
    pub selected_internal_root: Option<String>,
    pub mod_name: String,
    pub author: Option<String>,
}

#[tauri::command]
pub fn analyze_archive(archive_path: String, state: State<DbState>) -> Result<ArchiveAnalysis, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    let maps = fetch_deduction_maps(&conn).map_err(|e| e.to_string())?;
    archive::analyze(&PathBuf::from(archive_path), &maps)
}

#[tauri::command]
pub fn import_archive(request: ImportArchiveRequest, state: State<DbState>) -> Result<i64, String> {
    let mods_path = get_mods_folder(&state)?;
    let mut conn = state.0.lock().map_err(|e| e.to_string())?;
    archive::import(
        &mut conn,
        &PathBuf::from(&request.archive_path),
        &mods_path,
        ImportRequest {
            agent_id: request.agent_id,
            category_id: request.category_id,
            category_item_id: request.category_item_id,
            selected_internal_root: request.selected_internal_root,
            mod_name: request.mod_name,
            author: request.author,
        },
    )
}
