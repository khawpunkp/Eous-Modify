<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue';
import { open } from '@tauri-apps/plugin-dialog';
import { invoke } from '@tauri-apps/api/core';
import { PhPencilSimple, PhTrash, PhX } from '@phosphor-icons/vue';
import VueButton from '@/components/ui/button/VueButton.vue';
import VueInput from '@/components/ui/input/VueInput.vue';
import Label from '@/components/ui/input/Label.vue';
import { VueSelect } from '@/components/ui/select';
import VueTypography from '@/components/ui/typography/VueTypography.vue';
import { useImageInput } from '@/composables/imageInput';
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
const RANK_OPTIONS = ['S', 'I', 'A'];
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
   name.value = agent?.fullName ?? '';
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

const RANK_DETAIL = { label: 'Rank', value: details.rank, icon: RANK_ICONS[details.rank] };

const statRows = computed(() =>
   [
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

// Clearing the image clears the stored override, so the seeded one comes back. Only offered for a
// built-in that actually has an override — either already saved (hasCustomImage) or picked just now.
// `baseImage` here is the resolved image, so it's non-null for a plain seeded agent too and can't
// stand in for "has an override" on its own.
const canResetImage = computed(() => {
   const agent = props.initialAgent;
   // A user-made agent has no bundled art to fall back to, so there is no default to restore.
   if (agent?.isBuiltin !== true) return false;
   // Already cleared: leaving the affordance up would offer to remove what is now the default.
   if (baseImage.value === null) return false;
   return agent.hasCustomImage || baseImage.value !== agent.baseImage;
});

/**
 * What the image slot shows: the pending pick, or the bundled art once a custom one is cleared.
 *
 * `baseImage` going null means "no custom image", not "no image" — the agent still has whatever
 * definitions/zzz.toml seeded. Falling through to `defaultImage` is what makes clearing show the real
 * portrait straight away instead of the anonymous placeholder.
 */
const previewImage = computed(() => baseImage.value ?? props.initialAgent?.defaultImage ?? null);

function useDefaultImage() {
   baseImage.value = null;
}

function applyImage(dataUrl: string) {
   baseImage.value = dataUrl;
}

async function pickImage() {
   const path = await open({
      multiple: false,
      filters: [
         {
            name: 'Images',
            extensions: [
               'png',
               'jpg',
               'jpeg',
               'jpe',
               'jif',
               'jfif',
               'webp',
               'gif',
               'svg',
               'ico',
               'bmp',
               'avif',
               'heic',
               'heif',
               'jxl',
               'apng',
               'tif',
               'tiff',
            ],
         },
      ],
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
            class="bg-foreground size-60 rounded-sm object-cover"
            :class="{ 'p-10': !baseImage }"
         />
         <div class="flex flex-1 flex-col items-start gap-4">
            <VueTypography variant="H1B" as="h2">{{ name }}</VueTypography>
            <div v-if="statRows.length > 0" class="flex flex-wrap gap-2">
               <div
                  class="bg-background/50 flex size-10 items-center gap-2 rounded-lg p-1"
                  v-auto-animate
               >
                  <img
                     v-if="RANK_DETAIL.icon"
                     :src="RANK_DETAIL.icon"
                     alt=""
                     class="object-contain"
                  />
                  <VueTypography v-if="RANK_DETAIL.label !== 'Rank'" variant="BodyR" as="span">
                     {{ RANK_DETAIL.value || '—' }}
                  </VueTypography>
               </div>
               <div
                  v-for="stat in statRows"
                  :key="stat.label"
                  class="bg-background/50 flex items-center gap-2 rounded-lg px-2 py-1"
                  v-auto-animate
               >
                  <img v-if="stat.icon" :src="stat.icon" alt="" class="size-7 object-contain" />
                  <VueTypography v-if="stat.label !== 'Rank'" variant="BodyR" as="span">
                     {{ stat.value || '—' }}
                  </VueTypography>
               </div>
            </div>

            <div class="flex flex-col gap-2">
               <Label>Aliases</Label>
               <div v-auto-animate class="flex flex-wrap gap-2">
                  <span
                     v-for="alias in aliases"
                     :key="alias"
                     class="bg-primary/50 rounded-full px-3 py-1 text-sm"
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
         <div class="flex h-full w-60 flex-col items-center gap-2" v-auto-animate>
            <div class="group relative size-60">
               <img
                  :src="resolveAgentImageSrc(previewImage)"
                  alt=""
                  class="bg-foreground size-60 rounded-sm border object-cover transition-colors"
                  :class="[
                     { 'p-10': !previewImage },
                     isDraggingOver ? 'border-primary border-2' : 'border-white/10',
                  ]"
               />
               <!-- Only offered when there is a custom image to remove: on the bundled art there is
                    nothing to fall back to, so the button would do nothing. -->
               <button
                  v-if="canResetImage"
                  type="button"
                  class="bg-background/80 text-foreground/70 hover:text-destructive absolute top-2 right-2 cursor-pointer rounded-full p-2 opacity-0 transition-all group-hover:opacity-100"
                  title="Use default image"
                  @click="useDefaultImage"
               >
                  <PhTrash :size="20" weight="fill" />
               </button>
            </div>
            <VueTypography variant="CaptionR" as="p" class="text-muted-foreground text-center">
               {{ isDraggingOver ? 'Drop to use this image' : 'Drop an image here, or' }}
            </VueTypography>
            <!-- The image is editable on built-in agents too: it's stored separately from the
                 seeded one, so definition re-sync can't overwrite a user pick. -->
            <VueButton type="button" variant="outlined" size="sm" @click="pickImage">
               {{ previewImage ? 'Change Image' : 'Choose Image' }}
            </VueButton>
            <!-- Last child on purpose. auto-animate translates every sibling an insertion displaces,
                 so an error added mid-column animates the rest of the column; added at the end it
                 displaces nothing and only fades in itself. -->
            <VueTypography
               v-if="dropError"
               variant="CaptionR"
               as="p"
               class="text-destructive text-center"
            >
               {{ dropError }}
            </VueTypography>
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
                     class="bg-primary/50 flex items-center gap-2 rounded-full px-2 py-1 pl-3 text-sm"
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
