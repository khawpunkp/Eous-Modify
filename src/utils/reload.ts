import { invoke } from '@tauri-apps/api/core';
import { useSettingsStore } from '../stores/settings';

/** Settings-table key holding `'true'`/`'false'`. Off unless explicitly enabled. */
export const AUTO_RELOAD_KEY = 'auto_reload_on_toggle';

/** Settings-table key holding which delivery the user picked. Mirrors the backend's key of the
 * same name — both sides read it, so they have to agree. */
export const RELOAD_METHOD_KEY = 'reload_method';

/**
 * How the reload reaches the game. Same outcome either way; the difference is what it costs you.
 *
 * `deferred` waits for the game window, where 3DMigoto already accepts hotkeys — so it needs Eous
 * running as administrator, because Windows will not let an ordinary process send a keypress into
 * the elevated game. `immediate` instead sets 3DMigoto to accept hotkeys from any window, which
 * needs no administrator but also means every mod's own keybinds fire while you are typing
 * elsewhere.
 */
export type ReloadMethod = 'deferred' | 'immediate';

export function isAutoReloadEnabled(): boolean {
   return useSettingsStore().settings[AUTO_RELOAD_KEY] === 'true';
}

/**
 * Notes that the game owes a reload after a mod or group toggle, if the user opted in.
 *
 * Returns straight away. The F10 goes out later, once the game window is back in front — 3DMigoto
 * ignores hotkeys while another window is focused, and this app is that window at the moment of a
 * toggle. Waiting is what lets its own default stay in place; the alternative was telling 3DMigoto to
 * accept hotkeys from anywhere, which also handed every mod's keybinds to whatever you were typing in.
 *
 * Errors are logged rather than thrown so a failed reload can never break the toggle itself — the
 * folder rename has already succeeded by this point, and F10 still works by hand.
 */
export async function maybeReloadXxmi(): Promise<void> {
   if (!isAutoReloadEnabled()) return;
   try {
      await invoke('request_reload');
   } catch (e) {
      console.warn('[reload] could not queue a reload:', e);
   }
}
