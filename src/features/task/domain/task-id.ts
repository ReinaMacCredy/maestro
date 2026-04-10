import { randomBytes } from "node:crypto";

/**
 * Task ID format: `tsk-<6 hex chars>` (e.g. `tsk-a1b2c3`).
 * 6 hex gives 16M possibilities — vanishingly rare collision at human scale.
 * The adapter still checks against the loaded store on create and retries on clash.
 */
export const TASK_ID_PATTERN = /^tsk-[0-9a-f]{6}$/;

export function generateTaskId(): string {
  return `tsk-${randomBytes(3).toString("hex")}`;
}

export function isTaskId(value: string): boolean {
  return TASK_ID_PATTERN.test(value);
}
