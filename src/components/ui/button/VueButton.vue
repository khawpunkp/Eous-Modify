<template>
   <button
      type="button"
      data-slot="button"
      ref="rippleButtonRef"
      :class="
         cn(buttonVariants({ color, variant, size, disabled: isDisabled, loading }), props.class)
      "
      :style="{ '--duration': $props.duration + 'ms' }"
      @click="handleClick"
      :disabled="isDisabled || disabledWithOutChangeStyle"
   >
      <div v-if="loading" class="loader size-5 border-4! border-b-transparent!" />
      <slot v-else />
      <span class="pointer-events-none absolute inset-0">
         <span
            v-for="ripple in buttonRipples"
            :key="ripple.key"
            class="ripple-animation absolute rounded-lg opacity-30"
            :style="{
               width: ripple.size + 'px',
               height: ripple.size + 'px',
               top: ripple.y + 'px',
               left: ripple.x + 'px',
               backgroundColor: $props.rippleColor,
               transform: 'scale(0)',
               animationDuration: $props.duration + 'ms',
            }"
         />
      </span>
   </button>
</template>

<script setup lang="ts">
import type { HTMLAttributes } from 'vue';
import { ref, computed, watchEffect } from 'vue';
import { cva, type VariantProps } from 'class-variance-authority';
import { cn } from '@/utils/general';

const baseStyle = [
   'relative items-center justify-center overflow-hidden hover:scale-[1.03]',
   'flex gap-2 px-4 shrink-0',
   'rounded-lg',
   'transition-all duration-500',
   'cursor-pointer',
   'leading-normal',
];

// `primary`/`text-destructive` route through this project's own --color-* theme tokens —
// swap --color-primary / --color-primary-foreground to restyle instead of editing these classes.
const buttonVariants = cva(baseStyle, {
   variants: {
      variant: {
         contained: '',
         outlined: 'bg-white/5 border-[1.5px]',
         ghost: 'bg-transparent bg-none',
      },
      color: {
         primary: 'text-primary',
         gray: 'bg-white/10 text-foreground',
         error: 'text-destructive bg-destructive/10',
         disabled: 'bg-gray-600! text-white! border-gray-600!',
      },
      size: {
         default: 'h-12 text-[14px] font-[600]',
         sm: 'h-[28px] px-2 text-[14px] font-[400]',
         xs: 'h-6 px-4 text-[14px] font-[300]',
         custom: '',
      },
      disabled: {
         true: 'bg-gray-600! text-white! border-gray-600! cursor-not-allowed',
      },
      loading: {
         true: '',
      },
   },
   compoundVariants: [
      {
         variant: 'contained',
         color: 'primary',
         class: 'border-primary bg-primary text-primary-foreground',
      },
      {
         // contained + error is a solid destructive fill, not the pale tint with red text that
         // outlined/ghost error keeps — a filled "dangerous action" button, not an error-tinted surface.
         variant: 'contained',
         color: 'error',
         class: 'bg-destructive text-white',
      },
      {
         variant: 'outlined',
         color: 'primary',
         class: 'border-primary',
      },
      {
         variant: 'ghost',
         disabled: true,
         class: '!bg-transparent !text-gray-500',
      },
      {
         // `ghost` must always render transparent — `color`'s own background class would
         // otherwise win the twMerge conflict, since color's class is concatenated after variant's.
         variant: 'ghost',
         color: 'gray',
         class: 'bg-transparent',
      },
      {
         variant: 'ghost',
         color: 'error',
         class: 'bg-transparent',
      },
   ],
   defaultVariants: {
      variant: 'contained',
      color: 'primary',
      size: 'default',
   },
});

type ButtonVariants = VariantProps<typeof buttonVariants>;

interface Props {
   variant?: ButtonVariants['variant'];
   color?: ButtonVariants['color'];
   size?: ButtonVariants['size'];
   class?: HTMLAttributes['class'];
   rippleColor?: string;
   duration?: number;
   disabled?: boolean;
   disabledWithOutChangeStyle?: boolean;
   loading?: boolean;
}

const props = withDefaults(defineProps<Props>(), {
   rippleColor: '#0000001a',
   duration: 300,
});

const emit = defineEmits<{
   (e: 'click', event: MouseEvent): void;
}>();

const rippleButtonRef = ref<HTMLButtonElement | null>(null);
const buttonRipples = ref<Array<{ x: number; y: number; size: number; key: number }>>([]);
let rippleIdCounter = 0;

function handleClick(event: MouseEvent) {
   createRipple(event);
   emit('click', event);
}

function createRipple(event: MouseEvent) {
   const button = rippleButtonRef.value;
   if (!button) return;

   const rect = button.getBoundingClientRect();
   const size = Math.max(rect.width, rect.height);
   const x = event.clientX - rect.left - size / 2;
   const y = event.clientY - rect.top - size / 2;

   const newRipple = { x, y, size, key: rippleIdCounter++ };
   buttonRipples.value.push(newRipple);
}

watchEffect(() => {
   if (buttonRipples.value.length > 0) {
      const lastRipple = buttonRipples.value[buttonRipples.value.length - 1];
      setTimeout(() => {
         buttonRipples.value = buttonRipples.value.filter(
            (ripple) => ripple.key !== lastRipple?.key,
         );
      }, props.duration);
   }
});

const isDisabled = computed(() => props.disabled || props.loading);
</script>

<style scoped>
@keyframes rippling {
   0% {
      opacity: 1;
   }
   100% {
      transform: scale(2);
      opacity: 0;
   }
}

.ripple-animation {
   animation: rippling var(--duration) ease-out;
}
</style>
