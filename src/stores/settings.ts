import { defineStore } from 'pinia';
import { invoke } from '@tauri-apps/api/core';

export const useSettingsStore = defineStore('settings', {
   state: () => ({
      settings: {} as Record<string, string>,
      isLoading: false,
   }),
   actions: {
      async fetch(key: string) {
         const value = await invoke<string | null>('get_setting', { key });
         if (value !== null) {
            this.settings[key] = value;
         } else {
            delete this.settings[key];
         }
         return value;
      },
      async set(key: string, value: string) {
         await invoke('set_setting', { key, value });
         this.settings[key] = value;
      },
   },
});
