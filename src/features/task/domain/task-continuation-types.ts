import type { AgentSlug } from "@/features/session";
import { TASK_STATUSES, type TaskStatus } from "./task-types.js";

export interface TaskContinuationAgent {
  readonly type: AgentSlug;
  readonly sessionId?: string;
  readonly lastSeenAt: string;
}

export interface TaskContinuationSummary {
  readonly taskId: string;
  readonly status: TaskStatus;
  readonly lastActiveAt: string;
  readonly currentState: string;
  readonly nextAction: string;
  readonly keyDecisions: readonly string[];
  readonly activeAgent?: TaskContinuationAgent;
}

export type TaskContinuationEvent =
  | {
      readonly kind: "snapshot";
      readonly at: string;
      readonly summary: string;
      readonly currentState: string;
    }
  | {
      readonly kind: "decision";
      readonly at: string;
      readonly summary: string;
      readonly decision: string;
      readonly active: boolean;
    }
  | {
      readonly kind: "next_action_set";
      readonly at: string;
      readonly summary: string;
      readonly nextAction: string;
    }
  | {
      readonly kind: "blocker_set";
      readonly at: string;
      readonly summary: string;
      readonly blockerTaskIds: readonly string[];
    }
  | {
      readonly kind: "handoff_created";
      readonly at: string;
      readonly summary: string;
      readonly handoffId: string;
      readonly agent: AgentSlug;
      readonly sessionId?: string;
    }
  | {
      readonly kind: "handoff_picked_up";
      readonly at: string;
      readonly summary: string;
      readonly handoffId: string;
      readonly agent: AgentSlug;
      readonly sessionId?: string;
    }
  | {
      readonly kind: "agent_takeover";
      readonly at: string;
      readonly summary: string;
      readonly reason: "resume" | "handoff_pickup" | "claim" | "start";
      readonly from?: TaskContinuationAgent;
      readonly to: TaskContinuationAgent;
    }
  | {
      readonly kind: "task_completed";
      readonly at: string;
      readonly summary: string;
      readonly reason?: string;
    }
  | {
      readonly kind: "task_reopened";
      readonly at: string;
      readonly summary: string;
      readonly reason?: string;
    };

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function isIsoString(value: unknown): value is string {
  return typeof value === "string" && value.length > 0 && !Number.isNaN(Date.parse(value));
}

function isStringArray(value: unknown): value is readonly string[] {
  return Array.isArray(value) && value.every((item) => typeof item === "string");
}

function isTaskStatus(value: unknown): value is TaskStatus {
  return typeof value === "string" && TASK_STATUSES.includes(value as TaskStatus);
}

export function validateTaskContinuationAgent(value: unknown): TaskContinuationAgent | undefined {
  if (!isRecord(value)) return undefined;
  if (typeof value.type !== "string") return undefined;
  if (!isIsoString(value.lastSeenAt)) return undefined;
  if (value.sessionId !== undefined && typeof value.sessionId !== "string") return undefined;
  return {
    type: value.type,
    ...(typeof value.sessionId === "string" ? { sessionId: value.sessionId } : {}),
    lastSeenAt: value.lastSeenAt,
  };
}

export function validateTaskContinuationSummary(value: unknown): TaskContinuationSummary | undefined {
  if (!isRecord(value)) return undefined;
  if (typeof value.taskId !== "string" || value.taskId.length === 0) return undefined;
  if (!isTaskStatus(value.status)) return undefined;
  if (!isIsoString(value.lastActiveAt)) return undefined;
  if (typeof value.currentState !== "string") return undefined;
  if (typeof value.nextAction !== "string") return undefined;
  if (!isStringArray(value.keyDecisions)) return undefined;

  const activeAgent = value.activeAgent === undefined
    ? undefined
    : validateTaskContinuationAgent(value.activeAgent);
  if (value.activeAgent !== undefined && activeAgent === undefined) return undefined;

  return {
    taskId: value.taskId,
    status: value.status,
    lastActiveAt: value.lastActiveAt,
    currentState: value.currentState,
    nextAction: value.nextAction,
    keyDecisions: value.keyDecisions,
    ...(activeAgent ? { activeAgent } : {}),
  };
}

function validateHandoffActorFields(value: Record<string, unknown>) {
  if (typeof value.handoffId !== "string" || value.handoffId.length === 0) return undefined;
  if (typeof value.agent !== "string" || value.agent.length === 0) return undefined;
  if (value.sessionId !== undefined && typeof value.sessionId !== "string") return undefined;
  return {
    handoffId: value.handoffId,
    agent: value.agent,
    ...(typeof value.sessionId === "string" ? { sessionId: value.sessionId } : {}),
  };
}

export function validateTaskContinuationEvent(value: unknown): TaskContinuationEvent | undefined {
  if (!isRecord(value)) return undefined;
  if (!isIsoString(value.at)) return undefined;
  if (typeof value.kind !== "string") return undefined;
  if (typeof value.summary !== "string") return undefined;

  switch (value.kind) {
    case "snapshot":
      if (typeof value.currentState !== "string") return undefined;
      return { kind: value.kind, at: value.at, summary: value.summary, currentState: value.currentState };
    case "decision":
      if (typeof value.decision !== "string" || typeof value.active !== "boolean") return undefined;
      return { kind: value.kind, at: value.at, summary: value.summary, decision: value.decision, active: value.active };
    case "next_action_set":
      if (typeof value.nextAction !== "string") return undefined;
      return { kind: value.kind, at: value.at, summary: value.summary, nextAction: value.nextAction };
    case "blocker_set":
      if (!isStringArray(value.blockerTaskIds)) return undefined;
      return { kind: value.kind, at: value.at, summary: value.summary, blockerTaskIds: value.blockerTaskIds };
    case "handoff_created":
    case "handoff_picked_up": {
      const fields = validateHandoffActorFields(value);
      if (!fields) return undefined;
      return {
        kind: value.kind,
        at: value.at,
        summary: value.summary,
        handoffId: fields.handoffId,
        agent: fields.agent,
        ...(fields.sessionId ? { sessionId: fields.sessionId } : {}),
      };
    }
    case "agent_takeover": {
      if (
        value.reason !== "resume" &&
        value.reason !== "handoff_pickup" &&
        value.reason !== "claim" &&
        value.reason !== "start"
      ) {
        return undefined;
      }
      const to = validateTaskContinuationAgent(value.to);
      if (!to) return undefined;
      const from = value.from === undefined ? undefined : validateTaskContinuationAgent(value.from);
      if (value.from !== undefined && from === undefined) return undefined;
      return {
        kind: value.kind,
        at: value.at,
        summary: value.summary,
        reason: value.reason,
        ...(from ? { from } : {}),
        to,
      };
    }
    case "task_completed":
    case "task_reopened":
      if (value.reason !== undefined && typeof value.reason !== "string") return undefined;
      return {
        kind: value.kind,
        at: value.at,
        summary: value.summary,
        ...(typeof value.reason === "string" ? { reason: value.reason } : {}),
      };
    default:
      return undefined;
  }
}
