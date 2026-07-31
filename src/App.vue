<script setup lang="ts">
import { onMounted } from 'vue';
import { useRouter } from 'vue-router';
import AppShell from './layouts/AppShell.vue';
import { useSettingsStore } from './stores/settings';
import { useUpdaterStore } from './stores/updater';

const updaterStore = useUpdaterStore();
const settingsStore = useSettingsStore();
const router = useRouter();

onMounted(async () => {
   // Silent startup check — just populates updaterStore.update for the Sidebar badge.
   // No modal pops up unprompted; the user opens it via the badge/Settings.
   // check() never rejects (errors are caught internally onto updaterStore.errorMessage).
   updaterStore.check();

   // Land on Settings when either path is still unconfigured: nothing in the app works without the
   // mods folder, and Quick Launch needs the game executable.
   const [modsFolder, gameExecutable] = await Promise.all([
      settingsStore.fetch('mods_folder_path'),
      settingsStore.fetch('game_executable_path'),
   ]);
   if (!modsFolder || !gameExecutable) router.push('/settings');
});
</script>

<template>
   <AppShell />
</template>
