<script setup lang="ts">
import { ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import {
   PhPencilSimple,
   PhArrowsClockwise,
   PhFolderOpen,
   PhKeyboard,
   PhTrash,
} from '@phosphor-icons/vue';
import { confirmAction } from '@/composables/confirm';
import VueCard from '@/components/ui/card/VueCard.vue';
import VueTypography from '@/components/ui/typography/VueTypography.vue';
import PreviewImage from './PreviewImage.vue';
import VueSwitch from '@/components/ui/switch/VueSwitch.vue';
import VueCheckbox from '@/components/ui/checkbox/VueCheckbox.vue';
import { useModsStore } from '@/stores/mods';
import type { Mod } from '@/types';

const props = defineProps<{
   mod: Mod;
   selectMode?: boolean;
   selected?: boolean;
}>();
const emit = defineEmits<{
   edit: [mod: Mod];
   delete: [mod: Mod];
   keybinds: [mod: Mod];
   'toggle-select': [mod: Mod];
}>();

const modsStore = useModsStore();
const imageSrc = ref<string | null>(null);

// Resolved backend-side: a disabled mod's folder carries the DISABLED_ prefix, so a path built here
// from `mod.folderName` (which is always the enabled name) would miss and show the placeholder.
async function loadPreview() {
   if (!props.mod.imageFilename) {
      imageSrc.value = null;
      return;
   }
   try {
      imageSrc.value = await invoke<string | null>('get_mod_preview', { modId: props.mod.id });
   } catch {
      imageSrc.value = null;
   }
}

// Every reason the file behind this card can change:
//  - id: the card was reused for a different mod
//  - imageFilename: a preview was added, removed, or saved with a different extension
//  - isEnabled: the folder was renamed, so the image lives at a different path
//  - previewVersion: a preview was overwritten keeping the same name, which none of the above shows
watch(
   () => [
      props.mod.id,
      props.mod.imageFilename,
      props.mod.isEnabled,
      modsStore.previewVersion[props.mod.id],
   ],
   loadPreview,
   { immediate: true },
);

function toggle() {
   modsStore.toggle(props.mod.id);
}

function openFolder() {
   modsStore.openFolder(props.mod.id);
}

const ARCHIVE_EXTENSIONS = ['zip', '7z', 'rar'];

/**
 * Picks a file and puts it into this mod: an archive is a new version and takes over the whole
 * folder, anything else goes in over a file of the same name.
 *
 * The picker deliberately carries no extension filter. A loose file could be any of the dozen things
 * a mod is made of, so any list would be a guess that hides whatever the user actually came for.
 * Which of the two happens is the backend's decision — the check here only chooses how to word the
 * question.
 */
async function updateFiles() {
   const path = await open({ multiple: false });
   if (typeof path !== 'string') return;

   const fileName = path.slice(Math.max(path.lastIndexOf('/'), path.lastIndexOf('\\')) + 1);
   const isArchive = ARCHIVE_EXTENSIONS.includes(fileName.split('.').pop()?.toLowerCase() ?? '');

   const confirmed = await confirmAction(
      isArchive
         ? {
              title: `Update "${props.mod.name}" from ${fileName}?`,
              message:
                 'Everything in the mod folder is replaced by what the archive holds. The mod keeps its name, category, group and its own preview picture.',
              confirmLabel: 'Update',
              destructive: true,
           }
         : {
              title: `Put ${fileName} into "${props.mod.name}"?`,
              message:
                 'A file of that name already in the mod folder is overwritten. Nothing else is touched.',
              confirmLabel: 'Add file',
           },
   );
   if (!confirmed) return;

   try {
      await modsStore.updateFiles(props.mod.id, path);
   } catch (e) {
      await confirmAction({
         title: 'Update failed',
         message: String(e),
         confirmLabel: 'OK',
         acknowledgeOnly: true,
      });
   }
}
</script>

<template>
   <!-- min-w-0: a grid item will not shrink below its own min-content width, so one unbroken mod name
        would otherwise stretch its whole column and knock the grid out of alignment. -->
   <VueCard
      class="relative flex min-w-0 flex-col gap-4 p-4 transition-all"
      :class="[
         !mod.isEnabled && 'opacity-50',
         selectMode && selected && 'outline-primary outline-2',
      ]"
      @click="selectMode && emit('toggle-select', mod)"
      v-auto-animate
   >
      <VueCheckbox v-if="selectMode" :model-value="selected" class="absolute top-2 left-2 z-10" />
      <PreviewImage :src="imageSrc ?? '/images/no-data.png'" class="aspect-4/3 rounded-sm" />
      <div class="flex min-w-0 flex-1 flex-col gap-1" v-auto-animate>
         <!-- wrap-anywhere rather than break-words: mod names arrive straight from folder names and
              are regularly one long unpunctuated run, which break-words refuses to split. -->
         <VueTypography variant="BodyB" class="wrap-anywhere">{{ mod.name }}</VueTypography>
         <VueTypography
            v-if="mod.author"
            variant="CaptionR"
            as="div"
            class="text-muted-foreground wrap-anywhere"
         >
            by {{ mod.author }}
         </VueTypography>
      </div>
      <div v-if="!selectMode" class="flex items-center gap-2">
         <VueSwitch
            :model-value="mod.isEnabled"
            :title="mod.isEnabled ? 'Enabled' : 'Disabled'"
            class="mr-auto"
            @update:model-value="toggle"
         />
         <button
            type="button"
            class="text-foreground cursor-pointer p-1 opacity-70 transition-all hover:opacity-100"
            title="Edit"
            @click="emit('edit', mod)"
         >
            <PhPencilSimple :size="20" weight="fill" />
         </button>
         <button
            type="button"
            class="text-foreground cursor-pointer p-1 opacity-70 transition-all hover:opacity-100"
            title="Update files"
            @click="updateFiles"
         >
            <PhArrowsClockwise :size="20" weight="fill" />
         </button>
         <button
            type="button"
            class="text-foreground cursor-pointer p-1 opacity-70 transition-all hover:opacity-100"
            title="Open folder"
            @click="openFolder"
         >
            <PhFolderOpen :size="20" weight="fill" />
         </button>
         <button
            type="button"
            class="text-foreground cursor-pointer p-1 opacity-70 transition-all hover:opacity-100"
            title="Keybinds"
            @click="emit('keybinds', mod)"
         >
            <PhKeyboard :size="20" weight="fill" />
         </button>
         <button
            type="button"
            class="text-destructive cursor-pointer p-1 opacity-70 transition-all hover:opacity-100"
            title="Delete"
            @click="emit('delete', mod)"
         >
            <PhTrash :size="20" weight="fill" />
         </button>
      </div>
   </VueCard>
</template>
