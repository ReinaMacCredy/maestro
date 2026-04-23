import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { access, mkdtemp, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { FsHandoffStoreAdapter, HANDOFF_DIR } from "@/features/handoff";
import { MAESTRO_DIR } from "@/shared/domain/defaults.js";

let storeRoot: string;

beforeEach(async () => {
  storeRoot = await mkdtemp(join(tmpdir(), "maestro-handoff-store-"));
});

afterEach(async () => {
  await rm(storeRoot, { recursive: true, force: true });
});

describe("FsHandoffStoreAdapter", () => {
  it("persists prompt, output log, and metadata under .maestro/handoff/<id>/", async () => {
    const store = new FsHandoffStoreAdapter(storeRoot);

    const record = await store.create({
      task: "Investigate the failing build",
      name: "[Handoff] Investigate the failing build",
      agent: "codex",
      model: "gpt-5.4",
      wait: false,
      sourceDir: storeRoot,
      targetDir: storeRoot,
      refs: { missionId: "2026-04-20-001", featureId: "f1", milestoneId: "m1" },
      prompt: "## Task\n\nInvestigate the failing build\n",
    });

    await access(join(storeRoot, record.promptPath));
    await access(join(storeRoot, record.outputPath));
    await access(join(storeRoot, MAESTRO_DIR, HANDOFF_DIR, record.id, "handoff.json"));

    const listed = await store.list();
    expect(listed).toHaveLength(1);
    expect(listed[0]).toMatchObject({
      id: record.id,
      agent: "codex",
      model: "gpt-5.4",
      status: "launching",
    });
  });

  it("updates handoff metadata after the agent process finishes", async () => {
    const store = new FsHandoffStoreAdapter(storeRoot);
    const created = await store.create({
      task: "Fix tests",
      name: "[Handoff] Fix tests",
      agent: "claude",
      model: "opus",
      wait: true,
      sourceDir: storeRoot,
      targetDir: join(storeRoot, "worktree"),
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

  it("consumes a handoff once and rejects a second pickup", async () => {
    const store = new FsHandoffStoreAdapter(storeRoot);
    const created = await store.create({
      task: "Fix tests",
      name: "[Handoff] Fix tests",
      agent: "codex",
      model: "gpt-5.4",
      wait: false,
      sourceDir: storeRoot,
      targetDir: storeRoot,
      refs: { taskId: "tsk-123" },
      prompt: "## Task\n\nFix tests\n",
    });

    const consumed = await store.consume({
      id: created.id,
      agent: "claude",
      sessionId: "pickup-1",
      pickedUpAt: "2026-04-21T10:00:00.000Z",
    });

    expect(consumed).toMatchObject({
      id: created.id,
      pickedUpByAgent: "claude",
      pickedUpBySessionId: "pickup-1",
      consumedAt: "2026-04-21T10:00:00.000Z",
    });

    await expect(
      store.consume({
        id: created.id,
        agent: "codex",
        sessionId: "pickup-2",
        pickedUpAt: "2026-04-21T10:01:00.000Z",
      }),
    ).rejects.toThrow("already consumed");
  });

  it("surfaces corrupt handoff metadata instead of treating it as missing", async () => {
    const store = new FsHandoffStoreAdapter(storeRoot);
    const created = await store.create({
      task: "Corrupt me",
      name: "[Handoff] Corrupt me",
      agent: "codex",
      model: "gpt-5.4",
      wait: false,
      sourceDir: storeRoot,
      targetDir: storeRoot,
      refs: {},
      prompt: "## Task\n\nCorrupt me\n",
    });

    await writeFile(join(storeRoot, MAESTRO_DIR, HANDOFF_DIR, created.id, "handoff.json"), "{bad json\n");

    await expect(store.get(created.id)).rejects.toThrow();
  });
});
