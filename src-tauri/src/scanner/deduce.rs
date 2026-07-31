use std::fs;
use std::path::{Path, PathBuf};

use ini::Ini;
use once_cell::sync::Lazy;
use regex::Regex;
use rusqlite::Connection;

pub const DISABLED_PREFIX: &str = "DISABLED_";

const EXCLUDED_INI_FILENAMES: &[&str] = &[
    "orfix.ini",
    "region.ini",
    "offset.ini",
    "water.ini",
    "fixdash.ini",
    "deltatime.ini",
    "object.ini",
    "timer.ini",
];

const PREVIEW_IMAGE_NAMES: &[&str] = &[
    "preview.png",
    "preview.jpg",
    "icon.png",
    "icon.jpg",
    "thumbnail.png",
    "thumbnail.jpg",
];

static VERSION_TAG_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)_v\d+(\.\d+)*").unwrap());
static DISABLED_TAG_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)\(disabled\)|_disabled|^disabled_?").unwrap());
static SEPARATOR_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"[_\-.\s]+").unwrap());
static MOD_NAME_CLEANUP_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)(_v\d+(\.\d+)*|_disabled|disabled_|\(disabled\))").unwrap());

/// Lowercase, separator-collapsed, version/disabled-tag-stripped form of a hint string —
/// aliases and category names are matched against this, not the raw hint.
pub fn normalize_hint(hint: &str) -> String {
    let lower = hint.to_lowercase();
    let no_version = VERSION_TAG_REGEX.replace_all(&lower, "");
    let no_disabled = DISABLED_TAG_REGEX.replace_all(&no_version, "");
    SEPARATOR_REGEX.replace_all(&no_disabled, " ").trim().to_string()
}

pub fn clean_mod_name(name: &str, fallback: &str) -> String {
    let cleaned = MOD_NAME_CLEANUP_REGEX.replace_all(name, "").trim().to_string();
    if cleaned.is_empty() {
        fallback.to_string()
    } else {
        cleaned
    }
}

pub struct DeductionMaps {
    /// (lowercase alias, agent_id)
    agent_aliases: Vec<(String, i64)>,
    /// (lowercase name, lowercase slug, category_id, category_item_id)
    category_items: Vec<(String, String, i64, i64)>,
    /// (lowercase name, lowercase slug, category_id)
    categories: Vec<(String, String, i64)>,
}

pub fn fetch_deduction_maps(conn: &Connection) -> rusqlite::Result<DeductionMaps> {
    let mut agent_aliases = Vec::new();
    {
        let mut stmt = conn.prepare("SELECT alias, agent_id FROM agent_aliases")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?.to_lowercase(), row.get::<_, i64>(1)?))
        })?;
        for row in rows {
            agent_aliases.push(row?);
        }
    }

    let mut category_items = Vec::new();
    {
        let mut stmt = conn.prepare("SELECT name, slug, category_id, id FROM category_items")?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?.to_lowercase(),
                row.get::<_, String>(1)?.to_lowercase(),
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
            ))
        })?;
        for row in rows {
            category_items.push(row?);
        }
    }

    let mut categories = Vec::new();
    {
        let mut stmt = conn.prepare("SELECT name, slug, id FROM categories")?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?.to_lowercase(),
                row.get::<_, String>(1)?.to_lowercase(),
                row.get::<_, i64>(2)?,
            ))
        })?;
        for row in rows {
            categories.push(row?);
        }
    }

    Ok(DeductionMaps { agent_aliases, category_items, categories })
}

/// Substring-match a hint against every agent's aliases. Longest matching alias wins on collision.
pub fn find_agent_match(hint: &str, maps: &DeductionMaps) -> Option<i64> {
    let normalized = normalize_hint(hint);
    if normalized.is_empty() {
        return None;
    }
    maps.agent_aliases
        .iter()
        .filter(|(alias, _)| !alias.is_empty() && normalized.contains(alias.as_str()))
        .max_by_key(|(alias, _)| alias.len())
        .map(|(_, agent_id)| *agent_id)
}

/// (category_item_id, category_id) — tries category_items first (more specific), then categories.
pub fn find_category_match(hint: &str, maps: &DeductionMaps) -> Option<(Option<i64>, i64)> {
    let normalized = normalize_hint(hint);
    if normalized.is_empty() {
        return None;
    }

    let item_match = maps
        .category_items
        .iter()
        .filter(|(name, slug, _, _)| normalized.contains(name.as_str()) || normalized.contains(slug.as_str()))
        .max_by_key(|(name, slug, _, _)| name.len().max(slug.len()));

    if let Some((_, _, category_id, item_id)) = item_match {
        return Some((Some(*item_id), *category_id));
    }

    maps.categories
        .iter()
        .filter(|(name, slug, _)| normalized.contains(name.as_str()) || normalized.contains(slug.as_str()))
        .max_by_key(|(name, slug, _)| name.len().max(slug.len()))
        .map(|(_, _, category_id)| (None, *category_id))
}

pub fn has_ini_file(dir: &Path) -> bool {
    let Ok(entries) = fs::read_dir(dir) else {
        return false;
    };
    for entry in entries.filter_map(|e| e.ok()) {
        if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
            continue;
        }
        let path = entry.path();
        let Some(ext) = path.extension() else {
            continue;
        };
        if !ext.eq_ignore_ascii_case("ini") {
            continue;
        }
        let Some(filename) = path.file_name() else {
            continue;
        };
        let filename_lower = filename.to_string_lossy().to_lowercase();
        let base_filename = filename_lower
            .strip_prefix(&DISABLED_PREFIX.to_lowercase())
            .unwrap_or(&filename_lower);
        if !EXCLUDED_INI_FILENAMES.contains(&base_filename) {
            return true;
        }
    }
    false
}

