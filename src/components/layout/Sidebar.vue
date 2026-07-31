<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { open } from '@tauri-apps/plugin-dialog';
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { ArchiveAnalysis } from '../../types';
import {
   PhUsers,
   PhGear,
   PhPlay,
   PhFileArrowDown,
   PhFolderOpen,
   PhDotsThreeCircle,
   PhArrowsClockwise,
} from '@phosphor-icons/vue';
import VueButton from '@/components/ui/button/VueButton.vue';
import VueTypography from '@/components/ui/typography/VueTypography.vue';
import { useSettingsStore } from '../../stores/settings';
import { useUpdaterStore } from '../../stores/updater';
import { CATEGORY_ICONS } from '../../utils/category';
import ImportModal from '../mods/ImportModal.vue';

const navItems = [
   { label: 'Agents', to: '/agents', icon: PhUsers },
   { label: 'UI', to: '/categories/ui', icon: CATEGORY_ICONS.ui },
   { label: 'NPCs', to: '/categories/npcs', icon: CATEGORY_ICONS.npcs },
   { label: 'Enemies', to: '/categories/enemies', icon: CATEGORY_ICONS.enemies },
   { label: 'Weapons', to: '/categories/weapons', icon: CATEGORY_ICONS.weapons },
   { label: 'Objects', to: '/categories/objects', icon: CATEGORY_ICONS.objects },
   { label: 'Other', to: '/other', icon: PhDotsThreeCircle },
   { label: 'Settings', to: '/settings', icon: PhGear },
];

const route = useRoute();
const router = useRouter();
const settingsStore = useSettingsStore();
const updaterStore = useUpdaterStore();

function isActive(path: string) {
   return route.path === path || route.path.startsWith(path + '/');
}

// Read straight off the store rather than snapshotting into local refs: the Settings page writes
// through `settingsStore.set`, so a copy taken at mount would go stale — leaving the nav disabled
// (and the user stranded on Settings) even after they'd filled both paths in.
const gameExecutablePath = computed(() => settingsStore.settings.game_executable_path ?? null);
const modsFolderPath = computed(() => settingsStore.settings.mods_folder_path ?? null);
const isLaunching = ref(false);
const launchError = ref<string | null>(null);
const isImporting = ref(false);
const isAnalyzingImport = ref(false);
const importArchivePath = ref<string | null>(null);
const importAnalysis = ref<ArchiveAnalysis | null>(null);
const importError = ref<string | null>(null);
const openFolderError = ref<string | null>(null);
const isScanning = ref(false);
const scanMessage = ref<string | null>(null);
const scanError = ref<string | null>(null);
const showScanResult = ref(false);

let unlistenProgress: UnlistenFn | null = null;
let unlistenComplete: UnlistenFn | null = null;
let unlistenError: UnlistenFn | null = null;

onMounted(async () => {
   // Populates the store the computeds above read from.
   await Promise.all([
      settingsStore.fetch('game_executable_path'),
      settingsStore.fetch('mods_folder_path'),
   ]);

   unlistenProgress = await listen<{ message: string }>('scan-progress', (event) => {
      scanMessage.value = event.payload.message;
   });
   unlistenComplete = await listen<string>('scan-complete', (event) => {
      scanMessage.value = event.payload;
      isScanning.value = false;
      showScanResult.value = true;
   });
   unlistenError = await listen<string>('scan-error', (event) => {
      scanError.value = event.payload;
      isScanning.value = false;
      showScanResult.value = true;
   });
});

onUnmounted(() => {
   unlistenProgress?.();
   unlistenComplete?.();
   unlistenError?.();
});

async function launchGame() {
   isLaunching.value = true;
   launchError.value = null;
   try {
      await invoke('launch_game');
   } catch (e) {
      launchError.value = String(e);
   } finally {
      isLaunching.value = false;
   }
}

async function startImport() {
   importError.value = null;
   const path = await open({
      multiple: false,
      filters: [{ name: 'Mod archive', extensions: ['zip', '7z', 'rar'] }],
   });
   if (typeof path !== 'string') return;

   isAnalyzingImport.value = true;
   try {
      importAnalysis.value = await invoke<ArchiveAnalysis>('analyze_archive', {
         archivePath: path,
      });
      importArchivePath.value = path;
      isImporting.value = true;
   } catch (e) {
      importError.value = String(e);
   } finally {
      isAnalyzingImport.value = false;
   }
}

function closeImport() {
   isImporting.value = false;
   importArchivePath.value = null;
   importAnalysis.value = null;
}

async function openModsFolder() {
   openFolderError.value = null;
   try {
      await invoke('open_mods_folder');
   } catch (e) {
      openFolderError.value = String(e);
   }
}

