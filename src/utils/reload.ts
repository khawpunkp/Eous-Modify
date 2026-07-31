import { invoke } from '@tauri-apps/api/core';
import { useSettingsStore } from '../stores/settings';

/** Settings-table key holding `'true'`/`'false'`. Off unless explicitly enabled. */
export const AUTO_RELOAD_KEY = 'auto_reload_on_toggle';

export function isAutoReloadEnabled(): boolean {
   return useSettingsStore().settings[AUTO_RELOAD_KEY] === 'true';
}

/**
 * Turns 3DMigoto's background-hotkey handling on or off by editing `check_foreground_window` in the
 * XXMI install's `d3dx.ini`. Without it, 3DMigoto ignores our F10 whenever this app is the focused
 * window — which is exactly when a mod toggle happens.
 *
 * Throws, unlike {@link maybeReloadXxmi}: this runs when the user flips the switch, and a switch that
 * silently fails to do the one thing that makes the feature work is worse than an error message.
 */
export async function setXxmiBackgroundKeys(enabled: boolean): Promise<void> {
   await invoke('set_xxmi_background_keys', { enabled });
}

/**
 * Asks XXMI to reload after a mod/group toggle, if the user opted in.
 *
 * Errors are logged rather than thrown so a failed reload can never break the toggle itself — the
 * folder rename has already succeeded by this point, and the user can always press F10 themselves.
 */
export async function maybeReloadXxmi(): Promise<void> {
   if (!isAutoReloadEnabled()) return;
   try {
      await invoke('reload_xxmi');
   } catch (e) {
      console.warn('[reload] could not send F10 to XXMI:', e);
   }
}
