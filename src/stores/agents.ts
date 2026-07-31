import { defineStore } from 'pinia';
import { invoke } from '@tauri-apps/api/core';
import type { Agent, AgentInput } from '../types';

export const useAgentsStore = defineStore('agents', {
   state: () => ({
      agents: [] as Agent[],
      isLoading: false,
   }),
   actions: {
      async fetchAll() {
         this.isLoading = true;
         try {
            this.agents = await invoke<Agent[]>('list_agents');
         } finally {
            this.isLoading = false;
         }
      },
      async fetchOne(slug: string) {
         return invoke<Agent>('get_agent', { slug });
      },
      async create(input: AgentInput) {
         const agent = await invoke<Agent>('create_agent', { input });
         this.agents.push(agent);
         return agent;
      },
      async update(slug: string, input: AgentInput) {
         const agent = await invoke<Agent>('update_agent', { slug, input });
         const index = this.agents.findIndex((a) => a.slug === slug);
         if (index !== -1) this.agents[index] = agent;
         return agent;
      },
      async remove(slug: string) {
         await invoke('delete_agent', { slug });
         this.agents = this.agents.filter((a) => a.slug !== slug);
      },
   },
});
