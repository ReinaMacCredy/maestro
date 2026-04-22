import { beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { FsLaunchStoreAdapter, pickupHandoff } from "@/features/handoff";
import { FsContractStoreAdapter } from "@/features/task/adapters/fs-contract-store.adapter.js";
import { createContract } from "@/features/task/usecases/contract/create-contract.usecase.js";
import { lockContract } from "@/features/task/usecases/contract/lock-contract.usecase.js";
import {
  FsTaskContinuationHistoryStoreAdapter,
  FsTaskContinuationStoreAdapter,
  JsonlTaskStoreAdapter,
  claimTask,
  createTask,
  syncTaskContinuation,
  updateTask,
} from "@/features/task";

describe("pickupHandoff", () => {
  let tmpDir: string;
  let launchStore: FsLaunchStoreAdapter;
  let taskStore: JsonlTaskStoreAdapter;
  let contractStore: FsContractStoreAdapter;
  let continuationStore: FsTaskContinuationStoreAdapter;
  let continuationHistory: FsTaskContinuationHistoryStoreAdapter;

  beforeEach(async () => {
    tmpDir = await mkdtemp(join(tmpdir(), "handoff-pickup-usecase-"));
    launchStore = new FsLaunchStoreAdapter(tmpDir);
    taskStore = new JsonlTaskStoreAdapter(tmpDir);
    contractStore = new FsContractStoreAdapter(tmpDir);
    continuationStore = new FsTaskContinuationStoreAdapter(tmpDir);
    continuationHistory = new FsTaskContinuationHistoryStoreAdapter(tmpDir);
  });

  it("consumes the handoff, force-claims the linked task, and records pickup history", async () => {
    const task = await createTask(taskStore, { title: "Resume me" });
    await claimTask(taskStore, task.id, { sessionId: "codex-old-session" });
    const started = (await updateTask(
      taskStore,
      task.id,
      { status: "in_progress" },
      { sessionId: "codex-old-session" },
    )).task;
    const drafted = await createContract(taskStore, contractStore, {
      taskId: task.id,
      repoRoot: tmpDir,
      intent: "Keep pickup ownership aligned with the linked task",
      scope: {
        filesExpected: ["README.md"],
        filesForbidden: [],
      },
      doneWhen: [{ text: "pickup keeps contract ownership aligned", kind: "manual" }],
      createdBy: "codex-old-session",
      configSnapshot: {
        strict: false,
        overlapPolicy: "fail",
        rebaseFallback: "best-effort",
        staleReclaimContractPolicy: "inherit",
      },
    });
    await lockContract(contractStore, {
      ref: drafted.id,
      actorId: "codex-old-session",
      configSnapshot: {
        strict: false,
        overlapPolicy: "fail",
        rebaseFallback: "best-effort",
        staleReclaimContractPolicy: "inherit",
      },
    });
    await syncTaskContinuation(
      {
        continuationStore,
        continuationHistory,
      },
      {
        task: started,
        summary: {
          currentState: "Existing state before handoff pickup",
          nextAction: "Keep working on the task",
        },
      },
    );

    const launch = await launchStore.create({
      task: "Pick this up",
      name: "[Handoff] Pick this up",
      agent: "claude",
      model: "opus",
      wait: false,
      sourceDir: tmpDir,
      targetDir: tmpDir,
      refs: { taskId: task.id },
      createdByAgent: "codex",
      createdBySessionId: "old-session",
      prompt: "## Task\n\nPick this up\n",
    });

    const result = await pickupHandoff(
      {
        launchStore,
        taskStore,
        contractStore,
        continuationStore,
        continuationHistory,
      },
      {
        id: launch.id,
        actorAgent: "claude",
        actorSessionId: "pickup-1",
        ownerId: "claude-code-pickup-1",
      },
    );

    expect(result.record).toMatchObject({
      id: launch.id,
      pickedUpByAgent: "claude",
      pickedUpBySessionId: "pickup-1",
    });

    const resumed = await taskStore.get(task.id);
    expect(resumed).toMatchObject({
      status: "in_progress",
      assignee: "claude-code-pickup-1",
    });
    const contract = await contractStore.getByTaskId(task.id);
    expect(contract).toMatchObject({
      status: "locked",
      lockedBy: "claude-code-pickup-1",
    });

    const summary = await continuationStore.getActive(task.id);
    expect(summary?.currentState).toContain(`Resumed from handoff ${launch.id}`);
    expect(summary?.nextAction).toBe("Keep working on the task");

    const history = await continuationHistory.listRecent(task.id, 5);
    expect(history.map((event) => event.kind)).toContain("handoff_picked_up");
  });

  it("rejects pickup when the linked task has already completed", async () => {
    const task = await createTask(taskStore, { title: "Done already" });
    const completed = (await updateTask(taskStore, task.id, { status: "completed", reason: "done" })).task;
    await syncTaskContinuation(
      {
        continuationStore,
        continuationHistory,
      },
      {
        task: completed,
        summary: {
          currentState: "Completed already",
          nextAction: "Nothing left to do",
          activeAgent: null,
        },
      },
    );

    const launch = await launchStore.create({
      task: "Should fail",
      name: "[Handoff] Should fail",
      agent: "codex",
      model: "gpt-5.4",
      wait: false,
      sourceDir: tmpDir,
      targetDir: tmpDir,
      refs: { taskId: task.id },
      prompt: "## Task\n\nShould fail\n",
    });

    await expect(
      pickupHandoff(
        {
          launchStore,
          taskStore,
          contractStore,
          continuationStore,
          continuationHistory,
        },
        {
          id: launch.id,
          actorAgent: "codex",
          actorSessionId: "pickup-2",
          ownerId: "codex-pickup-2",
        },
      ),
    ).rejects.toThrow("already completed");

    const reloaded = await launchStore.get(launch.id);
    expect(reloaded?.consumedAt).toBeUndefined();
  });

  it("keeps pickup successful when contract ownership transfer fails after resume", async () => {
    const task = await createTask(taskStore, { title: "Resume even if contract transfer fails" });
    await claimTask(taskStore, task.id, { sessionId: "codex-old-session" });
    const started = (await updateTask(
      taskStore,
      task.id,
      { status: "in_progress" },
      { sessionId: "codex-old-session" },
    )).task;
    const drafted = await createContract(taskStore, contractStore, {
      taskId: task.id,
      repoRoot: tmpDir,
      intent: "Keep pickup resilient even if contract transfer fails",
      scope: {
        filesExpected: ["README.md"],
        filesForbidden: [],
      },
      doneWhen: [{ text: "pickup succeeds", kind: "manual" }],
      createdBy: "codex-old-session",
      configSnapshot: {
        strict: false,
        overlapPolicy: "fail",
        rebaseFallback: "best-effort",
        staleReclaimContractPolicy: "inherit",
      },
    });
    await lockContract(contractStore, {
      ref: drafted.id,
      actorId: "codex-old-session",
      configSnapshot: {
        strict: false,
        overlapPolicy: "fail",
        rebaseFallback: "best-effort",
        staleReclaimContractPolicy: "inherit",
      },
    });
    await syncTaskContinuation(
      {
        continuationStore,
        continuationHistory,
      },
      {
        task: started,
        summary: {
          currentState: "Existing state before handoff pickup",
          nextAction: "Keep working on the task",
        },
      },
    );

    const launch = await launchStore.create({
      task: "Pick this up",
      name: "[Handoff] Pick this up",
      agent: "claude",
      model: "opus",
      wait: false,
      sourceDir: tmpDir,
      targetDir: tmpDir,
      refs: { taskId: task.id },
      createdByAgent: "codex",
      createdBySessionId: "old-session",
      prompt: "## Task\n\nPick this up\n",
    });

    const result = await pickupHandoff(
      {
        launchStore,
        taskStore,
        contractStore: {
          ...contractStore,
          getByTaskId: async () => {
            throw new Error("contract store offline");
          },
        },
        continuationStore,
        continuationHistory,
      },
      {
        id: launch.id,
        actorAgent: "claude",
        actorSessionId: "pickup-3",
        ownerId: "claude-code-pickup-3",
      },
    );

    expect(result.taskId).toBe(task.id);
    const resumed = await taskStore.get(task.id);
    expect(resumed).toMatchObject({
      status: "in_progress",
      assignee: "claude-code-pickup-3",
    });
    const reloaded = await launchStore.get(launch.id);
    expect(reloaded?.consumedAt).toBeTruthy();
    expect(result.contractTransferWarning).toMatch(/contract ownership transfer failed/);
    expect(result.contractTransferWarning).toContain("contract store offline");
  });
});
