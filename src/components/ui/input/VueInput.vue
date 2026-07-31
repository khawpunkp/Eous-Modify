<template>
   <div :class="cn('flex flex-col gap-2', containerClass)">
      <Label v-if="label" :for="effectiveId" :class="labelClass" :required="required">
         {{ label }}
      </Label>

      <div class="relative flex">
         <div
            v-if="$slots.iconStart"
            class="pointer-events-none absolute inset-y-0 left-0 ml-4 flex items-center"
         >
            <slot name="iconStart" :color="iconColor" />
         </div>

         <!--
         Masked mode (mask="number" | "phone" | "custom"): IMask owns the element's displayed
         value directly, so there is intentionally NO v-model here — the Vue model is
         synced manually via mask.on('accept') (mask -> model) and a watch on
         props.modelValue (model -> mask). Adding v-model would make Vue and IMask
         fight over the DOM value. Keep this class list identical to the plain input below.
      -->
         <input
            v-if="mask"
            :id="effectiveId"
            ref="inputRef"
            :type="maskedType"
            :inputmode="maskedInputmode"
            v-bind="maskedAttrs()"
            @blur="touched = true"
            data-slot="input"
            :aria-invalid="ariaInvalid"
            :disabled="disabled"
            :class="
               cn(
                  'flex h-12 w-full min-w-0 px-4 py-3',
                  'text-foreground rounded-lg border border-white/10 bg-white/5',
                  'truncate text-base font-normal placeholder:text-white/30',
                  'transition-all outline-none',
                  'aria-invalid:border-destructive aria-invalid:text-destructive aria-invalid:bg-destructive/10',
                  'disabled:text-muted-foreground disabled:cursor-not-allowed disabled:border-white/5 disabled:bg-white/5 disabled:placeholder:text-white/20',
                  $slots.iconStart ? 'pl-12' : '',
                  $slots.iconEnd ? 'pr-12' : '',
                  props.class,
               )
            "
         />
         <!-- Plain mode (no mask prop): native v-model, no IMask instance is created. -->
         <input
            v-else
            :id="effectiveId"
            ref="inputRef"
            v-model="modelValue"
            v-bind="rawAttrs"
            @blur="touched = true"
            @input="touched = true"
            data-slot="input"
            :aria-invalid="ariaInvalid"
            :disabled="disabled"
            :class="
               cn(
                  'flex h-12 w-full min-w-0 px-4 py-3',
                  'text-foreground rounded-lg border border-white/10 bg-white/5',
                  'truncate text-base font-normal placeholder:text-white/30',
                  'transition-all outline-none',
                  'aria-invalid:border-destructive aria-invalid:text-destructive aria-invalid:bg-destructive/10',
                  'disabled:text-muted-foreground disabled:cursor-not-allowed disabled:border-white/5 disabled:bg-white/5 disabled:placeholder:text-white/20',
                  $slots.iconStart ? 'pl-12' : '',
                  $slots.iconEnd ? 'pr-12' : '',
                  props.class,
               )
            "
         />

         <div v-if="$slots.iconEnd" class="absolute inset-y-0 right-0 mr-4 flex items-center">
            <slot name="iconEnd" :color="iconColor" />
         </div>
         <div v-if="$slots.otherIcon" class="inset-y-0 right-0 ml-2 flex items-center">
            <slot name="otherIcon" />
         </div>
      </div>
   </div>
</template>

<script setup lang="ts">
import type { HTMLAttributes } from 'vue';
import { ref, computed, watch, onMounted, onBeforeUnmount, useAttrs, useId } from 'vue';
import { useVModel } from '@vueuse/core';
import { cn } from '@/utils/general';
import IMask, { InputMask } from 'imask';
import Label from './Label.vue';

defineOptions({ inheritAttrs: false });

