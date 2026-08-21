use std::fs;
use std::path::Path;

use crate::models::KeybindInfo;
use crate::mods::find_mod_ini_paths;

/// Parses a single INI file's content for keybinds: collects `key = value` lines inside every
/// `[Key...]` section.
///
/// This used to read nothing until it had seen a `; Constants` comment, a rule inherited from the old
/// app's `get_ini_keybinds`. Measured against a real 174-mod library it never once skipped something
/// it should have, and cost the keybinds of six files outright — mods that write `[Constants]` as a
/// section header rather than a comment, and mods that write no such line at all — plus two sections
/// in a seventh, whose author put those keys above the comment. To 3DMigoto a `[Key...]` section is a
/// keybind wherever it sits in a file the game has loaded, so its position was never evidence of
/// anything. A commented-out section reads as `;[KeySwap]`, which does not start with `[`, so
/// dropping the rule cannot start picking up disabled examples either.
pub fn parse_keybinds_from_ini(content: &str) -> Vec<KeybindInfo> {
    let mut current_section_title: Option<String> = None;
    let mut keybinds = Vec::new();

    for line in content.lines() {
        let line = line.trim();

        if line.starts_with('[') && line.ends_with(']') {
            let section_name = line[1..line.len() - 1].trim().to_string();
            current_section_title = if section_name.to_lowercase().starts_with("key") {
                Some(section_name)
            } else {
                None
            };
        } else if let Some(title) = &current_section_title {
            if line.to_lowercase().starts_with("key") && line.contains('=') {
                if let Some(value_part) = line.splitn(2, '=').nth(1) {
                    let keybind_value = value_part.trim().to_string();
                    if !keybind_value.is_empty() {
                        keybinds.push(KeybindInfo { title: title.clone(), key: keybind_value });
                    }
                }
            }
        }
    }

    keybinds
}

