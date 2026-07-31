use std::fs;
use std::path::{Path, PathBuf};

use base64::{engine::general_purpose::STANDARD, Engine};
use rusqlite::{params, Connection};

use crate::models::{ModInput, ModWithState};
use crate::scanner::archive::resolve_category_subpath;
use crate::scanner::deduce::DISABLED_PREFIX;

const MOD_PREVIEW_BASENAME: &str = "mod_preview";

const MOD_COLUMNS: &str = "m.id, m.agent_id, m.category_id, m.category_item_id, m.name, \
     m.folder_name, m.image_filename, m.author, mgm.group_id";
const MOD_FROM: &str = "mods m LEFT JOIN mod_group_members mgm ON mgm.mod_id = m.id";

fn enabled_disabled_paths(base_mods_path: &Path, folder_name: &str) -> (PathBuf, PathBuf) {
    let relative = PathBuf::from(folder_name);
    let filename = relative.file_name().unwrap_or_default().to_string_lossy().to_string();
    let disabled_filename = format!("{}{}", DISABLED_PREFIX, filename);
    let parent = relative.parent();

    let enabled_path = base_mods_path.join(&relative);
    let disabled_path = match parent {
        Some(p) if !p.as_os_str().is_empty() => base_mods_path.join(p).join(&disabled_filename),
        _ => base_mods_path.join(&disabled_filename),
    };
    (enabled_path, disabled_path)
}

pub fn is_mod_enabled(base_mods_path: &Path, folder_name: &str) -> Option<bool> {
    let (enabled_path, disabled_path) = enabled_disabled_paths(base_mods_path, folder_name);
    if enabled_path.is_dir() {
        Some(true)
    } else if disabled_path.is_dir() {
        Some(false)
    } else {
        None
    }
}

/// Whichever of the enabled/DISABLED_ path variants currently exists on disk, if either.
pub fn current_mod_path(base_mods_path: &Path, folder_name: &str) -> Option<PathBuf> {
    let (enabled_path, disabled_path) = enabled_disabled_paths(base_mods_path, folder_name);
    if enabled_path.is_dir() {
        Some(enabled_path)
    } else if disabled_path.is_dir() {
        Some(disabled_path)
    } else {
        None
    }
}

pub fn toggle_mod(base_mods_path: &Path, folder_name: &str) -> Result<bool, String> {
    let (enabled_path, disabled_path) = enabled_disabled_paths(base_mods_path, folder_name);
    let (current_path, target_path, new_state) = if enabled_path.is_dir() {
        (enabled_path, disabled_path, false)
    } else if disabled_path.is_dir() {
        (disabled_path, enabled_path, true)
    } else {
        return Err(format!(
            "Mod folder not found on disk for '{}' (checked both the enabled and DISABLED_ variants).",
            folder_name
        ));
    };

    fs::rename(&current_path, &target_path)
        .map_err(|e| format!("Failed to rename '{}' to '{}': {}", current_path.display(), target_path.display(), e))?;

    Ok(new_state)
}

fn row_to_mod(row: &rusqlite::Row) -> rusqlite::Result<ModWithState> {
    Ok(ModWithState {
        id: row.get(0)?,
        agent_id: row.get(1)?,
        category_id: row.get(2)?,
        category_item_id: row.get(3)?,
        name: row.get(4)?,
        folder_name: row.get(5)?,
        image_filename: row.get(6)?,
        author: row.get(7)?,
        is_enabled: false,
        group_id: row.get(8)?,
    })
}

pub fn list_mods(
    conn: &Connection,
    base_mods_path: &Path,
    agent_id: Option<i64>,
    category_id: Option<i64>,
    category_item_id: Option<i64>,
) -> Result<Vec<ModWithState>, String> {
    let sql = format!("SELECT {} FROM {} ORDER BY m.name", MOD_COLUMNS, MOD_FROM);
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let rows = stmt.query_map([], row_to_mod).map_err(|e| e.to_string())?;
    let all: Vec<ModWithState> = rows.collect::<Result<_, _>>().map_err(|e| e.to_string())?;

    Ok(all
        .into_iter()
        .filter(|m| {
            (agent_id.is_none() || m.agent_id == agent_id)
                && (category_id.is_none() || m.category_id == category_id)
                && (category_item_id.is_none() || m.category_item_id == category_item_id)
        })
        .map(|mut m| {
            m.is_enabled = is_mod_enabled(base_mods_path, &m.folder_name).unwrap_or(false);
            m
        })
        .collect())
}

