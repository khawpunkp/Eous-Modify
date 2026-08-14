//! Undoes the one `d3dx.ini` edit 0.0.4 made, on behalf of anyone upgrading from it.
//!
//! That version set `check_foreground_window = 0` so a synthetic F10 would register while this app
//! held focus. It worked, and it also told 3DMigoto to act on *every* hotkey system-wide — mods
//! routinely bind bare letters and digits, so typing in a chat window silently toggled them. 0.0.5
//! stopped doing this (the reload waits for the game window instead), but stopping does nothing for
//! the installs already carrying the line.
//!
//! So this runs at startup and puts the value back. Only the revert direction survives from the
//! original `xxmi_config`; nothing here can ever write that setting again.
//!
//! Two rules make editing a file we don't own defensible. We only touch a line carrying [`MARKER`],
//! which is proof we wrote it and which records what it replaced — a bare
//! `check_foreground_window` line is the user's own deliberate setting and is left alone. And the
//! rest of the file survives byte for byte, line endings included.

use std::path::{Path, PathBuf};

/// The `[System]` key that gates background hotkeys.
const KEY: &str = "check_foreground_window";

/// Stamped onto every line 0.0.4 wrote, carrying the state it replaced.
const MARKER: &str = "; managed by Eous Modify, was ";

/// Marker payload meaning the key wasn't in the file at all before we added it, so reverting takes
/// the whole line back out rather than inventing a value for it.
const WAS_ABSENT: &str = "absent";

/// `d3dx.ini` sits next to the Mods folder, not inside it — that's where both XXMI and plain
/// 3DMigoto put it.
fn d3dx_ini_path(mods_folder: &Path) -> Option<PathBuf> {
    Some(mods_folder.parent()?.join("d3dx.ini"))
}

/// True for an active (non-commented) `check_foreground_window` line.
fn is_key_line(trimmed: &str) -> bool {
    !trimmed.starts_with(';')
        && trimmed.len() >= KEY.len()
        && trimmed[..KEY.len()].eq_ignore_ascii_case(KEY)
}

/// The state 0.0.4 recorded on this line, or `None` if it isn't one of ours.
fn marked_previous(line: &str) -> Option<&str> {
    let at = line.find(MARKER)?;
    Some(line[at + MARKER.len()..].trim())
}

/// Restores any line this app wrote, and returns the rest unchanged.
///
/// A pure string transform so the decision table is testable without a real XXMI install. Idempotent
/// by construction: reverting removes the marker, so a second pass finds nothing of ours to act on.
pub fn revert(contents: &str) -> String {
    let newline = if contents.contains("\r\n") { "\r\n" } else { "\n" };

    let mut out: Vec<String> = Vec::new();
    let mut in_system = false;
    let mut handled = false;

    for line in contents.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            in_system = trimmed.eq_ignore_ascii_case("[System]");
            out.push(line.to_string());
            continue;
        }

        if in_system && !handled && is_key_line(trimmed) {
            handled = true;
            match marked_previous(line) {
                // Ours, and the key was absent before we added it — take the line back out.
                Some(previous) if previous == WAS_ABSENT => {}
                Some(previous) => out.push(format!("{KEY} = {previous}")),
                // Not ours. Whatever it says, the user meant it.
                None => out.push(line.to_string()),
            }
            continue;
        }

        out.push(line.to_string());
    }

    let mut result = out.join(newline);
    if contents.ends_with('\n') {
        result.push_str(newline);
    }
    result
}

/// Puts the `d3dx.ini` beside `mods_folder` back the way 0.0.4 found it.
///
/// Silent on every outcome, including failure. Nothing here is worth interrupting a launch for: the
/// overwhelmingly common case is a file with nothing of ours in it, which is indistinguishable from
/// success, and the rest — no Mods folder set yet, a path that isn't a `*MI` install, a file we
/// can't read — leaves the user no worse off than before this ran.
pub fn revert_if_ours(mods_folder: &Path) {
    let Some(path) = d3dx_ini_path(mods_folder) else {
        return;
    };
    let Ok(contents) = std::fs::read_to_string(&path) else {
        return;
    };

    let updated = revert(&contents);
    if updated == contents {
        return;
    }

    if let Err(e) = std::fs::write(&path, updated) {
        eprintln!("[cleanup] could not restore {}: {e}", path.display());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHIPPED: &str = "[Include]\nfoo = bar\n\n[System]\nload_library_redirect = 2\n\n; Only enable key input processing when the game is in the foreground:\ncheck_foreground_window = 1\n";

    fn managed(previous: &str) -> String {
        SHIPPED.replace(
            "check_foreground_window = 1",
            &format!("{KEY} = 0   {MARKER}{previous}"),
        )
    }

    #[test]
    fn d3dx_ini_sits_beside_the_mods_folder() {
        let path = d3dx_ini_path(Path::new(r"C:\XXMI\ZZMI\Mods")).unwrap();
        assert_eq!(path, PathBuf::from(r"C:\XXMI\ZZMI\d3dx.ini"));
    }

    #[test]
    fn restores_the_recorded_value() {
        assert_eq!(revert(&managed("1")), SHIPPED);
    }

    #[test]
    fn removes_a_line_that_was_not_there_before() {
        let out = revert(&managed(WAS_ABSENT));
        assert!(!out.contains(KEY));
        assert!(out.contains("load_library_redirect = 2"));
    }

    #[test]
    fn never_touches_a_line_we_did_not_write() {
        assert_eq!(revert(SHIPPED), SHIPPED);
        let theirs = SHIPPED.replace("check_foreground_window = 1", "check_foreground_window = 0");
        assert_eq!(revert(&theirs), theirs);
    }

    #[test]
    fn running_twice_changes_nothing_further() {
        let once = revert(&managed("1"));
        assert_eq!(revert(&once), once);
    }

    #[test]
    fn preserves_crlf_line_endings() {
        let crlf = managed("1").replace('\n', "\r\n");
        let out = revert(&crlf);
        assert!(out.contains("\r\n"));
        assert_eq!(out, SHIPPED.replace('\n', "\r\n"));
    }

    #[test]
    fn leaves_a_matching_key_outside_the_system_section_alone() {
        let elsewhere = format!("[Other]\n{KEY} = 0   {MARKER}1\n");
        assert_eq!(revert(&elsewhere), elsewhere);
    }
}
