<script setup lang="ts">
import { computed, onMounted, ref } from 'vue';
import { open } from '@tauri-apps/plugin-dialog';
import { getVersion } from '@tauri-apps/api/app';
import {
   PhArrowsClockwise,
   PhBoxArrowDown,
   PhCaretRight,
   PhFolderOpen,
   PhGear,
   PhWarning,
} from '@phosphor-icons/vue';
import VueButton from '@/components/ui/button/VueButton.vue';
import VueTypography from '@/components/ui/typography/VueTypography.vue';
import VueSwitch from '@/components/ui/switch/VueSwitch.vue';
import { useSettingsStore } from '../../stores/settings';
import { AUTO_RELOAD_KEY, setXxmiBackgroundKeys } from '../../utils/reload';
import { CHANGELOG } from '../../utils/changelog';
import { SKIP_XXMI_LAUNCHER_KEY, isXxmiLauncherPath } from '../../utils/launcher';
import { useUpdaterStore } from '../../stores/updater';
import UpdateModal from '../../components/UpdateModal.vue';

const settingsStore = useSettingsStore();
const updaterStore = useUpdaterStore();

const modsFolderPath = ref<string | null>(null);
const gameExecutablePath = ref<string | null>(null);

const currentVersion = ref<string>('');
const showUpdateModal = ref(false);
const noUpdateFound = ref(false);

// Bundled with the build rather than fetched, so it describes the version actually installed and
// works offline. Collapsed by default to keep this page short as the list grows.
const changelog = CHANGELOG;
const showChangelog = ref(true);

onMounted(async () => {
   modsFolderPath.value = await settingsStore.fetch('mods_folder_path');
   gameExecutablePath.value = await settingsStore.fetch('game_executable_path');
   currentVersion.value = await getVersion();
   autoReload.value = (await settingsStore.fetch(AUTO_RELOAD_KEY)) === 'true';
   skipXxmiLauncher.value = (await settingsStore.fetch(SKIP_XXMI_LAUNCHER_KEY)) === 'true';
   await updaterStore.check();
   if (Boolean(updaterStore.update)) showUpdateModal.value = true;
});

const autoReload = ref(false);
const autoReloadError = ref<string | null>(null);

const skipXxmiLauncher = ref(false);
// The backend applies the launcher-only flags just for XXMI Launcher, so surface when the configured
// executable means this switch does nothing — an "Enabled" switch that silently no-ops is worse than
// one that says why.
const gameExeIsXxmiLauncher = computed(() => isXxmiLauncherPath(gameExecutablePath.value));

async function setSkipXxmiLauncher(enabled: boolean) {
   skipXxmiLauncher.value = enabled;
   await settingsStore.set(SKIP_XXMI_LAUNCHER_KEY, String(enabled));
}

/**
 * Editing d3dx.ini is what makes the reload actually land, so a failure there must leave the switch
 * off: an "Enabled" switch over a feature that can't work is the bug this whole thing exists to fix.
 * Turning it off is honoured either way — the user asked for off, and a d3dx.ini we couldn't revert
 * is a separate problem to report, not a reason to stay on.
 */
async function setAutoReload(enabled: boolean) {
   autoReloadError.value = null;
   try {
      await setXxmiBackgroundKeys(enabled);
   } catch (e) {
      autoReloadError.value = String(e);
      if (enabled) return;
   }
   autoReload.value = enabled;
   await settingsStore.set(AUTO_RELOAD_KEY, String(enabled));
}

async function chooseFolder() {
   const path = await open({ directory: true, multiple: false });
   if (typeof path === 'string') {
      await settingsStore.set('mods_folder_path', path);
      modsFolderPath.value = path;
   }
}

async function chooseGameExecutable() {
   const path = await open({
      multiple: false,
      filters: [{ name: 'Executable', extensions: ['exe'] }],
   });
   if (typeof path === 'string') {
      await settingsStore.set('game_executable_path', path);
      gameExecutablePath.value = path;
   }
}