pub fn list_uncategorized_mods(
    conn: &Connection,
    base_mods_path: &Path,
) -> Result<Vec<ModWithState>, String> {
    let sql = format!("SELECT {} FROM {} ORDER BY m.name", MOD_COLUMNS, MOD_FROM);
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let rows = stmt.query_map([], row_to_mod).map_err(|e| e.to_string())?;
    let all: Vec<ModWithState> = rows.collect::<Result<_, _>>().map_err(|e| e.to_string())?;

    Ok(all
        .into_iter()
        .filter(|m| m.agent_id.is_none() && m.category_id.is_none())
        .map(|mut m| {
            m.is_enabled = is_mod_enabled(base_mods_path, &m.folder_name).unwrap_or(false);
            m
        })
        .collect())
}

pub fn get_mod(conn: &Connection, base_mods_path: &Path, mod_id: i64) -> Result<ModWithState, String> {
    let sql = format!("SELECT {} FROM {} WHERE m.id = ?1", MOD_COLUMNS, MOD_FROM);
    let mut m = conn
        .query_row(&sql, params![mod_id], row_to_mod)
        .map_err(|e| e.to_string())?;
    m.is_enabled = is_mod_enabled(base_mods_path, &m.folder_name).unwrap_or(false);
    Ok(m)
}

fn decode_data_url(data_url: &str) -> Result<(Vec<u8>, String), String> {
    let comma = data_url.find(',').ok_or_else(|| "Invalid image data URL".to_string())?;
    let header = &data_url[..comma];
    let bytes = STANDARD
        .decode(&data_url[comma + 1..])
        .map_err(|e| format!("Failed to decode image data: {}", e))?;
    let ext = if header.contains("png") {
        "png"
    } else if header.contains("jpeg") || header.contains("jpg") {
        "jpg"
    } else if header.contains("webp") {
        "webp"
    } else if header.contains("gif") {
        "gif"
    } else {
        "png"
    };
    Ok((bytes, ext.to_string()))
}

pub fn update_mod(
    conn: &Connection,
    base_mods_path: &Path,
    mod_id: i64,
    input: &ModInput,
) -> Result<ModWithState, String> {
    let folder_name: String = conn
        .query_row("SELECT folder_name FROM mods WHERE id = ?1", params![mod_id], |row| row.get(0))
        .map_err(|e| e.to_string())?;

    let mut new_image_filename: Option<String> = None;
    if let Some(data_url) = &input.image_data_url {
        let (bytes, ext) = decode_data_url(data_url)?;
        let mod_dir = current_mod_path(base_mods_path, &folder_name)
            .ok_or_else(|| "Mod folder not found on disk; cannot save image.".to_string())?;
        let filename = format!("{}.{}", MOD_PREVIEW_BASENAME, ext);
        fs::write(mod_dir.join(&filename), bytes).map_err(|e| format!("Failed to save mod image: {}", e))?;
        new_image_filename = Some(filename);
    }

    match &new_image_filename {
        Some(filename) => conn.execute(
            "UPDATE mods SET name = ?1, author = ?2, image_filename = ?3 WHERE id = ?4",
            params![input.name, input.author, filename, mod_id],
        ),
        None => conn.execute(
            "UPDATE mods SET name = ?1, author = ?2 WHERE id = ?3",
            params![input.name, input.author, mod_id],
        ),
    }
    .map_err(|e| e.to_string())?;

    get_mod(conn, base_mods_path, mod_id)
}

/// If `category_item_id` is already set, returns it unchanged. Otherwise, if `category_id` is set,
/// resolves to that category's permanent "Other" item (seeded by `db::seed::sync_categories`) so a
/// mod filed under a category is never left without an item. Returns `None` if `category_id` is
/// also `None` (an agent-scoped, or fully uncategorized, mod).
pub fn resolve_category_item_or_other(
    conn: &Connection,
    category_id: Option<i64>,
    category_item_id: Option<i64>,
) -> Result<Option<i64>, String> {
    if category_item_id.is_some() {
        return Ok(category_item_id);
    }
    let Some(category_id) = category_id else { return Ok(None) };

    let category_slug: String = conn
        .query_row("SELECT slug FROM categories WHERE id = ?1", params![category_id], |row| row.get(0))
        .map_err(|e| e.to_string())?;
    let other_slug = format!("{}-other", category_slug);
    conn.query_row("SELECT id FROM category_items WHERE slug = ?1", params![other_slug], |row| row.get(0))
        .map(Some)
        .map_err(|e| e.to_string())
}

