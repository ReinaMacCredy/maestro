import { afterEach, beforeAll, beforeEach, describe, expect, it } from "bun:test";
import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
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

let tmpDir: string;

beforeAll(buildCompiledCli, BUILD_TIMEOUT_MS);

beforeEach(async () => {
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-task-e2e-"));
  await initGitRepo(tmpDir);
});

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true });
});

describe("compiled task feature E2E", () => {
  it(
    "runs the full Claude-style task loop against ./dist/maestro",
    async () => {
      const captured = await runCompiled(["task", "q", "login endpoint", "--priority", "1"], tmpDir);
      const apiId = captured.stdout;
      expect(apiId).toMatch(/^tsk-[0-9a-f]{6}$/);

      const mwResult = await runCompiled(
        [
          "task",
          "create",
          "JWT middleware",
          "--blocked-by",
          apiId,
          "--priority",
          "1",
          "--type",
          "feature",
          "--labels",
          "auth,backend",
          "--json",
        ],
        tmpDir,
      );
      const mw = expectJson<{
        id: string;
        status: string;
        priority: number;
        type: string;
        labels: string[];
        blockedBy: string[];
      }>(mwResult);
      expect(mw.status).toBe("pending");
      expect(mw.priority).toBe(1);
      expect(mw.type).toBe("feature");
      expect(mw.labels).toEqual(["auth", "backend"]);
      expect(mw.blockedBy).toEqual([apiId]);

      const jsonlPath = join(tmpDir, ".maestro", "tasks", "tasks.jsonl");
      const rawJsonl = await readFile(jsonlPath, "utf8");
      const lines = rawJsonl.split("\n").filter((line) => line.length > 0);
      expect(lines.length).toBe(2);
      for (const line of lines) {
        expect(() => JSON.parse(line)).not.toThrow();
      }

      const readyBefore = await runCompiled(["task", "ready", "--json"], tmpDir);
      expect(expectJson<Array<{ id: string }>>(readyBefore).map((task) => task.id)).toEqual([apiId]);

      const claimed = await runCompiled(
        ["task", "claim", apiId, "--session", "operator-a", "--json"],
        tmpDir,
      );
      const claimedTask = expectJson<{ assignee: string; status: string; claimedAt: string }>(claimed);
      expect(claimedTask.assignee).toBe("operator-a");
      expect(claimedTask.status).toBe("pending");
      expect(claimedTask.claimedAt).toMatch(/^\d{4}-\d{2}-\d{2}T/);

        const started = await runCompiled(
          [
            "task",
            "update",
            apiId,
            "--title",
            "POST /login endpoint",
            "--status",
            "in_progress",
            "--session",
            "operator-a",
            "--json",
          ],
          tmpDir,
        );
      expect(expectJson<{ title: string; status: string }>(started)).toEqual(
        expect.objectContaining({ title: "POST /login endpoint", status: "in_progress" }),
      );

        const relabeled = await runCompiled(
          ["task", "update", apiId, "--add-label", "urgent", "--session", "operator-a", "--json"],
          tmpDir,
        );
        expect(expectJson<{ labels: string[] }>(relabeled).labels).toEqual(["urgent"]);

        const completed = await runCompiled(
          [
            "task",
            "update",
            apiId,
            "--status",
            "completed",
            "--reason",
            "shipped",
            "--session",
            "operator-a",
            "--json",
          ],
          tmpDir,
        );
      const closed = expectJson<{ status: string; closeReason: string }>(completed);
      expect(closed.status).toBe("completed");
      expect(closed.closeReason).toBe("shipped");

      const readyAfter = await runCompiled(["task", "ready", "--json"], tmpDir);
      expect(expectJson<Array<{ id: string }>>(readyAfter).map((task) => task.id)).toEqual([mw.id]);

      const listPending = await runCompiled(["task", "list", "--status", "pending", "--json"], tmpDir);
      expect(expectJson<Array<{ id: string }>>(listPending).map((task) => task.id)).toEqual([mw.id]);

      const listCompleted = await runCompiled(["task", "list", "--status", "completed", "--json"], tmpDir);
      expect(expectJson<Array<{ id: string }>>(listCompleted).map((task) => task.id)).toEqual([apiId]);

      const showCompleted = await runCompiled(["task", "show", apiId, "--json"], tmpDir);
      expect(expectJson<{ status: string; closeReason: string; title: string }>(showCompleted)).toEqual(
        expect.objectContaining({
          status: "completed",
          closeReason: "shipped",
          title: "POST /login endpoint",
        }),
      );
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  it(
    "task claim fails loudly when session detection is unavailable",
    async () => {
      const id = (await runCompiled(["task", "q", "unclaimed"], tmpDir)).stdout;
      const claim = await runCompiled(
        ["task", "claim", id],
        tmpDir,
        { env: { CLAUDECODE: "", CODEX_THREAD_ID: "" } },
      );
      expect(claim.exitCode).not.toBe(0);
      expect(claim.stderr).toContain("Could not detect current session");
      expect(claim.stderr).toContain("--session <id>");
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  it(
    "task force-claim and force-unclaim support explicit session overrides without agent env",
    async () => {
      const id = (await runCompiled(["task", "q", "recoverable"], tmpDir)).stdout;

      const initialClaim = await runCompiled(
        ["task", "claim", id, "--session", "session-a", "--json"],
        tmpDir,
      );
      expect(expectJson<{ assignee: string }>(initialClaim).assignee).toBe("session-a");

      const takeover = await runCompiled(
        ["task", "claim", id, "--force", "--session", "operator-recovery", "--json"],
        tmpDir,
        { env: { CLAUDECODE: "", CODEX_THREAD_ID: "" } },
      );
      expect(expectJson<{ assignee: string; status: string }>(takeover)).toEqual(
        expect.objectContaining({ assignee: "operator-recovery", status: "pending" }),
      );

      const release = await runCompiled(
        ["task", "unclaim", id, "--force", "--session", "operator-recovery", "--json"],
        tmpDir,
        { env: { CLAUDECODE: "", CODEX_THREAD_ID: "" } },
      );
      const unclaimed = expectJson<{ assignee?: string; status: string }>(release);
      expect(unclaimed.assignee).toBeUndefined();
      expect(unclaimed.status).toBe("pending");
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  it(
    "rejects legacy ownership, completion, and dependency commands with migration guidance",
    async () => {
      const id = (await runCompiled(["task", "q", "legacy flags"], tmpDir)).stdout;
      const dep = (await runCompiled(["task", "q", "dep"], tmpDir)).stdout;

      const updateAssignee = await runCompiled(["task", "update", id, "--assignee", "someone-else"], tmpDir);
      expect(updateAssignee.exitCode).not.toBe(0);
      expect(updateAssignee.stderr).toContain("task claim");

      const updateClaim = await runCompiled(["task", "update", id, "--claim"], tmpDir);
      expect(updateClaim.exitCode).not.toBe(0);
      expect(updateClaim.stderr).toContain("task claim");

      const badClose = await runCompiled(["task", "close", id], tmpDir);
      expect(badClose.exitCode).not.toBe(0);
      expect(badClose.stderr).toContain("status completed");

      const badDeps = await runCompiled(["task", "deps", "add", id, dep], tmpDir);
      expect(badDeps.exitCode).not.toBe(0);
      expect(badDeps.stderr).toContain("task block");
    },
    SLOW_CLI_TIMEOUT_MS,
  );

    it(
      "supports blocker lifecycle edits after creation",
    async () => {
      const blockerId = (await runCompiled(["task", "q", "blocker"], tmpDir)).stdout;
      const blockedId = (await runCompiled(["task", "q", "blocked"], tmpDir)).stdout;

      const initialReady = await runCompiled(["task", "ready", "--json"], tmpDir);
      expect(expectJson<Array<{ id: string }>>(initialReady).map((task) => task.id).sort()).toEqual(
        [blockerId, blockedId].sort(),
      );

      const added = await runCompiled(["task", "block", blockerId, blockedId, "--json"], tmpDir);
      expect(expectJson<{ blocks: string[] }>(added).blocks).toEqual([blockedId]);

      const blockedReady = await runCompiled(["task", "ready", "--json"], tmpDir);
      expect(expectJson<Array<{ id: string }>>(blockedReady).map((task) => task.id)).toEqual([blockerId]);

      const blockedClaim = await runCompiled(
        ["task", "claim", blockedId, "--force", "--session", "blocked-owner"],
        tmpDir,
      );
      expect(blockedClaim.exitCode).not.toBe(0);
      expect(blockedClaim.stderr).toContain("blocked by unresolved");

      const removed = await runCompiled(["task", "unblock", blockerId, blockedId, "--json"], tmpDir);
      expect(expectJson<{ blocks: string[] }>(removed).blocks).toEqual([]);

      const unblockedReady = await runCompiled(["task", "ready", "--json"], tmpDir);
      expect(expectJson<Array<{ id: string }>>(unblockedReady).map((task) => task.id).sort()).toEqual(
        [blockerId, blockedId].sort(),
      );
      },
      SLOW_CLI_TIMEOUT_MS,
    );

    it(
      "releases unresolved tasks owned by a dead session",
        async () => {
          const id = (await runCompiled(["task", "q", "recover me"], tmpDir)).stdout;
          await runCompiled(["task", "claim", id, "--session", "dead-session", "--json"], tmpDir);
          await runCompiled(["task", "update", id, "--status", "in_progress", "--session", "dead-session", "--json"], tmpDir);

        const released = await runCompiled(["task", "release-owned", "dead-session", "--json"], tmpDir);
        const payload = expectJson<Array<{ id: string; status: string; assignee?: string }>>(released);
        expect(payload).toHaveLength(1);
        expect(payload[0]).toEqual(expect.objectContaining({ id, status: "pending" }));
        expect(payload[0]?.assignee).toBeUndefined();
      },
      SLOW_CLI_TIMEOUT_MS,
    );

    it(
      "enforces ownership and status invariants through update paths",
        async () => {
          const id = (await runCompiled(["task", "q", "ownership invariants"], tmpDir)).stdout;
          await runCompiled(["task", "claim", id, "--session", "session-a", "--json"], tmpDir);

        const retitle = await runCompiled(
          ["task", "update", id, "--title", "same owner edit", "--session", "session-a", "--json"],
          tmpDir,
        );
        expect(expectJson<{ title: string; status: string }>(retitle)).toEqual(
          expect.objectContaining({ title: "same owner edit", status: "pending" }),
        );

        const foreignComplete = await runCompiled(
          ["task", "update", id, "--status", "completed", "--reason", "stolen", "--json"],
          tmpDir,
          { env: { CLAUDECODE: "", CODEX_THREAD_ID: "" } },
        );
        expect(foreignComplete.exitCode).not.toBe(0);
        expect(`${foreignComplete.stdout}\n${foreignComplete.stderr}`).toContain("requires the owner session or --force");

        await runCompiled(["task", "update", id, "--status", "in_progress", "--session", "session-a", "--json"], tmpDir);

        const reopen = await runCompiled(["task", "update", id, "--status", "pending", "--session", "session-a"], tmpDir);
        expect(reopen.exitCode).not.toBe(0);
        expect(reopen.stderr).toContain("cannot move to 'pending' while still claimed");

      const unclaimedId = (await runCompiled(["task", "q", "unclaimed progress"], tmpDir)).stdout;
      const badStart = await runCompiled(["task", "update", unclaimedId, "--status", "in_progress"], tmpDir);
      expect(badStart.exitCode).not.toBe(0);
      expect(badStart.stderr).toContain("requires task ownership");

      const completedId = (await runCompiled(["task", "q", "closed immutable"], tmpDir)).stdout;
      await runCompiled(
        ["task", "update", completedId, "--status", "completed", "--reason", "done"],
        tmpDir,
      );

        const completedEdit = await runCompiled(["task", "update", completedId, "--title", "still mutable?"], tmpDir);
        expect(completedEdit.exitCode).not.toBe(0);
        expect(completedEdit.stderr).toContain("already completed");

        const blockerId = (await runCompiled(["task", "q", "blocking"], tmpDir)).stdout;
        const blockedId = (await runCompiled(["task", "q", "blocked later", "--blocked-by", blockerId], tmpDir)).stdout;
        const blockedComplete = await runCompiled(
          ["task", "update", blockedId, "--status", "completed", "--reason", "nope"],
          tmpDir,
        );
        expect(blockedComplete.exitCode).not.toBe(0);
        expect(blockedComplete.stderr).toContain("blocked by unresolved");
      },
        SLOW_CLI_TIMEOUT_MS,
      );

    it(
      "auto-releases stale known-agent owners during ready queries",
      async () => {
        const id = (await runCompiled(["task", "q", "stale owner"], tmpDir)).stdout;
        await runCompiled(["task", "claim", id, "--session", "codex-dead-thread", "--json"], tmpDir);

        const ready = await runCompiled(["task", "ready", "--json"], tmpDir);
        expect(ready.stderr).toContain("Released 1 stale task(s) owned by codex-dead-thread");
        expect(expectJson<Array<{ id: string }>>(ready).map((task) => task.id)).toContain(id);

        const shown = await runCompiled(["task", "show", id, "--json"], tmpDir);
        const releasedTask = expectJson<{ assignee?: string; status: string }>(shown);
        expect(releasedTask.status).toBe("pending");
        expect(releasedTask.assignee).toBeUndefined();
      },
      SLOW_CLI_TIMEOUT_MS,
    );

  it(
    "validates --blocked-by against existing tasks",
    async () => {
      const bad = await runCompiled(
        ["task", "create", "references a ghost", "--blocked-by", "tsk-000000"],
        tmpDir,
      );
      expect(bad.exitCode).not.toBe(0);
      expect(bad.stderr).toContain("unknown blocker");
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  it(
    "active memory: completion seeds a candidate and ready surfaces a hint",
    async () => {
      const pastId = (await runCompiled(["task", "q", "Implement argon2 password hashing"], tmpDir)).stdout;
      const close = await runCompiled(
        ["task", "update", pastId, "--status", "completed", "--reason", "argon2 compare was backwards, wasted a day"],
        tmpDir,
      );
      expect(close.exitCode).toBe(0);

      const candidatePath = join(tmpDir, ".maestro", "tasks", "candidates", `${pastId}.json`);
      const rawCandidate = await readFile(candidatePath, "utf8");
      const candidate: {
        id: string;
        sourceTaskId: string;
        sourceType: string;
        reason: string;
        keywords: string[];
        capturedAt: string;
      } = JSON.parse(rawCandidate);
      expect(candidate.id).toBe(pastId);
      expect(candidate.sourceTaskId).toBe(pastId);
      expect(candidate.sourceType).toBe("task-close");
      expect(candidate.reason).toBe("argon2 compare was backwards, wasted a day");

      await runCompiled(["task", "q", "JWT password middleware"], tmpDir);
      await runCompiled(["task", "q", "Protected routes"], tmpDir);

      const ready = await runCompiled(["task", "ready", "--json"], tmpDir);
      const briefings = expectJson<Array<{
        title: string;
        hints: Array<{ sourceTaskId: string; matchedKeywords: string[] }>;
      }>>(ready);
      const byTitle = new Map(briefings.map((briefing) => [briefing.title, briefing] as const));

      expect(byTitle.get("JWT password middleware")?.hints[0]?.sourceTaskId).toBe(pastId);
      expect(byTitle.get("JWT password middleware")?.hints[0]?.matchedKeywords).toContain("password");
      expect(byTitle.get("Protected routes")?.hints).toEqual([]);
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  it(
    "active memory: --no-hints disables hint attachment for scripts",
    async () => {
      const pastId = (await runCompiled(["task", "q", "Implement auth module"], tmpDir)).stdout;
      await runCompiled(
        ["task", "update", pastId, "--status", "completed", "--reason", "auth token expiry wrong"],
        tmpDir,
      );
      await runCompiled(["task", "q", "Refactor auth handler"], tmpDir);

      const withHints = await runCompiled(["task", "ready", "--json"], tmpDir);
      expect(expectJson<Array<{ hints: unknown[] }>>(withHints)[0]?.hints.length).toBeGreaterThanOrEqual(1);

      const noHints = await runCompiled(["task", "ready", "--no-hints", "--json"], tmpDir);
      expect(expectJson<Array<{ hints: unknown[] }>>(noHints)[0]?.hints).toEqual([]);
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  it(
    "warns but still succeeds when candidate capture fails after completion",
    async () => {
      const id = (await runCompiled(["task", "q", "candidate write fail"], tmpDir)).stdout;
      const candidatePath = join(tmpDir, ".maestro", "tasks", "candidates", `${id}.json`);
      await mkdir(candidatePath, { recursive: true });

      const completed = await runCompiled(
        ["task", "update", id, "--status", "completed", "--reason", "done"],
        tmpDir,
      );
      expect(completed.exitCode).toBe(0);
      expect(completed.stdout).toContain("Task updated:");
      expect(completed.stderr).toContain("hint capture failed");

      const shown = await runCompiled(["task", "show", id, "--json"], tmpDir);
      expect(expectJson<{ status: string; closeReason: string }>(shown)).toEqual(
        expect.objectContaining({ status: "completed", closeReason: "done" }),
      );
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  it(
    "keeps task ready working when a candidate file is malformed",
    async () => {
      const id = (await runCompiled(["task", "q", "candidate reader"], tmpDir)).stdout;
      const candidatesDir = join(tmpDir, ".maestro", "tasks", "candidates");
      await mkdir(candidatesDir, { recursive: true });
      await writeFile(join(candidatesDir, "broken.json"), "{bad json\n");

      const ready = await runCompiled(["task", "ready", "--json"], tmpDir);
      expect(expectJson<Array<{ id: string }>>(ready).map((task) => task.id)).toEqual([id]);
    },
    SLOW_CLI_TIMEOUT_MS,
  );
});