pub fn find_preview_image(dir: &Path) -> Option<String> {
    let entries = fs::read_dir(dir).ok()?;
    for entry in entries.filter_map(|e| e.ok()) {
        if entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
            if let Some(filename) = entry.path().file_name().and_then(|n| n.to_str()) {
                if PREVIEW_IMAGE_NAMES.contains(&filename.to_lowercase().as_str()) {
                    return Some(filename.to_string());
                }
            }
        }
    }
    None
}

fn find_ini_path(dir: &Path) -> Option<PathBuf> {
    fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok())
        .find(|entry| {
            entry.file_type().map(|t| t.is_file()).unwrap_or(false)
                && entry
                    .path()
                    .extension()
                    .map(|ext| ext.eq_ignore_ascii_case("ini"))
                    .unwrap_or(false)
        })
        .map(|e| e.path())
}

struct IniHints {
    name: Option<String>,
    author: Option<String>,
    target: Option<String>,
    r#type: Option<String>,
}

fn parse_ini_hints(ini_content: &str) -> IniHints {
    let mut hints = IniHints { name: None, author: None, target: None, r#type: None };
    if let Ok(ini) = Ini::load_from_str(ini_content) {
        for section_name in ["Mod", "Settings", "Info", "General"] {
            if let Some(section) = ini.section(Some(section_name)) {
                if let Some(name) = section.get("Name").or_else(|| section.get("ModName")) {
                    hints.name = Some(name.trim().to_string());
                }
                if let Some(author) = section.get("Author") {
                    hints.author = Some(author.trim().to_string());
                }
                if let Some(target) = section.get("Target").or_else(|| section.get("Entity")).or_else(|| section.get("Character")) {
                    hints.target = Some(target.trim().to_string());
                }
                if let Some(typ) = section.get("Type").or_else(|| section.get("Category")) {
                    hints.r#type = Some(typ.trim().to_string());
                }
            }
        }
    }
    hints
}

#[derive(Debug, Clone)]
pub struct DeducedModInfo {
    pub agent_id: Option<i64>,
    pub category_id: Option<i64>,
    pub category_item_id: Option<i64>,
    pub name: String,
    pub author: Option<String>,
    pub image_filename: Option<String>,
}

/// Hint cascade: mod folder name -> parent folders -> INI Target/Type hint -> internal filenames.
/// Falls back to category/category_item matching if no agent matches; leaves both NULL if nothing does.
pub fn deduce_mod_info(mod_folder_path: &Path, base_mods_path: &Path, maps: &DeductionMaps) -> DeducedModInfo {
    let mod_folder_name = mod_folder_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();

    let mut info = DeducedModInfo {
        agent_id: None,
        category_id: None,
        category_item_id: None,
        name: mod_folder_name.clone(),
        author: None,
        image_filename: find_preview_image(mod_folder_path),
    };

    let mut ini_target_hint: Option<String> = None;
    let mut ini_type_hint: Option<String> = None;

    let mut found_agent_id = find_agent_match(&mod_folder_name, maps);

    if found_agent_id.is_none() {
        let mut current = mod_folder_path.parent();
        while let Some(path) = current {
            if path == base_mods_path || path.parent() == Some(base_mods_path) {
                break;
            }
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if let Some(agent_id) = find_agent_match(name, maps) {
                    found_agent_id = Some(agent_id);
                    break;
                }
            }
            current = path.parent();
        }
    }

    if let Some(ini_path) = find_ini_path(mod_folder_path) {
        if let Ok(ini_content) = fs::read_to_string(&ini_path) {
            let hints = parse_ini_hints(&ini_content);
            if let Some(name) = hints.name {
                info.name = name;
            }
            info.author = hints.author;
            ini_target_hint = hints.target;
            ini_type_hint = hints.r#type;
        }
    }

    if found_agent_id.is_none() {
        if let Some(hint) = &ini_target_hint {
            found_agent_id = find_agent_match(hint, maps);
        }
    }

    if found_agent_id.is_none() {
        if let Ok(entries) = fs::read_dir(mod_folder_path) {
            for entry in entries.filter_map(|e| e.ok()) {
                if entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
                    if let Some(stem) = entry.path().file_stem().and_then(|s| s.to_str()) {
                        if let Some(agent_id) = find_agent_match(stem, maps) {
                            found_agent_id = Some(agent_id);
                            break;
                        }
                    }
                }
            }
        }
    }

    if let Some(agent_id) = found_agent_id {
        info.agent_id = Some(agent_id);
    } else {
        let mut fallback: Option<(Option<i64>, i64)> = None;

        let mut current = mod_folder_path.parent();
        while let Some(path) = current {
            if path == base_mods_path || path.parent() == Some(base_mods_path) {
                break;
            }
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if let Some(m) = find_category_match(name, maps) {
                    fallback = Some(m);
                    break;
                }
            }
            current = path.parent();
        }

        if fallback.is_none() {
            if let Some(hint) = &ini_type_hint {
                fallback = find_category_match(hint, maps);
            }
        }

        if fallback.is_none() {
            if let Ok(relative) = mod_folder_path.strip_prefix(base_mods_path) {
                if let Some(top) = relative.components().next() {
                    if let Some(top_name) = top.as_os_str().to_str() {
                        fallback = find_category_match(top_name, maps);
                    }
                }
            }
        }

        if let Some((item_id, category_id)) = fallback {
            info.category_item_id = item_id;
            info.category_id = Some(category_id);
        }
    }

    info.name = clean_mod_name(&info.name, &mod_folder_name);
    info
}
