<script setup lang="ts">
import { SelectValue, type SelectValueProps } from 'reka-ui';

const props = defineProps<SelectValueProps>();
</script>

<template>
   <SelectValue
      data-slot="select-value"
      class="line-clamp-1 text-start text-ellipsis"
      v-bind="props"
   >
      <slot />
   </SelectValue>
</template>
