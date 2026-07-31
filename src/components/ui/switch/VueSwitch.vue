<script setup lang="ts">
import type { HTMLAttributes } from 'vue';
import { reactiveOmit } from '@vueuse/core';
import {
   SwitchRoot,
   type SwitchRootEmits,
   type SwitchRootProps,
   SwitchThumb,
   useForwardPropsEmits,
} from 'reka-ui';
import { cn } from '@/utils/general';

const props = defineProps<SwitchRootProps & { class?: HTMLAttributes['class'] }>();

const emits = defineEmits<SwitchRootEmits>();

const delegatedProps = reactiveOmit(props, 'class');

const forwarded = useForwardPropsEmits(delegatedProps, emits);
</script>

<template>
   <SwitchRoot
      data-slot="switch"
      v-bind="forwarded"
      :class="
         cn(
            'data-[state=checked]:bg-primary data-[state=unchecked]:bg-white/15',
            // h-7/w-12 with p-1 keeps the geometry exact on whole-number steps: 28-8 = 20px inner
            // height for the size-5 thumb, and 48-8 = 40px inner width for its translate-x-5 travel.
            'h-7 w-12 rounded-full p-1',
            'inline-flex shrink-0 items-center',
            'transition-all outline-none',
            'disabled:cursor-not-allowed disabled:opacity-50',
            props.class,
         )
      "
   >
      <SwitchThumb
         data-slot="switch-thumb"
         :class="
            cn(
               'bg-white',
               'pointer-events-none flex',
               'aspect-square size-5 rounded-full',
               'transition-transform',
               'data-[state=checked]:translate-x-5 data-[state=unchecked]:translate-x-0',
            )
         "
      >
         <slot name="thumb" />
      </SwitchThumb>
   </SwitchRoot>
</template>
