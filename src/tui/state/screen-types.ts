/**
 * DTOs for the conductor TUI screens: agent grid, dispatch console,
 * event stream, task board, and mission timeline.
 */
import type { TaskStatus, TaskPriority } from "@/features/task";
import type {
  MilestoneKind,
  MilestoneProfile,
  FeatureStatus,
} from "@/shared/domain/legacy-mission";
import type { PrincipleMode } from "@/features/principle";
import type { ReplyOutcome, ReplyAuthor } from "@/features/reply";
import type { EvidenceKind, WitnessLevel } from "@/features/evidence";
import type { MissionControlEvent } from "./types.js";

export interface EvidenceSummary {
  readonly id: string;
  readonly kind: EvidenceKind;
  readonly witness_level: WitnessLevel;
  readonly created_at: string;
}

export type InferredAgentStatus = "active" | "waiting" | "completed" | "stale";

export interface AgentGridRow {
  readonly agentType: string;
  readonly status: InferredAgentStatus;
  readonly activeFeatureId: string | undefined;
  readonly activeFeatureTitle: string | undefined;
  readonly lastActivityAt: string | undefined;
  readonly featureCount: number;
  readonly completedCount: number;
}

export interface DispatchQueueItem {
  readonly featureId: string;
  readonly featureTitle: string;
  readonly milestoneId: string;
  readonly milestoneTitle: string;
  readonly milestoneOrder: number;
  readonly agentType: string;
}

export type EventStreamEntryKind = MissionControlEvent["kind"] | "task" | "reply";

export interface EventStreamEntry {
  readonly timestamp: string;
  readonly relativeMs: number;
  readonly kind: EventStreamEntryKind;
  readonly title: string;
  readonly detail?: string;
}

export interface TaskBoardItem {
  readonly id: string;
  readonly title: string;
  readonly status: TaskStatus;
  readonly priority: TaskPriority;
  readonly assignee: string | undefined;
  readonly labels: readonly string[];
  readonly blockedByCount: number;
  readonly evidenceCount: number;
  readonly recentEvidence: readonly EvidenceSummary[];
}

export interface TaskBoardSnapshot {
  readonly columns: Readonly<Record<TaskStatus, readonly TaskBoardItem[]>>;
  readonly totalCount: number;
}

export interface PrincipleEffectivenessRow {
  readonly id: string;
  readonly name: string;
  readonly mode: PrincipleMode;
  readonly helpful: number;
  readonly unhelpful: number;
  readonly pending: number;
  readonly total: number;
  /** helpful / (helpful + unhelpful) expressed as 0..100. Undefined when no decided outcomes exist. */
  readonly effectivenessPct?: number;
  /** true when decided outcomes < small-sample threshold. UI should visually de-emphasize. */
  readonly lowSample: boolean;
  /** Recent launch summaries where this principle's outcome ended as unhelpful. Empty when none. */
  readonly recentKickbackExamples: readonly string[];
}

export interface ReplyInboxEntry {
  readonly featureId: string;
  readonly outcome: ReplyOutcome;
  readonly writtenAt: string;
  readonly writtenBy: ReplyAuthor;
  readonly featureTitle?: string;
  readonly featureStatus?: FeatureStatus;
  readonly pending: boolean;
  readonly notes?: string;
}

export interface TimelineMilestoneEntry {
  readonly id: string;
  readonly title: string;
  readonly order: number;
  readonly kind: MilestoneKind;
  readonly profile: MilestoneProfile;
  readonly features: readonly {
    readonly id: string;
    readonly title: string;
    readonly status: FeatureStatus;
  }[];
  readonly progressPct: number;
}
