<script setup lang="ts">
import { computed, ref } from 'vue';
import { open } from '@tauri-apps/plugin-dialog';
import { invoke } from '@tauri-apps/api/core';
import { PhPlus, PhX } from '@phosphor-icons/vue';
import VueButton from '@/components/ui/button/VueButton.vue';
import VueInput from '@/components/ui/input/VueInput.vue';
import Label from '@/components/ui/input/Label.vue';
import { VueSelect } from '@/components/ui/select';
import VueTypography from '@/components/ui/typography/VueTypography.vue';
import { useModGroupsStore } from '../../stores/modGroups';
import type { Mod, ModGroup } from '../../types';

const props = defineProps<{
   /** Edit mode when set; otherwise this creates a new group from `modIds`. */
   group?: ModGroup;
   /** Create mode only — the mods that were select-mode picked on the page. */
   modIds?: number[];
   /** Ungrouped mods in the current page's scope, offered as add-to-group candidates. */
   availableMods?: Mod[];
}>();
const emit = defineEmits<{
   saved: [];
   close: [];
}>();

const modGroupsStore = useModGroupsStore();
const isEditing = computed(() => props.group !== undefined);

const name = ref(props.group?.name ?? '');
const baseImage = ref<string | null>(props.group?.baseImage ?? null);
const isSaving = ref(false);
const errorMessage = ref<string | null>(null);

// Member add/remove hit the backend immediately — each is its own command — so they can't be
// deferred to Save the way name/image are. `members` tracks the live server state as they land.
const members = ref([...(props.group?.members ?? [])]);
const pendingModId = ref<string>('');

const addableMods = computed(() => {
   const alreadyIn = new Set(members.value.map((m) => m.modId));
   return (props.availableMods ?? [])
      .filter((m) => m.groupId === null && !alreadyIn.has(m.id))
      .map((m) => ({ label: m.name, value: String(m.id) }));
});

async function pickImage() {
   const path = await open({
      multiple: false,
      filters: [{ name: 'Images', extensions: ['png', 'jpg', 'jpeg', 'webp', 'gif'] }],
   });
   if (typeof path === 'string') {
      baseImage.value = await invoke<string>('read_image_as_data_url', { path });
   }
}

async function addMod() {
   if (!props.group || !pendingModId.value) return;
   errorMessage.value = null;
   try {
      const updated = await modGroupsStore.addMember(props.group.id, Number(pendingModId.value));
      members.value = [...updated.members];
      pendingModId.value = '';
      emit('saved');
   } catch (e) {
      errorMessage.value = String(e);
   }
}

async function removeMod(modId: number) {
   if (!props.group) return;
   errorMessage.value = null;
   try {
      const updated = await modGroupsStore.removeMember(props.group.id, modId);
      emit('saved');
      members.value = [...updated.members];
   } catch (e) {
      errorMessage.value = String(e);
   }
}

async function handleSubmit() {
   if (!name.value.trim()) return;
   isSaving.value = true;
   errorMessage.value = null;
   try {
      if (props.group) {
         await modGroupsStore.update(props.group.id, name.value.trim(), baseImage.value);
      } else {
         await modGroupsStore.create(name.value.trim(), baseImage.value, props.modIds ?? []);
      }
      emit('saved');
      emit('close');
   } catch (e) {
      errorMessage.value = String(e);
   } finally {
      isSaving.value = false;
   }
}
</script>

<template>
   <div class="fixed inset-0 z-100 flex items-center justify-center bg-black/60">
      <form
         class="bg-card max-h-[85vh] w-11/12 max-w-120 overflow-y-auto rounded-lg border border-white/10 p-6"
         @submit.prevent="handleSubmit"
      >
         <VueTypography variant="TitleB" as="h2" class="mb-2">
            {{ isEditing ? 'Edit Group' : 'New Group' }}
         </VueTypography>

         <div class="mb-5 flex flex-col items-center gap-4">
            <img
               :src="baseImage ?? '/images/no-data.png'"
               alt=""
               class="bg-foreground aspect-video w-full rounded-lg border border-white/10 object-cover"
            />
            <VueButton type="button" variant="outlined" size="sm" @click="pickImage">
               {{ baseImage ? 'Choose Different Image' : 'Choose Image' }}
            </VueButton>
         </div>

         <VueInput id="group-name" v-model="name" label="Name" container-class="mb-4" required />

         <div v-if="isEditing" class="mb-4 flex flex-col gap-2">
            <Label>Mods in this group ({{ members.length }})</Label>
            <ul v-auto-animate class="flex flex-col gap-2">
               <li
                  v-for="member in members"
                  :key="member.modId"
                  class="flex items-center justify-between rounded-md bg-white/5 px-3 py-2 text-[13px]"
               >
                  <span :class="{ 'opacity-50': !member.isEnabled }">{{ member.name }}</span>
                  <button
                     type="button"
                     class="text-foreground/70 hover:text-destructive cursor-pointer p-1"
                     title="Remove from group"
                     @click="removeMod(member.modId)"
                  >
                     <PhX :size="16" />
                  </button>
               </li>
            </ul>
         </div>

         <div v-if="isEditing && addableMods.length > 0" class="mb-4 flex items-end gap-2">
            <VueSelect
               v-model="pendingModId"
               label="Add a mod"
               class="flex-1"
               placeholder="Choose a mod…"
               :options="addableMods"
               searchable
            />
            <VueButton type="button" variant="outlined" :disabled="!pendingModId" @click="addMod">
               <PhPlus :size="20" weight="bold" />
               Add
            </VueButton>
         </div>

         <VueTypography v-if="errorMessage" variant="CaptionR" as="p" class="text-destructive mb-4">
            {{ errorMessage }}
         </VueTypography>

         <div class="mt-2 flex items-center justify-end gap-3">
            <VueButton type="button" variant="outlined" class="min-w-32" @click="emit('close')">
               Cancel
            </VueButton>
            <VueButton type="submit" class="min-w-32" :disabled="isSaving || !name.trim()">
               {{ isSaving ? 'Saving…' : isEditing ? 'Save' : 'Create Group' }}
            </VueButton>
         </div>
      </form>
   </div>
</template>
