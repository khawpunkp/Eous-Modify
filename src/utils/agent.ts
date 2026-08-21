import type { AgentDetails } from '../types';

export function resolveAgentImageSrc(baseImage: string | null): string {
   if (!baseImage) return '/images/agents/anonymous.webp';
   if (baseImage.startsWith('data:')) return baseImage;
   return `/images/agents/${baseImage}`;
}

const EMPTY_DETAILS: AgentDetails = {
   rank: '',
   attribute: '',
   speciality: '',
   factionImage: '',
};

/**
 * Where a faction badge's image lives, or null for an agent that has no faction.
 *
 * The detail holds the file name itself, the way an agent's own base_image does, so a faction added
 * to the definitions needs no code change here and nothing has to be kept in step with the contents
 * of the folder.
 */
export function resolveFactionImageSrc(factionImage: string): string | null {
   return factionImage ? `/images/factions/${factionImage}` : null;
}

export function parseAgentDetails(details: string | null): AgentDetails {
   if (!details) return { ...EMPTY_DETAILS };
   try {
      const parsed = JSON.parse(details);
      return {
         rank: parsed.rank ?? '',
         attribute: parsed.attribute ?? '',
         speciality: parsed.speciality ?? '',
         factionImage: parsed.factionImage ?? '',
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
// I-Rank is a decorated S — its own badge in-game, but the same tier. Listed here so a card can draw
// the right icon, and left out of the filter chips on the agents page, exactly as HonedEdge, Frost
// and AuricInk are for their parent attributes.
export const RANK_ICONS: Record<string, string> = {
   I: '/images/filters/i-rank.png',
   S: '/images/filters/s-rank.png',
   A: '/images/filters/a-rank.png',
};

/** Ranks that are a flavour of another rank, so filtering by the parent still finds them. */
export const RANK_GROUPS: Record<string, string[]> = {
   S: ['S', 'I'],
};

export const ATTRIBUTE_ICONS: Record<string, string> = {
   Physical: '/images/filters/physical.png',
   HonedEdge: '/images/filters/honed-edge.png',
   Fire: '/images/filters/fire.png',
   Ice: '/images/filters/ice.png',
   Frost: '/images/filters/frost.png',
   Electric: '/images/filters/electric.png',
   Wind: '/images/filters/wind.png',
   Ether: '/images/filters/ether.png',
   AuricInk: '/images/filters/auric-ink.png',
   Lumiflux: '/images/filters/lumiflux.png',
};

export const SPECIALITY_ICONS: Record<string, string> = {
   Attack: '/images/filters/attack.png',
   Stun: '/images/filters/stun.png',
   Anomaly: '/images/filters/anomaly.png',
   Support: '/images/filters/support.png',
   Defense: '/images/filters/defense.png',
   Rupture: '/images/filters/rupture.png',
};
