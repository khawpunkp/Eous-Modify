import { PhLayout, PhFinnTheHuman, PhShapes } from '@phosphor-icons/vue';
import type { Component } from 'vue';

// Shared with Sidebar.vue's nav items, so a category's page-title icon always matches whichever
// icon links to it from the sidebar. Keyed by the slugs in definitions/zzz.toml — a category with no
// entry here still works, it just falls back to the generic shapes icon.
export const CATEGORY_ICONS: Record<string, Component> = {
   bangboos: PhFinnTheHuman,
   ui: PhLayout,
};

export function categoryIcon(slug: string | undefined): Component {
   return (slug && CATEGORY_ICONS[slug]) || PhShapes;
}
