<script setup lang="ts">
import { onBeforeUnmount, onMounted } from 'vue';
import VueButton from '@/components/ui/button/VueButton.vue';
import VueTypography from '@/components/ui/typography/VueTypography.vue';
import { pendingConfirm, resolveConfirm } from '@/composables/confirm';

// Escape is heard on the window rather than on the dialog: nothing here takes focus when it opens,
// so a key event aimed at the element would never arrive.
function handleKeydown(event: KeyboardEvent) {
   if (event.key === 'Escape' && pendingConfirm.value) resolveConfirm(false);
}

onMounted(() => window.addEventListener('keydown', handleKeydown));
onBeforeUnmount(() => window.removeEventListener('keydown', handleKeydown));
</script>

<template>
   <!-- .self so only the backdrop itself dismisses this: a click that started on the panel bubbles
        up here too. Safe because the dialog holds no unsaved input of its own, unlike the form
        modals, which deliberately don't close this way. -->
   <div
      v-if="pendingConfirm"
      class="fixed inset-0 z-100 flex items-center justify-center bg-black/60"
      @click.self="resolveConfirm(false)"
   >
      <div
         class="bg-card flex max-h-[85vh] w-11/12 max-w-120 flex-col gap-4 overflow-y-auto rounded-lg border border-white/10 p-6"
      >
         <VueTypography variant="TitleB" as="h2" class="wrap-anywhere">
            {{ pendingConfirm.title }}
         </VueTypography>
         <VueTypography variant="CaptionR" as="p" class="text-muted-foreground">
            {{ pendingConfirm.message }}
         </VueTypography>
         <div class="flex items-center justify-end gap-3">
            <VueButton
               v-if="!pendingConfirm.acknowledgeOnly"
               type="button"
               variant="outlined"
               class="min-w-32"
               @click="resolveConfirm(false)"
            >
               Cancel
            </VueButton>
            <VueButton
               type="button"
               :color="pendingConfirm.destructive ? 'error' : 'primary'"
               class="min-w-32"
               @click="resolveConfirm(true)"
            >
               {{ pendingConfirm.confirmLabel ?? 'Confirm' }}
            </VueButton>
         </div>
      </div>
   </div>
</template>
