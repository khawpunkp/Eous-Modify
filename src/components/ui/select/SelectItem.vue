<script setup lang="ts">
import type { HTMLAttributes } from 'vue';
import { reactiveOmit } from '@vueuse/core';
import { SelectItem, type SelectItemProps, useForwardProps } from 'reka-ui';
import { cn } from '@/utils/general';
import SelectItemText from './SelectItemText.vue';

const props = defineProps<SelectItemProps & { class?: HTMLAttributes['class'] }>();

const delegatedProps = reactiveOmit(props, 'class');

const forwardedProps = useForwardProps(delegatedProps);
</script>

<template>
   <SelectItem
      data-slot="select-item"
      v-bind="forwardedProps"
      :class="
         cn(
            `text-foreground relative flex w-full cursor-default items-center gap-2 rounded-sm px-2 py-2 text-[16px] outline-hidden select-none data-disabled:pointer-events-none data-disabled:opacity-50 [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-4 *:[span]:last:flex *:[span]:last:items-center *:[span]:last:gap-2`,
            'data-highlighted:bg-white/10',
            'data-[state=checked]:bg-primary data-[state=checked]:text-primary-foreground',
            props.class,
         )
      "
   >
      <SelectItemText>
         <slot />
      </SelectItemText>
   </SelectItem>
</template>
