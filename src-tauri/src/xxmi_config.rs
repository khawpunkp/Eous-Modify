//! Manages the one `d3dx.ini` setting that decides whether 3DMigoto reacts to hotkeys while its
//! game window is in the background.
//!
//! Toggling a mod only renames its folder; the game keeps rendering what it already loaded until
//! 3DMigoto re-reads the Mods folder, which it does on F10 (`reload_config = no_modifiers VK_F10`).
//! Synthesizing that keypress is easy, but 3DMigoto ships with `check_foreground_window = 1`, and
//! that makes it discard every hotkey unless its own window is focused — so an F10 sent while *our*
//! window is in front is thrown away. Setting it to `0` is what makes auto-reload work at all;
//! FlairX-Mod-Manager solves it the same way.
//!
//! This edits a file we don't own, so it is opt-in, it records the value it replaced, and it only
//! ever reverts a line it can prove it wrote — see [`MARKER`].

use std::path::{Path, PathBuf};

/// The `[System]` key that gates background hotkeys.
const KEY: &str = "check_foreground_window";

/// Stamped onto the line we write, carrying the state we replaced so [`rewrite`] can put it back
/// verbatim. A `check_foreground_window` line *without* this marker is the user's own deliberate
/// setting: we never revert it, because we have no idea what they wanted it to be.
const MARKER: &str = "; managed by Eous Modify, was ";

/// Marker payload used when the key wasn't in the file at all, so turning the feature off removes
/// the line we added instead of inventing a value for it.
const WAS_ABSENT: &str = "absent";

/// `d3dx.ini` sits next to the Mods folder, not inside it — that's where both XXMI and plain
/// 3DMigoto put it.
pub fn d3dx_ini_path(mods_folder: &Path) -> Option<PathBuf> {
    Some(mods_folder.parent()?.join("d3dx.ini"))
}

/// True for an active (non-commented) `check_foreground_window` line.
fn is_key_line(trimmed: &str) -> bool {
    !trimmed.starts_with(';')
        && trimmed.len() >= KEY.len()
        && trimmed[..KEY.len()].eq_ignore_ascii_case(KEY)
}

/// The state we recorded when we wrote this line, or `None` if we didn't write it.
fn marked_previous(line: &str) -> Option<&str> {
    let at = line.find(MARKER)?;
    Some(line[at + MARKER.len()..].trim())
}

/// The value currently assigned, e.g. `1` from `check_foreground_window = 1 ; comment`.
fn current_value(line: &str) -> &str {
    let after_eq = line.split_once('=').map_or("", |(_, v)| v);
    // Drop any trailing comment before reading the value.
    after_eq.split(';').next().unwrap_or("").trim()
}

fn managed_line(previous: &str) -> String {
    format!("{KEY} = 0   {MARKER}{previous}")
}

/// Rewrites `d3dx.ini` contents to enable or disable background hotkeys.
///
/// Kept as a pure string transform so the decision table below is testable without touching a real
/// XXMI install. Line endings and trailing-newline style are preserved: this file belongs to the
/// user, and rewriting every line ending would turn a one-line change into a whole-file diff.
pub fn rewrite(contents: &str, enable: bool) -> String {
    let newline = if contents.contains("\r\n") { "\r\n" } else { "\n" };

    let mut out: Vec<String> = Vec::new();
    let mut in_system = false;
    let mut saw_system = false;
    let mut handled = false;

    for line in contents.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            // Leaving [System] without having found the key: add it here, while we're still in the
            // section it belongs to.
            if in_system && !handled && enable {
                out.push(managed_line(WAS_ABSENT));
                handled = true;
            }
            in_system = trimmed.eq_ignore_ascii_case("[System]");
            saw_system |= in_system;
            out.push(line.to_string());
            continue;
        }

        if in_system && !handled && is_key_line(trimmed) {
            handled = true;
            match (enable, marked_previous(line)) {
                // Already ours: keep the recorded previous state, don't stack markers.
                (true, Some(previous)) => out.push(managed_line(previous)),
                // The user already runs with background hotkeys on. Nothing to change, and nothing
                // for us to claim ownership of.
                (true, None) if current_value(trimmed) == "0" => out.push(line.to_string()),
                (true, None) => out.push(managed_line(current_value(trimmed))),
                // Ours, and the key was absent before we added it — take the line back out.
                (false, Some(previous)) if previous == WAS_ABSENT => {}
                (false, Some(previous)) => out.push(format!("{KEY} = {previous}")),
                // Not ours; leave the user's setting exactly as it is.
                (false, None) => out.push(line.to_string()),
            }
            continue;
        }

        out.push(line.to_string());
    }

    if enable && !handled {
        if in_system {
            // File ended while still inside [System].
            out.push(managed_line(WAS_ABSENT));
        } else if !saw_system {
            out.push(String::new());
            out.push("[System]".to_string());
            out.push(managed_line(WAS_ABSENT));
        }
    }

    let mut result = out.join(newline);
    if contents.ends_with('\n') {
        result.push_str(newline);
    }
    result
}

