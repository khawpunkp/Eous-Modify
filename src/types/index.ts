export interface Agent {
   id: number;
   name: string;
   /** The name in full. Resolved backend-side, so it equals `name` when there is no separate one. */
   fullName: string;
   slug: string;
   details: string | null;
   baseImage: string | null;
   /** The bundled image on its own, so clearing a custom pick can preview it without a round trip. */
   defaultImage: string | null;
   /** True when baseImage came from a user pick rather than the bundled definitions. */
   hasCustomImage: boolean;
   isBuiltin: boolean;
   aliases: string[];
}

// Structured shape serialized into Agent.details (a free-form TEXT column in the schema).
export interface AgentDetails {
   rank: string;
   attribute: string;
   speciality: string;
}

export interface AgentInput {
   name: string;
   details: string | null;
   baseImage: string | null;
   aliases: string[];
}

export interface Category {
   id: number;
   name: string;
   slug: string;
}

export interface Mod {
   id: number;
   agentId: number | null;
   categoryId: number | null;
   categoryItemId: number | null;
   name: string;
   folderName: string;
   imageFilename: string | null;
   author: string | null;
   isEnabled: boolean;
   groupId: number | null;
}

export interface ModInput {
   name: string;
   author: string | null;
   imageDataUrl: string | null;
   /** Drop the image this app saved and fall back to whatever the mod itself ships with. */
   clearImage?: boolean;
}

export interface ModGroupMember {
   modId: number;
   name: string;
   folderName: string;
   isEnabled: boolean;
}

export interface ModGroup {
   id: number;
   name: string;
   baseImage: string | null;
   isEnabled: boolean;
   members: ModGroupMember[];
}

export interface ArchiveEntry {
   path: string;
   isDir: boolean;
   isLikelyModRoot: boolean;
}

export interface ArchiveAnalysis {
   filePath: string;
   entries: ArchiveEntry[];
   deducedName: string | null;
   deducedAuthor: string | null;
   deducedAgentId: number | null;
   deducedCategoryId: number | null;
   deducedCategoryItemId: number | null;
   detectedPreviewInternalPath: string | null;
}

export interface ImportArchiveRequest {
   archivePath: string;
   agentId: number | null;
   categoryId: number | null;
   categoryItemId: number | null;
   selectedInternalRoot: string | null;
   modName: string;
   author: string | null;
}

export interface KeybindInfo {
   title: string;
   key: string;
}

/** One folder a scan would move, as reported by `preview_scan_moves`. */
export interface PlannedMove {
   from: string;
   to: string;
}
