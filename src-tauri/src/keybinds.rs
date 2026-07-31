use std::fs;
use std::path::Path;

use crate::models::KeybindInfo;
use crate::mods::find_mod_ini_paths;

/// Parses a single INI file's content for keybinds: looks for a `; Constants` comment marker,
/// then collects `key = value` lines inside `[Key...]` sections that appear after it. Ports the
/// old app's `get_ini_keybinds` line-scanning algorithm.
pub fn parse_keybinds_from_ini(content: &str) -> Vec<KeybindInfo> {
    let mut current_section_title: Option<String> = None;
    let mut found_constants_tag = false;
    let mut keybinds = Vec::new();

    for line in content.lines() {
        let line = line.trim();

        if !found_constants_tag {
            if line.starts_with(';') && line[1..].trim_start().to_lowercase().contains("constants") {
                found_constants_tag = true;
            }
            continue;
        }

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
    fn parses_keybinds_after_constants_marker() {
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
    fn ignores_key_sections_before_constants_marker() {
        let ini = r#"
[KeyBeforeConstants]
key = x

; Constants
[KeyAfter]
key = y
"#;
        let keybinds = parse_keybinds_from_ini(ini);
        assert_eq!(keybinds, vec![KeybindInfo { title: "KeyAfter".to_string(), key: "y".to_string() }]);
    }

    #[test]
    fn returns_empty_when_no_constants_marker() {
        let ini = "[KeySomething]\nkey = z\n";
        assert!(parse_keybinds_from_ini(ini).is_empty());
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
