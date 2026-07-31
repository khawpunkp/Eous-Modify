<script setup lang="ts">
import type { HTMLAttributes } from 'vue';
import { reactiveOmit } from '@vueuse/core';
import { Label, type LabelProps } from 'reka-ui';
import { cn } from '@/utils/general';

const props = defineProps<LabelProps & { class?: HTMLAttributes['class']; required?: boolean }>();

const delegatedProps = reactiveOmit(props, 'class', 'required');
</script>

<template>
   <Label
      data-slot="label"
      v-bind="delegatedProps"
      :class="
         cn(
            'text-foreground/70 text-sm leading-none font-normal',
            'flex flex-row gap-1',
            'select-none',
            'group-data-[disabled=true]:pointer-events-none group-data-[disabled=true]:opacity-50',
            'peer-disabled:cursor-not-allowed peer-disabled:opacity-50',
            props.class,
         )
      "
   >
      <slot />
      <span v-if="required" class="text-destructive">*</span>
   </Label>
</template>
