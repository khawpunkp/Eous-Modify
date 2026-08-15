# Changelog

The section matching a release's version is what the release workflow publishes: it becomes both the
GitHub release body **and** the `notes` field inside `latest.json`, which is what the in-app update
prompt shows. Add the section before tagging — the build fails if a tag has no matching section here.

Keep the prose plain. The update prompt renders it as preformatted text, so Markdown headings and
emphasis show up literally; bullets and blank lines are fine.

## 0.0.5

Added

- Reloading mods in-game now lets you pick how it gets there, because both ways cost something.
  "When you switch back to the game" leaves your mods' keybinds alone, and runs Eous as
  administrator, which is what lets the keypress reach the game once its own window is in front.
  "Straight away" reloads the moment you toggle and needs no administrator, at the price of your
  mods' keybinds also firing while you type in other apps — which is what 0.0.4 did, without saying
  so. The switch stays off until you turn it on, and the choice sits under it.

  If you had the setting on in 0.0.4, it starts off here — what it meant back then no longer exists,
  and one of the two options now asks for administrator. Turn it on again once you have seen both.

- Quick Launch shows "Running…" and stops offering to start the game when it is already open.

Fixed

- Reloading never ran at all unless you had opened Settings that session, so for most people the
  setting did nothing whatsoever.

- Turning it on in 0.0.4 handed every one of your mods' keybinds to whatever you were typing in, with
  no way to say no — mods bound to plain letters and digits fired in other apps. That is now the
  labelled cost of one option rather than a surprise attached to both. This update also puts the line
  0.0.4 wrote in your d3dx.ini back the way it was: nothing to do, and nothing you changed yourself
  is touched.

## 0.0.4

Added

- Drop an image straight onto a mod or an agent to use it as the picture.
- Hover a picture to remove the one you set and go back to the default.
- Settings now lists the changelog under Updates.
- Skip XXMI Launcher: Quick Launch starts the game directly instead of opening the launcher window.
- Portraits for Remielle and Sigrid.
- The keybinds popup closes when you click outside it.

Fixed

- Mods no longer stay stale after a scan. The agent, category and Other pages update as soon as it
  finishes.
- Disabled mods show their preview image again.
- A mod card picks up a new picture straight away instead of after leaving the page.
- The update prompt no longer says you are up to date while offering an update, and closing it keeps
  the update on offer.

Changed

- The two switches in Settings now share one Preferences card.
- Text that appears and disappears eases in and out rather than popping.

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
