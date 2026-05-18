/**
 * L1.E2E — Full L1 evidence flow against ./dist/maestro.
 *
 * Exercises: init -> task create -> claim -> record 3 evidence rows
 * (command, command-with-optional-fields, manual-note) -> list -> show ->
 * complete task -> verify evidence preserved -> verify mission-control
 * snapshot reports evidenceCount.
 *
 * Per ROADMAP.md L1.E2E.
 */
import { afterEach, beforeAll, beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  BUILD_TIMEOUT_MS,
  SLOW_CLI_TIMEOUT_MS,
  buildCompiledCli,
  expectJson,
  initGitRepo,
  runCompiled,
} from "../helpers/run-compiled-cli.js";
import type { TaskBoardSnapshot } from "../../src/tui/state/screen-types.js";

let tmpDir: string;

beforeAll(buildCompiledCli, BUILD_TIMEOUT_MS);

beforeEach(async () => {
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-l1-e2e-"));
  await initGitRepo(tmpDir);
});

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true });
});

// TODO(v2/phase-4): re-enable or remove. v1 `task claim` is detached per ADR-0007 big-bang;
// v2 equivalents live in src/runtime/task.command.ts.
describe.skip("L1 evidence flow E2E", () => {
  it(
    "records, lists, shows, and preserves evidence through task completion",
    async () => {
      // 1. Create a task and capture its id.
      const quickResult = await runCompiled(
        ["task", "q", "L1 evidence integration test"],
        tmpDir,
      );
      expect(quickResult.exitCode).toBe(0);
      const taskId = quickResult.stdout.trim();
      expect(taskId).toMatch(/^tsk-[0-9a-f]{6}$/);

      // 2. Claim the task.
      const claimed = await runCompiled(
        ["task", "claim", taskId, "--session", "l1-e2e-session", "--json"],
        tmpDir,
      );
      expect(claimed.exitCode).toBe(0);
      const claimedTask = expectJson<{ assignee: string; status: string }>(claimed);
      expect(claimedTask.assignee).toBe("l1-e2e-session");

      // 3a. Record evidence: kind=command, minimal fields.
      const rec1 = await runCompiled(
        [
          "evidence", "record",
          "--task", taskId,
          "--command", "bun test",
          "--exit", "0",
          "--json",
        ],
        tmpDir,
      );
      expect(rec1.exitCode).toBe(0);
      const row1 = expectJson<{
        id: string;
        task_id: string;
        kind: string;
        witness_level: string;
        created_at: string;
      }>(rec1);
      expect(row1.id).toMatch(/^evd-\d{13}-[0-9a-f]{6}$/);
      expect(row1.task_id).toBe(taskId);
      expect(row1.kind).toBe("command");
      expect(row1.witness_level).toBe("agent-claimed-locally");

      // 3b. Record evidence: kind=command, with optional fields.
      const rec2 = await runCompiled(
        [
          "evidence", "record",
          "--task", taskId,
          "--command", "bun run build",
          "--exit", "0",
          "--duration", "1234",
          "--log", "./build.log",
          "--criterion", "ui-01",
          "--json",
        ],
        tmpDir,
      );
      expect(rec2.exitCode).toBe(0);
      const row2 = expectJson<{
        id: string;
        kind: string;
        witness_level: string;
        payload: { command: string; exit: number; duration_ms: number; log_path: string; criterion_id: string };
      }>(rec2);
      expect(row2.id).toMatch(/^evd-\d{13}-[0-9a-f]{6}$/);
      expect(row2.kind).toBe("command");
      expect(row2.witness_level).toBe("agent-claimed-locally");
      expect(row2.payload.command).toBe("bun run build");
      expect(row2.payload.exit).toBe(0);
      expect(row2.payload.duration_ms).toBe(1234);
      expect(row2.payload.log_path).toBe("./build.log");
      expect(row2.payload.criterion_id).toBe("ui-01");

      // 3c. Record evidence: kind=manual-note.
      const rec3 = await runCompiled(
        [
          "evidence", "record",
          "--task", taskId,
          "--kind", "manual-note",
          "--note", "Verified UI on staging",
          "--json",
        ],
        tmpDir,
      );
      expect(rec3.exitCode).toBe(0);
      const row3 = expectJson<{
        id: string;
        kind: string;
        witness_level: string;
        payload: { note: string };
      }>(rec3);
      expect(row3.id).toMatch(/^evd-\d{13}-[0-9a-f]{6}$/);
      expect(row3.kind).toBe("manual-note");
      expect(row3.witness_level).toBe("agent-claimed-and-not-reproducible");
      expect(row3.payload.note).toBe("Verified UI on staging");

      // 4. List all evidence for the task. Assert 3 rows in chronological order.
      const listResult = await runCompiled(
        ["evidence", "list", "--task", taskId, "--json"],
        tmpDir,
      );
      expect(listResult.exitCode).toBe(0);
      const listPayload = expectJson<{
        items: Array<{ id: string; kind: string; witness_level: string; created_at: string }>;
        system_items?: ReadonlyArray<unknown>;
      }>(listResult);
      const rows = listPayload.items;
      expect(rows.length).toBe(3);
      // Chronological order: row1 first, row3 last.
      expect(rows[0]!.id).toBe(row1.id);
      expect(rows[1]!.id).toBe(row2.id);
      expect(rows[2]!.id).toBe(row3.id);
      expect(rows[0]!.kind).toBe("command");
      expect(rows[1]!.kind).toBe("command");
      expect(rows[2]!.kind).toBe("manual-note");

      // 5. Show one row by id.
      const showResult = await runCompiled(
        ["evidence", "show", row2.id, "--json"],
        tmpDir,
      );
      expect(showResult.exitCode).toBe(0);
      const shown = expectJson<{
        id: string;
        kind: string;
        payload: { command: string; duration_ms: number };
      }>(showResult);
      expect(shown.id).toBe(row2.id);
      expect(shown.kind).toBe("command");
      expect(shown.payload.command).toBe("bun run build");
      expect(shown.payload.duration_ms).toBe(1234);

      // 6. Complete the task.
      const completed = await runCompiled(
        [
          "task", "update", taskId,
          "--status", "completed",
          "--reason", "all evidence recorded",
          "--session", "l1-e2e-session",
          "--json",
        ],
        tmpDir,
      );
      expect(completed.exitCode).toBe(0);
      const completedTask = expectJson<{ status: string }>(completed);
      expect(completedTask.status).toBe("completed");

      // 7. Assert evidence is preserved after completion: re-list, expect 3 rows.
      const listAfter = await runCompiled(
        ["evidence", "list", "--task", taskId, "--json"],
        tmpDir,
      );
      expect(listAfter.exitCode).toBe(0);
      const listAfterPayload = expectJson<{
        items: Array<{ id: string }>;
        system_items?: ReadonlyArray<unknown>;
      }>(listAfter);
      expect(listAfterPayload.items.length).toBe(3);

      // 8. Assert Mission Control snapshot exposes evidenceCount: 3 for this task.
      const mcResult = await runCompiled(
        ["mission-control", "--json"],
        tmpDir,
      );
      expect(mcResult.exitCode).toBe(0);
      const snapshot = expectJson<{ taskBoard: TaskBoardSnapshot | null }>(mcResult);
      expect(snapshot.taskBoard).not.toBeNull();
      const taskBoard = snapshot.taskBoard!;
      // The completed task should appear in the completed column.
      const completedItems = taskBoard.columns["completed"] ?? [];
      const evidenceItem = completedItems.find((item) => item.id === taskId);
      expect(evidenceItem).toBeDefined();
      expect(evidenceItem!.evidenceCount).toBe(3);
    },
    SLOW_CLI_TIMEOUT_MS,
  );
});
