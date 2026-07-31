<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { useRouter } from 'vue-router';
import { PhUsers, PhMagnifyingGlass, PhUserPlus } from '@phosphor-icons/vue';
import AgentCard from '../../components/agents/AgentCard.vue';
import VueButton from '@/components/ui/button/VueButton.vue';
import VueInput from '@/components/ui/input/VueInput.vue';
import { VueSelect } from '@/components/ui/select';
import VueTypography from '@/components/ui/typography/VueTypography.vue';
import { useAgentsStore } from '../../stores/agents';
import {
   ATTRIBUTE_ICONS,
   parseAgentDetails,
   RANK_ICONS,
   SPECIALITY_ICONS,
} from '../../utils/agent';
import type { Agent, Mod } from '../../types';

const SORT_STORAGE_KEY = 'sort_agents';

const RANKS = Object.entries(RANK_ICONS).map(([key, icon]) => ({ key, icon }));
const ATTRIBUTES = Object.entries(ATTRIBUTE_ICONS).map(([key, icon]) => ({
   key,
   icon,
}));
const SPECIALITIES = Object.entries(SPECIALITY_ICONS).map(([key, icon]) => ({
   key,
   icon,
}));

const SORT_OPTIONS = [
   { label: 'Name (A-Z)', value: 'name-asc' },
   { label: 'Name (Z-A)', value: 'name-desc' },
   { label: 'Total Mods (High-Low)', value: 'mods-desc' },
   { label: 'Total Mods (Low-High)', value: 'mods-asc' },
   { label: 'Enabled Mods (High-Low)', value: 'enabled-desc' },
   { label: 'Enabled Mods (Low-High)', value: 'enabled-asc' },
];

const agentsStore = useAgentsStore();

const search = ref('');
const sortOption = ref(localStorage.getItem(SORT_STORAGE_KEY) ?? 'name-asc');
const selectedRank = ref('');
const selectedAttribute = ref('');
const selectedSpeciality = ref('');
const modCounts = ref(new Map<number, { total: number; enabled: number }>());
// Gates the grid until the first load + sort has settled. The agents store persists across
// navigations, so without this the grid renders stale cards for a frame — and the late-arriving
// modCounts can re-sort them — making v-auto-animate play on every page enter.
const isLoading = ref(true);

const router = useRouter();

onMounted(async () => {
   try {
      await agentsStore.fetchAll();
      const allMods = await invoke<Mod[]>('list_mods', {
         agentId: null,
         categoryId: null,
         categoryItemId: null,
      });
      const counts = new Map<number, { total: number; enabled: number }>();
      for (const mod of allMods) {
         if (mod.agentId === null) continue;
         const entry = counts.get(mod.agentId) ?? { total: 0, enabled: 0 };
         entry.total += 1;
         if (mod.isEnabled) entry.enabled += 1;
         counts.set(mod.agentId, entry);
      }
      modCounts.value = counts;
   } finally {
      isLoading.value = false;
   }
});

watch(sortOption, (value) => localStorage.setItem(SORT_STORAGE_KEY, value));

function toggleRank(value: string) {
   selectedRank.value = selectedRank.value === value ? '' : value;
}
function toggleAttribute(value: string) {
   selectedAttribute.value = selectedAttribute.value === value ? '' : value;
}
function toggleSpeciality(value: string) {
   selectedSpeciality.value = selectedSpeciality.value === value ? '' : value;
}

function countOf(agent: Agent) {
   return modCounts.value.get(agent.id) ?? { total: 0, enabled: 0 };
}

const visibleAgents = computed(() => {
   const query = search.value.trim().toLowerCase();
   const filtered = agentsStore.agents.filter((agent) => {
      if (query && !agent.name.toLowerCase().includes(query)) return false;
      const details = parseAgentDetails(agent.details);
      if (selectedRank.value && details.rank !== selectedRank.value) return false;
      if (selectedAttribute.value && details.attribute !== selectedAttribute.value) return false;
      if (selectedSpeciality.value && details.speciality !== selectedSpeciality.value) return false;
      return true;
   });

   return [...filtered].sort((a, b) => {
      switch (sortOption.value) {
         case 'name-desc':
            return b.name.localeCompare(a.name);
         case 'mods-desc':
            return countOf(b).total - countOf(a).total;
         case 'mods-asc':
            return countOf(a).total - countOf(b).total;
         case 'enabled-desc':
            return countOf(b).enabled - countOf(a).enabled;
         case 'enabled-asc':
            return countOf(a).enabled - countOf(b).enabled;
         default:
            return a.name.localeCompare(b.name);
      }
   });
});
</script>

