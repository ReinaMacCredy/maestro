/**
 * Core types for maestroCLI.
 * Updated for v2 plugin model -- 6-state task model, no worker/session/sandbox types.
 */

// ============================================================================
// Feature Types
// ============================================================================

export type FeatureStatusType = 'planning' | 'approved' | 'executing' | 'handed-off' | 'review-pending' | 'completed';

export interface FeatureJson {
  name: string;
  status: FeatureStatusType;
  ticket?: string;
  sessionId?: string;
  createdAt: string;
  approvedAt?: string;
  handedOffAt?: string;
  reviewPendingAt?: string;
  completedAt?: string;
}

// ============================================================================
// Task Types
// ============================================================================

export type TaskStatusType = 'pending' | 'claimed' | 'done' | 'blocked' | 'review' | 'revision';
export type TaskOrigin = 'plan' | 'manual';

export interface TaskStatus {
  schemaVersion?: number;
  status: TaskStatusType;
  origin: TaskOrigin;
  planTitle?: string;
  summary?: string;
  claimedBy?: string;
  claimedAt?: string;
  completedAt?: string;
  blockerReason?: string;
  blockerDecision?: string;
  dependsOn?: string[];
  // Verification fields (stored inline; full report in verification.json)
  revisionCount?: number;
  revisionFeedback?: string;
}

export interface TaskInfo extends Omit<TaskStatus, 'schemaVersion'> {
  /** Primary identifier. Decoupled from storage. */
  id: string;
  /** @deprecated Internal storage path segment. Use `id` for public identity. */
  folder: string;
  name: string;
  /** Set when an idempotent transition finds the entity already in the target state. */
  already?: boolean;
}

// ============================================================================
// Plan Types
// ============================================================================

export interface PlanComment {
  id: string;
  line: number;
  body: string;
  author: string;
  timestamp: string;
}

export interface CommentsJson {
  threads: PlanComment[];
}

export interface PlanReadResult {
  content: string;
  status: FeatureStatusType;
  comments: PlanComment[];
}

export interface TasksSyncResult {
  created: string[];
  removed: string[];
  kept: string[];
  manual: string[];
  warnings?: string[];
}

// ============================================================================
// Feature Info
// ============================================================================

export interface FeatureInfo {
  name: string;
  status: FeatureStatusType;
  tasks: TaskInfo[];
  hasPlan: boolean;
  commentCount: number;
}

// ============================================================================
// Memory Types
// ============================================================================

export interface MemoryFile {
  name: string;
  content: string;
  updatedAt: string;
  sizeBytes: number;
}

export const MEMORY_CATEGORIES = ['decision', 'research', 'architecture', 'convention', 'debug', 'execution'] as const;
export type MemoryCategory = typeof MEMORY_CATEGORIES[number];

export type MemoryRelation = 'related' | 'supersedes' | 'contradicts' | 'extends';

export interface MemoryConnection {
  target: string;
  relation: MemoryRelation;
}

export interface MemoryMetadata {
  tags?: string[];
  priority?: number;       // 0 (highest) to 4 (lowest), default 2
  category?: MemoryCategory;
  selectionCount?: number;   // DCP selection frequency (incremented on each inclusion)
  lastSelectedAt?: string;   // ISO timestamp of last DCP selection
  connections?: MemoryConnection[];  // relationships to other memories
}

export interface MemoryFileWithMeta extends MemoryFile {
  metadata: MemoryMetadata;
  bodyContent: string;     // content WITHOUT frontmatter block
}

// ============================================================================
// Config Types
// ============================================================================

export interface AgentModelConfig {
  model?: string;
  temperature?: number;
  skills?: string[];
  autoLoadSkills?: string[];
  variant?: string;
}

export const DEFAULT_AGENT_MODELS = {
  'hive-master': 'github-copilot/claude-opus-4.5',
  'architect-planner': 'github-copilot/gpt-5.2-codex',
  'swarm-orchestrator': 'github-copilot/claude-opus-4.5',
  'scout-researcher': 'zai-coding-plan/glm-4.7',
  'forager-worker': 'github-copilot/gpt-5.2-codex',
  'hygienic-reviewer': 'github-copilot/gpt-5.2-codex',
} as const;

export type AgentName = keyof typeof DEFAULT_AGENT_MODELS;
export const AGENT_NAMES = Object.keys(DEFAULT_AGENT_MODELS) as AgentName[];
