import type { AgentDetails } from '../types';

export function resolveAgentImageSrc(baseImage: string | null): string {
   if (!baseImage) return '/images/entities/anonymous.webp';
   if (baseImage.startsWith('data:')) return baseImage;
   return `/images/entities/${baseImage}`;
}

const EMPTY_DETAILS: AgentDetails = {
   rank: '',
   attribute: '',
   speciality: '',
};

export function parseAgentDetails(details: string | null): AgentDetails {
   if (!details) return { ...EMPTY_DETAILS };
   try {
      const parsed = JSON.parse(details);
      return {
         rank: parsed.rank ?? '',
         attribute: parsed.attribute ?? '',
         speciality: parsed.speciality ?? '',
      };
   } catch {
      return { ...EMPTY_DETAILS };
   }
}

export function serializeAgentDetails(details: AgentDetails): string {
   return JSON.stringify(details);
}

// Fixed filter/badge icon taxonomy, ported from the old app — shared between the rank/attribute/
// speciality filter chips and the character card's attribute icon chips, so both always agree.
export const RANK_ICONS: Record<string, string> = {
   S: '/images/filters/zzz/s-rank.webp',
   A: '/images/filters/zzz/a-rank.webp',
};

export const ATTRIBUTE_ICONS: Record<string, string> = {
   Physical: '/images/filters/zzz/phisical.webp',
   HonedEdge: '/images/filters/zzz/honed-edge.webp',
   Fire: '/images/filters/zzz/fire.webp',
   Ice: '/images/filters/zzz/ice.webp',
   Frost: '/images/filters/zzz/frost.webp',
   Electric: '/images/filters/zzz/electric.webp',
   Ether: '/images/filters/zzz/ether.webp',
   AuricInk: '/images/filters/zzz/auric-ink.webp',
   Lumiflux: '/images/filters/zzz/lumiflux.webp',
};

export const SPECIALITY_ICONS: Record<string, string> = {
   Attack: '/images/filters/zzz/attack.webp',
   Stun: '/images/filters/zzz/stun.webp',
   Anomaly: '/images/filters/zzz/anomaly.webp',
   Support: '/images/filters/zzz/support.webp',
   Defense: '/images/filters/zzz/defense.webp',
   Rupture: '/images/filters/zzz/rupture.webp',
};

/** Flat 3-color rank system ported from the old app's `getRarityColor` (case-insensitive, also
 * accepts the legacy "5 star"/"4 star" strings some early data used in place of S/A). */
export function rankColor(rank: string): string {
   const normalized = rank.toLowerCase();
   if (normalized === 's' || normalized === '5 star') return '#ffcc00';
   if (normalized === 'a' || normalized === '4 star') return '#a259ec';
   return '#888888';
}
