<script setup lang="ts">
import type { HTMLAttributes } from 'vue';
import { reactiveOmit } from '@vueuse/core';
import {
   SelectContent,
   type SelectContentEmits,
   type SelectContentProps,
   SelectPortal,
   SelectViewport,
   useForwardPropsEmits,
} from 'reka-ui';
import { cn } from '@/utils/general';

defineOptions({
   inheritAttrs: false,
});

const props = withDefaults(
   defineProps<SelectContentProps & { class?: HTMLAttributes['class'] }>(),
   {
      position: 'popper',
   },
);
const emits = defineEmits<SelectContentEmits>();

const delegatedProps = reactiveOmit(props, 'class');

const forwarded = useForwardPropsEmits(delegatedProps, emits);
</script>

<template>
   <SelectPortal>
      <SelectContent
         data-slot="select-content"
         v-bind="{ ...forwarded, ...$attrs }"
         :class="
            cn(
               'content-shadow bg-card text-foreground relative z-110 flex max-h-65 flex-col overflow-x-hidden rounded-lg border border-white/10',
               position === 'popper'
                  ? 'w-(--reka-select-trigger-width)'
                  : 'w-[calc(100dvw-48px)] max-w-75',
               'data-[state=open]:animate-in data-[state=open]:fade-in-0 data-[state=open]:zoom-in-95',
               'data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=closed]:zoom-out-95',
               'data-[side=bottom]:slide-in-from-top-2 data-[side=left]:slide-in-from-right-2',
               'data-[side=right]:slide-in-from-left-2 data-[side=top]:slide-in-from-bottom-2',
               position === 'popper' &&
                  'data-[side=bottom]:translate-y-1 data-[side=left]:-translate-x-1 data-[side=right]:translate-x-1 data-[side=top]:-translate-y-1',
               props.class,
            )
         "
      >
         <slot name="input" />
         <SelectViewport
            :class="
               cn(
                  'min-h-0 flex-1 overflow-y-auto p-1',
                  position === 'popper' && 'scrollbar w-full scroll-my-1',
               )
            "
         >
            <slot />
         </SelectViewport>
      </SelectContent>
   </SelectPortal>
</template>

<style>
.content-shadow {
   box-shadow: 2px 2px 10px 0px #00000066;
}

.scrollbar {
   scrollbar-width: auto !important;
}
.scrollbar::-webkit-scrollbar {
   display: initial !important;
   width: 10px;
}
.scrollbar::-webkit-scrollbar-track {
   background-color: var(--color-background, #181825);
}

.scrollbar::-webkit-scrollbar-thumb {
   border-radius: 8px;
   background-color: var(--color-primary, #9c88ff);
   border: 2px solid var(--color-background, #181825);
}
</style>
