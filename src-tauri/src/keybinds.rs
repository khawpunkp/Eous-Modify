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

/// Finds the mod's INI file(s) on disk (via the same dual enabled/DISABLED_ path check used
/// everywhere else) and returns the first one with any keybinds found after its `; Constants`
/// marker.
pub fn get_keybinds(base_mods_path: &Path, folder_name: &str) -> Vec<KeybindInfo> {
    for ini_path in find_mod_ini_paths(base_mods_path, folder_name) {
        let Ok(content) = fs::read_to_string(&ini_path) else {
            continue;
        };
        let keybinds = parse_keybinds_from_ini(&content);
        if !keybinds.is_empty() {
            return keybinds;
        }
    }
    Vec::new()
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
}
