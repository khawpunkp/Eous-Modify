<script setup lang="ts">
import { onMounted } from 'vue';
import { useRouter } from 'vue-router';
import AppShell from './layouts/AppShell.vue';
import ConfirmDialog from '@/components/ui/confirm/ConfirmDialog.vue';
import { useSettingsStore } from './stores/settings';
import { useUpdaterStore } from './stores/updater';
import { AUTO_RELOAD_KEY } from './utils/reload';

const updaterStore = useUpdaterStore();
const settingsStore = useSettingsStore();
const router = useRouter();

onMounted(async () => {
   // Silent startup check — just populates updaterStore.update for the Sidebar badge.
   // No modal pops up unprompted; the user opens it via the badge/Settings.
   // check() never rejects (errors are caught internally onto updaterStore.errorMessage).
   await updaterStore.check();

   // Land on Settings when either path is still unconfigured: nothing in the app works without the
   // mods folder, and Quick Launch needs the game executable.
   // AUTO_RELOAD_KEY is fetched here rather than only on the Settings page: isAutoReloadEnabled()
   // reads it straight out of the store, so until something loads it the value is undefined and every
   // toggle silently skips its reload — the feature appeared broken unless you happened to open
   // Settings first that session.
   const [modsFolder, gameExecutable] = await Promise.all([
      settingsStore.fetch('mods_folder_path'),
      settingsStore.fetch('game_executable_path'),
      settingsStore.fetch(AUTO_RELOAD_KEY),
   ]);
   if (!modsFolder || !gameExecutable || Boolean(updaterStore.update)) router.push('/settings');
});
</script>

<template>
   <AppShell />
   <!-- Mounted here rather than per page: it is what every confirmAction() call renders into, and
        outside the router view a navigation cannot take the dialog down mid-question. -->
   <ConfirmDialog />
</template>
