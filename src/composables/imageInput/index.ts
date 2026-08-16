import { onBeforeUnmount, onMounted, ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import type { UnlistenFn } from '@tauri-apps/api/event';

/** Matches the file dialog's own filter, so dragging and picking accept the same things. */
const IMAGE_EXTENSIONS = [
   'png',
   'jpg',
   'jpeg',
   'jpe',
   'jif',
   'jfif',
   'webp',
   'gif',
   'svg',
   'ico',
   'bmp',
   'avif',
   'heic',
   'heif',
   'jxl',
   'apng',
   'tif',
   'tiff',
];

/**
 * The same set as above, by MIME type, for the clipboard — which hands over bytes and a type rather
 * than a path.
 *
 * Checked rather than waved through because the backend decides a saved file's extension from this
 * type and falls back to `.png` for anything it does not recognise. A pasted BMP would land as
 * `mod_preview.png` holding BMP bytes, which nothing can then display.
 */
const IMAGE_MIME_TYPES = [
   // รูปแบบมาตรฐานสากล (รองรับทุกเบราว์เซอร์)
   'image/jpeg',
   'image/png',
   'image/gif',
   'image/svg+xml',
   'image/webp',

   // ไอคอนและรูปแบบบิตแมปทั่วไป
   'image/x-icon', // หรือ "image/vnd.microsoft.icon"
   'image/bmp',

   // รูปแบบภาพสมัยใหม่ (Modern Formats)
   'image/avif',
   'image/heic', // Safari / Apple WebKit
   'image/heif', // Safari / Apple WebKit
   'image/jxl', // JPEG XL (ขึ้นอยู่กับการเปิด flag หรือรองรับในบางเบราว์เซอร์)

   // รูปแบบภาพเคลื่อนไหว / อื่นๆ
   'image/apng',
   'image/tiff', // Safari / บางแพลตฟอร์มเฉพาะทาง
];

/** The message both paths give for something that is not a usable image. */
const FORMAT_NOT_SUPPORTED = 'Unsupported file format.';

/** Reads clipboard bytes into the same `data:` URL shape `read_image_as_data_url` returns. */
function readAsDataUrl(file: File): Promise<string> {
   return new Promise((resolve, reject) => {
      const reader = new FileReader();
      reader.onload = () => resolve(String(reader.result));
      reader.onerror = () => reject(new Error('Could not read the pasted image.'));
      reader.readAsDataURL(file);
   });
}

/** Long enough to read a sentence, short enough not to linger over a corrected mistake. */
const ERROR_VISIBLE_MS = 5000;

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
 * Accepts an image dropped or pasted anywhere on the window while this component is the frontmost
 * target, handing back a data URL — the same shape the file dialog path produces, so callers treat a
 * drop, a paste and a pick identically.
 *
 * Both routes live here rather than in a composable each because they share the target stack below;
 * two registrations per component would leave whichever registered second permanently "active".
 *
 * `isDraggingOver` is for highlighting the drop zone; it only goes true for the active target.
 */
export function useImageInput(onImage: (dataUrl: string) => void) {
   const isDraggingOver = ref(false);
   const errorMessage = ref<string | null>(null);
   const token = Symbol('image-drop-target');
   let unlisten: UnlistenFn | null = null;
   let dismissTimer: ReturnType<typeof setTimeout> | null = null;

   const isActive = () => targets[targets.length - 1] === token;

   /**
    * Shows a message and takes it back down on a timer.
    *
    * Deliberately not tied to any drag event. Clearing it on the next `over` looked right but made
    * the message flash and vanish, because a drop is followed by another `over` — so the thing that
    * was supposed to retire the *previous* complaint immediately killed the new one. A timer does not
    * care what order the events arrive in.
    */
   function fail(message: string) {
      errorMessage.value = message;
      if (dismissTimer) clearTimeout(dismissTimer);
      dismissTimer = setTimeout(() => {
         errorMessage.value = null;
         dismissTimer = null;
      }, ERROR_VISIBLE_MS);
   }

   onMounted(async () => {
      targets.push(token);

      unlisten = await getCurrentWebview().onDragDropEvent(async (event) => {
         if (!isActive()) return;

         if (event.payload.type === 'over') {
            isDraggingOver.value = true;
            return;
         }

         if (event.payload.type === 'leave') {
            isDraggingOver.value = false;
            return;
         }

         isDraggingOver.value = false;

         // Only ever one image, even if several files are dropped — there is one slot to fill, and
         // silently picking the first of five is less surprising than refusing outright.
         const path = event.payload.paths.find(isImagePath);
         if (!path) {
            fail(FORMAT_NOT_SUPPORTED);
            return;
         }

         try {
            clearError();
            onImage(await invoke<string>('read_image_as_data_url', { path }));
         } catch (e) {
            fail(String(e));
         }
      });

      window.addEventListener('paste', handlePaste);
   });

   /**
    * Takes an image off the clipboard, so a screenshot can go straight in without being saved first.
    *
    * Unlike a drop there is no file on disk to point the backend at, so the bytes are read here into
    * the same `data:` URL the other routes produce — everything downstream is then identical.
    */
   async function handlePaste(event: ClipboardEvent) {
      if (!isActive()) return;

      // A paste while typing means text, even when the clipboard is also holding an image. Leaving
      // fields alone costs nothing: anywhere else in the modal still takes the image.
      const target = event.target as HTMLElement | null;
      if (target?.isContentEditable || target?.closest('input, textarea')) return;

      const items = Array.from(event.clipboardData?.items ?? []);
      const image = items.find((item) => item.kind === 'file' && item.type.startsWith('image/'));
      // Nothing image-shaped on the clipboard is not a mistake worth reporting — the user may simply
      // have pasted text with nowhere to put it.
      if (!image) return;

      event.preventDefault();

      const file = image.getAsFile();
      if (!file || !IMAGE_MIME_TYPES.includes(file.type)) {
         fail(FORMAT_NOT_SUPPORTED);
         return;
      }

      try {
         clearError();
         onImage(await readAsDataUrl(file));
      } catch (e) {
         fail(String(e));
      }
   }

   onBeforeUnmount(() => {
      unlisten?.();
      window.removeEventListener('paste', handlePaste);
      if (dismissTimer) clearTimeout(dismissTimer);
      const index = targets.lastIndexOf(token);
      if (index !== -1) targets.splice(index, 1);
   });

   /** For the callers that also set an image by other means — picking one should retire a drop error. */
   function clearError() {
      if (dismissTimer) clearTimeout(dismissTimer);
      dismissTimer = null;
      errorMessage.value = null;
   }

   return { isDraggingOver, errorMessage, clearError };
}
