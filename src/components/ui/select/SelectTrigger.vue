<script setup lang="ts">
import type { HTMLAttributes } from 'vue';
import { computed, useAttrs } from 'vue';
import { reactiveOmit } from '@vueuse/core';
import { SelectTrigger, type SelectTriggerProps, useForwardProps } from 'reka-ui';
import { cn } from '@/utils/general';
import { PhCaretDown, PhX } from '@phosphor-icons/vue';

defineOptions({ inheritAttrs: false });

const attrs = useAttrs();

const props = withDefaults(
   defineProps<
      SelectTriggerProps & {
         class?: HTMLAttributes['class'];
         error?: boolean;
         isOpen?: boolean;
         clearable?: boolean;
         hasValue?: boolean;
         loading?: boolean;
      }
   >(),
   { clearable: false, hasValue: false, loading: false },
);

const emits = defineEmits(['clear']);

const delegatedProps = reactiveOmit(props, 'class', 'clearable', 'hasValue', 'loading');
const forwardedProps = useForwardProps(delegatedProps);

const iconColor = computed(() => {
   if (props.loading) return 'var(--color-primary, #9c88ff)';
   if (props.disabled) return 'var(--color-muted-foreground, #6b7280)';
   if (props.error) return 'var(--color-destructive, #ff6b6b)';
   return 'var(--color-primary, #9c88ff)';
});
</script>

<template>
   <div class="relative flex">
      <SelectTrigger
         data-slot="select-trigger"
         v-bind="{ ...forwardedProps, ...attrs }"
         :aria-invalid="!!error"
         :class="
            cn(
               'flex h-12 w-full min-w-0 px-4 py-3 pr-12',
               'text-foreground rounded-lg border border-white/10 bg-white/5',
               'truncate text-base font-normal placeholder:text-white/30',
               'transition-all outline-none',
               'aria-invalid:border-destructive aria-invalid:text-destructive aria-invalid:bg-destructive/10',
               'disabled:text-muted-foreground disabled:cursor-not-allowed disabled:border-white/5 disabled:bg-white/5 disabled:placeholder:text-white/20',
               $slots.icon ? 'pl-12' : '',
               props.class,
            )
         "
      >
         <slot name="value" />
      </SelectTrigger>
      <div
         v-if="$slots.icon"
         class="pointer-events-none absolute inset-y-0 left-0 ml-4 flex items-center"
      >
         <slot name="icon" :color="iconColor" />
      </div>
      <div class="absolute inset-y-0 right-0 z-10 mr-4 flex items-center gap-1">
         <button
            v-if="clearable && hasValue && !disabled && !loading"
            type="button"
            aria-label="Clear"
            class="cursor-pointer transition-all hover:opacity-70"
            @click.stop="emits('clear')"
         >
            <PhX :size="18" weight="bold" color="var(--color-muted-foreground, #6b7280)" />
         </button>
         <div class="pointer-events-none flex items-center">
            <div v-if="loading" class="loader size-6 border-4! border-b-transparent!" />

            <PhCaretDown
               v-else
               :size="24"
               :color="iconColor"
               class="transition-all duration-300"
               :class="{ '-rotate-180': isOpen }"
            />
         </div>
      </div>
   </div>
</template>

<style scoped>
[data-placeholder] {
   color: rgba(255, 255, 255, 0.35);
}
</style>