type InputProps = {
   defaultValue?: string | number;
   modelValue?: string | number;
   class?: HTMLAttributes['class'];
   containerClass?: HTMLAttributes['class'];
   labelClass?: HTMLAttributes['class'];
   label?: string;
   required?: boolean;
   error?: boolean;
   disabled?: boolean;
   /**
    * Omit for a plain text input (no IMask instance is created).
    * 'number' -> imask numeric input; 'phone' -> imask 000-000-0000 input;
    * 'custom' -> pass your own IMask pattern via `maskPattern`.
    * Fixed at mount (the IMask instance is created once) — don't toggle it dynamically.
    */
   mask?: 'number' | 'phone' | 'custom';
   /** Plain mode only: characters matching this regex are stripped as the user types. */
   filterPattern?: RegExp;
   // ---- mask="number" only ----
   max?: number;
   decimal?: number;
   allowZero?: boolean;
   allowSigned?: boolean;
   formatNumber?: boolean;
   maxDigits?: number;
   formatTaxId?: boolean;
   inputMode?: 'string' | 'number';
   /**
    * mask="custom" only — any IMask `mask` option value (pattern string like
    * '0-0000-00000-00-0', a RegExp, etc.). See https://imask.js.org/guide.html for syntax —
    * pattern strings use `0` for a digit placeholder, `a` for a letter, `*` for any char.
    */
   maskPattern?: string | RegExp;
};

const props = withDefaults(defineProps<InputProps>(), {
   allowZero: true,
   inputMode: 'number',
});

const rawAttrs = useAttrs();
const generatedId = useId();
const effectiveId = computed(() => (rawAttrs.id as string | undefined) ?? generatedId);

const emits = defineEmits<{
   (e: 'update:modelValue', payload: string | number): void;
}>();

const modelValue = props.mask
   ? useVModel(props, 'modelValue', emits, {
        passive: true,
        defaultValue: props.defaultValue,
     })
   : useVModel(props, 'modelValue', emits, {
        passive: false,
        defaultValue: props.defaultValue,
     });

const touched = ref(false);

watch(modelValue, (newValue) => {
   if (!props.mask && props.filterPattern && typeof newValue === 'string') {
      const sanitized = newValue.replace(props.filterPattern, '');
      if (sanitized !== newValue) {
         modelValue.value = sanitized;
      }
   }
});

const isEmptyValue = (v: string | number | null | undefined) =>
   v === undefined || v === null || v === '';

const ariaInvalid = computed(
   () => props.error || (props.required && touched.value && isEmptyValue(props.modelValue)),
);

const maskedType = computed(() => (props.mask === 'custom' ? 'text' : 'tel'));
const maskedInputmode = computed(() => {
   if (props.mask === 'custom') return 'text';
   if (props.mask === 'number' && (props.decimal ?? 0) > 0) return 'decimal';
   return 'numeric';
});

const iconColor = computed(() => {
   if (props.disabled) return 'var(--color-muted-foreground, #6b7280)';
   if (ariaInvalid.value) return 'var(--color-destructive, #ff6b6b)';
   return 'var(--color-primary, #9c88ff)';
});

const inputRef = ref<HTMLInputElement | null>(null);
let maskInstance: InputMask<any> | null = null;
let isSyncingFromModel = false;

defineExpose({ inputRef, focus: () => inputRef.value?.focus() });

const limitDigits = (value: string) => {
   if (!props.maxDigits) return value;
   return value.slice(0, props.maxDigits);
};

const normalizeTaxId = (value: string) => {
   return String(value ?? '')
      .replace(/\D/g, '')
      .slice(0, 13);
};

const digitsOnly = (v: unknown) => {
   if (props.mask === 'phone') {
      return String(v ?? '').replace(/\D/g, '');
   }

   if (props.mask === 'custom') {
      return String(v ?? '');
   }

   if (props.formatTaxId) {
      return normalizeTaxId(v as string);
   }

   let pattern = /\D/g;
   if (props.allowSigned) {
      if (props.decimal && props.decimal > 0) {
         pattern = /[^\d.-]/g;
      } else {
         pattern = /[^\d-]/g;
      }
   } else if (props.decimal && props.decimal > 0) {
      pattern = /[^\d.]/g;
   }
   return String(v ?? '').replace(pattern, '');
};

function maskedAttrs() {
   const { value, modelValue: _ignored, ...rest } = rawAttrs as Record<string, any>;
   return rest;
}