async function checkForUpdates() {
   noUpdateFound.value = false;
   const update = await updaterStore.check();
   if (update) {
      showUpdateModal.value = true;
   } else if (!updaterStore.errorMessage) {
      noUpdateFound.value = true;
   }
}
</script>

<template>
   <div class="flex flex-col gap-6">
      <div class="flex items-center border-b border-white/10 pb-4">
         <VueTypography variant="H1B" as="h1" class="flex h-12 items-center gap-3">
            <PhGear :size="32" weight="fill" />
            Settings
         </VueTypography>
      </div>

      <div class="flex flex-col gap-4">
         <div class="bg-card flex w-full flex-col gap-4 rounded-lg border border-white/10 p-6">
            <VueTypography variant="H1B" as="h2">Paths Configuration</VueTypography>

            <div class="grid grid-cols-12 items-center">
               <VueTypography variant="BodyB" as="h3" class="col-span-2 flex items-center gap-2">
                  <PhWarning v-if="!modsFolderPath" :size="24" weight="fill" class="text-accent" />
                  Mods Folder
               </VueTypography>
               <VueTypography
                  variant="BodyR"
                  as="p"
                  class="text-muted-foreground col-span-8 break-all"
               >
                  {{ modsFolderPath ?? 'Not set' }}
               </VueTypography>
               <div class="col-span-2 flex justify-end">
                  <VueButton type="button" @click="chooseFolder">
                     <PhFolderOpen :size="24" weight="fill" />
                     Choose Folder
                  </VueButton>
               </div>
            </div>

            <div class="h-px w-full bg-white/10" />

            <div class="grid grid-cols-12 items-center">
               <VueTypography variant="BodyB" as="h3" class="col-span-2 flex items-center gap-2">
                  <PhWarning
                     v-if="!gameExecutablePath"
                     :size="24"
                     weight="fill"
                     class="text-accent"
                  />
                  Game Executable
               </VueTypography>
               <VueTypography
                  variant="BodyR"
                  as="p"
                  class="text-muted-foreground col-span-8 break-all"
               >
                  {{ gameExecutablePath ?? 'Not set' }}
               </VueTypography>
               <div class="col-span-2 flex justify-end">
                  <VueButton type="button" @click="chooseGameExecutable">
                     <PhFolderOpen :size="24" weight="fill" />
                     Choose Executable
                  </VueButton>
               </div>
            </div>
         </div>

         <div class="bg-card flex w-full flex-col gap-4 rounded-lg border border-white/10 p-6">
            <VueTypography variant="H1B" as="h2">Preferences</VueTypography>

            <div class="flex flex-col gap-2" v-auto-animate>
               <div class="flex items-center justify-between gap-6">
                  <div>
                     <VueTypography variant="BodyB" as="h3">Reload mods in-game</VueTypography>
                     <VueTypography variant="CaptionR" as="p" class="text-muted-foreground">
                        Reloads your mods in-game when you toggle one.
                     </VueTypography>
                  </div>
                  <VueSwitch
                     :model-value="autoReload"
                     :title="autoReload ? 'Enabled' : 'Disabled'"
                     @update:model-value="setAutoReload"
                  />
               </div>
               <VueTypography
                  v-if="autoReloadError"
                  variant="CaptionR"
                  as="p"
                  class="text-destructive"
               >
                  {{ autoReloadError }}
               </VueTypography>
            </div>

            <div class="h-px w-full bg-white/10" />

            <div class="flex flex-col gap-2" v-auto-animate>
               <div class="flex items-center justify-between gap-6">
                  <div>
                     <VueTypography variant="BodyB" as="h3">Skip XXMI Launcher</VueTypography>
                     <VueTypography variant="CaptionR" as="p" class="text-muted-foreground">
                        Quick Launch starts the game directly instead of opening the XXMI launcher.
                     </VueTypography>
                  </div>
                  <VueSwitch
                     :model-value="skipXxmiLauncher"
                     :title="skipXxmiLauncher ? 'Enabled' : 'Disabled'"
                     @update:model-value="setSkipXxmiLauncher"
                  />
               </div>
               <VueTypography
                  v-if="skipXxmiLauncher && !gameExeIsXxmiLauncher"
                  variant="CaptionR"
                  as="p"
                  class="text-accent"
               >
                  Your Game Executable isn't XXMI Launcher, so this has no effect.
               </VueTypography>
            </div>
         </div>

         <div
            class="bg-card flex w-full flex-col gap-4 rounded-lg border border-white/10 p-6"
            v-auto-animate
         >
            <VueTypography variant="H1B" as="h2">Updates</VueTypography>

            <div v-auto-animate>
               <div class="flex items-center justify-between">
                  <div v-auto-animate>
                     <VueTypography variant="BodyR" as="p" class="text-white">
                        Currently running v{{ currentVersion }}
                     </VueTypography>
                     <div
                        v-if="!showUpdateModal && (updaterStore.errorMessage || noUpdateFound)"
                        v-auto-animate
                     >
                        <VueTypography
                           v-if="updaterStore.errorMessage"
                           variant="CaptionR"
                           as="p"
                           class="text-destructive"
                        >
                           {{ updaterStore.errorMessage }}
                        </VueTypography>
                        <VueTypography
                           v-else-if="noUpdateFound"
                           variant="CaptionR"
                           as="p"
                           class="text-muted-foreground"
                        >
                           You're up to date.
                        </VueTypography>
                     </div>
                  </div>
                  <div class="flex items-center justify-start gap-3" v-auto-animate>
                     <VueButton
                        v-if="!updaterStore.update"
                        type="button"
                        :disabled="updaterStore.isChecking"
                        @click="checkForUpdates"
                     >
                        <PhArrowsClockwise
                           v-if="!updaterStore.isChecking"
                           :size="24"
                           weight="fill"
                        />
                        <div
                           v-else
                           class="loader size-5 border-4! border-white! border-b-transparent!"
                        />
                        {{ updaterStore.isChecking ? 'Checking…' : 'Check for Updates' }}
                     </VueButton>
                     <VueButton v-else type="button" @click="showUpdateModal = true">
                        <PhBoxArrowDown :size="24" weight="fill" />
                        Update Available: v{{ updaterStore.update.version }}
                     </VueButton>
                  </div>
               </div>
            </div>

            <div v-if="changelog.length > 0" class="h-px w-full bg-white/10" />

            <div v-if="changelog.length > 0" class="flex w-full flex-col gap-4" v-auto-animate>
               <button
                  type="button"
                  class="hover:text-primary flex w-full cursor-pointer items-center justify-between gap-2 self-start text-white transition-colors"
                  @click="showChangelog = !showChangelog"
               >
                  <VueTypography variant="BodyR" as="span">Changelog</VueTypography>
                  <PhCaretRight
                     :size="20"
                     weight="bold"
                     class="transition-transform"
                     :class="{ 'rotate-90': showChangelog }"
                  />
               </button>

               <div v-if="showChangelog" class="flex max-h-100 flex-col gap-5 overflow-y-auto pr-2">
                  <div v-for="entry in changelog" :key="entry.version" class="flex flex-col gap-2">
                     <div class="flex items-center gap-2" v-auto-animate>
                        <VueTypography variant="CaptionB" as="h4">
                           v{{ entry.version }}
                        </VueTypography>
                        <span
                           v-if="entry.version === currentVersion"
                           class="bg-primary/85 rounded-full px-2 py-1 text-xs font-bold text-white"
                        >
                           current
                        </span>
                     </div>
                     <VueTypography
                        variant="CaptionR"
                        as="p"
                        class="text-foreground whitespace-pre-wrap"
                     >
                        {{ entry.body }}
                     </VueTypography>
                  </div>
               </div>
            </div>
         </div>
      </div>

      <UpdateModal v-if="showUpdateModal" @close="showUpdateModal = false" />
   </div>
</template>
