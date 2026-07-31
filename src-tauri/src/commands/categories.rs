use tauri::State;

use crate::models::Category;
use crate::DbState;

#[tauri::command]
pub fn list_categories(state: State<DbState>) -> Result<Vec<Category>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT id, name, slug FROM categories ORDER BY name")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| Ok(Category { id: row.get(0)?, name: row.get(1)?, slug: row.get(2)? }))
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<_, _>>().map_err(|e| e.to_string())
}