function syncMaskFromModel() {
   if (!maskInstance) return;

   let target = digitsOnly(props.modelValue);

   if (props.mask === 'number') {
      target = limitDigits(target);
      if (props.formatTaxId) {
         target = normalizeTaxId(target);
      }
   }

   if (maskInstance.unmaskedValue !== target) {
      maskInstance.unmaskedValue = target;
      maskInstance.updateValue();
   }
}

onMounted(() => {
   if (!props.mask || !inputRef.value) return;
   const el = inputRef.value;

   if (props.mask === 'phone') {
      maskInstance = IMask(el, { mask: '000-000-0000' });

      maskInstance.on('accept', () => {
         if (!isSyncingFromModel) touched.value = true;
         const next = maskInstance!.unmaskedValue;
         if (digitsOnly(props.modelValue) !== next) {
            emits('update:modelValue', next);
         }
      });
   } else if (props.mask === 'custom') {
      if (!props.maskPattern && import.meta.env.DEV) {
         console.warn(
            '[VueInput] mask="custom" set with no maskPattern — input will accept nothing.',
         );
      }
      maskInstance = IMask(el, { mask: props.maskPattern ?? /^$/ } as any);

      maskInstance.on('accept', () => {
         if (!isSyncingFromModel) touched.value = true;
         const next = maskInstance!.unmaskedValue;
         if (modelValue.value !== next) {
            modelValue.value = next;
         }
      });
   } else {
      if (props.formatTaxId) {
         maskInstance = IMask(el, {
            mask: '0 0000 00000 00 0',
         });
      } else if (props.inputMode === 'string') {
         maskInstance = IMask(el, {
            mask: /^\d*$/,
         });
      } else {
         const minProp = props.allowSigned ? {} : { min: 0 };

         maskInstance = IMask(el, {
            mask: Number,
            scale: props.decimal ?? 0,
            thousandsSeparator: props.formatNumber ? ',' : '',
            padFractionalZeros: (props.decimal ?? 0) > 0,
            normalizeZeros: true,
            radix: '.',
            signed: props.allowSigned ?? false,
            ...minProp,
         });
      }

      maskInstance.on('accept', () => {
         if (!isSyncingFromModel) touched.value = true;
         let next = maskInstance!.unmaskedValue;

         next = limitDigits(next);

         if (maskInstance && maskInstance.unmaskedValue !== next) {
            maskInstance.unmaskedValue = next;
         }
         if (props.inputMode === 'number') {
            if (props.max !== undefined && next && Number(next) > props.max) {
               next = props.max === 0 ? '' : String(props.max);
               if (maskInstance) {
                  maskInstance.unmaskedValue = next;
               }
            }

            if (!props.allowSigned && props.allowZero === false && next === '0') {
               next = '';
               if (maskInstance) {
                  maskInstance.unmaskedValue = '';
               }
            }
         }

         if (props.formatTaxId) {
            next = normalizeTaxId(next);
            maskInstance!.unmaskedValue = next;
         }

         if (digitsOnly(modelValue.value) !== next) {
            modelValue.value = next;
         }
      });
   }

   isSyncingFromModel = true;
   const init = digitsOnly(props.modelValue);
   if (String(props.modelValue ?? '') !== init) {
      emits('update:modelValue', init);
   }
   syncMaskFromModel();
   isSyncingFromModel = false;
});

onBeforeUnmount(() => {
   maskInstance?.destroy();
   maskInstance = null;
});

if (props.mask) {
   watch(
      () => props.modelValue,
      (nv) => {
         isSyncingFromModel = true;
         try {
            if (props.mask === 'phone') {
               const cleaned = digitsOnly(nv);
               if (String(nv ?? '') !== cleaned) {
                  emits('update:modelValue', cleaned);
                  return;
               }
               if (maskInstance) syncMaskFromModel();
               return;
            }

            if (props.mask === 'custom') {
               if (maskInstance) syncMaskFromModel();
               return;
            }

            let cleaned = digitsOnly(nv);
            cleaned = limitDigits(cleaned);

            if (props.inputMode === 'number' && !!cleaned && String(nv ?? '') !== cleaned) {
               emits('update:modelValue', cleaned);
               return;
            }

            if (maskInstance) syncMaskFromModel();
         } finally {
            isSyncingFromModel = false;
         }
      },
   );
}
</script>
