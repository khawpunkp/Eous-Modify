<script setup lang="ts">
import { onMounted, ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import VueButton from '@/components/ui/button/VueButton.vue';
import VueTypography from '@/components/ui/typography/VueTypography.vue';
import { formatKeybind } from '../../utils/keybind';
import type { KeybindInfo } from '../../types';

const props = defineProps<{ modId: number }>();
const emit = defineEmits<{ close: [] }>();

const keybinds = ref<KeybindInfo[]>([]);
const isLoading = ref(true);
const errorMessage = ref<string | null>(null);

onMounted(async () => {
   try {
      keybinds.value = await invoke<KeybindInfo[]>('get_mod_keybinds', { modId: props.modId });
   } catch (e) {
      errorMessage.value = String(e);
   } finally {
      isLoading.value = false;
   }
});
</script>

<template>
   <div class="fixed inset-0 z-100 flex items-center justify-center bg-black/60">
      <div
         class="bg-card max-h-[85vh] w-11/12 max-w-120 overflow-y-auto rounded-lg border border-white/10 p-6"
      >
         <VueTypography variant="TitleB" as="h2" class="mb-2">Keybinds</VueTypography>

         <template v-if="errorMessage">
            <VueTypography variant="CaptionR" as="p" class="text-destructive">
               {{ errorMessage }}
            </VueTypography>
         </template>
         <template v-else>
            <div v-if="keybinds.length === 0" class="flex flex-1 items-center justify-center">
               <img src="/images/no-data.png" class="w-50" />
            </div>

            <ul v-else v-auto-animate class="mt-2 mb-5 flex flex-col gap-2">
               <li
                  v-for="kb in keybinds"
                  :key="kb.title"
                  class="flex items-center justify-between rounded-md bg-white/5 px-3 py-2 text-[13px]"
               >
                  <span>{{ kb.title }}</span>
                  <span class="rounded bg-black/30 px-2 py-1 font-mono">
                     {{ formatKeybind(kb.key) }}
                  </span>
               </li>
            </ul>
         </template>

         <div class="flex items-center justify-end">
            <VueButton type="button" variant="outlined" @click="emit('close')" class="min-w-32">
               Close
            </VueButton>
         </div>
      </div>
   </div>
</template>
