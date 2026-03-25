/**
 * HostBackend -- port interface for host-native task integration.
 *
 * Maps maestro task IDs to host-native task IDs (e.g., Claude Code tasks,
 * Codex threads). The filesystem is always the source of truth; the host
 * backend is an acceleration layer for state transitions and native UI.
 */

import type { HostType } from '../../infra/utils/host-detect.ts';
import type { TaskStatusType } from '../types.ts';

/** Mapping from maestro task id to host-native task id. */
export interface HostMapping {
  tasks: Record<string, string>;
  reconciledAt?: string;
}

export interface HostBackend {
  readonly hostType: HostType;

  /** Create a task in the host system. Returns host-native id, or null if unavailable. */
  createTask(feature: string, taskId: string, title: string): Promise<string | null>;

  /** Update task status in the host system. Best-effort. */
  updateStatus(feature: string, taskId: string, status: TaskStatusType): Promise<void>;

  /** Read the host mapping for a feature. */
  getMapping(feature: string): HostMapping;

  /** Write the host mapping for a feature. */
  setMapping(feature: string, mapping: HostMapping): void;
}
