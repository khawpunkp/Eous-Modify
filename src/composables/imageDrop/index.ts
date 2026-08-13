import { onBeforeUnmount, onMounted, ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import type { UnlistenFn } from '@tauri-apps/api/event';

/** Matches the file dialog's own filter, so dragging and picking accept the same things. */
const IMAGE_EXTENSIONS = ['png', 'jpg', 'jpeg', 'webp', 'gif'];

/**
 * Drop targets in mount order. Only the last one handles a drop.
 *
 * Tauri's drag-drop event is delivered to the *window*, not to whatever element is under the cursor,
 * so every mounted target hears every drop. Without this, dropping an image into the mod editor
 * would also replace the agent image on the page behind it. Modals mount after the page they open
 * over, so "most recently mounted" is the one the user is looking at.
 */
const targets: symbol[] = [];

function isImagePath(path: string): boolean {
   const extension = path.split('.').pop()?.toLowerCase() ?? '';
   return IMAGE_EXTENSIONS.includes(extension);
}

/**
 * Accepts an image dropped anywhere on the window while this component is the frontmost drop target,
 * handing back a data URL — the same shape the file dialog path produces, so callers treat a drop and
 * a pick identically.
 *
 * `isDraggingOver` is for highlighting the drop zone; it only goes true for the active target.
 */
export function useImageDrop(onImage: (dataUrl: string) => void) {
   const isDraggingOver = ref(false);
   const errorMessage = ref<string | null>(null);
   const token = Symbol('image-drop-target');
   let unlisten: UnlistenFn | null = null;

   const isActive = () => targets[targets.length - 1] === token;

   onMounted(async () => {
      targets.push(token);

      unlisten = await getCurrentWebview().onDragDropEvent(async (event) => {
         if (!isActive()) return;

         if (event.payload.type === 'over') {
            isDraggingOver.value = true;
            // A new drag supersedes the last complaint. Without this the message from a rejected file
            // sits there indefinitely, since nothing else ever takes it down.
            errorMessage.value = null;
            return;
         }

         if (event.payload.type === 'leave') {
            isDraggingOver.value = false;
            return;
         }

         isDraggingOver.value = false;
         errorMessage.value = null;

         // Only ever one image, even if several files are dropped — there is one slot to fill, and
         // silently picking the first of five is less surprising than refusing outright.
         const path = event.payload.paths.find(isImagePath);
         if (!path) {
            errorMessage.value = 'That file is not an image. Try a PNG, JPG, WEBP or GIF.';
            return;
         }

         try {
            onImage(await invoke<string>('read_image_as_data_url', { path }));
         } catch (e) {
            errorMessage.value = String(e);
         }
      });
   });

   onBeforeUnmount(() => {
      unlisten?.();
      const index = targets.lastIndexOf(token);
      if (index !== -1) targets.splice(index, 1);
   });

   /** For the callers that also set an image by other means — picking one should retire a drop error. */
   function clearError() {
      errorMessage.value = null;
   }

   return { isDraggingOver, errorMessage, clearError };
}
