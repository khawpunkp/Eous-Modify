/**
 * Settings-table key holding `'true'`/`'false'`. Off unless explicitly enabled — skipping the launcher
 * also skips the update check it performs, so it has to be the user's choice rather than a default.
 *
 * Read backend-side by `launch_game`; keep the string in step with `commands/launcher.rs`.
 */
export const SKIP_XXMI_LAUNCHER_KEY = 'skip_xxmi_launcher';

/**
 * Whether a configured executable path looks like XXMI Launcher rather than a game the user pointed
 * at directly. Mirrors the same check in `commands/launcher.rs`, which is what actually gates the
 * flags — this copy only exists so the UI can say when the setting won't apply instead of appearing
 * to be on while doing nothing.
 */
export function isXxmiLauncherPath(path: string | null): boolean {
   if (!path) return false;
   const fileName = path.split(/[\\/]/).pop() ?? '';
   return fileName.toLowerCase().includes('xxmi');
}
