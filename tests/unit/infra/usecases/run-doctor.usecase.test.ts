import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { chmod, mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { runDoctor } from "@/infra/usecases/run-doctor.usecase.js";
import {
  mockConfig,
  mockGit,
  mockRepoTaskStore,
  mockVerdictStore,
} from "../../../helpers/mocks.js";
import type { Task } from "@/types/task.js";
import type { Verdict } from "@/features/verdict/domain/types.js";

let cwd: string;
let savedEnv: string | undefined;

beforeEach(async () => {
  cwd = await mkdtemp(join(tmpdir(), "maestro-doctor-"));
  await mkdir(join(cwd, ".maestro"), { recursive: true });
  savedEnv = process.env.MAESTRO_VERDICT_STALE_DAYS;
});

afterEach(async () => {
  await rm(cwd, { recursive: true, force: true });
  if (savedEnv === undefined) delete process.env.MAESTRO_VERDICT_STALE_DAYS;
  else process.env.MAESTRO_VERDICT_STALE_DAYS = savedEnv;
});

function baseDeps(projectDir: string) {
  return {
    git: mockGit(),
    config: mockConfig({ exists: async () => true }),
    taskStore: mockRepoTaskStore(),
    verdictStore: mockVerdictStore(),
    projectDir,
  };
}

function makeTask(overrides: Partial<Task> = {}): Task {
  const now = new Date().toISOString();
  return {
    id: "tsk-test-0001",
    slug: "demo",
    title: "Demo",
    state: "draft",
    blocked_by: [],
    created_at: now,
    updated_at: now,
    ...overrides,
  };
}

describe("runDoctor (fast form)", () => {
  it("returns exactly three dimensions by default: scaffold, init-script, verdict-freshness", async () => {
    const checks = await runDoctor(baseDeps(cwd));

    expect(checks.map((c) => c.name).sort()).toEqual([
      "init-script",
      "scaffold",
      "verdict-freshness",
    ]);
  });

  it("init-script dimension warns when init.sh is absent", async () => {
    const checks = await runDoctor(baseDeps(cwd));

    const initCheck = checks.find((c) => c.name === "init-script");
    expect(initCheck?.status).toBe("warn");
  });

  it("verdict-freshness fails when tasks exist but no verdicts have ever been written", async () => {
    const checks = await runDoctor({
      ...baseDeps(cwd),
      taskStore: mockRepoTaskStore([makeTask()]),
    });

    const v = checks.find((c) => c.name === "verdict-freshness");
    expect(v?.status).toBe("fail");
  });
});

describe("runDoctor (--full)", () => {
  it("appends build and tests dimensions when full: true", async () => {
    const checks = await runDoctor({ ...baseDeps(cwd), full: true });

    const names = checks.map((c) => c.name);
    expect(names).toContain("build");
    expect(names).toContain("tests");
  });

  it("build and tests dimensions never escalate to fail (warn-only)", async () => {
    const checks = await runDoctor({ ...baseDeps(cwd), full: true });

    expect(checks.find((c) => c.name === "build")?.status).not.toBe("fail");
    expect(checks.find((c) => c.name === "tests")?.status).not.toBe("fail");
  });
});

describe("runDoctor (additional coverage)", () => {
  it("scaffold returns fail when expected directories are missing", async () => {
    // Default tmpdir has only .maestro/; setup-check expects .maestro/tasks etc.
    const checks = await runDoctor(baseDeps(cwd));
    const scaffold = checks.find((c) => c.name === "scaffold");
    expect(scaffold?.status).toBe("fail");
  });

  it("init-script is ok when present and executable", async () => {
    await writeFile(join(cwd, "init.sh"), "#!/bin/sh\nexit 0\n");
    await chmod(join(cwd, "init.sh"), 0o755);
    const checks = await runDoctor(baseDeps(cwd));
    expect(checks.find((c) => c.name === "init-script")?.status).toBe("ok");
  });

  it("verdict-freshness is ok in a vacuous repo (no tasks, no verdicts)", async () => {
    const checks = await runDoctor(baseDeps(cwd));
    expect(checks.find((c) => c.name === "verdict-freshness")?.status).toBe("ok");
  });

  it("verdict-freshness falls back to config.doctor.verdictStaleDays when env is unset", async () => {
    delete process.env.MAESTRO_VERDICT_STALE_DAYS;
    const oldVerdict: Verdict = {
      schemaVersion: 1,
      id: "vrd-old",
      taskId: "tsk-test-0001",
      contractVersion: 1,
      computedAt: new Date(Date.now() - 10 * 86_400_000).toISOString(),
      decision: "PASS",
      effectiveRiskClass: "low",
      reasons: [],
      evidenceConsulted: [],
      policiesConsulted: [],
      trustVerifier: { findingsCount: 0, errors: 0, warns: 0, infos: 0 },
    };
    const tightConfig = mockConfig({
      loadLayers: async () => ({
        defaults: {},
        effective: { doctor: { verdictStaleDays: 1 } },
        global: {},
        project: {},
        errors: [],
        paths: { project: "", global: "" },
      }),
    });
    const checks = await runDoctor({
      ...baseDeps(cwd),
      config: tightConfig,
      taskStore: mockRepoTaskStore([makeTask()]),
      verdictStore: mockVerdictStore([oldVerdict]),
    });
    expect(checks.find((c) => c.name === "verdict-freshness")?.status).toBe("warn");
  });
});

describe("runDoctor (env-driven staleness threshold)", () => {
  it("verdict-freshness threshold flips with MAESTRO_VERDICT_STALE_DAYS", async () => {
    const oldVerdict: Verdict = {
      schemaVersion: 1,
      id: "vrd-old",
      taskId: "tsk-test-0001",
      contractVersion: 1,
      // 10 days ago
      computedAt: new Date(Date.now() - 10 * 86_400_000).toISOString(),
      decision: "PASS",
      effectiveRiskClass: "low",
      reasons: [],
      evidenceConsulted: [],
      policiesConsulted: [],
      trustVerifier: { findingsCount: 0, errors: 0, warns: 0, infos: 0 },
    };
    const deps = {
      ...baseDeps(cwd),
      taskStore: mockRepoTaskStore([makeTask()]),
      verdictStore: mockVerdictStore([oldVerdict]),
    };

    process.env.MAESTRO_VERDICT_STALE_DAYS = "1";
    const stale = await runDoctor(deps);
    expect(stale.find((c) => c.name === "verdict-freshness")?.status).toBe("warn");

    process.env.MAESTRO_VERDICT_STALE_DAYS = "30";
    const fresh = await runDoctor(deps);
    expect(fresh.find((c) => c.name === "verdict-freshness")?.status).toBe("ok");
  });
});
