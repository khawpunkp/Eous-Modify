<script setup lang="ts">
import type { SelectRootEmits, SelectRootProps, AcceptableValue } from 'reka-ui';
import { SelectRoot, useForwardPropsEmits } from 'reka-ui';
import { debounce } from 'lodash-es';
import { type HTMLAttributes } from 'vue';
import { ref, computed, watch, onMounted, onUnmounted, nextTick, useId, useAttrs } from 'vue';
import { reactiveOmit } from '@vueuse/core';
import { cn, normalize } from '@/utils/general';

defineOptions({ inheritAttrs: false });
import { useScroll } from '@/composables/scroll';
import Label from '@/components/ui/input/Label.vue';
import VueInput from '@/components/ui/input/VueInput.vue';
import SelectTrigger from './SelectTrigger.vue';
import SelectValue from './SelectValue.vue';
import SelectContent from './SelectContent.vue';
import SelectGroup from './SelectGroup.vue';
import SelectItem from './SelectItem.vue';

const props = withDefaults(
   defineProps<
      SelectRootProps & {
         placeholder?: string;
         options?: { label: string; value: any; disabled?: boolean }[];
         error?: boolean;
         searchable?: boolean;
         searchPlaceholder?: string;
         class?: HTMLAttributes['class'];
         containerClass?: HTMLAttributes['class'];
         align?: 'start' | 'center' | 'end' | undefined;
         label?: string;
         clearable?: boolean;
         loading?: boolean;
         noOptionsText?: string;
         minSearchLength?: number;
         minSearchLengthText?: string;
      }
   >(),
   {
      noOptionsText: 'No options found',
      minSearchLength: 0,
      align: 'end',
   },
);
const emits = defineEmits<SelectRootEmits>();

const delegatedProps = reactiveOmit(
   props,
   'placeholder',
   'options',
   'error',
   'searchable',
   'searchPlaceholder',
   'class',
   'containerClass',
   'align',
   'label',
   'clearable',
   'loading',
   'noOptionsText',
   'minSearchLength',
   'minSearchLengthText',
);
const forwarded = useForwardPropsEmits(delegatedProps, emits);

const attrs = useAttrs();

const triggerId = useId();

const searchInput = ref('');
const searchInputRef = ref<InstanceType<typeof VueInput> | null>(null);

const debouncedSearch = ref('');
watch(
   searchInput,
   debounce((val: string) => {
      debouncedSearch.value = val;
   }, 200),
);

const isBelowMinSearchLength = computed(
   () => props.minSearchLength > 0 && debouncedSearch.value.length < props.minSearchLength,
);

const minSearchLengthText = computed(
   () => props.minSearchLengthText ?? `Type at least ${props.minSearchLength} characters to search`,
);

const filteredOptions = computed(() => {
   if (!props.options) return [];
   if (isBelowMinSearchLength.value) return [];
   if (!debouncedSearch.value) return props.options;

   const searchLower = debouncedSearch.value.toLowerCase();

   return props.options
      .filter((option) => normalize(option.label).includes(normalize(searchLower)))
      .slice(0, 100);
});

const isOpen = ref(props.open ?? props.defaultOpen ?? false);

watch(
   () => props.open,
   (val) => {
      if (val !== undefined) isOpen.value = val;
   },
);

function onOpenUpdate(val: boolean) {
   isOpen.value = val;
}

const focusSearchInput = () => {
   nextTick(() => {
      setTimeout(() => {
         searchInputRef.value?.focus();
      }, 0);
   });
};

watch(isOpen, (newVal) => {
   if (newVal) focusSearchInput();
   else searchInput.value = '';
});

const SEARCH_INPUT_PASSTHROUGH_KEYS = ['Escape', 'ArrowDown', 'ArrowUp', 'Enter', 'Tab'];
function onSearchInputKeydown(event: KeyboardEvent) {
   if (!SEARCH_INPUT_PASSTHROUGH_KEYS.includes(event.key)) {
      event.stopPropagation();
   }
}

const screenHeight = ref(window.innerHeight);

onMounted(() => {
   const handleResize = () => {
      screenHeight.value = window.innerHeight;
   };

   window.addEventListener('resize', handleResize);
   onUnmounted(() => window.removeEventListener('resize', handleResize));
});

const selectTriggerRef = ref<HTMLElement | null>(null);
const { scrollRef } = useScroll();
const scrollTriggerIntoScrollRefWithOffset = () => {
   const container = scrollRef.value;
   const trigger = selectTriggerRef.value;

   if (!container || !trigger) return;

   const containerRect = container.getBoundingClientRect();
   const triggerRect = trigger.getBoundingClientRect();

   const offsetTop = triggerRect.top - containerRect.top + container.scrollTop;

   container.scrollTo({
      top: offsetTop - 130,
      behavior: 'smooth',
   });
};

watch([screenHeight, isOpen], ([_, open]) => {
   if (open && selectTriggerRef.value) {
      nextTick(() => {
         scrollTriggerIntoScrollRefWithOffset();
      });
   }
});
</script>

<template>
   <SelectRoot
      data-slot="select"
      v-bind="forwarded"
      :open="isOpen"
      @update:open="onOpenUpdate"
      v-slot="{ open: slotOpen }"
   >
      <div
         ref="selectTriggerRef"
         v-bind="attrs"
         :class="cn('w-full', containerClass, props.label ? 'flex flex-col gap-2' : '')"
      >
         <Label v-if="label" :for="triggerId" :required="required">{{ label }}</Label>
         <SelectTrigger
            :id="triggerId"
            :error="error"
            :class="props.class"
            :is-open="slotOpen"
            :disabled="disabled || loading"
            :clearable="clearable"
            :loading="loading"
            :has-value="
               props.modelValue !== undefined &&
               props.modelValue !== null &&
               props.modelValue !== ''
            "
            @clear="emits('update:modelValue', undefined as unknown as AcceptableValue)"
         >
            <template v-if="$slots.icon" #icon="slotProps">
               <slot name="icon" v-bind="slotProps" />
            </template>

            <template #value>
               <SelectValue :placeholder="placeholder">
                  {{ options?.find((opt) => opt.value === modelValue)?.label ?? placeholder }}
               </SelectValue>
            </template>
         </SelectTrigger>
      </div>
      <SelectContent :align="align">
         <template #input>
            <VueInput
               type="text"
               v-if="searchable"
               class="h-10 rounded-sm px-2 py-1"
               container-class="m-1"
               ref="searchInputRef"
               v-model="searchInput"
               :placeholder="searchPlaceholder"
               @keydown="onSearchInputKeydown"
            />
         </template>
         <SelectGroup>
            <template v-if="isBelowMinSearchLength">
               <SelectItem disabled :value="null">{{ minSearchLengthText }}</SelectItem>
            </template>
            <template v-else-if="filteredOptions.length === 0">
               <SelectItem disabled :value="null">{{ noOptionsText }}</SelectItem>
            </template>
            <template v-else>
               <SelectItem
                  v-for="option in filteredOptions"
                  :key="option.value"
                  :value="option.value"
                  :disabled="option.disabled"
               >
                  {{ option.label }}
               </SelectItem>
            </template>
         </SelectGroup>
      </SelectContent>
   </SelectRoot>
</template>