async function runScan() {
   if (!modsFolderPath.value) {
      scanError.value = 'Set a mods folder in Settings first.';
      showScanResult.value = true;
      return;
   }
   isScanning.value = true;
   scanError.value = null;
   scanMessage.value = null;
   try {
      const summary = await invoke<string>('scan_mods_directory');
      scanMessage.value = summary;
      showScanResult.value = true;
   } catch (e) {
      scanError.value = String(e);
      showScanResult.value = true;
   } finally {
      isScanning.value = false;
   }
}
</script>

<template>
   <aside class="bg-card flex h-full max-w-65 shrink-0 grow flex-col overflow-y-auto p-5">
      <div class="mb-8 flex items-center justify-center">
         <img src="/images/logo.webp" class="w-6/10" />
      </div>
      <div class="flex flex-col border-b border-white/10 pb-4">
         <VueButton
            class="w-full justify-center"
            :disabled="isLaunching || !gameExecutablePath"
            @click="launchGame"
         >
            <PhPlay :size="24" weight="fill" />
            {{ isLaunching ? 'Launching…' : 'Quick Launch' }}
         </VueButton>
         <VueTypography
            v-if="launchError"
            variant="CaptionR"
            as="p"
            class="text-destructive mt-2 mb-2"
         >
            {{ launchError }}
         </VueTypography>

         <VueButton
            variant="outlined"
            class="mt-4 w-full justify-center"
            :disabled="isAnalyzingImport || !modsFolderPath || !gameExecutablePath"
            @click="startImport"
         >
            <PhFileArrowDown :size="24" weight="fill" />
            {{ isAnalyzingImport ? 'Analyzing…' : 'Import Mod' }}
         </VueButton>
         <VueTypography v-if="importError" variant="CaptionR" as="p" class="text-destructive mt-2">
            {{ importError }}
         </VueTypography>
      </div>
      <ul class="mt-4 grow list-none border-b border-white/10 pb-2">
         <li v-for="item in navItems" :key="item.to" class="mb-2">
            <VueButton
               type="button"
               variant="ghost"
               color="gray"
               class="w-full justify-start gap-3 rounded-lg px-4 py-3"
               :class="isActive(item.to) ? 'bg-primary text-white' : 'hover:bg-primary/10'"
               @click="router.push(item.to)"
               :disabled="!modsFolderPath || !gameExecutablePath"
            >
               <component :is="item.icon" :size="24" weight="fill" />
               {{ item.label }}
               <span
                  v-if="item.to === '/settings' && updaterStore.update"
                  class="bg-accent ml-auto size-2 rounded-full"
                  title="Update available"
               />
            </VueButton>
         </li>
      </ul>

      <VueButton
         :disabled="!modsFolderPath"
         class="mt-4 w-full justify-center"
         @click="openModsFolder"
      >
         <PhFolderOpen :size="24" weight="fill" />
         Open Mods Folder
      </VueButton>
      <VueTypography v-if="openFolderError" variant="CaptionR" as="p" class="text-destructive mt-2">
         {{ openFolderError }}
      </VueTypography>
      <VueButton
         variant="outlined"
         class="mt-4 w-full justify-center"
         :disabled="isScanning || !modsFolderPath"
         @click="runScan"
      >
         <PhArrowsClockwise :size="24" weight="fill" />
         {{ isScanning ? 'Scanning…' : 'Scan Mods Folder' }}
      </VueButton>

      <ImportModal
         v-if="isImporting && importArchivePath && importAnalysis"
         :archive-path="importArchivePath"
         :analysis="importAnalysis"
         @imported="closeImport"
         @close="closeImport"
      />

      <div
         v-if="showScanResult"
         class="fixed inset-0 z-100 flex items-center justify-center bg-black/60"
      >
         <div
            class="bg-card max-h-[85vh] w-11/12 max-w-120 overflow-y-auto rounded-lg border border-white/10 p-6"
         >
            <VueTypography variant="TitleB" as="h2" class="mb-4">Scan Result</VueTypography>
            <VueTypography
               v-if="scanMessage && !scanError"
               variant="BodyB"
               as="p"
               class="text-muted-foreground"
            >
               {{ scanMessage }}
            </VueTypography>
            <VueTypography v-if="scanError" variant="CaptionR" as="p" class="text-destructive">
               {{ scanError }}
            </VueTypography>
            <div class="mt-5 flex justify-end">
               <VueButton type="button" @click="showScanResult = false" class="min-w-32">
                  Close
               </VueButton>
            </div>
         </div>
      </div>
   </aside>
</template>
