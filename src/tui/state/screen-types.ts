/**
 * DTOs for the conductor TUI screens: agent grid, dispatch console,
 * event stream, task board, and mission timeline.
 */
import type { TaskStatus, TaskPriority } from "@/features/task";
import type { MilestoneKind, MilestoneProfile, FeatureStatus } from "@/features/mission";
import type { ReplyOutcome, ReplyAuthor } from "@/features/reply";
import type { MissionControlEvent } from "./types.js";

export type InferredAgentStatus = "active" | "waiting" | "completed" | "stale";

export interface AgentGridRow {
  readonly workerType: string;
  readonly status: InferredAgentStatus;
  readonly activeFeatureId: string | undefined;
  readonly activeFeatureTitle: string | undefined;
  readonly lastActivityAt: string | undefined;
  readonly featureCount: number;
  readonly completedCount: number;
  readonly pendingHandoffCount: number;
}

export interface DispatchQueueItem {
  readonly featureId: string;
  readonly featureTitle: string;
  readonly milestoneId: string;
  readonly milestoneTitle: string;
  readonly milestoneOrder: number;
  readonly workerType: string;
  readonly fitReason: string;
}

export type EventStreamEntryKind = MissionControlEvent["kind"] | "handoff" | "task" | "reply";

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
  readonly dependsOnCount: number;
}

export interface TaskBoardSnapshot {
  readonly columns: Readonly<Record<TaskStatus, readonly TaskBoardItem[]>>;
  readonly totalCount: number;
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
