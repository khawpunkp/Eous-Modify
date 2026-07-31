import { defineStore } from 'pinia';
import { invoke } from '@tauri-apps/api/core';
import { useModsStore } from './mods';
import type { ModGroup } from '../types';

export const useModGroupsStore = defineStore('modGroups', {
   state: () => ({
      groups: [] as ModGroup[],
      isLoading: false,
   }),
   actions: {
      async fetchAll() {
         this.isLoading = true;
         try {
            this.groups = await invoke<ModGroup[]>('list_mod_groups');
         } finally {
            this.isLoading = false;
         }
      },
      async create(name: string, baseImage: string | null, modIds: number[]) {
         const group = await invoke<ModGroup>('create_mod_group', { name, baseImage, modIds });
         this.groups.push(group);
         const grouped = new Set(modIds);
         useModsStore().mods.forEach((m) => {
            if (grouped.has(m.id)) m.groupId = group.id;
         });
         return group;
      },
      async toggle(groupId: number) {
         const isEnabled = await invoke<boolean>('toggle_mod_group', { groupId });
         const group = this.groups.find((g) => g.id === groupId);
         if (group) {
            group.isEnabled = isEnabled;
            group.members.forEach((m) => (m.isEnabled = isEnabled));
         }
         return isEnabled;
      },
      async update(groupId: number, name: string, baseImage: string | null) {
         const updated = await invoke<ModGroup>('update_mod_group', { groupId, name, baseImage });
         const index = this.groups.findIndex((g) => g.id === groupId);
         if (index !== -1) this.groups[index] = updated;
         return updated;
      },
      async addMember(groupId: number, modId: number) {
         const updated = await invoke<ModGroup>('add_mod_to_group', { groupId, modId });
         const index = this.groups.findIndex((g) => g.id === groupId);
         if (index !== -1) this.groups[index] = updated;
         const addedMod = useModsStore().mods.find((m) => m.id === modId);
         if (addedMod) addedMod.groupId = groupId;
         return updated;
      },
      async removeMember(groupId: number, modId: number) {
         const groupBefore = this.groups.find((g) => g.id === groupId);
         const updated = await invoke<ModGroup | null>('remove_mod_from_group', { groupId, modId });
         const modsStore = useModsStore();
         if (updated) {
            const index = this.groups.findIndex((g) => g.id === groupId);
            if (index !== -1) this.groups[index] = updated;
            const removedMod = modsStore.mods.find((m) => m.id === modId);
            if (removedMod) removedMod.groupId = null;
         } else {
            // Backend auto-disbands (and clears every remaining member's group_id) once a group
            // would drop to <=1 member — so every original member needs clearing here, not just modId.
            this.groups = this.groups.filter((g) => g.id !== groupId);
            const memberIds = new Set(groupBefore?.members.map((m) => m.modId) ?? [modId]);
            modsStore.mods.forEach((m) => {
               if (memberIds.has(m.id)) m.groupId = null;
            });
         }
         return updated;
      },
      async disband(groupId: number) {
         const group = this.groups.find((g) => g.id === groupId);
         await invoke('delete_mod_group', { groupId });
         this.groups = this.groups.filter((g) => g.id !== groupId);
         if (group) {
            const memberIds = new Set(group.members.map((m) => m.modId));
            useModsStore().mods.forEach((m) => {
               if (memberIds.has(m.id)) m.groupId = null;
            });
         }
      },
   },
});
