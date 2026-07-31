import { ref, computed, onMounted, onBeforeUnmount, nextTick, watch } from 'vue';

const scrollRef = ref<HTMLElement | null>(null);
const scrollHeight = computed(() => scrollRef.value?.scrollHeight);
const isAtBottom = ref(true);

export function useScroll() {
   const scrolling = () => {
      const el = scrollRef.value;
      if (!el) {
         isAtBottom.value = false;
         return;
      }
      isAtBottom.value = !(el.scrollTop + el.clientHeight < el.scrollHeight - 1);
   };

   onMounted(() => {
      nextTick(() =>
         setTimeout(() => {
            scrolling();
         }, 100),
      );
   });

   watch(
      scrollRef,
      (el, prevEl) => {
         prevEl?.removeEventListener('scroll', scrolling);
         el?.addEventListener('scroll', scrolling);
      },
      { immediate: true },
   );

   onBeforeUnmount(() => {
      scrollRef.value?.removeEventListener('scroll', scrolling);
   });

   return {
      scrollRef,
      scrollHeight,
      isAtBottom,
      scrolling,
   };
}