/// Every keybind the mod defines, gathered from all of its INI files (found via the same dual
/// enabled/DISABLED_ path check used everywhere else).
///
/// Reads every file rather than stopping at the first one with a match: a mod that splits its
/// toggles across several `.ini`s would otherwise show only whichever file happened to be read
/// first, silently hiding the rest. Exact duplicates are dropped, since the same section can be
/// repeated across files.
pub fn get_keybinds(base_mods_path: &Path, folder_name: &str) -> Vec<KeybindInfo> {
    let mut keybinds: Vec<KeybindInfo> = Vec::new();

    for ini_path in find_mod_ini_paths(base_mods_path, folder_name) {
        let Ok(content) = fs::read_to_string(&ini_path) else {
            continue;
        };
        for keybind in parse_keybinds_from_ini(&content) {
            if !keybinds.contains(&keybind) {
                keybinds.push(keybind);
            }
        }
    }

    keybinds
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_key_sections_a_mod_declares() {
        let ini = r#"
[Mod]
Name = Test Mod

; Constants
[Constants]
global persist $active = 0

[KeySwap]
key = ]
type = cycle
$active = 0,1

[KeyToggleHat]
key = h
type = toggle
$hat_on = 0,1
"#;
        let keybinds = parse_keybinds_from_ini(ini);
        assert_eq!(
            keybinds,
            vec![
                KeybindInfo { title: "KeySwap".to_string(), key: "]".to_string() },
                KeybindInfo { title: "KeyToggleHat".to_string(), key: "h".to_string() },
            ]
        );
    }

    #[test]
    fn parses_a_mod_that_writes_constants_as_a_section_rather_than_a_comment() {
        // RabbitFX's shape, and the case that showed the old marker rule was wrong: the keys sit well
        // below a [Constants] section header, and the file carries no such comment anywhere.
        let ini = r#"
namespace = RabbitFX
; RabbitFX -Zenless Zone Zero- Version 7.7

[Constants]
persist global $censor = 0

[KeyCensor]
key = no_modifiers \
type = cycle
$censor = 0,1
"#;
        assert_eq!(
            parse_keybinds_from_ini(ini),
            vec![KeybindInfo { title: "KeyCensor".to_string(), key: r"no_modifiers \".to_string() }]
        );
    }

    #[test]
    fn parses_a_mod_with_no_constants_line_at_all() {
        // Several mods put their keys straight at the top of the file. The game reads them, so this
        // has to as well.
        let ini = r#"
[KeySwap_0]
condition = $active0 == 1
key = O
type = cycle
"#;
        assert_eq!(
            parse_keybinds_from_ini(ini),
            vec![KeybindInfo { title: "KeySwap_0".to_string(), key: "O".to_string() }]
        );
    }

    #[test]
    fn parses_a_key_section_sitting_above_the_constants_comment() {
        // The reverse of what this used to assert. One real mod declares two keys above its comment
        // and the rest below it, and the marker rule silently dropped the first two.
        let ini = r#"
[KeyBefore]
key = x

; Constants
[KeyAfter]
key = y
"#;
        assert_eq!(
            parse_keybinds_from_ini(ini),
            vec![
                KeybindInfo { title: "KeyBefore".to_string(), key: "x".to_string() },
                KeybindInfo { title: "KeyAfter".to_string(), key: "y".to_string() },
            ]
        );
    }

    /// Builds `<base>/<mod>/…` with the given `(relative path, contents)` INI files.
    fn build_mod_dir(files: &[(&str, &str)]) -> (std::path::PathBuf, String) {
        use std::sync::atomic::{AtomicU32, Ordering};
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let unique = COUNTER.fetch_add(1, Ordering::SeqCst);
        let base = std::env::temp_dir()
            .join(format!("eous_modify_keybinds_test_{}_{}", std::process::id(), unique));
        let _ = fs::remove_dir_all(&base);

        let folder_name = "TestMod".to_string();
        for (relative, contents) in files {
            let path = base.join(&folder_name).join(relative);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, contents).unwrap();
        }

        (base, folder_name)
    }

    const TOGGLE_INI: &str = "; Constants\n[KeyToggleHat]\nkey = h\n";
    const SWAP_INI: &str = "; Constants\n[KeySwap]\nkey = ]\n";

    #[test]
    fn finds_keybinds_in_a_nested_ini() {
        // Mods that keep their .ini in a subfolder used to report no keybinds at all.
        let (base, folder) = build_mod_dir(&[("parts/hat/hat.ini", TOGGLE_INI)]);
        assert_eq!(
            get_keybinds(&base, &folder),
            vec![KeybindInfo { title: "KeyToggleHat".to_string(), key: "h".to_string() }]
        );
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn merges_keybinds_across_every_ini() {
        // Previously only the first .ini with a match was reported, hiding the others.
        let (base, folder) = build_mod_dir(&[("a_swap.ini", SWAP_INI), ("b_hat.ini", TOGGLE_INI)]);
        assert_eq!(
            get_keybinds(&base, &folder),
            vec![
                KeybindInfo { title: "KeySwap".to_string(), key: "]".to_string() },
                KeybindInfo { title: "KeyToggleHat".to_string(), key: "h".to_string() },
            ]
        );
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn drops_a_keybind_repeated_across_inis() {
        let (base, folder) = build_mod_dir(&[("a.ini", TOGGLE_INI), ("nested/b.ini", TOGGLE_INI)]);
        assert_eq!(
            get_keybinds(&base, &folder),
            vec![KeybindInfo { title: "KeyToggleHat".to_string(), key: "h".to_string() }]
        );
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn reads_keybinds_from_a_disabled_mod_folder() {
        // Toggling a mod off renames its folder; its keybinds must still be readable.
        let (base, folder) = build_mod_dir(&[("mod.ini", TOGGLE_INI)]);
        fs::rename(base.join(&folder), base.join(format!("DISABLED_{folder}"))).unwrap();
        assert_eq!(
            get_keybinds(&base, &folder),
            vec![KeybindInfo { title: "KeyToggleHat".to_string(), key: "h".to_string() }]
        );
        let _ = fs::remove_dir_all(&base);
    }
}
