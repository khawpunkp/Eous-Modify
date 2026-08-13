// The repo's CHANGELOG.md, bundled at build time rather than fetched. It ships with the installed
// build, so it always describes the version actually running — and needs no network to read.
import changelogSource from '../../CHANGELOG.md?raw';

export interface ChangelogEntry {
   version: string;
   /** Everything under the version heading, verbatim. Plain text by convention — see CHANGELOG.md. */
   body: string;
}

/** Matches a `## 1.2.3` heading, tolerating the `## [1.2.3]` form some changelogs use. */
const VERSION_HEADING = /^##\s+\[?(\d+\.\d+\.\d+)\]?/;

/**
 * Splits CHANGELOG.md into its per-version sections, in file order (newest first by convention).
 *
 * Deliberately the same rule the release workflow applies when it extracts a tag's notes, so what the
 * app shows here and what the update prompt shows can't drift apart.
 */
export function parseChangelog(markdown: string): ChangelogEntry[] {
   const entries: ChangelogEntry[] = [];
   let current: { version: string; lines: string[] } | null = null;

   for (const line of markdown.split(/\r?\n/)) {
      const heading = VERSION_HEADING.exec(line);
      if (heading) {
         if (current)
            entries.push({ version: current.version, body: current.lines.join('\n').trim() });
         current = { version: heading[1], lines: [] };
         continue;
      }
      current?.lines.push(line);
   }

   if (current) entries.push({ version: current.version, body: current.lines.join('\n').trim() });

   return entries.filter((entry) => entry.body.length > 0);
}

export const CHANGELOG: ChangelogEntry[] = parseChangelog(changelogSource);
