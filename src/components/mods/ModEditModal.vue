<script setup lang="ts">
import { computed, onMounted, reactive, ref } from 'vue';
import { open } from '@tauri-apps/plugin-dialog';
import { invoke } from '@tauri-apps/api/core';
import VueButton from '@/components/ui/button/VueButton.vue';
import VueInput from '@/components/ui/input/VueInput.vue';
import { VueSelect } from '@/components/ui/select';
import VueTypography from '@/components/ui/typography/VueTypography.vue';
import { useAgentsStore } from '../../stores/agents';
import { useCategoriesStore } from '../../stores/categories';
import { useSettingsStore } from '../../stores/settings';
import type { Mod, ModInput } from '../../types';

const props = defineProps<{ mod: Mod }>();
const emit = defineEmits<{
   submit: [input: ModInput];
   recategorize: [target: { agentId?: number; categoryId?: number }];
   close: [];
}>();

const agentsStore = useAgentsStore();
const categoriesStore = useCategoriesStore();
const settingsStore = useSettingsStore();

const form = reactive({
   name: props.mod.name,
   author: props.mod.author ?? '',
});
// Shown in the preview; distinct from newImageDataUrl so opening/saving without touching the
// image doesn't resend the existing preview as if it were a fresh one.
const previewSrc = ref<string | null>(null);
const newImageDataUrl = ref<string | null>(null);

const currentTarget =
   props.mod.agentId !== null
      ? `agent:${props.mod.agentId}`
      : props.mod.categoryId !== null
        ? `category:${props.mod.categoryId}`
        : '';
const selectedTarget = ref(currentTarget);

const canMove = computed(
   () => selectedTarget.value !== '' && selectedTarget.value !== currentTarget,
);

const categoryOptions = computed(() => [
   ...agentsStore.agents.map((agent) => ({
      label: `Character: ${agent.name}`,
      value: `agent:${agent.id}`,
   })),
   ...categoriesStore.categories.map((category) => ({
      label: `Category: ${category.name}`,
      value: `category:${category.id}`,
   })),
]);

onMounted(async () => {
   if (agentsStore.agents.length === 0) agentsStore.fetchAll();
   if (categoriesStore.categories.length === 0) categoriesStore.fetchAll();

   if (props.mod.imageFilename) {
      const modsFolderPath =
         settingsStore.settings.mods_folder_path ?? (await settingsStore.fetch('mods_folder_path'));
      if (modsFolderPath) {
         const fullPath = `${modsFolderPath}/${props.mod.folderName}/${props.mod.imageFilename}`;
         try {
            previewSrc.value = await invoke<string>('read_image_as_data_url', { path: fullPath });
         } catch {
            previewSrc.value = null;
         }
      }
   }
});

async function pickImage() {
   const path = await open({
      multiple: false,
      filters: [{ name: 'Images', extensions: ['png', 'jpg', 'jpeg', 'webp', 'gif'] }],
   });
   if (typeof path === 'string') {
      const dataUrl = await invoke<string>('read_image_as_data_url', { path });
      newImageDataUrl.value = dataUrl;
      previewSrc.value = dataUrl;
   }
}

function handleSubmit() {
   emit('submit', {
      name: form.name.trim(),
      author: form.author.trim() || null,
      imageDataUrl: newImageDataUrl.value,
   });

   if (canMove.value) {
      const [kind, idStr] = selectedTarget.value.split(':');
      const id = Number(idStr);
      emit('recategorize', kind === 'agent' ? { agentId: id } : { categoryId: id });
   }
}
</script>

<template>
   <div class="fixed inset-0 z-100 flex items-center justify-center bg-black/60">
      <form
         class="bg-card flex max-h-[85vh] w-11/12 max-w-120 flex-col gap-4 overflow-y-auto rounded-lg border border-white/10 p-6"
         @submit.prevent="handleSubmit"
      >
         <VueTypography variant="TitleB" as="h2">Edit Mod</VueTypography>
         <div class="flex flex-col items-center gap-4">
            <img
               :src="previewSrc ?? '/images/placeholder.jpg'"
               alt=""
               class="aspect-video w-full rounded-lg border border-white/10 object-cover"
            />
            <VueButton type="button" variant="outlined" size="sm" @click="pickImage">
               Choose New Image
            </VueButton>

            <VueInput
               id="mod-name"
               v-model="form.name"
               label="Name"
               required
               container-class="w-full"
            />

            <VueInput
               id="mod-author"
               v-model="form.author"
               label="Author"
               container-class="w-full"
            />

            <VueSelect
               v-model="selectedTarget"
               label="Category"
               :options="categoryOptions"
               placeholder="Uncategorized"
               searchable
            />

            <div class="flex w-full items-center justify-end gap-3">
               <VueButton type="button" variant="outlined" @click="emit('close')" class="min-w-32">
                  Cancel
               </VueButton>
               <VueButton type="submit" class="min-w-32" :disabled="!form.name.trim()">
                  Save
               </VueButton>
            </div>
         </div>
      </form>
   </div>
</template>
