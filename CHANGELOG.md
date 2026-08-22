# Changelog

The section matching a release's version is what the release workflow publishes: it becomes both the
GitHub release body **and** the `notes` field inside `latest.json`, which is what the in-app update
prompt shows. Add the section before tagging — the build fails if a tag has no matching section here.

Keep the prose plain. The update prompt renders it as preformatted text, so Markdown headings and
emphasis show up literally; bullets and blank lines are fine.

## 0.0.7

Added

- Mods are filed into folders of their own category the next time you scan.
  Your existing folders are moved for you.
- Mods that arrive together in one folder are put into a group.
- Agents show the faction they belong to.
- Confirmations popup.

Changed

- Importing a mod takes you to the page it landed on, and the list updates on
  its own.

Fixed

- An archive inside an imported archive was never unpacked, leaving a file in
  the mod folder that the game could not read.
- Keybinds on punctuation keys showed a code name instead of the key.
- Some mods showed no keybinds at all.
- A mod could not be moved back into Other/Misc.

## 0.0.6

Added

- Paste an image straight onto a mod or an agent to use it.
- More image formats support for previews.
- Bangboos category.
- Agents' full name.

Changed

- Remove NPCs, Enemies, Weapons and Objects category from sidebar.
- New artwork for every agent.
- UI Overhaul

Fixed

- A mod with a very long name overflowed outside its card.

## 0.0.5

Added

- Quick Launch shows "Running…" when the game is already open.
- New in-game reload method.
  "When you switch back to the game" reloads once you are back in the game,
  and needs Eous to run as administrator, but no keybinds leak.
  "Straight away" reloads the moment you toggle and needs no administrator,
  but lets your mods' own keybinds fire while you type in other apps.
  The switch stays off until you turn it on, and the choice sits under it.

Fixed

- Reloading never ran unless you opened the Settings page.
- Turning on "Reload mods in-game" changed a line in your d3dx.ini and never reverted it.

Devlog

The old reload told 3DMigoto to accept all keystrokes from any window, and made every keybind
fire while you were typing anywhere else.

The new method waits for the game window to focus instead. The catch: Windows will not let
an ordinary program send a keypress to the game that runs as administrator,
so Eous has to run as administrator too.
If you would rather not, the old method is still there under "Straight away".

## 0.0.4

Added

- Portraits for Remielle and Sigrid.
- Drop an image onto a mod or an agent directly to use it.
- Remove the custom image and go back to the default.
- Settings now lists the changelog under Updates.
- Skip XXMI Launcher: Quick Launch starts the game directly.

Fixed

- Mods remained unchanged after a scan or image change.
- Disabled mods did not show their preview image.
- The update prompt said you were up to date while it offered an update.
- Closing the update prompt did not keep the update on offer.

Changed

- The two switches in Settings now share one Preferences card.
- Text that appears and disappears now fades instead of popping.
- The keybinds popup closes when you click outside it.

## 0.0.3

Fixed

- The update prompt showed placeholder text instead of the real release notes.

## 0.0.2

Fixed

- Sending F10 after a toggle was being skipped whenever Eous Modify was the focused window.
- The keybind list was hiding keybinds from mods that keep their .ini in a subfolder.

## 0.0.1

First release.

A mod manager for Zenless Zone Zero's 3DMigoto/ZZMI mods folder. It scans your mods, works out which
agent or category each one belongs to, and lets you toggle, group and edit them from one window.

- Mods filed under a specific agent, or under NPCs / Enemies / Weapons / Objects / UI
- Name, author and target deduced from folder structure, internal filenames and .ini hints.
- Enable and disable via the standard DISABLED_ folder rename, so nothing is locked into this app.
- Import .zip, .7z and .rar archives with the destination pre-filled from what was deduced.
- Group mods so one switch toggles all of them
- Keybind viewer, per-page search, sort and filters, custom preview images
- Quick Launch, with an admin prompt when the game needs one
- Built-in updater

Windows only.
