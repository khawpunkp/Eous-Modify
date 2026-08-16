<script setup lang="ts">
/**
 * A preview image that never crops, on any background it happens to land on.
 *
 * The sources here are wildly inconsistent. A GameBanana preview is a composed 16:9 banner carrying
 * title text and a collage of shots; a user's own capture is a raw 16:9 game screenshot; a picked
 * file can be any shape at all. Cropping to fill destroys the composed ones — the title is the first
 * thing to go — and there is no way to tell the two apart, because a banner and a screenshot share
 * the exact same aspect ratio.
 *
 * So it is always contained, and the leftover space is filled with a blurred, over-scaled copy of
 * the same image rather than a flat colour. Nothing is ever cut off, the fill matches whatever the
 * picture happens to be, and the result reads as deliberate rather than letterboxed. The browser
 * decodes the file once and reuses it, so the second element costs no extra fetch.
 *
 * Shape and corners come from the caller's own classes, which Vue merges onto the root.
 */
defineProps<{ src: string }>();
</script>

<template>
   <div class="bg-foreground relative w-full overflow-hidden">
      <!-- Decorative: the same picture again, standing in for a background. Over-scaled so the blur
           has something to bleed into instead of fading out at the edges. -->
      <img
         v-if="!src.includes('no-data')"
         :src="src"
         alt=""
         aria-hidden="true"
         class="absolute inset-0 size-full scale-110 object-cover blur-md"
      />
      <img :src="src" alt="" class="relative size-full object-contain" />
   </div>
</template>
