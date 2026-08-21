<script setup lang="ts">
import { computed, onMounted, reactive, ref } from 'vue';
import { open } from '@tauri-apps/plugin-dialog';
import { invoke } from '@tauri-apps/api/core';
import { PhTrash } from '@phosphor-icons/vue';
import VueButton from '@/components/ui/button/VueButton.vue';
import VueInput from '@/components/ui/input/VueInput.vue';
import { VueSelect } from '@/components/ui/select';
import { useImageInput } from '@/composables/imageInput';
import VueTypography from '@/components/ui/typography/VueTypography.vue';
import PreviewImage from './PreviewImage.vue';
import { useAgentsStore } from '../../stores/agents';
import { useCategoriesStore } from '../../stores/categories';
import type { Mod, ModInput } from '../../types';

const props = defineProps<{ mod: Mod }>();
const emit = defineEmits<{
   submit: [input: ModInput];
   recategorize: [target: { agentId?: number; categoryId?: number }];
   close: [];
}>();

const agentsStore = useAgentsStore();
const categoriesStore = useCategoriesStore();

const form = reactive({
   name: props.mod.name,
   author: props.mod.author ?? '',
});
// Shown in the preview; distinct from newImageDataUrl so opening/saving without touching the
// image doesn't resend the existing preview as if it were a fresh one.
const previewSrc = ref<string | null>(null);
const newImageDataUrl = ref<string | null>(null);
// Set by the delete affordance, cleared by picking or dropping a replacement. Applied on save, like
// every other field here, so Cancel still backs out of it.
const clearImage = ref(false);

/**
 * Whether there is an image of ours to remove.
 *
 * Only what this app saved counts — a `preview.png` the mod shipped with belongs to its author, is
 * never deleted, and is exactly what removing ours falls back to, so offering to remove it would be
 * a button that appears to do nothing.
 */
const canClearImage = computed(() => {
   // Already cleared: what's on screen is the default now, so offering to remove it again would be a
   // button that does nothing. Picking or dropping a replacement brings it back.
   if (clearImage.value) return false;
   return (
      newImageDataUrl.value !== null || Boolean(props.mod.imageFilename?.startsWith('mod_preview.'))
   );
});

/** Belonging to no agent and no category — what the Other/Misc page lists. */
const NO_TARGET = 'none';

const currentTarget =
   props.mod.agentId !== null
      ? `agent:${props.mod.agentId}`
      : props.mod.categoryId !== null
        ? `category:${props.mod.categoryId}`
        : NO_TARGET;
const selectedTarget = ref(currentTarget);

const canMove = computed(() => selectedTarget.value !== currentTarget);

// Other/Misc is offered like any other destination. It used to be the empty string — a current value
// with no option to match it — so a mod could be moved out of Other/Misc but never back into it,
// which became the only way out of a category once four of them were retired.
const categoryOptions = computed(() => [
   ...agentsStore.agents.map((agent) => ({
      label: agent.name,
      value: `agent:${agent.id}`,
   })),
   ...categoriesStore.categories.map((category) => ({
      label: category.name,
      value: `category:${category.id}`,
   })),
   { label: 'Other/Misc', value: NO_TARGET },
]);

onMounted(async () => {
   if (agentsStore.agents.length === 0) agentsStore.fetchAll();
   if (categoriesStore.categories.length === 0) categoriesStore.fetchAll();

   // Resolved backend-side, same reason as ModCard: a disabled mod's folder carries the DISABLED_
   // prefix, which a path built from `mod.folderName` here would miss.
   if (props.mod.imageFilename) {
      try {
         previewSrc.value = await invoke<string | null>('get_mod_preview', { modId: props.mod.id });
      } catch {
         previewSrc.value = null;
      }
   }
});

function applyImage(dataUrl: string) {
   newImageDataUrl.value = dataUrl;
   previewSrc.value = dataUrl;
   clearImage.value = false;
}

// Shows what saving would actually produce — the mod's own image where it ships one — rather than a
// placeholder that misrepresents the result.
async function useDefaultImage() {
   newImageDataUrl.value = null;
   previewSrc.value = null;
   clearImage.value = true;
   try {
      previewSrc.value = await invoke<string | null>('get_mod_default_preview', {
         modId: props.mod.id,
      });
   } catch {
      previewSrc.value = null;
   }
}

async function pickImage() {
   const path = await open({
      multiple: false,
      filters: [{ name: 'Images', extensions: ['png', 'jpg', 'jpeg', 'webp', 'gif'] }],
   });
   if (typeof path === 'string') {
      clearDropError();
      applyImage(await invoke<string>('read_image_as_data_url', { path }));
   }
}

// A dropped file and a picked one both arrive as a data URL, so they share applyImage.
const {
   isDraggingOver,
   errorMessage: dropError,
   clearError: clearDropError,
} = useImageInput(applyImage);

function handleSubmit() {
   emit('submit', {
      name: form.name.trim(),
      author: form.author.trim() || null,
      imageDataUrl: newImageDataUrl.value,
      clearImage: clearImage.value,
   });

   if (canMove.value) {
      if (selectedTarget.value === NO_TARGET) {
         // Neither id set: the backend reads that as uncategorised and moves the folder accordingly.
         emit('recategorize', {});
      } else {
         const [kind, idStr] = selectedTarget.value.split(':');
         const id = Number(idStr);
         emit('recategorize', kind === 'agent' ? { agentId: id } : { categoryId: id });
      }
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
            <div class="flex w-full flex-col items-center gap-2" v-auto-animate>
               <div class="group relative w-full">
                  <PreviewImage
                     :src="previewSrc ?? '/images/no-data.png'"
                     class="aspect-4/3 rounded-lg border transition-colors"
                     :class="isDraggingOver ? 'border-primary border-2' : 'border-white/10'"
                  />
                  <button
                     v-if="canClearImage"
                     type="button"
                     class="bg-background/80 text-foreground/70 hover:text-destructive absolute top-2 right-2 cursor-pointer rounded-full p-2 opacity-0 transition-all group-hover:opacity-100"
                     title="Use default image"
                     @click="useDefaultImage"
                  >
                     <PhTrash :size="20" weight="fill" />
                  </button>
               </div>
               <VueTypography
                  variant="CaptionR"
                  as="p"
                  class="text-muted-foreground w-full text-center"
               >
                  {{ isDraggingOver ? 'Drop to use this image' : 'Drop an image here, or' }}
               </VueTypography>
               <VueButton type="button" variant="outlined" size="sm" @click="pickImage">
                  Choose New Image
               </VueButton>
               <!-- Last child on purpose. auto-animate translates every sibling an insertion
                    displaces, so an error added mid-column animates the whole form; added at the end
                    it displaces nothing and only fades in itself. -->
               <VueTypography
                  v-if="dropError"
                  variant="CaptionR"
                  as="p"
                  class="text-destructive w-full text-center"
               >
                  {{ dropError }}
               </VueTypography>
            </div>
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
