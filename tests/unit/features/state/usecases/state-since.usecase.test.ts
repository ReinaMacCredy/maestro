import { beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { FsEvidenceStoreAdapter, recordEvidence } from "@/features/evidence";
import type { Verdict, VerdictStorePort } from "@/features/verdict";
import type { Task, TaskQueryPort } from "@/features/task";
import { stateSince } from "@/features/state";

class FakeVerdictStore implements VerdictStorePort {
  private rows: Verdict[] = [];
  push(v: Verdict): void {
    this.rows.push(v);
  }
  async write(_t: string, v: Verdict): Promise<void> {
    this.rows.push(v);
  }
  async readLatest(taskId: string): Promise<Verdict | undefined> {
    return [...this.rows].reverse().find((v) => v.taskId === taskId);
  }
  async readVersion(): Promise<Verdict | undefined> {
    return undefined;
  }
  async history(taskId: string): Promise<readonly Verdict[]> {
    return this.rows.filter((v) => v.taskId === taskId);
  }
  async findByTreeSha(): Promise<readonly Verdict[]> {
    return [];
  }
}

class FakeTaskStore implements TaskQueryPort {
  constructor(private tasks: Task[]) {}
  async get(id: string): Promise<Task | undefined> {
    return this.tasks.find((t) => t.id === id);
  }
  async all(): Promise<readonly Task[]> {
    return this.tasks;
  }
}

function passVerdict(taskId: string, computedAt: string, id = "vrd-1"): Verdict {
  return {
    schemaVersion: 1,
    id,
    taskId,
    contractVersion: 1,
    computedAt,
    decision: "PASS",
    effectiveRiskClass: "low",
    reasons: [],
    evidenceConsulted: [],
    policiesConsulted: [],
    trustVerifier: { findingsCount: 0, errors: 0, warns: 0, infos: 0 },
  };
}

function tinyTask(id: string): Task {
  return {
    id,
    title: id,
    status: "pending",
    type: "task",
    priority: 2,
    createdAt: "2026-01-01T00:00:00Z",
    updatedAt: "2026-01-01T00:00:00Z",
  } as unknown as Task;
}

interface Fixtures {
  tmpDir: string;
  evidenceStore: FsEvidenceStoreAdapter;
  verdictStore: FakeVerdictStore;
  taskStore: FakeTaskStore;
}

let f: Fixtures;

beforeEach(async () => {
  const tmpDir = await mkdtemp(join(tmpdir(), "state-since-"));
  f = {
    tmpDir,
    evidenceStore: new FsEvidenceStoreAdapter(tmpDir),
    verdictStore: new FakeVerdictStore(),
    taskStore: new FakeTaskStore([tinyTask("tsk-aaa111"), tinyTask("tsk-bbb222")]),
  };
});

describe("stateSince", () => {
  it("returns events ordered by timestamp", async () => {
    await recordEvidence(f.evidenceStore, {
      task_id: "tsk-aaa111",
      kind: "manual-note",
      witness_level: "agent-claimed-and-not-reproducible",
      payload: { note: "first" },
    });
    f.verdictStore.push(passVerdict("tsk-aaa111", "2026-05-08T10:00:00Z"));
    await recordEvidence(f.evidenceStore, {
      task_id: "tsk-aaa111",
      kind: "manual-note",
      witness_level: "agent-claimed-and-not-reproducible",
      payload: { note: "second" },
    });

    const result = await stateSince(f, { since: "2026-01-01T00:00:00Z" });
    const sorted = [...result.events].map((e) => e.at);
    expect(sorted).toEqual([...sorted].sort());
    expect(result.events.length).toBe(3);
  });

  it("filters by --task", async () => {
    await recordEvidence(f.evidenceStore, {
      task_id: "tsk-aaa111",
      kind: "manual-note",
      witness_level: "agent-claimed-and-not-reproducible",
      payload: { note: "a" },
    });
    await recordEvidence(f.evidenceStore, {
      task_id: "tsk-bbb222",
      kind: "manual-note",
      witness_level: "agent-claimed-and-not-reproducible",
      payload: { note: "b" },
    });
    const result = await stateSince(f, { since: "2026-01-01T00:00:00Z", taskId: "tsk-aaa111" });
    expect(result.events.every((e) => e.taskId === "tsk-aaa111")).toBe(true);
  });

  it("respects --until upper bound", async () => {
    f.verdictStore.push(passVerdict("tsk-aaa111", "2026-05-08T10:00:00Z", "v-old"));
    f.verdictStore.push(passVerdict("tsk-aaa111", "2026-06-01T10:00:00Z", "v-new"));
    const result = await stateSince(f, {
      since: "2026-05-01T00:00:00Z",
      until: "2026-05-15T00:00:00Z",
    });
    expect(result.events.length).toBe(1);
    expect(result.events[0]!.id).toBe("v-old");
  });

  it("throws on invalid --since", async () => {
    expect(stateSince(f, { since: "not-a-date" })).rejects.toThrow(/Invalid --since/);
  });

  it("includes verdict events with risk class in summary", async () => {
    f.verdictStore.push(passVerdict("tsk-aaa111", "2026-05-08T10:00:00Z"));
    const result = await stateSince(f, { since: "2026-01-01T00:00:00Z" });
    const verdictEvents = result.events.filter((e) => e.kind === "verdict");
    expect(verdictEvents.length).toBe(1);
    expect(verdictEvents[0]!.summary).toContain("PASS");
    expect(verdictEvents[0]!.summary).toContain("low");
  });
});
