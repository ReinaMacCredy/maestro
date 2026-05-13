import type { EvidenceStorePort, EvidenceRow, EvidenceKind } from "@/features/evidence";
import type { VerdictStorePort, Verdict } from "@/features/verdict";
import type { TaskQueryPort } from "@/features/task";

export type StateEventKind =
  | "evidence"
  | "verdict"
  | "task-claim"
  | "task-complete";

export interface StateEvent {
  readonly at: string;
  readonly kind: StateEventKind;
  readonly taskId?: string;
  readonly summary: string;
  readonly id?: string;
}

export interface StateSinceDeps {
  readonly evidenceStore: EvidenceStorePort;
  readonly verdictStore: VerdictStorePort;
  readonly taskStore: TaskQueryPort;
}

export interface StateSinceArgs {
  readonly since: string;
  readonly until?: string;
  readonly taskId?: string;
}

export interface StateSinceResult {
  readonly since: string;
  readonly until?: string;
  readonly events: readonly StateEvent[];
}

export async function stateSince(
  deps: StateSinceDeps,
  args: StateSinceArgs,
): Promise<StateSinceResult> {
  const sinceMs = Date.parse(args.since);
  if (Number.isNaN(sinceMs)) {
    throw new Error(`Invalid --since timestamp: ${args.since}`);
  }
  const untilMs = args.until !== undefined ? Date.parse(args.until) : Number.POSITIVE_INFINITY;

  const tasks = args.taskId !== undefined ? [args.taskId] : await listTaskIds(deps);

  const collectors = await Promise.all(
    tasks.map(async (taskId): Promise<{ taskId: string; evidence: readonly EvidenceRow<EvidenceKind>[]; verdicts: readonly Verdict[] }> => {
      const [evidence, verdicts] = await Promise.all([
        deps.evidenceStore.list({ task_id: taskId }),
        deps.verdictStore.history(taskId),
      ]);
      return { taskId, evidence, verdicts };
    }),
  );

  const events: StateEvent[] = [];
  for (const c of collectors) {
    for (const e of c.evidence) {
      events.push({
        at: e.created_at,
        kind: "evidence",
        taskId: c.taskId,
        summary: summarizeEvidence(e),
        id: e.id,
      });
    }
    for (const v of c.verdicts) {
      events.push({
        at: v.computedAt,
        kind: "verdict",
        taskId: c.taskId,
        summary: `verdict ${v.decision} (risk=${v.effectiveRiskClass})`,
        id: v.id,
      });
    }
  }

  return {
    since: args.since,
    until: args.until,
    events: events
      .filter((e) => {
        const t = Date.parse(e.at);
        if (Number.isNaN(t)) return false;
        return t >= sinceMs && t <= untilMs;
      })
      .sort((a, b) => a.at.localeCompare(b.at)),
  };
}

async function listTaskIds(deps: StateSinceDeps): Promise<string[]> {
  const list = await deps.taskStore.all();
  return list.map((t) => t.id);
}

function summarizeEvidence(e: EvidenceRow): string {
  const witness = e.witness_level;
  const detail = evidenceDetail(e);
  return detail ? `${e.kind} ${detail} (${witness})` : `${e.kind} (${witness})`;
}

function evidenceDetail(e: EvidenceRow): string | undefined {
  // Payload is a discriminated union but TypeScript doesn't narrow it in switch.
  // Safe to access properties dynamically since we check types at runtime.
  const payload = e.payload as any;
  if (!payload || typeof payload !== "object") return undefined;
  switch (e.kind) {
    case "ai-review": {
      const reviewer = typeof payload.reviewer === "string" ? payload.reviewer : undefined;
      const findings = Array.isArray(payload.findings) ? payload.findings.length : undefined;
      if (reviewer && findings !== undefined) return `${reviewer}/${findings} finding(s)`;
      return reviewer;
    }
    case "threat-model": {
      const threats = Array.isArray(payload.threats) ? payload.threats.length : undefined;
      return threats !== undefined ? `${threats} threat(s)` : undefined;
    }
    case "lint-violation": {
      const ruleId = typeof payload.ruleId === "string" ? payload.ruleId : undefined;
      const file = typeof payload.file === "string" ? payload.file : undefined;
      if (ruleId && file) return `${ruleId} @ ${file}`;
      return ruleId;
    }
    case "session-start":
    case "session-exit": {
      const headSha = typeof payload.headSha === "string" ? payload.headSha.slice(0, 7) : undefined;
      return headSha;
    }
    case "ralph-iteration": {
      const findingsHash = typeof payload.findingsHash === "string" ? payload.findingsHash.slice(0, 7) : undefined;
      const stuck = payload.stuck === true ? " stuck" : "";
      return findingsHash ? `${findingsHash}${stuck}` : undefined;
    }
    case "recovery": {
      const targetSha = typeof payload.targetSha === "string" ? payload.targetSha.slice(0, 7) : undefined;
      return targetSha ? `→ ${targetSha}` : undefined;
    }
    default:
      return undefined;
  }
}

export function formatStateSinceLines(r: StateSinceResult): string[] {
  const lines: string[] = [];
  lines.push(`State since ${r.since}${r.until ? ` until ${r.until}` : ""}`);
  lines.push(`${r.events.length} event${r.events.length !== 1 ? "s" : ""}`);
  for (const e of r.events) {
    const id = e.id ? ` ${e.id}` : "";
    const task = e.taskId ? ` [${e.taskId}]` : "";
    lines.push(`  ${e.at}${task} ${e.kind}${id}: ${e.summary}`);
  }
  return lines;
}
