import type { VerdictStorePort } from "@/features/verdict/ports/storage.js";
import type { Verdict, VerdictDecision } from "@/features/verdict/domain/types.js";
import type { Task } from "@/types/task.js";

export interface LatestVerdictSummary {
  readonly taskId: string;
  readonly decision: VerdictDecision;
  readonly computedAt: string;
}

export interface VerdictsByTaskResult {
  readonly byTaskId: ReadonlyMap<string, Verdict>;
  readonly latest: LatestVerdictSummary | undefined;
}

// A single corrupt verdict file must not poison consumers — they need the
// rest of the scan to surface for triage.
export async function loadLatestVerdictsByTask(
  tasks: readonly Task[],
  verdictStore: VerdictStorePort,
): Promise<VerdictsByTaskResult> {
  const entries = await Promise.all(
    tasks.map(async (t): Promise<[string, Verdict] | undefined> => {
      try {
        const v = await verdictStore.readLatest(t.id);
        return v ? [t.id, v] : undefined;
      } catch {
        return undefined;
      }
    }),
  );

  const byTaskId = new Map<string, Verdict>();
  let latest: LatestVerdictSummary | undefined;
  for (const entry of entries) {
    if (!entry) continue;
    const [taskId, verdict] = entry;
    byTaskId.set(taskId, verdict);
    if (!latest || verdict.computedAt.localeCompare(latest.computedAt) > 0) {
      latest = {
        taskId: verdict.taskId,
        decision: verdict.decision,
        computedAt: verdict.computedAt,
      };
    }
  }
  return { byTaskId, latest };
}
