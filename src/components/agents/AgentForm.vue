<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue';
import { open } from '@tauri-apps/plugin-dialog';
import { invoke } from '@tauri-apps/api/core';
import { PhPencilSimple, PhX } from '@phosphor-icons/vue';
import VueButton from '@/components/ui/button/VueButton.vue';
import VueInput from '@/components/ui/input/VueInput.vue';
import Label from '@/components/ui/input/Label.vue';
import { VueSelect } from '@/components/ui/select';
import VueTypography from '@/components/ui/typography/VueTypography.vue';
import type { Agent, AgentDetails, AgentInput } from '../../types';
import {
   ATTRIBUTE_ICONS,
   parseAgentDetails,
   RANK_ICONS,
   resolveAgentImageSrc,
   serializeAgentDetails,
   SPECIALITY_ICONS,
} from '../../utils/agent';

const props = defineProps<{
   initialAgent?: Agent;
   submitLabel: string;
}>();

const emit = defineEmits<{
   submit: [input: AgentInput];
}>();

// Reka UI's SelectItem forbids an empty-string value (that's reserved to mean "cleared, show the
// placeholder"), so the "unset" state isn't a selectable list item here — it's represented by
// `clearable` on each VueSelect below instead, via the rank/attribute/specialityModel proxies.
const RANK_OPTIONS = ['S', 'A'];
const ATTRIBUTE_OPTIONS = [
   'Electric',
   'Fire',
   'Ice',
   'Frost',
   'Ether',
   'Physical',
   'AuricInk',
   'HonedEdge',
   'Lumiflux',
];
const SPECIALITY_OPTIONS = ['Attack', 'Stun', 'Anomaly', 'Support', 'Defense', 'Rupture'];

const toSelectOptions = (opts: string[]) => opts.map((opt) => ({ label: opt, value: opt }));
const rankSelectOptions = toSelectOptions(RANK_OPTIONS);
const attributeSelectOptions = toSelectOptions(ATTRIBUTE_OPTIONS);
const specialitySelectOptions = toSelectOptions(SPECIALITY_OPTIONS);

// Creating an agent drops straight into the form; an existing one opens read-only.
const isEditing = ref(props.initialAgent === undefined);
// A built-in agent's name/image/stats come from definitions/zzz.toml and are rewritten on every
// version-gated re-sync, so edits here wouldn't survive an update. Aliases are the one field
// that sync treats as additive-only, so they stay editable.
const canEditDetails = computed(() => !props.initialAgent?.isBuiltin);

const name = ref('');
const baseImage = ref<string | null>(null);
const aliases = reactive<string[]>([]);
const aliasInput = ref('');
const details = reactive<AgentDetails>({ rank: '', attribute: '', speciality: '' });

function resetFromAgent() {
   const agent = props.initialAgent;
   name.value = agent?.name ?? '';
   baseImage.value = agent?.baseImage ?? null;
   aliases.splice(0, aliases.length, ...(agent?.aliases ?? []));
   Object.assign(details, parseAgentDetails(agent?.details ?? null));
   aliasInput.value = '';
}
resetFromAgent();

function detailModel(key: 'rank' | 'attribute' | 'speciality') {
   return computed({
      get: () => details[key] || undefined,
      set: (value) => {
         details[key] = value ?? '';
      },
   });
}
const rankModel = detailModel('rank');
const attributeModel = detailModel('attribute');
const specialityModel = detailModel('speciality');

// Mirrors create_agent/update_agent's own check: the slug is derived from the name and is the only
// handle the /agents/[slug] route has, so a name with nothing alphanumeric in it would produce an
// agent with no reachable detail page (and no way to reach its Delete button).
const canSubmit = computed(() => /[a-z0-9]/i.test(name.value));

const statRows = computed(() =>
   [
      { label: 'Rank', value: details.rank, icon: RANK_ICONS[details.rank] },
      { label: 'Attribute', value: details.attribute, icon: ATTRIBUTE_ICONS[details.attribute] },
      {
         label: 'Speciality',
         value: details.speciality,
         icon: SPECIALITY_ICONS[details.speciality],
      },
   ].filter((stat) => Boolean(stat.value)),
);

// Fires both on agent-to-agent navigation and after a save (the parent reassigns the agent with
// the server's response) — either way, drop back to the read-only view.
watch(
   () => props.initialAgent,
   (agent) => {
      if (!agent) return;
      resetFromAgent();
      isEditing.value = false;
   },
);

function cancelEdit() {
   resetFromAgent();
   isEditing.value = false;
}

function addAlias() {
   const value = aliasInput.value.trim().toLowerCase();
   if (value && !aliases.includes(value)) {
      aliases.push(value);
   }
   aliasInput.value = '';
}

function removeAlias(alias: string) {
   const index = aliases.indexOf(alias);
   if (index !== -1) aliases.splice(index, 1);
}

async function pickImage() {
   const path = await open({
      multiple: false,
      filters: [{ name: 'Images', extensions: ['png', 'jpg', 'jpeg', 'webp', 'gif'] }],
   });
   if (typeof path === 'string') {
      baseImage.value = await invoke<string>('read_image_as_data_url', { path });
   }
}

