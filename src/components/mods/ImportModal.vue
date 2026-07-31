<script setup lang="ts">
import { computed, onMounted, reactive, ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import VueButton from '@/components/ui/button/VueButton.vue';
import VueInput from '@/components/ui/input/VueInput.vue';
import { VueSelect } from '@/components/ui/select';
import VueTypography from '@/components/ui/typography/VueTypography.vue';
import { useAgentsStore } from '../../stores/agents';
import { useCategoriesStore } from '../../stores/categories';
import type { ArchiveAnalysis, ImportArchiveRequest } from '../../types';

const props = defineProps<{
   archivePath: string;
   analysis: ArchiveAnalysis;
   agentId?: number;
   categoryId?: number;
}>();
const emit = defineEmits<{
   close: [];
   imported: [];
}>();

const hasFixedTarget = props.agentId !== undefined || props.categoryId !== undefined;

const agentsStore = useAgentsStore();
const categoriesStore = useCategoriesStore();

const deducedTarget =
   props.analysis.deducedAgentId != null
      ? `agent:${props.analysis.deducedAgentId}`
      : props.analysis.deducedCategoryId != null
        ? `category:${props.analysis.deducedCategoryId}`
        : '';
const pickedTarget = ref(hasFixedTarget ? '' : deducedTarget);

const targetOptions = computed(() => [
   ...agentsStore.agents.map((agent) => ({
      label: `Character: ${agent.name}`,
      value: `agent:${agent.id}`,
   })),
   ...categoriesStore.categories.map((category) => ({
      label: `Category: ${category.name}`,
      value: `category:${category.id}`,
   })),
]);

const isImporting = ref(false);
const errorMessage = ref<string | null>(null);

function fallbackNameFromPath(path: string): string {
   const filename = path.split(/[\\/]/).pop() ?? 'New Mod';
   return filename.replace(/\.(zip|7z|rar)$/i, '');
}

const likelyRoots = computed(() => props.analysis.entries.filter((e) => e.isLikelyModRoot));
const rootOptions = computed(() =>
   likelyRoots.value.map((root) => ({ label: root.path, value: root.path })),
);

const form = reactive({
   modName: props.analysis.deducedName ?? fallbackNameFromPath(props.archivePath),
   author: props.analysis.deducedAuthor ?? '',
   selectedRoot: likelyRoots.value[0]?.path ?? '',
});

onMounted(() => {
   if (hasFixedTarget) return;
   if (agentsStore.agents.length === 0) agentsStore.fetchAll();
   if (categoriesStore.categories.length === 0) categoriesStore.fetchAll();
});

async function handleImport() {
   if (!form.modName.trim()) return;
   if (!hasFixedTarget && !pickedTarget.value) return;

   const [kind, idStr] = pickedTarget.value.split(':');
   const pickedId = Number(idStr);
   const targetIsStillDeduced = !hasFixedTarget && pickedTarget.value === deducedTarget;

   isImporting.value = true;
   errorMessage.value = null;
   try {
      const request: ImportArchiveRequest = {
         archivePath: props.archivePath,
         agentId: props.agentId ?? (kind === 'agent' ? pickedId : null),
         categoryId: props.categoryId ?? (kind === 'category' ? pickedId : null),
         categoryItemId:
            targetIsStillDeduced && kind === 'category'
               ? props.analysis.deducedCategoryItemId
               : null,
         selectedInternalRoot: form.selectedRoot || null,
         modName: form.modName.trim(),
         author: form.author.trim() || null,
      };
      await invoke('import_archive', { request });
      emit('imported');
   } catch (e) {
      errorMessage.value = String(e);
   } finally {
      isImporting.value = false;
   }
}
</script>

<template>
   <div class="fixed inset-0 z-100 flex items-center justify-center bg-black/60">
      <div
         class="bg-card flex max-h-[85vh] w-11/12 max-w-120 flex-col gap-2 overflow-y-auto rounded-lg border border-white/10 p-6"
      >
         <VueTypography variant="TitleB" as="h2">Import Mod</VueTypography>

         <form @submit.prevent="handleImport" class="flex flex-col gap-4">
            <VueTypography variant="CaptionR" as="p" class="text-muted-foreground break-all">
               {{ archivePath }}
            </VueTypography>

            <div v-if="!hasFixedTarget">
               <VueSelect
                  v-model="pickedTarget"
                  label="Import into"
                  :options="targetOptions"
                  placeholder="Choose a destination…"
                  searchable
               />
            </div>

            <div v-if="likelyRoots.length > 1">
               <VueSelect
                  v-model="form.selectedRoot"
                  label="Which folder is the mod?"
                  :options="rootOptions"
               />
            </div>

            <VueInput id="import-name" v-model="form.modName" label="Name" required />
            <VueInput id="import-author" v-model="form.author" label="Author" />

            <VueTypography v-if="errorMessage" variant="CaptionR" as="p" class="text-destructive">
               {{ errorMessage }}
            </VueTypography>

            <div class="flex items-center justify-end gap-3">
               <VueButton type="button" variant="outlined" @click="emit('close')" class="min-w-32">
                  Cancel
               </VueButton>
               <VueButton
                  type="submit"
                  :disabled="
                     isImporting || !form.modName.trim() || (!hasFixedTarget && !pickedTarget)
                  "
                  class="min-w-32"
               >
                  {{ isImporting ? 'Importing…' : 'Import' }}
               </VueButton>
            </div>
         </form>
      </div>
   </div>
</template>
