<script setup lang="ts">
import { useRoute } from 'vue-router';
import Sidebar from '../components/layout/Sidebar.vue';

const route = useRoute();
</script>

<template>
   <div class="flex h-screen bg-black/20">
      <Sidebar />
      <main class="relative flex-1 overflow-y-auto p-6">
         <RouterView v-slot="{ Component }">
            <Transition name="fade" mode="out-in">
               <component :is="Component" :key="route.fullPath" />
            </Transition>
         </RouterView>
      </main>
   </div>
</template>
