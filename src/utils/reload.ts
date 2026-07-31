import { invoke } from '@tauri-apps/api/core';

/** Settings-table key holding `'true'`/`'false'`. Off unless explicitly enabled. */
export const AUTO_RELOAD_KEY = 'auto_reload_on_toggle';

/**
 * Turns 3DMigoto's background-hotkey handling on or off by editing `check_foreground_window` in the
 * XXMI install's `d3dx.ini`. Without it, 3DMigoto ignores our F10 whenever this app is the focused
 * window — which is exactly when a mod toggle happens.
 *
 * Throws, deliberately: this runs when the user flips the switch, and a switch that silently fails to
 * do the one thing that makes the feature work is worse than an error message.
 */
export async function setXxmiBackgroundKeys(enabled: boolean): Promise<void> {
   await invoke('set_xxmi_background_keys', { enabled });
}

// Sending the reload itself lives in the backend, not here. It has to happen immediately after the
// mod's persisted 3DMigoto variables are written back to d3dx_user.ini — 3DMigoto rewrites that file
// from its own memory whenever it saves, so an IPC round trip in between is a window where a restored
// value can be overwritten before it is ever read. See `src-tauri/src/commands/toggle.rs`.