/// Reassigns a mod to a different agent or category (mutually exclusive — passing `agent_id`
/// clears any category assignment, and vice versa), moving its folder on disk to match via
/// `resolve_category_subpath` — the same logic archive import uses — so `folder_name` and the
/// mod's actual location never drift apart.
pub fn update_mod_category(
    conn: &Connection,
    base_mods_path: &Path,
    mod_id: i64,
    agent_id: Option<i64>,
    category_id: Option<i64>,
    category_item_id: Option<i64>,
) -> Result<ModWithState, String> {
    let old_folder_name: String = conn
        .query_row("SELECT folder_name FROM mods WHERE id = ?1", params![mod_id], |row| row.get(0))
        .map_err(|e| e.to_string())?;

    let (agent_id, category_id) = if agent_id.is_some() { (agent_id, None) } else { (None, category_id) };
    let resolved_item_id = resolve_category_item_or_other(conn, category_id, category_item_id)?;

    let dest_subpath = resolve_category_subpath(conn, agent_id, category_id, resolved_item_id)?;
    let base_name = Path::new(&old_folder_name)
        .file_name()
        .ok_or_else(|| "Invalid mod folder name.".to_string())?
        .to_string_lossy()
        .to_string();
    let new_folder_name = dest_subpath.join(&base_name).to_string_lossy().replace('\\', "/");

    if new_folder_name != old_folder_name {
        let old_path = current_mod_path(base_mods_path, &old_folder_name)
            .ok_or_else(|| "Mod folder not found on disk.".to_string())?;
        let is_disabled = old_path
            .file_name()
            .map(|n| n.to_string_lossy().starts_with(DISABLED_PREFIX))
            .unwrap_or(false);
        let new_dir = base_mods_path.join(&dest_subpath);
        fs::create_dir_all(&new_dir).map_err(|e| e.to_string())?;
        let new_filename = if is_disabled { format!("{}{}", DISABLED_PREFIX, base_name) } else { base_name.clone() };
        let new_path = new_dir.join(&new_filename);
        fs::rename(&old_path, &new_path)
            .map_err(|e| format!("Failed to move mod folder to '{}': {}", new_path.display(), e))?;
    }

    conn.execute(
        "UPDATE mods SET agent_id = ?1, category_id = ?2, category_item_id = ?3, folder_name = ?4 WHERE id = ?5",
        params![agent_id, category_id, resolved_item_id, new_folder_name, mod_id],
    )
    .map_err(|e| e.to_string())?;

    get_mod(conn, base_mods_path, mod_id)
}

/// Every `.ini` inside whichever path variant currently exists on disk for this mod, nested ones
/// included, in a stable order — used by keybinds parsing (and, later, skin-toggle memory in Phase 7).
///
/// Recurses because plenty of mods keep their `.ini` in a subfolder rather than at the top level, and
/// a single-level `read_dir` finds nothing at all for those. Sorted because `read_dir` order is
/// unspecified, and these feed the keybind list the user reads.
pub fn find_mod_ini_paths(base_mods_path: &Path, folder_name: &str) -> Vec<PathBuf> {
    let Some(mod_dir) = current_mod_path(base_mods_path, folder_name) else {
        return Vec::new();
    };

    let mut ini_paths = Vec::new();
    let mut pending = vec![mod_dir];

    while let Some(dir) = pending.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.filter_map(|e| e.ok()) {
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            let path = entry.path();
            if file_type.is_dir() {
                pending.push(path);
            } else if file_type.is_file()
                && path.extension().map(|ext| ext.eq_ignore_ascii_case("ini")).unwrap_or(false)
            {
                ini_paths.push(path);
            }
        }
    }

    ini_paths.sort();
    ini_paths
}

