<script setup lang="ts">
import { onMounted, ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { PhPencilSimple, PhFolderOpen, PhKeyboard, PhTrash } from '@phosphor-icons/vue';
import VueCard from '@/components/ui/card/VueCard.vue';
import VueTypography from '@/components/ui/typography/VueTypography.vue';
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
onMounted(async () => {
   if (!props.mod.imageFilename) return;
   try {
      imageSrc.value = await invoke<string | null>('get_mod_preview', { modId: props.mod.id });
   } catch {
      imageSrc.value = null;
   }
});

function toggle() {
   modsStore.toggle(props.mod.id);
}

function openFolder() {
   modsStore.openFolder(props.mod.id);
}
</script>

<template>
   <VueCard
      class="relative flex flex-col gap-4 p-4 transition-all"
      :class="[
         !mod.isEnabled && 'opacity-50',
         selectMode && selected && 'outline-primary outline-2',
      ]"
      @click="selectMode && emit('toggle-select', mod)"
      v-auto-animate
   >
      <VueCheckbox v-if="selectMode" :model-value="selected" class="absolute top-2 left-2 z-10" />
      <img
         :src="imageSrc ?? '/images/no-data.png'"
         alt=""
         class="bg-foreground aspect-video w-full rounded-sm object-cover"
      />
      <div class="flex flex-1 flex-col gap-1" v-auto-animate>
         <VueTypography variant="BodyB">{{ mod.name }}</VueTypography>
         <VueTypography v-if="mod.author" variant="CaptionR" as="div" class="text-muted-foreground">
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
