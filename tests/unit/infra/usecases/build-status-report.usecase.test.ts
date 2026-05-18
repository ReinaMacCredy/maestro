import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdir, mkdtemp, rm } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { buildStatusReport } from "@/infra/usecases/build-status-report.usecase.js";
import {
  mockConfig,
  mockGit,
  mockMissionStore,
  mockRepoTaskStore,
  mockRepoEvidenceStore,
  mockVerdictStore,
} from "../../../helpers/mocks.js";

let cwd: string;

beforeEach(async () => {
  cwd = await mkdtemp(join(tmpdir(), "maestro-build-status-"));
});

afterEach(async () => {
  await rm(cwd, { recursive: true, force: true });
});

function baseDeps(projectDir: string) {
  return {
    config: mockConfig({ exists: async () => true }),
    git: mockGit(),
    taskStore: mockRepoTaskStore(),
    featureMissionStore: mockMissionStore(),
    verdictStore: mockVerdictStore(),
    evidenceStore: mockRepoEvidenceStore(),
    projectDir,
  };
}

describe("buildStatusReport", () => {
  it("returns the five top-level sections in the locked order", async () => {
    await mkdir(join(cwd, ".maestro"), { recursive: true });
    const report = await buildStatusReport(baseDeps(cwd));

    expect(Object.keys(report)).toEqual([
      "maestro_health",
      "project_state",
      "missions",
      "next_ready",
      "recent_transitions",
    ]);
  });

  it("emits empty-state hints under each empty section", async () => {
    await mkdir(join(cwd, ".maestro"), { recursive: true });
    const report = await buildStatusReport(baseDeps(cwd));

    expect(report.missions).toEqual([]);
    expect(report.next_ready).toBeUndefined();
    expect(report.recent_transitions).toEqual([]);
    expect(report.project_state.stuck_verifying_count).toBe(0);
    expect(report.project_state.stale_handoff_count).toBe(0);
    expect(report.project_state.latest_verdict).toBeUndefined();
  });

  it("project_state has stable JSON keys", async () => {
    await mkdir(join(cwd, ".maestro"), { recursive: true });
    const report = await buildStatusReport(baseDeps(cwd));

    expect(Object.keys(report.project_state).sort()).toEqual([
      "latest_verdict",
      "stale_handoff_count",
      "stuck_verifying_count",
    ]);
  });

  it("terse mode omits recent_transitions and collapses maestro_health", async () => {
    await mkdir(join(cwd, ".maestro"), { recursive: true });
    const report = await buildStatusReport({ ...baseDeps(cwd), terse: true });

    expect(report.recent_transitions).toBeUndefined();
    expect(Array.isArray(report.maestro_health)).toBe(true);
  });

  it("hard-refuses when .maestro/ directory is missing", async () => {
    // .maestro/ intentionally not created
    await expect(
      buildStatusReport({
        ...baseDeps(cwd),
        config: mockConfig({ exists: async () => false }),
      }),
    ).rejects.toThrow(/not initialized/i);
  });
});