pub fn delete_mod(conn: &Connection, base_mods_path: &Path, mod_id: i64) -> Result<(), String> {
    let folder_name: String = conn
        .query_row("SELECT folder_name FROM mods WHERE id = ?1", params![mod_id], |row| row.get(0))
        .map_err(|e| e.to_string())?;

    // If the folder's already gone (moved/deleted outside the app), proceed with the DB delete
    // anyway rather than failing — matches the old app's `delete_asset` behavior.
    if let Some(path) = current_mod_path(base_mods_path, &folder_name) {
        fs::remove_dir_all(&path).map_err(|e| format!("Failed to delete mod folder: {}", e))?;
    }

    conn.execute("DELETE FROM mods WHERE id = ?1", params![mod_id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    fn build_test_mod_dir() -> (PathBuf, String) {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let unique = COUNTER.fetch_add(1, Ordering::SeqCst);
        let base = std::env::temp_dir().join(format!("eous_modify_mods_test_{}_{}", std::process::id(), unique));
        let _ = fs::remove_dir_all(&base);

        let folder_name = "TestMod".to_string();
        fs::create_dir_all(base.join(&folder_name)).unwrap();
        fs::write(base.join(&folder_name).join("mod.ini"), "").unwrap();

        (base, folder_name)
    }

    #[test]
    fn toggle_flips_enabled_state_and_renames_on_disk() {
        let (base, folder_name) = build_test_mod_dir();

        assert_eq!(is_mod_enabled(&base, &folder_name), Some(true));

        let new_state = toggle_mod(&base, &folder_name).expect("toggle should succeed");
        assert!(!new_state, "first toggle should disable the mod");
        assert!(base.join(format!("{}{}", DISABLED_PREFIX, folder_name)).is_dir());
        assert!(!base.join(&folder_name).is_dir());
        assert_eq!(is_mod_enabled(&base, &folder_name), Some(false));

        let new_state2 = toggle_mod(&base, &folder_name).expect("second toggle should succeed");
        assert!(new_state2, "second toggle should re-enable the mod");
        assert!(base.join(&folder_name).is_dir());
        assert_eq!(is_mod_enabled(&base, &folder_name), Some(true));

        fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn toggle_errors_when_folder_missing() {
        let base = std::env::temp_dir().join(format!("eous_modify_mods_test_missing_{}", std::process::id()));
        let result = toggle_mod(&base, "DoesNotExist");
        assert!(result.is_err());
    }

    fn setup_category_test_db_and_dir() -> (Connection, PathBuf) {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(crate::db::schema::SCHEMA).unwrap();

        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let unique = COUNTER.fetch_add(1, Ordering::SeqCst);
        let base = std::env::temp_dir().join(format!("eous_modify_mods_category_test_{}_{}", std::process::id(), unique));
        let _ = fs::remove_dir_all(&base);

        conn.execute("INSERT INTO categories (id, name, slug) VALUES (1, 'NPCs', 'npcs')", [])
            .unwrap();
        conn.execute(
            "INSERT INTO category_items (id, category_id, name, slug) VALUES (1, 1, 'Other NPCs', 'npcs-other')",
            [],
        )
        .unwrap();
        conn.execute("INSERT INTO agents (id, name, slug) VALUES (1, 'Ellen', 'ellen')", [])
            .unwrap();

        (conn, base)
    }

    #[test]
    fn resolve_category_item_or_other_returns_given_item_unchanged() {
        let (conn, base) = setup_category_test_db_and_dir();
        assert_eq!(resolve_category_item_or_other(&conn, Some(1), Some(42)).unwrap(), Some(42));
        fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn resolve_category_item_or_other_falls_back_to_other_item() {
        let (conn, base) = setup_category_test_db_and_dir();
        assert_eq!(resolve_category_item_or_other(&conn, Some(1), None).unwrap(), Some(1));
        fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn resolve_category_item_or_other_returns_none_without_category() {
        let (conn, base) = setup_category_test_db_and_dir();
        assert_eq!(resolve_category_item_or_other(&conn, None, None).unwrap(), None);
        fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn update_mod_category_moves_folder_and_updates_row() {
        let (conn, base) = setup_category_test_db_and_dir();

        let old_folder = "npcs/npcs-other/SomeMod";
        fs::create_dir_all(base.join(old_folder)).unwrap();
        fs::write(base.join(old_folder).join("mod.ini"), "").unwrap();
        conn.execute(
            "INSERT INTO mods (category_id, category_item_id, name, folder_name) VALUES (1, 1, 'Some Mod', ?1)",
            params![old_folder],
        )
        .unwrap();
        let mod_id = conn.last_insert_rowid();

        let updated = update_mod_category(&conn, &base, mod_id, Some(1), None, None).expect("move should succeed");

        assert_eq!(updated.agent_id, Some(1));
        assert_eq!(updated.category_id, None);
        assert_eq!(updated.category_item_id, None);
        assert_eq!(updated.folder_name, "ellen/SomeMod");
        assert!(base.join("ellen").join("SomeMod").is_dir());
        assert!(!base.join(old_folder).is_dir());

        fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn update_mod_category_preserves_disabled_prefix() {
        let (conn, base) = setup_category_test_db_and_dir();

        let old_folder = "npcs/npcs-other/SomeMod";
        fs::create_dir_all(base.join("npcs/npcs-other")).unwrap();
        fs::create_dir_all(base.join("npcs/npcs-other").join(format!("{}SomeMod", DISABLED_PREFIX))).unwrap();
        conn.execute(
            "INSERT INTO mods (category_id, category_item_id, name, folder_name) VALUES (1, 1, 'Some Mod', ?1)",
            params![old_folder],
        )
        .unwrap();
        let mod_id = conn.last_insert_rowid();

        let updated = update_mod_category(&conn, &base, mod_id, Some(1), None, None).expect("move should succeed");

        assert_eq!(updated.folder_name, "ellen/SomeMod");
        assert!(base.join("ellen").join(format!("{}SomeMod", DISABLED_PREFIX)).is_dir());

        fs::remove_dir_all(&base).ok();
    }
}
