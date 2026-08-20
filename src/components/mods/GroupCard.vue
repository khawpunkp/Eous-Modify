<script setup lang="ts">
import { ref } from 'vue';
import {
   PhCaretDown,
   PhCaretRight,
   PhStack,
   PhPencilSimple,
   PhArrowsOut,
   PhX,
} from '@phosphor-icons/vue';
import VueCard from '@/components/ui/card/VueCard.vue';
import VueTypography from '@/components/ui/typography/VueTypography.vue';
import PreviewImage from './PreviewImage.vue';
import VueSwitch from '@/components/ui/switch/VueSwitch.vue';
import { confirmAction } from '@/composables/confirm';
import { useModGroupsStore } from '../../stores/modGroups';
import type { ModGroup } from '../../types';

const props = defineProps<{ group: ModGroup }>();
const emit = defineEmits<{ edit: [group: ModGroup] }>();

const modGroupsStore = useModGroupsStore();
const isExpanded = ref(false);

function toggle() {
   modGroupsStore.toggle(props.group.id);
}

async function removeMember(modId: number) {
   await modGroupsStore.removeMember(props.group.id, modId);
}

async function disband() {
   if (
      !(await confirmAction({
         title: `Ungroup "${props.group.name}"?`,
         message: "The mods themselves won't be touched.",
         confirmLabel: 'Ungroup',
         destructive: true,
      }))
   )
      return;
   await modGroupsStore.disband(props.group.id);
}
</script>

<template>
   <!-- min-w-0 for the same reason as ModCard: without it one unbroken group name stretches its whole
        grid column. -->
   <VueCard
      class="flex min-w-0 flex-col gap-4 p-4 transition-all"
      :class="{ 'opacity-50': !group.isEnabled }"
      v-auto-animate
   >
      <PreviewImage :src="group.baseImage ?? '/images/no-data.png'" class="aspect-4/3 rounded-sm" />
      <div class="flex items-center gap-2">
         <button
            type="button"
            class="text-foreground/70 cursor-pointer p-1 hover:opacity-100"
            title="Expand"
            @click="isExpanded = !isExpanded"
         >
            <PhCaretDown v-if="isExpanded" :size="16" />
            <PhCaretRight v-else :size="16" />
         </button>
         <PhStack :size="20" class="text-primary" weight="fill" />
         <VueTypography variant="BodyB" as="span" class="min-w-0 grow wrap-anywhere">
            {{ group.name }}
         </VueTypography>
         <span
            class="bg-primary flex size-6 items-center justify-center rounded-full text-[11px] font-semibold text-white"
         >
            {{ group.members.length }}
         </span>
      </div>

      <div class="flex items-center gap-2">
         <VueSwitch
            :model-value="group.isEnabled"
            :title="group.isEnabled ? 'Enabled' : 'Disabled'"
            class="mr-auto"
            @update:model-value="toggle"
         />
         <button
            type="button"
            class="text-foreground cursor-pointer p-1 opacity-70 transition-all hover:opacity-100"
            title="Edit group"
            @click="emit('edit', group)"
         >
            <PhPencilSimple :size="20" weight="fill" />
         </button>
         <button
            type="button"
            class="text-destructive cursor-pointer p-1 opacity-70 transition-all hover:opacity-100"
            title="Ungroup"
            @click="disband"
         >
            <PhArrowsOut :size="20" weight="fill" />
         </button>
      </div>

      <ul
         v-if="isExpanded"
         v-auto-animate
         class="flex flex-col gap-2 border-t border-white/10 pt-3"
      >
         <li
            v-for="member in group.members"
            :key="member.modId"
            class="flex items-center justify-between text-[13px]"
         >
            <span :class="{ 'opacity-50': !member.isEnabled }">{{ member.name }}</span>
            <button
               type="button"
               class="text-foreground/70 cursor-pointer p-1 hover:opacity-100"
               title="Remove from group"
               @click="removeMember(member.modId)"
            >
               <PhX :size="16" />
            </button>
         </li>
      </ul>
   </VueCard>
</template>
