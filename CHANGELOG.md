# Changelog

The section matching a release's version is what the release workflow publishes: it becomes both the
GitHub release body **and** the `notes` field inside `latest.json`, which is what the in-app update
prompt shows. Add the section before tagging — the build fails if a tag has no matching section here.

Keep the prose plain. The update prompt renders it as preformatted text, so Markdown headings and
emphasis show up literally; bullets and blank lines are fine.

## 0.0.3

Fixed

- Release notes now reach this update prompt. Previous versions showed a placeholder here no matter
  what the release page said, because the notes are baked into the update manifest when the installer
  is built, and editing the release page afterwards does not touch it.

## 0.0.2

Fixed

- In-game reload now actually works. Sending F10 after a toggle was being skipped whenever Eous Modify
  was the focused window, which is exactly when you toggle a mod. Turning the setting on now adjusts
  your XXMI d3dx.ini so the keypress registers, and puts the old value back when you turn it off.
- The keybind list was hiding keybinds. Mods keeping their .ini in a subfolder showed none at all, and
  mods splitting keybinds across several .ini files only ever showed one file's worth.

Changed

- Spacing throughout the UI now uses flexbox gaps rather than margins. The enable/disable switch is
  slightly larger as a result.

## 0.0.1

First release.

A mod manager for Zenless Zone Zero's 3DMigoto/ZZMI mods folder. It scans your mods, works out which
agent or category each one belongs to, and lets you toggle, group and edit them from one window.

- Mods filed under a specific agent, or under NPCs / Enemies / Weapons / Objects / UI
- Name, author and target deduced from folder structure, internal filenames and .ini hints
- Enable and disable via the standard DISABLED_ folder rename, so nothing is locked into this app
- Import .zip, .7z and .rar archives with the destination pre-filled from what was deduced
- Group mods so one switch toggles all of them
- Keybind viewer, per-page search, sort and filters, custom preview images
- Quick Launch, with an admin prompt when the game needs one
- Built-in updater

Windows only (ships as an .msi), and Zenless Zone Zero only.
