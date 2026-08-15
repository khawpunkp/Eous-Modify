<script setup lang="ts">
import { computed, onMounted, ref } from 'vue';
import { open } from '@tauri-apps/plugin-dialog';
import { invoke } from '@tauri-apps/api/core';
import { getVersion } from '@tauri-apps/api/app';
import {
   PhArrowsClockwise,
   PhBoxArrowDown,
   PhCaretRight,
   PhFolderOpen,
   PhGear,
   PhShieldCheck,
   PhWarning,
} from '@phosphor-icons/vue';
import VueButton from '@/components/ui/button/VueButton.vue';
import VueTypography from '@/components/ui/typography/VueTypography.vue';
import VueSwitch from '@/components/ui/switch/VueSwitch.vue';
import { useSettingsStore } from '../../stores/settings';
import { AUTO_RELOAD_KEY, RELOAD_METHOD_KEY, type ReloadMethod } from '../../utils/reload';
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
   reloadMethod.value =
      (await settingsStore.fetch(RELOAD_METHOD_KEY)) === 'immediate' ? 'immediate' : 'deferred';
   isElevated.value = await invoke<boolean>('is_elevated');
   skipXxmiLauncher.value = (await settingsStore.fetch(SKIP_XXMI_LAUNCHER_KEY)) === 'true';
   await updaterStore.check();
   if (Boolean(updaterStore.update)) showUpdateModal.value = true;
});

const autoReload = ref(false);

const skipXxmiLauncher = ref(false);
// The backend applies the launcher-only flags just for XXMI Launcher, so surface when the configured
// executable means this switch does nothing — an "Enabled" switch that silently no-ops is worse than
// one that says why.
const gameExeIsXxmiLauncher = computed(() => isXxmiLauncherPath(gameExecutablePath.value));

async function setSkipXxmiLauncher(enabled: boolean) {
   skipXxmiLauncher.value = enabled;
   await settingsStore.set(SKIP_XXMI_LAUNCHER_KEY, String(enabled));
}

const reloadMethod = ref<ReloadMethod>('deferred');

// Both halves of the trade, said plainly at the moment of choosing. The cost of each is the part
// that matters: one asks for administrator, the other lets every mod keybind loose, and neither is
// something the user can discover on their own after the fact.
const RELOAD_METHODS: { value: ReloadMethod; label: string; detail: string }[] = [
   {
      value: 'deferred',
      label: 'When you switch back to the game',
      detail: "Runs as administrator. Your mods' keybinds stay in the game.",
   },
   {
      value: 'immediate',
      label: 'Straight away',
      detail: "No administrator. Your mods' keybinds will also fire while you type in other apps.",
   },
];

// Deferred delivery needs Eous elevated: Windows won't let an ordinary process send a keypress into
// the elevated game, and reports success anyway, so the failure is invisible. Startup handles it
// once the setting is on — this covers the gap in between, when it has just been switched on and
// the running copy still isn't elevated.
const isElevated = ref(true);
const relaunchError = ref<string | null>(null);
const reloadError = ref<string | null>(null);

const needsAdmin = computed(
   () => autoReload.value && reloadMethod.value === 'deferred' && !isElevated.value,
);

// One call for both controls. Immediate mode edits the user's d3dx.ini and deferred mode puts it
// back, so the switch and the method decide the file together — writing them separately would leave
// a moment where whichever landed second won.
async function applyReloadConfig(enabled: boolean, method: ReloadMethod) {
   reloadError.value = null;
   relaunchError.value = null;
   try {
      await invoke('set_reload_config', { enabled, method });
   } catch (e) {
      reloadError.value = String(e);
      return;
   }

   autoReload.value = enabled;
   reloadMethod.value = method;
   // Cached by hand because this bypasses settingsStore.set — the command above writes both keys
   // itself, and the toggle path reads the switch straight out of this store.
   settingsStore.settings[AUTO_RELOAD_KEY] = String(enabled);
   settingsStore.settings[RELOAD_METHOD_KEY] = method;
}

// Succeeds by closing this window — the elevated copy takes over — so there is nothing to do after it
// but report a refusal.
async function relaunchAsAdmin() {
   relaunchError.value = null;
   try {
      await invoke('relaunch_as_admin');
   } catch (e) {
      relaunchError.value = String(e);
   }
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
   await updaterStore.check();
   // Only when the check came back empty. Without the first guard a successful find still reported
   // "You're up to date." — next to a button offering the version it had just found.
   if (!updaterStore.update && !updaterStore.errorMessage) {
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
                     @update:model-value="applyReloadConfig($event, reloadMethod)"
                  />
               </div>

               <div v-if="autoReload" class="flex flex-col gap-2 pt-1">
                  <button
                     v-for="option in RELOAD_METHODS"
                     :key="option.value"
                     type="button"
                     class="flex cursor-pointer flex-col gap-1 rounded-lg border p-3 text-left transition-colors"
                     :class="
                        reloadMethod === option.value
                           ? 'border-primary bg-primary/10'
                           : 'border-white/10 bg-white/5 hover:border-white/25'
                     "
                     @click="applyReloadConfig(autoReload, option.value)"
                  >
                     <VueTypography variant="CaptionB" as="span">{{ option.label }}</VueTypography>
                     <VueTypography variant="CaptionR" as="span" class="text-muted-foreground">
                        {{ option.detail }}
                     </VueTypography>
                  </button>
               </div>

               <div v-if="needsAdmin" class="flex items-center gap-3">
                  <VueButton type="button" variant="outlined" size="sm" @click="relaunchAsAdmin">
                     <PhShieldCheck :size="20" weight="fill" />
                     Restart as administrator
                  </VueButton>
                  <VueTypography variant="CaptionR" as="p" class="text-accent">
                     Your mods won't reload in-game until you do.
                  </VueTypography>
               </div>

               <VueTypography
                  v-if="reloadError || relaunchError"
                  variant="CaptionR"
                  as="p"
                  class="text-destructive"
               >
                  {{ reloadError ?? relaunchError }}
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
                        variant="CaptionL"
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
