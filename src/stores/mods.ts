import { defineStore } from 'pinia';
import { invoke } from '@tauri-apps/api/core';
import type { Mod, ModGroup, ModInput } from '../types';

export const useModsStore = defineStore('mods', {
   state: () => ({
      mods: [] as Mod[],
      groups: [] as ModGroup[],
      isLoading: false,
   }),
   actions: {
      async fetchByAgent(agentId: number) {
         this.isLoading = true;
         try {
            this.mods = await invoke<Mod[]>('list_mods', {
               agentId,
               categoryId: null,
               categoryItemId: null,
            });
         } finally {
            this.isLoading = false;
         }
      },
      async fetchByCategory(categoryId: number) {
         this.isLoading = true;
         try {
            this.mods = await invoke<Mod[]>('list_mods', {
               agentId: null,
               categoryId,
               categoryItemId: null,
            });
         } finally {
            this.isLoading = false;
         }
      },
      async fetchUncategorized() {
         this.isLoading = true;
         try {
            this.mods = await invoke<Mod[]>('list_uncategorized_mods');
         } finally {
            this.isLoading = false;
         }
      },
      async updateCategory(modId: number, target: { agentId?: number; categoryId?: number }) {
         const updated = await invoke<Mod>('update_mod_category', {
            modId,
            agentId: target.agentId ?? null,
            categoryId: target.categoryId ?? null,
            categoryItemId: null,
         });
         this.mods = this.mods.filter((m) => m.id !== modId);
         return updated;
      },
      // No reload call here: the backend has to send it itself, immediately after writing the mod's
      // persisted variables back, or 3DMigoto can overwrite them before it reads them.
      async toggle(modId: number) {
         const isEnabled = await invoke<boolean>('toggle_mod_enabled', { modId });
         const mod = this.mods.find((m) => m.id === modId);
         if (mod) mod.isEnabled = isEnabled;
         return isEnabled;
      },
      async update(modId: number, input: ModInput) {
         const updated = await invoke<Mod>('update_mod_info', { modId, input });
         const index = this.mods.findIndex((m) => m.id === modId);
         if (index !== -1) this.mods[index] = updated;
         return updated;
      },
      async remove(modId: number) {
         await invoke('delete_mod', { modId });
         this.mods = this.mods.filter((m) => m.id !== modId);
      },
      async openFolder(modId: number) {
         await invoke('open_mod_folder', { modId });
      },
   },
});
