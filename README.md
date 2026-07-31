# Eous Modify

**A mod manager for Zenless Zone Zero, built with Tauri and Vue.**

[![Latest Release](https://img.shields.io/github/v/release/khawpunkp/Eous-Modify?label=Latest%20Release&style=for-the-badge)](https://github.com/khawpunkp/Eous-Modify/releases/latest)
[![Downloads](https://img.shields.io/github/downloads/khawpunkp/Eous-Modify/total?style=for-the-badge)](https://github.com/khawpunkp/Eous-Modify/releases)

Eous Modify organizes the mods in your 3DMigoto/ZZMI mods folder — it scans them, works out which
agent or category each one belongs to, and lets you toggle, group and edit them from one window.
Enabling and disabling is just the standard `DISABLED_` folder-prefix rename, so your mods keep
working with 3DMigoto exactly as before and nothing is locked into this app.

**Windows only** (ships as an `.msi`), and **Zenless Zone Zero only**.

---

## ✨ Features

- **🗂️ Organized library** — mods are filed under a specific agent, or under one of the
  NPCs / Enemies / Weapons / Objects / UI categories. An **Other** page collects anything that
  couldn't be matched to either, so nothing is ever hidden.
- **🤖 Automatic deduction** — scanning and importing work out a mod's name, author and target from
  its folder structure, internal filenames and `.ini` hints, matching against a built-in list of
  agents and their aliases. A later scan re-checks mods that didn't match before, so adding an
  agent or alias picks up mods that were previously unfiled.
- **🖱️ Enable / disable** — a switch per mod, backed by the `DISABLED_` prefix rename.
- **📦 Archive import** — pick a `.zip`, `.7z` or `.rar`; the archive is analyzed, the destination is
  pre-selected from what was deduced, and you confirm the details before it lands.
- **🧩 Mod groups** — select two or more mods and group them under one name and image. Toggling the
  group toggles every member; members can be added or removed later.
- **⌨️ Keybind viewer** — shows the keybinds a mod defines (the `key = …` lines in its `[Key…]`
  sections after a `; Constants` marker), translated into readable labels like `Ctrl + Arrow Up`.
- **🔍 Search, sort and filter** — per-page search and sort, plus rank / attribute / speciality
  filters on the agents list. Your sort choice is remembered per page.
- **🖼️ Previews** — mod preview images are detected automatically, and can be replaced per mod.
  Agents and groups take custom images too.
- **🚀 Quick Launch** — launch the game straight from the sidebar.
- **🔄 Built-in updater** — new versions are picked up from GitHub Releases (Tauri's updater).

---

## 💾 Installation

1. Download the `.msi` from the [**latest release**](https://github.com/khawpunkp/Eous-Modify/releases/latest).
2. Run it and follow the installer.
3. On first launch you'll land on **Settings** — set your **Mods Folder** (your 3DMigoto/ZZMI `Mods`
   directory) and your **Game Executable**. The rest of the app stays locked until both are set.
4. Then use **Scan Mods Folder** in the sidebar to populate the library.

Updates arrive through the built-in updater.

---

## 🚀 Usage

**Scanning.** *Scan Mods Folder* walks your mods directory, adds anything new, files each mod under
the agent or category it deduced, re-checks previously unmatched mods, and drops database entries for
mods no longer on disk. Safe to re-run any time — it reports what it did when it finishes.

**Importing.** *Import Mod* in the sidebar takes a `.zip`, `.7z` or `.rar`. The archive is inspected
first, so the destination, name and author come pre-filled from what could be deduced; adjust
anything and confirm.

**Browsing.** Use the sidebar for the agents list or a category. Agent cards show how many mods each
agent has and how many are currently enabled. Anything unmatched sits on the **Other** page, where
you can edit a mod to assign it properly.

**Managing a mod.** Each card has a switch to enable/disable, and buttons to edit it (name, author,
preview image, or move it to a different agent/category), view its keybinds, open its folder, or
delete it — deleting removes the folder from disk.

**Grouping.** *Select Mods to Group* on any mod page, tick two or more, then name the group and give
it an image. The group card toggles all members at once; open it later to rename it, change the
image, or add and remove members. Removing a group down to one mod disbands it.

**Agents.** Built-in agents come from the app's own definitions and are refreshed on each update, so
only their **aliases** are editable — those feed the deduction described above. Agents you add
yourself are fully editable and deletable.

---

## 🛠️ Development

**Prerequisites**

- [Node.js](https://nodejs.org/) (LTS) and npm
- [Rust toolchain](https://www.rust-lang.org/tools/install)
- [Tauri prerequisites](https://tauri.app/start/prerequisites/) for your platform

**Setup**

```bash
git clone https://github.com/khawpunkp/Eous-Modify.git
cd Eous-Modify
npm install
npm run tauri dev
```

**Build**

```bash
npm run tauri build
```

**Other scripts**

```bash
npm run build      # type-check the frontend and bundle it
npm run format     # Prettier
```

Backend tests live with the Rust source:

```bash
cd src-tauri && cargo test
```

Releasing is documented in [RELEASE_CHECKLIST.md](RELEASE_CHECKLIST.md).

---

## 💻 Tech stack

| | |
|---|---|
| Shell | Tauri v2 |
| Backend | Rust, SQLite via `rusqlite` |
| Frontend | Vue 3 (`<script setup>` + TypeScript), Vite |
| Routing / state | vue-router (file-based routes), Pinia |
| Styling | Tailwind CSS v4 |
| Components | in-house primitives built on Reka UI + CVA |
| Icons | Phosphor |

The built-in agent and category data lives in [`src-tauri/definitions/zzz.toml`](src-tauri/definitions/zzz.toml),
including the aliases used for deduction — add an alias there and rescanning will pick up mods named
that way.

---

## 🙏 Credits

Eous Modify started as a rebuild of [Eidenz/gmm](https://github.com/Eidenz/gmm), which supported
several gacha games with a React frontend. This version was rewritten around Zenless Zone Zero
alone, on Vue 3 and Tauri v2. Thanks to the original project for the groundwork.

## 📄 License

[GPL-3.0](LICENSE), same as the project it was rebuilt from.
