import { computed, onBeforeUnmount, onMounted, ref } from 'vue';

const openCount = ref(0);

/**
 * Whether a dialog is on screen.
 *
 * Tauri delivers drag-drop events to the window rather than to whatever sits under the cursor, so a
 * drop handler cannot tell the pointer was over a dialog. This is that missing signal: a dialog owns
 * the window while it is up, and the importer leaves drops alone rather than opening a second dialog
 * behind the first. Every full-screen dialog here is z-100, so which one paints on top comes down to
 * DOM order — an accident of where its component is mounted, not a decision anyone made.
 */
export const isModalOpen = computed(() => openCount.value > 0);

/**
 * Counts the calling component as a dialog for as long as it is mounted.
 *
 * Every dialog is mounted behind a `v-if`, so being mounted and being open are the same thing here.
 * ConfirmDialog is deliberately left out: it stays mounted for the app's whole life and shows itself
 * from its own state, and it is the one dialog a drop can raise by itself.
 */
export function useModalPresence() {
   onMounted(() => {
      openCount.value += 1;
   });
   onBeforeUnmount(() => {
      openCount.value -= 1;
   });
}
