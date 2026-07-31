import { PhUsersThree, PhGhost, PhCube, PhPalette, PhShapes, PhSword } from '@phosphor-icons/vue';
import type { Component } from 'vue';

// Shared with Sidebar.vue's nav items, so a category's page-title icon always matches whichever
// icon links to it from the sidebar.
export const CATEGORY_ICONS: Record<string, Component> = {
   npcs: PhUsersThree,
   enemies: PhGhost,
   weapons: PhSword,
   objects: PhCube,
   ui: PhPalette,
};

export function categoryIcon(slug: string | undefined): Component {
   return (slug && CATEGORY_ICONS[slug]) || PhShapes;
}