function handleSubmit() {
   emit('submit', {
      name: name.value.trim(),
      details: serializeAgentDetails(details),
      baseImage: baseImage.value,
      aliases: [...aliases],
   });
}
</script>

<template>
   <div v-auto-animate class="bg-card rounded-lg border border-white/10 p-6">
      <div v-if="!isEditing" class="flex gap-6">
         <img
            :src="resolveAgentImageSrc(baseImage)"
            alt=""
            class="bg-foreground size-60 rounded-lg border border-white/10 object-cover"
            :class="{ 'p-2': !baseImage }"
         />
         <div class="flex flex-1 flex-col items-start gap-4">
            <VueTypography variant="H1B" as="h2">{{ name }}</VueTypography>
            <div v-if="statRows.length > 0" class="flex flex-wrap gap-4">
               <div v-for="stat in statRows" :key="stat.label" class="flex flex-col gap-2">
                  <div class="bg-background/50 flex items-center gap-2 rounded-lg px-3 py-2">
                     <img v-if="stat.icon" :src="stat.icon" alt="" class="size-6 object-contain" />
                     <VueTypography v-if="stat.label !== 'Rank'" variant="BodyR" as="span">
                        {{ stat.value || '—' }}
                     </VueTypography>
                  </div>
               </div>
            </div>

            <div class="flex flex-col gap-2">
               <Label>Aliases</Label>
               <div v-auto-animate class="flex flex-wrap gap-2">
                  <span
                     v-for="alias in aliases"
                     :key="alias"
                     class="bg-primary/15 rounded-full px-3 py-1 text-sm"
                  >
                     {{ alias }}
                  </span>
                  <VueTypography
                     v-if="aliases.length === 0"
                     variant="CaptionR"
                     as="span"
                     class="text-muted-foreground"
                  >
                     No aliases yet
                  </VueTypography>
               </div>
            </div>

            <div class="mt-auto flex w-full items-center justify-end gap-4">
               <slot name="actions" />
               <VueButton type="button" class="min-w-32" @click="isEditing = true">
                  <PhPencilSimple :size="20" weight="fill" />
                  Edit
               </VueButton>
            </div>
         </div>
      </div>

      <form v-else @submit.prevent="handleSubmit" class="flex gap-6">
         <div class="flex h-full w-60 flex-col items-center gap-4">
            <img
               :src="resolveAgentImageSrc(baseImage)"
               alt=""
               class="bg-foreground size-60 rounded-lg border border-white/10 object-cover"
               :class="{ 'p-2': !baseImage }"
            />
            <VueButton
               v-if="canEditDetails"
               type="button"
               variant="outlined"
               size="sm"
               @click="pickImage"
            >
               Choose Image
            </VueButton>
         </div>
         <div class="flex flex-1 flex-col gap-4">
            <VueInput v-if="canEditDetails" id="agent-name" v-model="name" label="Name" required />

            <div v-if="canEditDetails" class="flex flex-wrap gap-4">
               <VueSelect
                  v-model="rankModel"
                  label="Rank"
                  placeholder="—"
                  clearable
                  class="min-w-40 flex-1"
                  :options="rankSelectOptions"
               />
               <VueSelect
                  v-model="attributeModel"
                  label="Attribute"
                  placeholder="—"
                  clearable
                  class="min-w-40 flex-1"
                  :options="attributeSelectOptions"
               />
               <VueSelect
                  v-model="specialityModel"
                  label="Speciality"
                  placeholder="—"
                  clearable
                  class="min-w-40 flex-1"
                  :options="specialitySelectOptions"
               />
            </div>

            <div class="flex flex-col gap-2" v-auto-animate>
               <Label>Aliases</Label>
               <div v-if="aliases.length > 0" v-auto-animate class="flex flex-wrap gap-2">
                  <span
                     v-for="alias in aliases"
                     :key="alias"
                     class="bg-primary/15 flex items-center gap-2 rounded-full px-2 py-1 pl-3 text-sm"
                  >
                     {{ alias }}
                     <button
                        type="button"
                        class="text-foreground/60 hover:text-destructive cursor-pointer p-1"
                        @click="removeAlias(alias)"
                     >
                        <PhX :size="12" />
                     </button>
                  </span>
               </div>
               <div class="flex gap-4">
                  <VueInput
                     v-model="aliasInput"
                     container-class="flex-1"
                     placeholder="Add an alias…"
                     @keydown.enter.prevent="addAlias"
                  />
                  <VueButton
                     type="button"
                     variant="outlined"
                     :disabled="!aliasInput.trim()"
                     @click="addAlias"
                  >
                     Add
                  </VueButton>
               </div>
            </div>

            <div class="mt-auto flex w-full items-center justify-end gap-4">
               <VueButton
                  v-if="initialAgent"
                  type="button"
                  variant="outlined"
                  class="min-w-32"
                  @click="cancelEdit"
               >
                  Cancel
               </VueButton>
               <VueButton type="submit" class="min-w-32" :disabled="!canSubmit">
                  {{ submitLabel }}
               </VueButton>
            </div>
         </div>
      </form>
   </div>
</template>
