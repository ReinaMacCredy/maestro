import { describe, it, expect, beforeEach, afterEach } from "bun:test";
import { mkdtemp, rm, mkdir, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { FsSpecStoreAdapter, coerceSpec } from "@/shared/domain/legacy-spec/index.js";
import type { Spec } from "@/shared/domain/legacy-spec/index.js";

// ---------------------------------------------------------------------------
// coerceSpec unit tests (in-memory, no filesystem)
// ---------------------------------------------------------------------------

describe("coerceSpec", () => {
  it("v1 file → returns v2 Spec with runtime_signals: [] and rollout_plan: undefined", () => {
    const v1Raw = {
      schema_version: 1,
      mission_id: "2026-05-04-001",
      acceptance_criteria: [{ id: "crt-1", text: "Tests pass" }],
      non_goals: [{ text: "No new deps" }],
      runtime_signals: [],
      created_at: "2026-05-04T00:00:00.000Z",
      updated_at: "2026-05-04T00:00:00.000Z",
    };

    const result = coerceSpec(v1Raw);
    expect(result).toBeDefined();
    expect(result!.schema_version).toBe(2);
    expect(result!.mission_id).toBe("2026-05-04-001");
    expect(result!.acceptance_criteria).toEqual([{ id: "crt-1", text: "Tests pass" }]);
    expect(result!.non_goals).toEqual([{ text: "No new deps" }]);
    expect(result!.runtime_signals).toEqual([]);
    expect(result!.rollout_plan).toBeUndefined();
  });

  it("v2 file with valid runtime_signals and rollout_plan returns intact", () => {
    const v2Raw = {
      schema_version: 2,
      mission_id: "2026-05-04-002",
      acceptance_criteria: [{ id: "crt-2", text: "Deploy succeeds" }],
      non_goals: [{ text: "No UI changes" }],
      runtime_signals: [
        {
          name: "error-rate",
          description: "HTTP 5xx rate",
          provider: "prometheus",
          query: 'rate(http_requests_total{status=~"5.."}[5m])',
          threshold: { operator: "<", value: 0.01 },
          severity: "critical",
        },
      ],
      rollout_plan: {
        feature_flag: "deploy-safety",
        canary: {
          stages: [
            { percent: 10, hold_minutes: 15 },
            { percent: 50, hold_minutes: 30 },
          ],
        },
        rollback_command: "kubectl rollout undo deploy/api",
      },
      created_at: "2026-05-04T00:00:00.000Z",
      updated_at: "2026-05-04T00:00:00.000Z",
    };

    const result = coerceSpec(v2Raw);
    expect(result).toBeDefined();
    expect(result!.schema_version).toBe(2);
    expect(result!.runtime_signals).toHaveLength(1);
    expect(result!.runtime_signals[0]!.name).toBe("error-rate");
    expect(result!.runtime_signals[0]!.threshold.operator).toBe("<");
    expect(result!.runtime_signals[0]!.threshold.value).toBe(0.01);
    expect(result!.runtime_signals[0]!.severity).toBe("critical");
    expect(result!.rollout_plan).toBeDefined();
    expect(result!.rollout_plan!.feature_flag).toBe("deploy-safety");
    expect(result!.rollout_plan!.canary!.stages).toHaveLength(2);
    expect(result!.rollout_plan!.rollback_command).toBe("kubectl rollout undo deploy/api");
  });

  it("v2 file with missing severity in runtime_signals[0] returns undefined", () => {
    const malformed = {
      schema_version: 2,
      mission_id: "2026-05-04-003",
      acceptance_criteria: [],
      non_goals: [],
      runtime_signals: [
        {
          name: "error-rate",
          provider: "prometheus",
          query: "rate(errors[5m])",
          threshold: { operator: "<", value: 0.01 },
          // severity is missing
        },
      ],
      created_at: "2026-05-04T00:00:00.000Z",
      updated_at: "2026-05-04T00:00:00.000Z",
    };

    const result = coerceSpec(malformed);
    expect(result).toBeUndefined();
  });

  it("unknown schema_version returns undefined", () => {
    const unknown = {
      schema_version: 99,
      mission_id: "2026-05-04-004",
      acceptance_criteria: [],
      non_goals: [],
      runtime_signals: [],
      created_at: "2026-05-04T00:00:00.000Z",
      updated_at: "2026-05-04T00:00:00.000Z",
    };
    expect(coerceSpec(unknown)).toBeUndefined();
  });

  it("null input returns undefined", () => {
    expect(coerceSpec(null)).toBeUndefined();
  });
});

// ---------------------------------------------------------------------------
// FsSpecStoreAdapter filesystem tests
// ---------------------------------------------------------------------------

let tempDir: string;

beforeEach(async () => {
  tempDir = await mkdtemp(join(tmpdir(), "maestro-spec-test-"));
});

afterEach(async () => {
  await rm(tempDir, { recursive: true, force: true });
});

function makeAdapter(): FsSpecStoreAdapter {
  return new FsSpecStoreAdapter(tempDir);
}

function specsDir(): string {
  return join(tempDir, ".maestro", "specs");
}

async function writeRawSpec(missionId: string, content: unknown): Promise<void> {
  await mkdir(specsDir(), { recursive: true });
  await writeFile(join(specsDir(), `${missionId}.json`), JSON.stringify(content, null, 2));
}

describe("FsSpecStoreAdapter.read", () => {
  it("reads a v1 file on disk and returns a v2 Spec without modifying the file", async () => {
    const v1Raw = {
      schema_version: 1,
      mission_id: "2026-05-04-001",
      acceptance_criteria: [{ id: "crt-1", text: "Tests pass" }],
      non_goals: [{ text: "No scope creep" }],
      runtime_signals: [],
      created_at: "2026-05-04T00:00:00.000Z",
      updated_at: "2026-05-04T00:00:00.000Z",
    };

    await writeRawSpec("2026-05-04-001", v1Raw);

    const adapter = makeAdapter();
    const result = await adapter.read("2026-05-04-001");

    expect(result).toBeDefined();
    expect(result!.schema_version).toBe(2);
    expect(result!.runtime_signals).toEqual([]);
    expect(result!.rollout_plan).toBeUndefined();

    // Verify the on-disk file was NOT modified (still v1)
    const { readFile } = await import("node:fs/promises");
    const onDisk = JSON.parse(await readFile(join(specsDir(), "2026-05-04-001.json"), "utf8")) as Record<string, unknown>;
    expect(onDisk["schema_version"]).toBe(1);
  });

  it("reads a v2 file with runtime_signals and rollout_plan intact", async () => {
    const v2Raw = {
      schema_version: 2,
      mission_id: "2026-05-04-002",
      acceptance_criteria: [{ id: "crt-2", text: "Deploy green" }],
      non_goals: [],
      runtime_signals: [
        {
          name: "p99-latency",
          provider: "datadog",
          query: "avg:trace.web.request.duration.by.service{*}",
          threshold: { operator: "<=", value: 500 },
          severity: "warn",
        },
      ],
      rollout_plan: {
        feature_flag: "canary-gate",
        canary: { stages: [{ percent: 5, hold_minutes: 10 }] },
      },
      created_at: "2026-05-04T00:00:00.000Z",
      updated_at: "2026-05-04T00:00:00.000Z",
    };

    await writeRawSpec("2026-05-04-002", v2Raw);

    const adapter = makeAdapter();
    const result = await adapter.read("2026-05-04-002");

    expect(result).toBeDefined();
    expect(result!.schema_version).toBe(2);
    expect(result!.runtime_signals).toHaveLength(1);
    expect(result!.runtime_signals[0]!.name).toBe("p99-latency");
    expect(result!.rollout_plan!.feature_flag).toBe("canary-gate");
    expect(result!.rollout_plan!.canary!.stages[0]!.percent).toBe(5);
  });

  it("returns undefined for a malformed v2 file (severity missing)", async () => {
    const malformed = {
      schema_version: 2,
      mission_id: "2026-05-04-003",
      acceptance_criteria: [],
      non_goals: [],
      runtime_signals: [
        {
          name: "bad-signal",
          provider: "prometheus",
          query: "up",
          threshold: { operator: ">", value: 0 },
          // severity omitted — invalid
        },
      ],
      created_at: "2026-05-04T00:00:00.000Z",
      updated_at: "2026-05-04T00:00:00.000Z",
    };

    await writeRawSpec("2026-05-04-003", malformed);

    const adapter = makeAdapter();
    const result = await adapter.read("2026-05-04-003");
    expect(result).toBeUndefined();
  });
});

describe("FsSpecStoreAdapter.list", () => {
  it("includes both v1 and v2 files post-coercion", async () => {
    const v1Raw = {
      schema_version: 1,
      mission_id: "2026-05-04-001",
      acceptance_criteria: [{ id: "crt-1", text: "V1 criterion" }],
      non_goals: [],
      runtime_signals: [],
      created_at: "2026-05-04T00:00:00.000Z",
      updated_at: "2026-05-04T00:00:00.000Z",
    };

    const v2Raw = {
      schema_version: 2,
      mission_id: "2026-05-04-002",
      acceptance_criteria: [{ id: "crt-2", text: "V2 criterion" }],
      non_goals: [],
      runtime_signals: [],
      created_at: "2026-05-04T01:00:00.000Z",
      updated_at: "2026-05-04T01:00:00.000Z",
    };

    await writeRawSpec("2026-05-04-001", v1Raw);
    await writeRawSpec("2026-05-04-002", v2Raw);

    const adapter = makeAdapter();
    const specs = await adapter.list();

    expect(specs).toHaveLength(2);
    // Both should come back as schema_version 2
    expect(specs.every((s: Spec) => s.schema_version === 2)).toBe(true);
    expect(specs[0]!.mission_id).toBe("2026-05-04-001");
    expect(specs[1]!.mission_id).toBe("2026-05-04-002");
  });

  it("excludes malformed files from list results", async () => {
    const good = {
      schema_version: 2,
      mission_id: "2026-05-04-001",
      acceptance_criteria: [],
      non_goals: [],
      runtime_signals: [],
      created_at: "2026-05-04T00:00:00.000Z",
      updated_at: "2026-05-04T00:00:00.000Z",
    };

    const bad = {
      schema_version: 2,
      mission_id: "2026-05-04-002",
      // missing acceptance_criteria
      created_at: "2026-05-04T00:00:00.000Z",
      updated_at: "2026-05-04T00:00:00.000Z",
    };

    await writeRawSpec("2026-05-04-001", good);
    await writeRawSpec("2026-05-04-002", bad);

    const adapter = makeAdapter();
    const specs = await adapter.list();

    expect(specs).toHaveLength(1);
    expect(specs[0]!.mission_id).toBe("2026-05-04-001");
  });
});
