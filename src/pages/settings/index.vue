<script setup lang="ts">
import { onMounted, ref } from 'vue';
import { open } from '@tauri-apps/plugin-dialog';
import { getVersion } from '@tauri-apps/api/app';
import { PhBoxArrowDown, PhFolderOpen, PhGear, PhWarning } from '@phosphor-icons/vue';
import VueButton from '@/components/ui/button/VueButton.vue';
import VueTypography from '@/components/ui/typography/VueTypography.vue';
import { useSettingsStore } from '../../stores/settings';
import { useUpdaterStore } from '../../stores/updater';
import UpdateModal from '../../components/UpdateModal.vue';

const settingsStore = useSettingsStore();
const updaterStore = useUpdaterStore();

const modsFolderPath = ref<string | null>(null);
const gameExecutablePath = ref<string | null>(null);

const currentVersion = ref<string>('');
const showUpdateModal = ref(false);
const noUpdateFound = ref(false);

onMounted(async () => {
   modsFolderPath.value = await settingsStore.fetch('mods_folder_path');
   gameExecutablePath.value = await settingsStore.fetch('game_executable_path');
   currentVersion.value = await getVersion();
   await updaterStore.check();
   //  if (Boolean(updaterStore.update)) showUpdateModal.value = true;
});

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
   <div>
      <div class="mb-6 flex items-center border-b border-white/10 pb-4">
         <VueTypography variant="H1B" as="h1" class="flex h-12 items-center gap-3">
            <PhGear :size="32" weight="fill" />
            Settings
         </VueTypography>
      </div>

      <div class="flex flex-col gap-5">
         <div class="bg-card w-full rounded-lg border border-white/10 p-6">
            <VueTypography variant="TitleB" as="h2" class="mb-2">Paths Configuration</VueTypography>

            <div class="mb-5 grid grid-cols-12 items-center border-b border-white/5 pb-5">
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

         <div class="bg-card w-full rounded-lg border border-white/10 p-6">
            <div class="flex items-center justify-between">
               <div>
                  <VueTypography variant="TitleB" as="h2" class="mb-2">Updates</VueTypography>
                  <VueTypography variant="BodyR" as="p" class="text-muted-foreground">
                     Currently running v{{ currentVersion }}
                  </VueTypography>
               </div>
               <div class="flex items-center justify-start gap-3">
                  <VueButton
                     v-if="!updaterStore.update"
                     type="button"
                     :disabled="updaterStore.isChecking"
                     @click="checkForUpdates"
                  >
                     {{ updaterStore.isChecking ? 'Checking…' : 'Check for Updates' }}
                  </VueButton>
                  <VueButton v-else type="button" @click="showUpdateModal = true">
                     <PhBoxArrowDown :size="24" weight="fill" />
                     Update Available: v{{ updaterStore.update.version }}
                  </VueButton>
               </div>
            </div>
            <div v-if="!showUpdateModal" class="mt-2">
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
      </div>

      <UpdateModal v-if="showUpdateModal" @close="showUpdateModal = false" />
   </div>
</template>
