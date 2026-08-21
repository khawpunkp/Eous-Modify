use std::collections::{HashMap, HashSet};
use std::ffi::OsStr;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use ini::Ini;
use rusqlite::{params, Connection};
use serde::Serialize;
use sevenz_rust::Password;
use unrar::Archive as RarArchive;
use walkdir::WalkDir;
use zip::ZipArchive;

use super::deduce::{clean_mod_name, find_agent_match, find_category_match, find_preview_image, DeductionMaps};

const PREVIEW_CANDIDATES: &[&str] =
    &["preview.png", "icon.png", "thumbnail.png", "preview.jpg", "icon.jpg", "thumbnail.jpg"];

const ARCHIVE_EXTENSIONS: &[&str] = &["zip", "7z", "rar"];

/// How far an archive inside an archive is followed. Double-wrapped downloads are common — a zip
/// holding the rar the author actually built — but nothing legitimate goes deeper than a couple of
/// levels, and a cap is what stops a crafted archive from unpacking itself forever.
const MAX_NESTED_ARCHIVE_DEPTH: usize = 4;

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveEntry {
    pub path: String,
    pub is_dir: bool,
    pub is_likely_mod_root: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveAnalysis {
    pub file_path: String,
    pub entries: Vec<ArchiveEntry>,
    pub deduced_name: Option<String>,
    pub deduced_author: Option<String>,
    pub deduced_agent_id: Option<i64>,
    pub deduced_category_id: Option<i64>,
    pub deduced_category_item_id: Option<i64>,
    pub detected_preview_internal_path: Option<String>,
}

pub struct ImportRequest {
    pub agent_id: Option<i64>,
    pub category_id: Option<i64>,
    pub category_item_id: Option<i64>,
    pub selected_internal_root: Option<String>,
    pub mod_name: String,
    pub author: Option<String>,
}

// --- Reading archive entries + INI contents (three formats, same shape) ---

fn read_zip_entries(path: &Path) -> Result<(Vec<ArchiveEntry>, HashMap<String, String>), String> {
    let file = fs::File::open(path).map_err(|e| format!("Failed to open zip file: {}", e))?;
    let mut archive = ZipArchive::new(file).map_err(|e| format!("Failed to read zip archive: {}", e))?;

    let mut entries = Vec::new();
    let mut ini_contents = HashMap::new();

    for i in 0..archive.len() {
        let mut file_entry = archive
            .by_index(i)
            .map_err(|e| format!("Failed to read zip entry #{}: {}", i, e))?;
        let Some(path_buf) = file_entry.enclosed_name().map(|p| p.to_path_buf()) else {
            continue;
        };
        let path_str = path_buf.to_string_lossy().replace('\\', "/");
        let is_dir = file_entry.is_dir();

        if !is_dir && path_str.to_lowercase().ends_with(".ini") {
            let mut content = String::new();
            if file_entry.read_to_string(&mut content).is_ok() {
                ini_contents.insert(path_str.clone(), content);
            }
        }
        entries.push(ArchiveEntry { path: path_str, is_dir, is_likely_mod_root: false });
    }

    Ok((entries, ini_contents))
}

fn read_7z_entries(path: &Path) -> Result<(Vec<ArchiveEntry>, HashMap<String, String>), String> {
    let mut archive =
        sevenz_rust::SevenZReader::open(path, Password::empty()).map_err(|e| format!("Failed to open 7z archive: {}", e))?;

    let mut entries = Vec::new();
    let mut ini_contents = HashMap::new();

    archive
        .for_each_entries(|entry, reader| {
            let path_str = entry.name().replace('\\', "/");
            let is_dir = entry.is_directory();

            if !is_dir && path_str.to_lowercase().ends_with(".ini") {
                let mut content_bytes = Vec::new();
                let mut buffer = [0u8; 4096];
                loop {
                    let bytes_read = reader.read(&mut buffer)?;
                    if bytes_read == 0 {
                        break;
                    }
                    content_bytes.extend_from_slice(&buffer[..bytes_read]);
                }
                ini_contents.insert(path_str.clone(), String::from_utf8_lossy(&content_bytes).to_string());
            }
            entries.push(ArchiveEntry { path: path_str, is_dir, is_likely_mod_root: false });
            Ok(true)
        })
        .map_err(|e: sevenz_rust::Error| format!("Error iterating 7z entries: {}", e))?;

    Ok((entries, ini_contents))
}

fn read_rar_entries(path: &Path) -> Result<(Vec<ArchiveEntry>, HashMap<String, String>), String> {
    let path_str = path.to_string_lossy().to_string();
    let mut list_archive = RarArchive::new(&path_str)
        .open_for_listing()
        .map_err(|e| e.to_string())?;

    let mut entries = Vec::new();
    let mut ini_filenames = Vec::new();

    for entry_result in &mut list_archive {
        match entry_result {
            Ok(header) => {
                let entry_path_str = header.filename.to_string_lossy().replace('\\', "/");
                let is_dir = header.is_directory();
                if !is_dir && entry_path_str.to_lowercase().ends_with(".ini") {
                    ini_filenames.push((entry_path_str.clone(), header.filename.clone()));
                }
                entries.push(ArchiveEntry { path: entry_path_str, is_dir, is_likely_mod_root: false });
            }
            Err(e) => {
                eprintln!("[archive] warning: skipping RAR entry due to header read error: {}", e);
            }
        }
    }

    let mut ini_contents = HashMap::new();
    if !ini_filenames.is_empty() {
        let mut processing_archive = RarArchive::new(&path_str)
            .open_for_processing()
            .map_err(|e| e.to_string())?;
        let mut read_count = 0;
        loop {
            match processing_archive.read_header().map_err(|e| e.to_string())? {
                Some(header_state) => {
                    let current_filename = header_state.entry().filename.clone();
                    if let Some(pos) = ini_filenames.iter().position(|(_, fname)| fname == &current_filename) {
                        let (path_str, _) = ini_filenames[pos].clone();
                        match header_state.read() {
                            Ok((bytes, next_state)) => {
                                ini_contents.insert(path_str, String::from_utf8_lossy(&bytes).to_string());
                                processing_archive = next_state;
                                read_count += 1;
                                if read_count == ini_filenames.len() {
                                    break;
                                }
                            }
                            Err(e) => return Err(format!("Error reading RAR INI content: {}", e)),
                        }
                    } else {
                        processing_archive = header_state.skip().map_err(|e| e.to_string())?;
                    }
                }
                None => break,
            }
        }
    }

    Ok((entries, ini_contents))
}

fn read_entries(path: &Path) -> Result<(Vec<ArchiveEntry>, HashMap<String, String>), String> {
    match path.extension().and_then(OsStr::to_str).map(|s| s.to_lowercase()).as_deref() {
        Some("zip") => read_zip_entries(path),
        Some("7z") => read_7z_entries(path),
        Some("rar") => read_rar_entries(path),
        other => Err(format!("Unsupported archive type: {:?}", other)),
    }
}

// --- Analysis: mod-root detection, preview detection, deduction ---

pub fn analyze(file_path: &Path, maps: &DeductionMaps) -> Result<ArchiveAnalysis, String> {
    let (mut entries, ini_contents) = read_entries(file_path)?;
    entries.sort_unstable_by(|a, b| a.path.cmp(&b.path));

    // A directory is a "likely mod root" if it directly contains an .ini file.
    let mut likely_root_indices = HashSet::new();
    for ini_entry in entries.iter().filter(|e| !e.is_dir && e.path.to_lowercase().ends_with(".ini")) {
        if let Some(parent) = Path::new(&ini_entry.path).parent() {
            let parent_norm = parent.to_string_lossy().replace('\\', "/");
            if parent_norm.is_empty() {
                continue;
            }
            if let Some(index) = entries.iter().position(|e| e.is_dir && e.path.trim_end_matches('/') == parent_norm) {
                likely_root_indices.insert(index);
            }
        }
    }

    let mut root_to_preview: HashMap<usize, String> = HashMap::new();
    for &root_index in &likely_root_indices {
        let root_prefix = {
            let p = &entries[root_index].path;
            if p.ends_with('/') { p.clone() } else { format!("{}/", p) }
        };
        for candidate in PREVIEW_CANDIDATES {
            let candidate_path = format!("{}{}", root_prefix, candidate);
            if entries.iter().any(|e| !e.is_dir && e.path.eq_ignore_ascii_case(&candidate_path)) {
                root_to_preview.insert(root_index, candidate_path);
                break;
            }
        }
    }

    for &index in &likely_root_indices {
        entries[index].is_likely_mod_root = true;
    }

    let mut deduced_name: Option<String> = None;
    let mut deduced_author: Option<String> = None;
    let mut deduced_agent_id: Option<i64> = None;
    let mut deduced_category_match: Option<(Option<i64>, i64)> = None;
    let mut detected_preview_internal_path: Option<String> = None;
    let mut ini_target_hint: Option<String> = None;
    let mut ini_type_hint: Option<String> = None;

    // Process the first likely root's INI (by path order) for metadata + hints.
    if let Some(&first_root_index) = likely_root_indices.iter().min_by_key(|&&i| &entries[i].path) {
        let root_prefix = {
            let p = &entries[first_root_index].path;
            if p.ends_with('/') { p.clone() } else { format!("{}/", p) }
        };

        if let Some((_, ini_content)) = ini_contents
            .iter()
            .find(|(p, _)| p.starts_with(&root_prefix) && !p.trim_start_matches(&root_prefix).contains('/'))
        {
            if let Ok(ini) = Ini::load_from_str(ini_content) {
                for section_name in ["Mod", "Settings", "Info", "General"] {
                    if let Some(section) = ini.section(Some(section_name)) {
                        if let Some(name) = section.get("Name").or_else(|| section.get("ModName")) {
                            let cleaned = clean_mod_name(name, "");
                            if !cleaned.is_empty() {
                                deduced_name = Some(cleaned);
                            }
                        }
                        if let Some(author) = section.get("Author") {
                            deduced_author = Some(author.trim().to_string());
                        }
                        if let Some(target) =
                            section.get("Target").or_else(|| section.get("Entity")).or_else(|| section.get("Character"))
                        {
                            ini_target_hint = Some(target.trim().to_string());
                        }
                        if let Some(typ) = section.get("Type").or_else(|| section.get("Category")) {
                            ini_type_hint = Some(typ.trim().to_string());
                        }
                    }
                }
            }
        }

        if let Some(hint) = &ini_target_hint {
            deduced_agent_id = find_agent_match(hint, maps);
        }
        if deduced_agent_id.is_none() {
            if let Some(hint) = &ini_type_hint {
                deduced_category_match = find_category_match(hint, maps);
            }
        }

        detected_preview_internal_path = root_to_preview.get(&first_root_index).cloned();
    }

    // Internal filenames, if still no agent match.
    if deduced_agent_id.is_none() {
        for entry in entries.iter().filter(|e| !e.is_dir) {
            let filename = entry.path.rsplit('/').next().unwrap_or(&entry.path);
            if let Some(stem) = Path::new(filename).file_stem().and_then(OsStr::to_str) {
                if let Some(agent_id) = find_agent_match(stem, maps) {
                    deduced_agent_id = Some(agent_id);
                    break;
                }
            }
        }
    }

    // Archive filename itself, lowest priority.
    if deduced_agent_id.is_none() && deduced_category_match.is_none() {
        if let Some(stem) = file_path.file_stem().and_then(OsStr::to_str) {
            deduced_agent_id = find_agent_match(stem, maps);
            if deduced_agent_id.is_none() {
                deduced_category_match = find_category_match(stem, maps);
            }
        }
    }

    if deduced_name.is_none() {
        deduced_name = file_path.file_stem().and_then(OsStr::to_str).map(|s| clean_mod_name(s, s));
    }

    let (deduced_category_item_id, deduced_category_id) = match deduced_category_match {
        Some((item_id, category_id)) => (item_id, Some(category_id)),
        None => (None, None),
    };

    Ok(ArchiveAnalysis {
        file_path: file_path.to_string_lossy().to_string(),
        entries,
        deduced_name,
        deduced_author,
        deduced_agent_id,
        deduced_category_id,
        deduced_category_item_id,
        detected_preview_internal_path,
    })
}

// --- Import: extraction + DB insert ---

/// Where a mod with this agent/category/category-item assignment lives on disk, relative to the
/// mods folder root. Shared by archive import and by `mods::update_mod_category`'s on-disk move,
/// so the two never disagree about a mod's expected location.
/// Everything belonging to a named agent lives under this one folder, so the top of the mods
/// directory holds a handful of category folders rather than one per character.
pub(crate) const AGENTS_SUBDIR: &str = "agents";

/// Where a mod with no agent and no category goes. A real folder rather than a marker: it is a
/// destination the user can also pick by hand, and it is what the Other/Misc page lists.
pub(crate) const MISC_SUBDIR: &str = "misc";

pub(crate) fn resolve_category_subpath(
    conn: &Connection,
    agent_id: Option<i64>,
    category_id: Option<i64>,
    category_item_id: Option<i64>,
) -> Result<PathBuf, String> {
    if let Some(agent_id) = agent_id {
        let slug: String = conn
            .query_row("SELECT slug FROM agents WHERE id = ?1", params![agent_id], |row| row.get(0))
            .map_err(|e| e.to_string())?;
        return Ok(PathBuf::from(AGENTS_SUBDIR).join(slug));
    }

    if let Some(item_id) = category_item_id {
        let (category_slug, item_slug): (String, String) = conn
            .query_row(
                "SELECT c.slug, ci.slug FROM category_items ci JOIN categories c ON ci.category_id = c.id WHERE ci.id = ?1",
                params![item_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(|e| e.to_string())?;
        return Ok(PathBuf::from(category_slug).join(item_slug));
    }

    if let Some(category_id) = category_id {
        let slug: String = conn
            .query_row("SELECT slug FROM categories WHERE id = ?1", params![category_id], |row| row.get(0))
            .map_err(|e| e.to_string())?;
        return Ok(PathBuf::from(slug));
    }

    Ok(PathBuf::from(MISC_SUBDIR))
}

pub fn import(
    conn: &mut Connection,
    archive_path: &Path,
    base_mods_path: &Path,
    request: ImportRequest,
) -> Result<i64, String> {
    if request.mod_name.trim().is_empty() {
        return Err("Mod name cannot be empty.".to_string());
    }
    if !archive_path.is_file() {
        return Err(format!("Archive file not found: {}", archive_path.display()));
    }

    let resolved_item_id =
        crate::mods::resolve_category_item(conn, request.category_id, request.category_item_id)?;
    let dest_subpath =
        resolve_category_subpath(conn, request.agent_id, request.category_id, resolved_item_id)?;
    let target_folder_name = request.mod_name.trim().replace([' ', '.', '\'', '"'], "_");
    if target_folder_name.is_empty() {
        return Err("Mod name results in an invalid folder name.".to_string());
    }
    let final_dest_path = base_mods_path.join(&dest_subpath).join(&target_folder_name);

    fs::create_dir_all(&final_dest_path)
        .map_err(|e| format!("Failed to create destination folder '{}': {}", final_dest_path.display(), e))?;

    let prefix_to_extract = request.selected_internal_root.as_deref().unwrap_or("").replace('\\', "/");
    let prefix_to_extract = prefix_to_extract.trim_end_matches('/').to_string();
    let extract_all = prefix_to_extract.is_empty();
    let prefix_path = Path::new(&prefix_to_extract);

    let extraction_result = extract_archive(archive_path, &final_dest_path, prefix_path, extract_all);

    let files_extracted = match extraction_result {
        Ok(count) => count,
        Err(e) => {
            fs::remove_dir_all(&final_dest_path).ok();
            return Err(e);
        }
    };
    let files_extracted = files_extracted + extract_nested_archives(&final_dest_path);
    println!("[import] Extracted {} files to '{}'.", files_extracted, final_dest_path.display());

    let image_filename = find_preview_image(&final_dest_path);
    let relative_path_str = dest_subpath.join(&target_folder_name).to_string_lossy().replace('\\', "/");

    let existing: Option<i64> = conn
        .query_row("SELECT id FROM mods WHERE folder_name = ?1", params![relative_path_str], |row| row.get(0))
        .ok();
    if existing.is_some() {
        fs::remove_dir_all(&final_dest_path).ok();
        return Err(format!("A mod already exists at '{}'.", relative_path_str));
    }

    let insert_result = conn.execute(
        "INSERT INTO mods (agent_id, category_id, category_item_id, name, folder_name, image_filename, author)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            request.agent_id,
            request.category_id,
            resolved_item_id,
            request.mod_name.trim(),
            relative_path_str,
            image_filename,
            request.author,
        ],
    );

    match insert_result {
        Ok(_) => Ok(conn.last_insert_rowid()),
        Err(e) => {
            fs::remove_dir_all(&final_dest_path).ok();
            Err(format!("Failed to add imported mod to the database: {}", e))
        }
    }
}

/// The one subdirectory `dir` holds, when that is all it holds.
fn sole_subdirectory(dir: &Path) -> Option<PathBuf> {
    let mut entries = fs::read_dir(dir).ok()?.filter_map(|entry| entry.ok());
    let only = entries.next()?;
    if entries.next().is_some() {
        return None;
    }
    only.file_type().ok()?.is_dir().then(|| only.path())
}

/// Replaces a mod's files from a file the user picked, and reports how many were written.
///
/// What happens depends on what was picked, which is what lets one button cover both jobs without
/// asking first. An archive is a new version of the mod: its contents replace everything in the mod
/// folder. Any other file is a patch, copied in over a file of the same name, and nothing else in
/// the folder is touched.
///
/// The row in the database is deliberately left as it stands. Name, author, category, group
/// membership and the enabled state describe the mod rather than the files inside it, and a new
/// version of a mod is still that mod. `image_filename` is the exception, since the new files decide
/// what preview there is to point at.
pub fn update_files(
    conn: &Connection,
    base_mods_path: &Path,
    mod_id: i64,
    source_path: &Path,
) -> Result<usize, String> {
    if !source_path.is_file() {
        return Err(format!("File not found: {}", source_path.display()));
    }
    let folder_name: String = conn
        .query_row("SELECT folder_name FROM mods WHERE id = ?1", params![mod_id], |row| row.get(0))
        .map_err(|e| e.to_string())?;
    let mod_dir = crate::mods::current_mod_path(base_mods_path, &folder_name)
        .ok_or_else(|| "Mod folder not found on disk.".to_string())?;

    let written = if is_archive(source_path) {
        replace_with_archive(&mod_dir, source_path)?
    } else {
        let file_name = source_path
            .file_name()
            .ok_or_else(|| "That file has no name to copy it under.".to_string())?;
        fs::copy(source_path, mod_dir.join(file_name))
            .map_err(|e| format!("Failed to copy the file into the mod folder: {}", e))?;
        1
    };

    let image_filename = crate::mods::saved_preview_filename(&mod_dir).or_else(|| find_preview_image(&mod_dir));
    conn.execute("UPDATE mods SET image_filename = ?1 WHERE id = ?2", params![image_filename, mod_id])
        .map_err(|e| e.to_string())?;

    println!("[update] Wrote {} files into '{}'.", written, mod_dir.display());
    Ok(written)
}

/// Swaps a mod's folder contents for an archive's, leaving the folder itself — and with it the
/// folder name, the DISABLED_ prefix and every path recorded in the database — exactly as it was.
///
/// The new files are unpacked into a staging folder beside the mod first, so an archive that turns
/// out to be corrupt or password-protected fails before anything has been lost. Beside the mod
/// rather than in the system temp folder because the swap itself is two renames, and a rename cannot
/// cross volumes: a mods folder on a second drive would otherwise force a full copy.
///
/// The old folder is held under a temporary name until the new one is in place, which is what makes
/// a failed swap recoverable rather than a mod that is simply gone. A preview this app saved is
/// carried across from it, since the user chose that picture for the mod rather than for the version
/// — but only when the new files brought nothing of that name themselves.
fn replace_with_archive(mod_dir: &Path, archive_path: &Path) -> Result<usize, String> {
    let folder_name = mod_dir.file_name().unwrap_or_default().to_string_lossy().to_string();
    let staging = vacant_dir(&mod_dir.with_file_name(format!(".eous-incoming-{}", folder_name)));
    fs::create_dir_all(&staging).map_err(|e| format!("Failed to make room for the new files: {}", e))?;

    let staged = extract_archive(archive_path, &staging, Path::new(""), true)
        .map(|count| count + extract_nested_archives(&staging));
    let staged = match staged {
        Ok(0) => {
            fs::remove_dir_all(&staging).ok();
            return Err("That archive holds no files.".to_string());
        }
        Ok(count) => count,
        Err(e) => {
            fs::remove_dir_all(&staging).ok();
            return Err(e);
        }
    };

    // A download is normally one folder wrapping the mod, sometimes two. Moving that wrapper in as it
    // stands would leave the new version a level below where the old one sat, so the chain of
    // single-child folders is followed down to the one that actually holds the files.
    let mut incoming = staging.clone();
    while let Some(only_child) = sole_subdirectory(&incoming) {
        incoming = only_child;
    }

    let previous = vacant_dir(&mod_dir.with_file_name(format!(".eous-previous-{}", folder_name)));
    if let Err(e) = fs::rename(mod_dir, &previous) {
        fs::remove_dir_all(&staging).ok();
        return Err(format!("Failed to set the mod's current files aside: {}", e));
    }
    if let Err(e) = fs::rename(&incoming, mod_dir) {
        // Nothing has been deleted yet, so the mod goes back exactly as it was.
        fs::rename(&previous, mod_dir).ok();
        fs::remove_dir_all(&staging).ok();
        return Err(format!("Failed to move the new files into place: {}", e));
    }

    if let Some(preview) = crate::mods::saved_preview_filename(&previous) {
        let destination = mod_dir.join(&preview);
        if !destination.exists() {
            fs::copy(previous.join(&preview), destination).ok();
        }
    }

    fs::remove_dir_all(&staging).ok();
    fs::remove_dir_all(&previous).ok();
    Ok(staged)
}

fn extract_archive(archive_path: &Path, dest: &Path, prefix_path: &Path, extract_all: bool) -> Result<usize, String> {
    match archive_path.extension().and_then(OsStr::to_str).map(|s| s.to_lowercase()).as_deref() {
        Some("zip") => extract_zip(archive_path, dest, prefix_path, extract_all),
        Some("7z") => extract_7z(archive_path, dest, prefix_path, extract_all),
        Some("rar") => extract_rar(archive_path, dest, prefix_path, extract_all),
        other => Err(format!("Unsupported archive type for extraction: {:?}", other)),
    }
}

fn is_archive(path: &Path) -> bool {
    path.extension()
        .and_then(OsStr::to_str)
        .map(|ext| ext.to_lowercase())
        .is_some_and(|ext| ARCHIVE_EXTENSIONS.contains(&ext.as_str()))
}

/// The first path of this name that nothing occupies yet: `skin01`, then `skin01 (2)`, and so on.
fn vacant_dir(preferred: &Path) -> PathBuf {
    if !preferred.exists() {
        return preferred.to_path_buf();
    }
    let name = preferred.file_name().unwrap_or_default().to_string_lossy().to_string();
    (2..)
        .map(|n| preferred.with_file_name(format!("{} ({})", name, n)))
        .find(|candidate| !candidate.exists())
        .unwrap_or_else(|| preferred.to_path_buf())
}

/// Unpacks every archive the extraction left behind, then everything those produce, and deletes each
/// one once it is open. Returns how many files this added.
///
/// A mod that arrives as an archive inside an archive used to import as a single unopened file: the
/// mod folder held a .zip and nothing the game could read.
///
/// An archive that is the only one in its folder unpacks into that folder rather than into a
/// subfolder named after itself. That keeps the mod's own preview.png at the top of the mod folder,
/// which is the only level `find_preview_image` looks at. Where a folder holds several archives — a
/// pack of variants — each gets its own subfolder, since flattening them into one place would have
/// them overwrite each other.
///
/// A failure is not fatal. The archive is left packed and the import keeps everything else, because
/// one password-protected extra is no reason to lose the mod it came with. It is also recorded, so a
/// later pass does not keep retrying it.
fn extract_nested_archives(dest: &Path) -> usize {
    let mut extracted = 0;
    let mut failed: HashSet<PathBuf> = HashSet::new();

    for _ in 0..MAX_NESTED_ARCHIVE_DEPTH {
        let archives: Vec<PathBuf> = WalkDir::new(dest)
            .into_iter()
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_type().is_file() && is_archive(entry.path()))
            .map(|entry| entry.path().to_path_buf())
            .filter(|path| !failed.contains(path))
            .collect();
        if archives.is_empty() {
            break;
        }

        for archive in &archives {
            let Some(parent) = archive.parent() else { continue };
            let alone_in_its_folder = archives.iter().filter(|other| other.parent() == Some(parent)).count() == 1;

            let target = if alone_in_its_folder {
                parent.to_path_buf()
            } else {
                vacant_dir(&parent.join(archive.file_stem().unwrap_or_default()))
            };
            // Only a folder this call brought into being may be cleaned up after a failure. When the
            // target is the mod folder itself, deleting it would take the rest of the import with it.
            let target_is_new = !target.exists();

            if let Err(e) = fs::create_dir_all(&target) {
                eprintln!("[import] No place to unpack '{}': {}", archive.display(), e);
                failed.insert(archive.clone());
                continue;
            }

            match extract_archive(archive, &target, Path::new(""), true) {
                Ok(count) => {
                    extracted += count;
                    if let Err(e) = fs::remove_file(archive) {
                        eprintln!("[import] Unpacked '{}' but could not remove it: {}", archive.display(), e);
                    }
                }
                Err(e) => {
                    eprintln!("[import] Leaving '{}' packed: {}", archive.display(), e);
                    if target_is_new {
                        fs::remove_dir_all(&target).ok();
                    }
                    failed.insert(archive.clone());
                }
            }
        }
    }

    extracted
}

fn extract_zip(archive_path: &Path, dest: &Path, prefix_path: &Path, extract_all: bool) -> Result<usize, String> {
    let file = fs::File::open(archive_path).map_err(|e| format!("Failed to open zip: {}", e))?;
    let mut archive = ZipArchive::new(file).map_err(|e| format!("Failed to read zip: {}", e))?;
    let mut count = 0;

    for i in 0..archive.len() {
        let mut file_entry = archive
            .by_index(i)
            .map_err(|e| format!("Failed to read zip entry #{}: {}", i, e))?;
        let Some(internal_path) = file_entry.enclosed_name().map(|p| p.to_path_buf()) else {
            continue;
        };

        let relative = if extract_all {
            Some(internal_path.clone())
        } else if internal_path.starts_with(prefix_path) {
            internal_path.strip_prefix(prefix_path).ok().map(|p| p.to_path_buf())
        } else {
            None
        };
        let Some(relative) = relative else { continue };
        if relative.as_os_str().is_empty() {
            continue;
        }
        let outpath = dest.join(&relative);

        if file_entry.is_dir() {
            fs::create_dir_all(&outpath).map_err(|e| format!("Failed to create dir '{}': {}", outpath.display(), e))?;
        } else {
            if let Some(parent) = outpath.parent() {
                if !parent.exists() {
                    fs::create_dir_all(parent).map_err(|e| format!("Failed to create parent '{}': {}", parent.display(), e))?;
                }
            }
            let mut outfile =
                fs::File::create(&outpath).map_err(|e| format!("Failed to create file '{}': {}", outpath.display(), e))?;
            std::io::copy(&mut file_entry, &mut outfile)
                .map_err(|e| format!("Failed to copy content to '{}': {}", outpath.display(), e))?;
            count += 1;
        }
    }

    Ok(count)
}

fn extract_7z(archive_path: &Path, dest: &Path, prefix_path: &Path, extract_all: bool) -> Result<usize, String> {
    let mut count = 0;
    let mut archive =
        sevenz_rust::SevenZReader::open(archive_path, Password::empty()).map_err(|e| format!("Failed to open 7z: {}", e))?;

    archive
        .for_each_entries(|entry, reader| {
            let internal_path = PathBuf::from(entry.name().replace('\\', "/"));

            let relative = if extract_all {
                Some(internal_path.clone())
            } else if internal_path.starts_with(prefix_path) {
                internal_path.strip_prefix(prefix_path).ok().map(|p| p.to_path_buf())
            } else {
                None
            };
            let Some(relative) = relative else { return Ok(true) };
            if relative.as_os_str().is_empty() {
                return Ok(true);
            }
            let outpath = dest.join(&relative);

            if entry.is_directory() {
                fs::create_dir_all(&outpath)?;
            } else {
                if let Some(parent) = outpath.parent() {
                    if !parent.exists() {
                        fs::create_dir_all(parent)?;
                    }
                }
                let mut outfile = fs::File::create(&outpath)?;
                let mut buffer = [0u8; 4096];
                loop {
                    let bytes_read = reader.read(&mut buffer)?;
                    if bytes_read == 0 {
                        break;
                    }
                    outfile.write_all(&buffer[..bytes_read])?;
                }
                count += 1;
            }
            Ok(true)
        })
        .map_err(|e: sevenz_rust::Error| format!("Error extracting 7z entries: {}", e))?;

    Ok(count)
}

fn extract_rar(archive_path: &Path, dest: &Path, prefix_path: &Path, extract_all: bool) -> Result<usize, String> {
    let path_str = archive_path.to_string_lossy().to_string();
    let mut archive = RarArchive::new(&path_str)
        .open_for_processing()
        .map_err(|e| e.to_string())?;
    let mut count = 0;

    loop {
        match archive.read_header().map_err(|e| e.to_string())? {
            Some(header_state) => {
                let internal_path = PathBuf::from(header_state.entry().filename.to_string_lossy().replace('\\', "/"));
                let is_dir = header_state.entry().is_directory();

                let relative = if extract_all {
                    Some(internal_path.clone())
                } else if internal_path.starts_with(prefix_path) {
                    internal_path.strip_prefix(prefix_path).ok().map(|p| p.to_path_buf())
                } else {
                    None
                };

                let Some(relative) = relative else {
                    archive = header_state.skip().map_err(|e| e.to_string())?;
                    continue;
                };
                if relative.as_os_str().is_empty() {
                    archive = header_state.skip().map_err(|e| e.to_string())?;
                    continue;
                }
                let outpath = dest.join(&relative);

                if is_dir {
                    fs::create_dir_all(&outpath).map_err(|e| format!("Failed to create dir '{}': {}", outpath.display(), e))?;
                    archive = header_state.skip().map_err(|e| e.to_string())?;
                } else {
                    if let Some(parent) = outpath.parent() {
                        if !parent.exists() {
                            fs::create_dir_all(parent)
                                .map_err(|e| format!("Failed to create parent '{}': {}", parent.display(), e))?;
                        }
                    }
                    archive = header_state.extract_to(&outpath).map_err(|e| e.to_string())?;
                    count += 1;
                }
            }
            None => break,
        }
    }

    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fresh, empty directory of its own, so no two tests tread on each other.
    fn temp_dir() -> PathBuf {
        use std::sync::atomic::{AtomicU32, Ordering};
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let unique = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir =
            std::env::temp_dir().join(format!("eous_modify_archive_test_{}_{}", std::process::id(), unique));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Writes a zip at `path` holding each `(internal path, contents)` pair.
    fn write_zip(path: &Path, files: &[(&str, &[u8])]) {
        let mut writer = zip::ZipWriter::new(fs::File::create(path).unwrap());
        for (name, contents) in files {
            writer.start_file(*name, zip::write::FileOptions::default()).unwrap();
            writer.write_all(contents).unwrap();
        }
        writer.finish().unwrap();
    }

    #[test]
    fn an_archive_alone_in_its_folder_unpacks_into_that_folder() {
        // preview.png has to land at the top of the mod folder: find_preview_image reads one level
        // only, so unpacking into a subfolder named after the archive loses the mod's picture.
        let dir = temp_dir();
        write_zip(&dir.join("inner.zip"), &[("preview.png", b"png"), ("skin/skin.ini", b"; Constants")]);

        assert_eq!(extract_nested_archives(&dir), 2);
        assert!(dir.join("preview.png").is_file());
        assert!(dir.join("skin/skin.ini").is_file());
        assert!(!dir.join("inner.zip").exists(), "the container should be gone once it is open");
    }

    #[test]
    fn several_archives_in_one_folder_each_get_their_own() {
        // Flattening a pack of variants into one folder would have them overwrite each other.
        let dir = temp_dir();
        write_zip(&dir.join("red.zip"), &[("body.ini", b"red")]);
        write_zip(&dir.join("blue.zip"), &[("body.ini", b"blue")]);

        assert_eq!(extract_nested_archives(&dir), 2);
        assert_eq!(fs::read_to_string(dir.join("red/body.ini")).unwrap(), "red");
        assert_eq!(fs::read_to_string(dir.join("blue/body.ini")).unwrap(), "blue");
    }

    #[test]
    fn an_archive_inside_an_archive_is_followed() {
        let dir = temp_dir();
        let staging = temp_dir();
        write_zip(&staging.join("inner.zip"), &[("deep.ini", b"; Constants")]);
        let inner = fs::read(staging.join("inner.zip")).unwrap();
        write_zip(&dir.join("outer.zip"), &[("inner.zip", inner.as_slice())]);

        assert_eq!(extract_nested_archives(&dir), 2);
        assert!(dir.join("deep.ini").is_file(), "the innermost file should have surfaced");
        assert!(!dir.join("outer.zip").exists());
        assert!(!dir.join("inner.zip").exists());
    }

    #[test]
    fn a_folder_with_no_archives_is_left_alone() {
        let dir = temp_dir();
        fs::write(dir.join("body.ini"), "; Constants").unwrap();

        assert_eq!(extract_nested_archives(&dir), 0);
        assert!(dir.join("body.ini").is_file());
    }

    /// A mod folder at `<base>/TestMod` holding the given `(name, contents)` files.
    fn build_mod_folder(files: &[(&str, &[u8])]) -> (PathBuf, String) {
        let base = temp_dir();
        let folder_name = "TestMod".to_string();
        fs::create_dir_all(base.join(&folder_name)).unwrap();
        for (name, contents) in files {
            let path = base.join(&folder_name).join(name);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, contents).unwrap();
        }
        (base, folder_name)
    }

    /// A mod row and an in-memory database to hold it, so update_files has something to read.
    fn db_with_mod(folder_name: &str, image_filename: Option<&str>) -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(crate::db::schema::SCHEMA).unwrap();
        conn.execute(
            "INSERT INTO mods (id, name, folder_name, image_filename) VALUES (1, 'Test', ?1, ?2)",
            params![folder_name, image_filename],
        )
        .unwrap();
        conn
    }

    #[test]
    fn an_update_swaps_the_contents_and_keeps_the_folder() {
        let (base, folder) = build_mod_folder(&[("old.ini", b"; old")]);
        let mod_dir = base.join(&folder);
        let source = temp_dir().join("v2.zip");
        write_zip(&source, &[("new.ini", b"; new")]);

        assert_eq!(replace_with_archive(&mod_dir, &source).unwrap(), 1);
        assert!(mod_dir.is_dir(), "every path in the database points at this folder, so it has to survive");
        assert!(mod_dir.join("new.ini").is_file());
        assert!(!mod_dir.join("old.ini").exists());
    }

    #[test]
    fn an_update_steps_past_the_folder_a_download_wraps_itself_in() {
        let (base, folder) = build_mod_folder(&[("old.ini", b"; old")]);
        let mod_dir = base.join(&folder);
        let source = temp_dir().join("v2.zip");
        write_zip(
            &source,
            &[("Astra Shining Eridu/body.ini", b"; new"), ("Astra Shining Eridu/preview.png", b"png")],
        );

        replace_with_archive(&mod_dir, &source).unwrap();
        assert!(mod_dir.join("body.ini").is_file(), "the new version belongs where the old one sat");
        assert!(mod_dir.join("preview.png").is_file());
    }

    #[test]
    fn an_update_carries_the_preview_the_user_chose_across() {
        let (base, folder) = build_mod_folder(&[("old.ini", b"; old"), ("mod_preview.png", b"chosen")]);
        let mod_dir = base.join(&folder);
        let source = temp_dir().join("v2.zip");
        write_zip(&source, &[("body.ini", b"; new")]);

        replace_with_archive(&mod_dir, &source).unwrap();
        assert_eq!(fs::read_to_string(mod_dir.join("mod_preview.png")).unwrap(), "chosen");
    }

    #[test]
    fn an_archive_that_cannot_be_read_leaves_the_mod_as_it_was() {
        let (base, folder) = build_mod_folder(&[("old.ini", b"; old")]);
        let mod_dir = base.join(&folder);
        let source = temp_dir().join("truncated.zip");
        fs::write(&source, b"not really a zip").unwrap();

        assert!(replace_with_archive(&mod_dir, &source).is_err());
        assert!(mod_dir.join("old.ini").is_file(), "nothing should be lost to an archive that will not open");
        assert_eq!(fs::read_dir(&base).unwrap().count(), 1, "and no staging folder should be left behind");
    }

    #[test]
    fn an_archive_holding_no_files_leaves_the_mod_as_it_was() {
        let (base, folder) = build_mod_folder(&[("old.ini", b"; old")]);
        let mod_dir = base.join(&folder);
        let source = temp_dir().join("empty.zip");
        write_zip(&source, &[]);

        assert!(replace_with_archive(&mod_dir, &source).is_err());
        assert!(mod_dir.join("old.ini").is_file());
        assert_eq!(fs::read_dir(&base).unwrap().count(), 1);
    }

    #[test]
    fn a_loose_file_goes_in_without_touching_anything_else() {
        let (base, folder) = build_mod_folder(&[("body.ini", b"; original"), ("texture.dds", b"keep")]);
        let conn = db_with_mod(&folder, None);
        let patch = temp_dir().join("body.ini");
        fs::write(&patch, b"; fixed").unwrap();

        assert_eq!(update_files(&conn, &base, 1, &patch).unwrap(), 1);
        assert_eq!(fs::read_to_string(base.join(&folder).join("body.ini")).unwrap(), "; fixed");
        assert_eq!(fs::read_to_string(base.join(&folder).join("texture.dds")).unwrap(), "keep");
    }

    #[test]
    fn an_update_points_the_mod_at_the_preview_the_new_files_ship() {
        let (base, folder) = build_mod_folder(&[("old.ini", b"; old")]);
        let conn = db_with_mod(&folder, Some("gone.png"));
        let source = temp_dir().join("v2.zip");
        write_zip(&source, &[("body.ini", b"; new"), ("preview.png", b"png")]);

        update_files(&conn, &base, 1, &source).unwrap();

        let image: Option<String> =
            conn.query_row("SELECT image_filename FROM mods WHERE id = 1", [], |row| row.get(0)).unwrap();
        assert_eq!(image.as_deref(), Some("preview.png"));
    }
}
