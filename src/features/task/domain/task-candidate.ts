/**
 * A lesson captured from a closed task, ready to be surfaced as a hint on a
 * future `task ready` query. `sourceType` is a discriminator — only
 * `task-close` exists today but the field is reserved so other signal
 * sources (handoff blind spots, ratchet failures) can be added without
 * breaking serialized candidates already on disk.
 */

import { TASK_ID_PATTERN } from "./task-id.js";

export type CandidateSourceType = "task-close";

export interface TaskCandidate {
  /** Stable candidate id. For task-close candidates this matches the task id. */
  readonly id: string;
  /** The task id that generated this candidate. */
  readonly sourceTaskId: string;
  /** Discriminator for future candidate sources. */
  readonly sourceType: CandidateSourceType;
  /** Original task title at the time of capture. */
  readonly title: string;
  /** Close reason (or blind-spot text, etc. in later phases). */
  readonly reason: string;
  /** Extracted keywords used for matching against ready tasks. */
  readonly keywords: readonly string[];
  /** ISO 8601 capture timestamp. */
  readonly capturedAt: string;
}

export function validateTaskCandidate(value: unknown): TaskCandidate | undefined {
  if (typeof value !== "object" || value === null) return undefined;
  const c = value as Record<string, unknown>;

  if (typeof c.id !== "string" || !TASK_ID_PATTERN.test(c.id)) return undefined;
  if (typeof c.sourceTaskId !== "string" || !TASK_ID_PATTERN.test(c.sourceTaskId)) return undefined;
  if (c.sourceType !== "task-close") return undefined;
  if (typeof c.title !== "string") return undefined;
  if (typeof c.reason !== "string") return undefined;
  if (!Array.isArray(c.keywords)) return undefined;
  if (!c.keywords.every((k) => typeof k === "string")) return undefined;
  if (typeof c.capturedAt !== "string") return undefined;

  return {
    id: c.id,
    sourceTaskId: c.sourceTaskId,
    sourceType: c.sourceType,
    title: c.title,
    reason: c.reason,
    keywords: c.keywords as readonly string[],
    capturedAt: c.capturedAt,
  };
}
