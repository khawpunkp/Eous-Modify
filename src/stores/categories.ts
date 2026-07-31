import { defineStore } from 'pinia';
import { invoke } from '@tauri-apps/api/core';
import type { Category } from '../types';

export const useCategoriesStore = defineStore('categories', {
   state: () => ({
      categories: [] as Category[],
      isLoading: false,
   }),
   actions: {
      async fetchAll() {
         this.isLoading = true;
         try {
            this.categories = await invoke<Category[]>('list_categories');
         } finally {
            this.isLoading = false;
         }
      },
      bySlug(slug: string) {
         return this.categories.find((c) => c.slug === slug) ?? null;
      },
   },
});
