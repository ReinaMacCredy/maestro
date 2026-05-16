// v1 -> v2 task state mapping. Pure functions; no I/O.
//
// Phase 4 (per ADR-0007 big-bang) will wire this into `setup --migrate-v2`.
// Defined here so v2 owns the migration shape; v1 types are not imported (to
// keep the repo layer free of v1 cross-cutting). Callers pass a v1-shaped
// record in.

import type { TaskState } from "../types/task-state.js";
import type { Task } from "../types/task.js";
import { generateTaskId } from "../types/task.js";

export type V1TaskStatus = "pending" | "in_progress" | "completed";

export type V1LegacyTaskStatus = "open" | "blocked" | "deferred" | "closed";

export interface V1TaskShape {
  readonly id: string;
  readonly slug?: string;
  readonly title: string;
  /** Stored status — may be a v1 status or one of the legacy strings. */
  readonly status: string;
  readonly assignee?: string;
  readonly claimedAt?: string;
  readonly blockedBy?: readonly string[];
  readonly closeReason?: string;
  readonly createdAt: string;
  readonly updatedAt: string;
}

export interface MapV1TaskOptions {
  /** Optional spec path to record on the migrated task (none by default). */
  readonly specPath?: string;
  /** Optional id override (otherwise the v1 id is preserved). */
  readonly idOverride?: string;
}

const LEGACY_TO_V1: Record<V1LegacyTaskStatus, V1TaskStatus | "blocked"> = {
  open: "pending",
  blocked: "blocked",
  deferred: "blocked",
  closed: "completed",
};

const ABANDON_HINT_RE = /\b(abandon(ed)?|cancel(led)?|won['’]?t.?fix|drop(ped)?)\b/i;

/**
 * Normalize a stored v1 status string into the closed v1 status set, with
 * `blocked` retained as a passthrough sentinel since v2 has a first-class
 * `blocked` state and v1's legacy `blocked` should land there directly.
 */
export function normalizeV1Status(value: string): V1TaskStatus | "blocked" | undefined {
  if (value === "pending" || value === "in_progress" || value === "completed") return value;
  if (value === "open" || value === "blocked" || value === "deferred" || value === "closed") {
    return LEGACY_TO_V1[value];
  }
  return undefined;
}

/**
 * Pure mapping from a v1 task status (plus assignee + close reason context)
 * to the v2 task state. Used by the v1->v2 migration in Phase 4.
 */
export function mapV1StatusToV2State(input: {
  readonly status: string;
  readonly assignee?: string;
  readonly closeReason?: string;
}): TaskState {
  const normalized = normalizeV1Status(input.status);
  if (normalized === "blocked") return "blocked";
  if (normalized === "pending") {
    return input.assignee ? "claimed" : "draft";
  }
  if (normalized === "in_progress") return "doing";
  if (normalized === "completed") {
    if (input.closeReason && ABANDON_HINT_RE.test(input.closeReason)) return "abandoned";
    return "shipped";
  }
  // Unknown v1 status: be conservative; treat as draft so the operator can
  // re-route by hand rather than silently dropping the task.
  return "draft";
}

/**
 * Build a v2 Task from a v1-shaped record. ID is preserved unless overridden;
 * slug falls back to a deterministic derivation from the title or the id when
 * the v1 record had no slug.
 */
export function mapV1TaskToV2(
  v1: V1TaskShape,
  options: MapV1TaskOptions = {},
): Task {
  const state = mapV1StatusToV2State({
    status: v1.status,
    assignee: v1.assignee,
    closeReason: v1.closeReason,
  });

  const slug = v1.slug ?? deriveSlugFromTitleOrId(v1);

  const base: Task = {
    id: options.idOverride ?? v1.id,
    slug,
    title: v1.title,
    state,
    blocked_by: v1.blockedBy ?? [],
    created_at: v1.createdAt,
    updated_at: v1.updatedAt,
  };

  const withSpec: Task = options.specPath ? { ...base, spec_path: options.specPath } : base;

  if (state === "claimed" || state === "doing" || state === "verifying") {
    return {
      ...withSpec,
      assignee: v1.assignee,
      claimed_at: v1.claimedAt,
    };
  }

  if (state === "blocked") {
    return {
      ...withSpec,
      block_reason: v1.closeReason,
    };
  }

  if (state === "abandoned") {
    return {
      ...withSpec,
      abandon_reason: v1.closeReason,
    };
  }

  return withSpec;
}

function deriveSlugFromTitleOrId(v1: V1TaskShape): string {
  const fromTitle = v1.title
    .toLowerCase()
    .replaceAll(/[^a-z0-9]+/g, "-")
    .replaceAll(/^-+|-+$/g, "")
    .slice(0, 64);
  if (fromTitle.length > 0) return fromTitle;
  return v1.id.replaceAll(/[^a-z0-9]+/gi, "-").toLowerCase();
}

export { generateTaskId };
