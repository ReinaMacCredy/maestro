import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { access, mkdtemp, rm } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { FsLaunchStoreAdapter } from "@/features/handoff";

let projectDir: string;

beforeEach(async () => {
  projectDir = await mkdtemp(join(tmpdir(), "maestro-launch-store-"));
});

afterEach(async () => {
  await rm(projectDir, { recursive: true, force: true });
});

describe("FsLaunchStoreAdapter", () => {
  it("persists prompt, output log, and metadata under .maestro/launches/<id>/", async () => {
    const store = new FsLaunchStoreAdapter(projectDir);

    const record = await store.create({
      task: "Investigate the failing build",
      name: "[Handoff] Investigate the failing build",
      provider: "codex",
      model: "gpt-5.4",
      wait: false,
      sourceDir: projectDir,
      targetDir: projectDir,
      refs: { missionId: "2026-04-20-001", featureId: "f1", milestoneId: "m1" },
      prompt: "## Task\n\nInvestigate the failing build\n",
    });

    await access(join(projectDir, record.promptPath));
    await access(join(projectDir, record.outputPath));
    await access(join(projectDir, ".maestro", "launches", record.id, "launch.json"));

    const listed = await store.list();
    expect(listed).toHaveLength(1);
    expect(listed[0]).toMatchObject({
      id: record.id,
      provider: "codex",
      model: "gpt-5.4",
      status: "launching",
    });
  });

  it("updates launch metadata after the provider process finishes", async () => {
    const store = new FsLaunchStoreAdapter(projectDir);
    const created = await store.create({
      task: "Fix tests",
      name: "[Handoff] Fix tests",
      provider: "claude",
      model: "opus",
      wait: true,
      sourceDir: projectDir,
      targetDir: join(projectDir, "worktree"),
      refs: {},
      prompt: "## Task\n\nFix tests\n",
    });

    const updated = await store.update({
      ...created,
      status: "completed",
      command: ["claude", "--print", "--permission-mode", "bypassPermissions", "Fix tests"],
      exitCode: 0,
    });

    expect(updated.exitCode).toBe(0);
    expect(updated.status).toBe("completed");

    const reloaded = await store.get(created.id);
    expect(reloaded).toMatchObject({
      id: created.id,
      status: "completed",
      exitCode: 0,
    });
  });
});