<template>
   <div class="flex h-full flex-col">
      <div
         class="mb-6 flex flex-wrap items-center justify-between gap-5 border-b border-white/10 pb-4"
      >
         <VueTypography variant="H1B" as="h1" class="mr-auto flex items-center gap-3">
            <PhUsers :size="32" weight="fill" />
            Agents
         </VueTypography>

         <VueButton type="button" @click="router.push('/agents/new')">
            <PhUserPlus :size="24" weight="fill" />
            Add Agent
         </VueButton>
      </div>

      <div class="mb-6 flex flex-col flex-wrap items-end gap-6 border-b border-white/10 pb-6">
         <div class="flex flex-wrap items-center gap-5">
            <div class="flex flex-wrap items-center gap-2">
               <button
                  v-for="rank in RANKS"
                  :key="rank.key"
                  type="button"
                  class="inline-flex size-9 items-center justify-center rounded-full border border-white/5 bg-white/5 p-2 transition-colors"
                  :class="
                     selectedRank === rank.key
                        ? 'border-white/40 bg-white/10'
                        : 'hover:border-white/40 hover:bg-white/10'
                  "
                  :title="rank.key"
                  @click="toggleRank(rank.key)"
               >
                  <img :src="rank.icon" alt="" class="size-5 object-contain" />
               </button>
            </div>
            <div class="flex flex-wrap items-center gap-2">
               <button
                  v-for="attribute in ATTRIBUTES"
                  :key="attribute.key"
                  type="button"
                  class="inline-flex size-9 items-center justify-center rounded-full border border-white/5 bg-white/5 p-2 transition-colors"
                  :class="
                     selectedAttribute === attribute.key
                        ? 'border-white/40 bg-white/10'
                        : 'hover:border-white/40 hover:bg-white/10'
                  "
                  :title="attribute.key"
                  @click="toggleAttribute(attribute.key)"
               >
                  <img :src="attribute.icon" alt="" class="size-5 object-contain" />
               </button>
            </div>
            <div class="flex flex-wrap items-center gap-2">
               <button
                  v-for="speciality in SPECIALITIES"
                  :key="speciality.key"
                  type="button"
                  class="inline-flex size-9 items-center justify-center rounded-full border border-white/5 bg-white/5 p-2 transition-colors"
                  :class="
                     selectedSpeciality === speciality.key
                        ? 'border-white/40 bg-white/10'
                        : 'hover:border-white/40 hover:bg-white/10'
                  "
                  :title="speciality.key"
                  @click="toggleSpeciality(speciality.key)"
               >
                  <img :src="speciality.icon" alt="" class="size-5 object-contain" />
               </button>
            </div>
         </div>
         <div class="flex w-full items-center justify-end gap-5">
            <VueInput
               v-model="search"
               container-class="w-full max-w-75"
               placeholder="Search Agents..."
               label="Search"
            >
               <template #iconStart="{ color }">
                  <PhMagnifyingGlass :size="24" :color="color" />
               </template>
            </VueInput>
            <div class="w-full max-w-75">
               <VueSelect v-model="sortOption" :options="SORT_OPTIONS" label="Sort by" />
            </div>
         </div>
      </div>

      <template v-if="!isLoading">
         <div v-if="visibleAgents.length === 0" class="flex flex-1 items-center justify-center">
            <img src="/images/no-data.png" class="w-50" />
         </div>
         <div
            v-else
            v-auto-animate
            class="grid gap-6 pb-6"
            style="grid-template-columns: repeat(auto-fill, minmax(200px, 1fr))"
         >
            <AgentCard
               v-for="agent in visibleAgents"
               :key="agent.slug"
               :agent="agent"
               :total-mods="countOf(agent).total"
               :enabled-mods="countOf(agent).enabled"
            />
         </div>
      </template>
   </div>
</template>