/// Applies [`rewrite`] to the `d3dx.ini` beside `mods_folder`.
///
/// Errors carry the resolved path, because "not found" almost always means the Mods Folder setting
/// points somewhere other than a real `*MI` install rather than that anything is broken.
pub fn apply(mods_folder: &Path, enable: bool) -> Result<(), String> {
    let path = d3dx_ini_path(mods_folder)
        .ok_or_else(|| "Could not work out where d3dx.ini lives from the Mods folder path.".to_string())?;

    if !path.exists() {
        return Err(format!(
            "No d3dx.ini at {}. It should sit next to your Mods folder — check the Mods Folder path in Settings.",
            path.display()
        ));
    }

    let contents = std::fs::read_to_string(&path)
        .map_err(|e| format!("Could not read {}: {e}", path.display()))?;

    let updated = rewrite(&contents, enable);
    if updated == contents {
        return Ok(());
    }

    std::fs::write(&path, updated).map_err(|e| format!("Could not write {}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHIPPED: &str = "[Include]\nfoo = bar\n\n[System]\nload_library_redirect = 2\n\n; Only enable key input processing when the game is in the foreground:\ncheck_foreground_window = 1\n";

    #[test]
    fn d3dx_ini_sits_beside_the_mods_folder() {
        let path = d3dx_ini_path(Path::new(r"C:\XXMI\ZZMI\Mods")).unwrap();
        assert_eq!(path, PathBuf::from(r"C:\XXMI\ZZMI\d3dx.ini"));
    }

    #[test]
    fn enabling_zeroes_the_key_and_records_the_old_value() {
        let out = rewrite(SHIPPED, true);
        assert!(out.contains("check_foreground_window = 0   ; managed by Eous Modify, was 1"));
        assert!(!out.contains("check_foreground_window = 1"));
        // Untouched lines survive verbatim.
        assert!(out.contains("load_library_redirect = 2"));
        assert!(out.contains("[Include]"));
    }

    #[test]
    fn enabling_twice_does_not_stack_markers() {
        let once = rewrite(SHIPPED, true);
        let twice = rewrite(&once, true);
        assert_eq!(once, twice);
    }

    #[test]
    fn disabling_restores_the_recorded_value() {
        let enabled = rewrite(SHIPPED, true);
        assert_eq!(rewrite(&enabled, false), SHIPPED);
    }

    #[test]
    fn enabling_leaves_a_users_own_zero_alone() {
        let already = SHIPPED.replace("check_foreground_window = 1", "check_foreground_window = 0");
        assert_eq!(rewrite(&already, true), already);
    }

    #[test]
    fn disabling_never_touches_a_line_we_did_not_write() {
        // No marker: this is the user's setting, whatever it says.
        assert_eq!(rewrite(SHIPPED, false), SHIPPED);
    }

    #[test]
    fn adds_the_key_inside_an_existing_system_section() {
        let no_key = "[System]\nload_library_redirect = 2\n\n[Logging]\ncalls = 1\n";
        let out = rewrite(no_key, true);
        let system_at = out.find("[System]").unwrap();
        let logging_at = out.find("[Logging]").unwrap();
        let key_at = out.find(KEY).unwrap();
        assert!(system_at < key_at && key_at < logging_at, "key must land inside [System]:\n{out}");
        // And it comes back out, since it wasn't there to begin with.
        assert_eq!(rewrite(&out, false), no_key);
    }

    #[test]
    fn creates_a_system_section_when_the_file_has_none() {
        let out = rewrite("[Logging]\ncalls = 1\n", true);
        assert!(out.contains("[System]"));
        assert!(out.contains("check_foreground_window = 0"));
        assert_eq!(rewrite(&out, false), "[Logging]\ncalls = 1\n\n[System]\n");
    }

    #[test]
    fn ignores_the_key_outside_the_system_section() {
        let elsewhere = "[Logging]\ncheck_foreground_window = 1\n\n[System]\nload_library_redirect = 2\n";
        let out = rewrite(elsewhere, true);
        assert!(out.contains("[Logging]\ncheck_foreground_window = 1"), "{out}");
    }

    #[test]
    fn ignores_a_commented_out_key() {
        let commented = "[System]\n;check_foreground_window = 1\n";
        let out = rewrite(commented, true);
        assert!(out.contains(";check_foreground_window = 1"));
        assert!(out.contains("check_foreground_window = 0   ; managed by Eous Modify, was absent"));
    }

    #[test]
    fn preserves_crlf_line_endings() {
        let crlf = SHIPPED.replace('\n', "\r\n");
        let out = rewrite(&crlf, true);
        assert!(!out.contains('\n') || out.contains("\r\n"));
        assert_eq!(out.matches('\n').count(), out.matches("\r\n").count());
        assert_eq!(rewrite(&out, false), crlf);
    }
}
