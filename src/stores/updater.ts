import { markRaw } from 'vue';
import { defineStore } from 'pinia';
import { check, type Update } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';

export const useUpdaterStore = defineStore('updater', {
   state: () => ({
      update: null as Update | null,
      isChecking: false,
      isDownloading: false,
      downloadedBytes: 0,
      totalBytes: 0,
      errorMessage: null as string | null,
      isReadyToRestart: false,
   }),
   actions: {
      async check() {
         this.isChecking = true;
         this.errorMessage = null;
         try {
            // The only place an Update is released. Closing the prompt deliberately keeps it, so the
            // Settings button can go on offering it, which leaves the next check to free the old one.
            if (this.update) await this.update.close();
            const found = await check();
            // `Update` is a class instance with #private fields. Pinia state is reactive, so storing
            // it bare wraps it in a Proxy — and reading a #private field through a Proxy throws
            // "Cannot read private member from an object whose class did not declare it", which broke
            // both downloadAndInstall() and close(). markRaw keeps the instance unproxied; the
            // `update` property itself is still reactive, so the UI still reacts to it being set.
            this.update = found ? markRaw(found) : null;
         } catch (e) {
            this.errorMessage = String(e);
         } finally {
            this.isChecking = false;
         }
         return this.update;
      },
      async downloadAndInstall() {
         if (!this.update) return;
         this.isDownloading = true;
         this.downloadedBytes = 0;
         this.totalBytes = 0;
         this.errorMessage = null;
         try {
            await this.update.downloadAndInstall((event) => {
               if (event.event === 'Started') {
                  this.totalBytes = event.data.contentLength ?? 0;
               } else if (event.event === 'Progress') {
                  this.downloadedBytes += event.data.chunkLength;
               }
            });
            this.isReadyToRestart = true;
         } catch (e) {
            this.errorMessage = String(e);
         } finally {
            this.isDownloading = false;
         }
      },
      async restart() {
         await relaunch();
      },
      /**
       * Drops a failed download's message while keeping the update itself on offer.
       *
       * Replaces the old `dismiss()`, which also closed the Update and nulled it — that made closing
       * the prompt forget the version it had just found. The error still has to go, or a failure
       * stays on the Settings page long after the prompt it belonged to was closed.
       */
      clearError() {
         this.errorMessage = null;
      },
   },
});
