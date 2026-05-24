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
  readonly corruptCount: number;
}

// A single corrupt verdict file must not poison consumers — they need the
// rest of the scan to surface for triage. The adapter swallows JSON.parse
// errors silently, so we ask it explicitly for the per-task corruption
// count via `readLatestWithCorruption`.
export async function loadLatestVerdictsByTask(
  tasks: readonly Task[],
  verdictStore: VerdictStorePort,
): Promise<VerdictsByTaskResult> {
  const entries = await Promise.all(
    tasks.map(async (t): Promise<{ taskId: string; verdict?: Verdict; corruptCount: number }> => {
      try {
        const { verdict, corruptCount } = await verdictStore.readLatestWithCorruption(t.id);
        return { taskId: t.id, verdict, corruptCount };
      } catch {
        // A throw here means the readdir itself failed (permissions, I/O).
        // Surface it as one corrupt entry so it shows up in the status
        // report rather than vanishing into the "no verdicts" path.
        return { taskId: t.id, corruptCount: 1 };
      }
    }),
  );

  const byTaskId = new Map<string, Verdict>();
  let latest: LatestVerdictSummary | undefined;
  let corruptCount = 0;
  for (const entry of entries) {
    corruptCount += entry.corruptCount;
    if (!entry.verdict) continue;
    byTaskId.set(entry.taskId, entry.verdict);
    if (!latest || entry.verdict.computedAt.localeCompare(latest.computedAt) > 0) {
      latest = {
        taskId: entry.verdict.taskId,
        decision: entry.verdict.decision,
        computedAt: entry.verdict.computedAt,
      };
    }
  }
  return { byTaskId, latest, corruptCount };
}
