import { invoke } from '@tauri-apps/api/core';
import { useSettingsStore } from '../stores/settings';

/** Settings-table key holding `'true'`/`'false'`. Off unless explicitly enabled. */
export const AUTO_RELOAD_KEY = 'auto_reload_on_toggle';

export function isAutoReloadEnabled(): boolean {
   return useSettingsStore().settings[AUTO_RELOAD_KEY] === 'true';
}

/**
 * Asks XXMI to reload after a mod/group toggle, if the user opted in.
 *
 * Deliberately silent: the backend sends F10 via SendInput, which only reaches the game while the
 * game has focus, so it returns false (no error) whenever our own window is focused. That's the
 * normal case when organizing mods in the app, and surfacing it as a failure every time would be
 * noise. Real errors are logged rather than thrown so a reload can never break the toggle itself.
 */
export async function maybeReloadXxmi(): Promise<void> {
   if (!isAutoReloadEnabled()) return;
   try {
      await invoke<boolean>('reload_xxmi');
   } catch (e) {
      console.warn('[reload] could not send F10 to XXMI:', e);
   }
}
