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
    "runs the full daily loop against ./dist/maestro",
    async () => {
      const captured = await runCompiled(
        ["task", "q", "login endpoint", "--priority", "1"],
        tmpDir,
      );
      expect(captured.exitCode).toBe(0);
      const apiId = captured.stdout;
      expect(apiId).toMatch(/^tsk-[0-9a-f]{6}$/);

      const mwResult = await runCompiled(
        [
          "task",
          "create",
          "JWT middleware",
          "--depends-on",
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
      expect(mwResult.exitCode).toBe(0);
      const mw = expectJson<{
        id: string;
        status: string;
        priority: number;
        type: string;
        labels: string[];
        dependsOn: string[];
      }>(mwResult);
      expect(mw.id).toMatch(/^tsk-[0-9a-f]{6}$/);
      expect(mw.status).toBe("open");
      expect(mw.priority).toBe(1);
      expect(mw.type).toBe("feature");
      expect(mw.labels).toEqual(["auth", "backend"]);
      expect(mw.dependsOn).toEqual([apiId]);

      // Verify the underlying JSONL file is one-object-per-line — this is
      // the load-bearing storage format contract, not just a smoke test.
      const jsonlPath = join(tmpDir, ".maestro", "tasks", "tasks.jsonl");
      const rawJsonl = await readFile(jsonlPath, "utf8");
      const lines = rawJsonl.split("\n").filter((l) => l.length > 0);
      expect(lines.length).toBe(2);
      for (const line of lines) {
        expect(() => JSON.parse(line)).not.toThrow();
      }

      const readyBefore = await runCompiled(["task", "ready", "--json"], tmpDir);
      const beforeList = expectJson<Array<{ id: string; title: string }>>(readyBefore);
      expect(beforeList.length).toBe(1);
      expect(beforeList[0]?.id).toBe(apiId);

      const readyText = await runCompiled(["task", "ready"], tmpDir);
      expect(readyText.exitCode).toBe(0);
      expect(readyText.stdout).toContain(apiId);
      expect(readyText.stdout).toContain("P1");

      const retitled = await runCompiled(
        ["task", "update", apiId, "--title", "POST /login endpoint", "--json"],
        tmpDir,
      );
      expect(expectJson<{ title: string }>(retitled).title).toBe("POST /login endpoint");

      const relabeled = await runCompiled(
        ["task", "update", apiId, "--add-label", "urgent", "--json"],
        tmpDir,
      );
      expect(expectJson<{ labels: string[] }>(relabeled).labels).toEqual(["urgent"]);

      const illegalClose = await runCompiled(
        ["task", "update", apiId, "--status", "closed"],
        tmpDir,
      );
      expect(illegalClose.exitCode).not.toBe(0);
      expect(illegalClose.stderr).toContain("Cannot set status to 'closed'");

      const closeResult = await runCompiled(
        ["task", "close", apiId, "--reason", "shipped", "--json"],
        tmpDir,
      );
      const closed = expectJson<{ status: string; closeReason: string }>(closeResult);
      expect(closed.status).toBe("closed");
      expect(closed.closeReason).toBe("shipped");

      const readyAfter = await runCompiled(["task", "ready", "--json"], tmpDir);
      const afterList = expectJson<Array<{ id: string }>>(readyAfter);
      expect(afterList.length).toBe(1);
      expect(afterList[0]?.id).toBe(mw.id);

      const listOpen = await runCompiled(
        ["task", "list", "--status", "open", "--json"],
        tmpDir,
      );
      const openList = expectJson<Array<{ id: string }>>(listOpen);
      expect(openList.length).toBe(1);
      expect(openList[0]?.id).toBe(mw.id);

      const listClosed = await runCompiled(
        ["task", "list", "--status", "closed", "--json"],
        tmpDir,
      );
      const closedList = expectJson<Array<{ id: string }>>(listClosed);
      expect(closedList.length).toBe(1);
      expect(closedList[0]?.id).toBe(apiId);

      const showClosed = await runCompiled(
        ["task", "show", apiId, "--json"],
        tmpDir,
      );
      const shown = expectJson<{
        id: string;
        status: string;
        closeReason: string;
        title: string;
      }>(showClosed);
      expect(shown.id).toBe(apiId);
      expect(shown.status).toBe("closed");
      expect(shown.closeReason).toBe("shipped");
      expect(shown.title).toBe("POST /login endpoint");

      const closeMw = await runCompiled(
        ["task", "close", mw.id, "--reason", "merged"],
        tmpDir,
      );
      expect(closeMw.exitCode).toBe(0);

      const readyEmpty = await runCompiled(["task", "ready", "--json"], tmpDir);
      expect(expectJson(readyEmpty)).toEqual([]);

      // Regression guard: adding the task feature must not clobber sibling
      // features — if the command is still parseable, services wiring is intact.
      const missionList = await runCompiled(["mission", "list", "--json"], tmpDir);
      expect(missionList.exitCode).toBe(0);
      expect(() => JSON.parse(missionList.stdout)).not.toThrow();
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  it(
    "--claim succeeds or fails loudly depending on session detection",
    async () => {
      // The test runner may or may not carry an agent env marker (CLAUDECODE,
      // CODEX_THREAD_ID). Force the "no session" branch so the assertion is
      // deterministic regardless of the parent shell.
      const created = await runCompiled(["task", "q", "unclaimed"], tmpDir);
      const id = created.stdout;

      const claim = await runCompiled(
        ["task", "update", id, "--claim"],
        tmpDir,
        { env: { CLAUDECODE: "", CODEX_THREAD_ID: "" } },
      );
      expect(claim.exitCode).not.toBe(0);
      expect(claim.stderr).toContain("Could not detect current session");
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  it(
    "validates --depends-on against existing tasks",
    async () => {
      const bad = await runCompiled(
        [
          "task",
          "create",
          "references a ghost",
          "--depends-on",
          "tsk-000000",
        ],
        tmpDir,
      );
      expect(bad.exitCode).not.toBe(0);
      expect(bad.stderr).toContain("unknown task");
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  it(
    "active memory: task close seeds a candidate and ready surfaces a hint",
    async () => {
      const pastId = (
        await runCompiled(
          ["task", "q", "Implement argon2 password hashing"],
          tmpDir,
        )
      ).stdout;
      expect(pastId).toMatch(/^tsk-[0-9a-f]{6}$/);

      const close = await runCompiled(
        [
          "task",
          "close",
          pastId,
          "--reason",
          "argon2 compare was backwards, wasted a day",
        ],
        tmpDir,
      );
      expect(close.exitCode).toBe(0);

      const candidatePath = join(
        tmpDir,
        ".maestro",
        "tasks",
        "candidates",
        `${pastId}.json`,
      );
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
      expect(candidate.reason).toBe(
        "argon2 compare was backwards, wasted a day",
      );
      expect(candidate.keywords).toContain("argon2");
      expect(candidate.keywords).toContain("password");
      expect(candidate.keywords).toContain("compare");
      expect(candidate.capturedAt).toMatch(/^\d{4}-\d{2}-\d{2}T/);

      await runCompiled(["task", "q", "JWT password middleware"], tmpDir);
      await runCompiled(["task", "q", "Protected routes"], tmpDir);

      const ready = await runCompiled(["task", "ready", "--json"], tmpDir);
      const briefings = expectJson<Array<{
        id: string;
        title: string;
        hints: Array<{
          sourceTaskId: string;
          reason: string;
          matchedKeywords: string[];
        }>;
      }>>(ready);
      expect(briefings.length).toBe(2);

      const byTitle = new Map(briefings.map((b) => [b.title, b] as const));
      const jwt = byTitle.get("JWT password middleware");
      const prot = byTitle.get("Protected routes");
      expect(jwt).toBeDefined();
      expect(prot).toBeDefined();

      expect(jwt?.hints.length).toBeGreaterThanOrEqual(1);
      expect(jwt?.hints[0]?.sourceTaskId).toBe(pastId);
      expect(jwt?.hints[0]?.reason).toBe(
        "argon2 compare was backwards, wasted a day",
      );
      expect(jwt?.hints[0]?.matchedKeywords).toContain("password");

      expect(prot?.hints).toEqual([]);
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  it(
    "active memory: --no-hints disables hint attachment for scripts",
    async () => {
      const pastId = (
        await runCompiled(["task", "q", "Implement auth module"], tmpDir)
      ).stdout;
      await runCompiled(
        ["task", "close", pastId, "--reason", "auth token expiry wrong"],
        tmpDir,
      );
      await runCompiled(["task", "q", "Refactor auth handler"], tmpDir);

      const withHints = await runCompiled(["task", "ready", "--json"], tmpDir);
      const withHintsBriefings = expectJson<Array<{ hints: unknown[] }>>(withHints);
      expect(withHintsBriefings[0]?.hints.length).toBeGreaterThanOrEqual(1);

      const noHints = await runCompiled(
        ["task", "ready", "--no-hints", "--json"],
        tmpDir,
      );
      const noHintsBriefings = expectJson<Array<{ hints: unknown[] }>>(noHints);
      expect(noHintsBriefings[0]?.hints).toEqual([]);

      const textWithHints = await runCompiled(["task", "ready"], tmpDir);
      expect(textWithHints.exitCode).toBe(0);
      expect(textWithHints.stdout).toContain(">>");

      const textNoHints = await runCompiled(
        ["task", "ready", "--no-hints"],
        tmpDir,
      );
      expect(textNoHints.exitCode).toBe(0);
      expect(textNoHints.stdout).not.toContain(">>");
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  it(
    "rejects re-closing an already-closed task without rewriting the close reason",
    async () => {
      const created = await runCompiled(["task", "q", "done once"], tmpDir);
      expect(created.exitCode).toBe(0);
      const id = created.stdout;

      const firstClose = await runCompiled(
        ["task", "close", id, "--reason", "first reason"],
        tmpDir,
      );
      expect(firstClose.exitCode).toBe(0);

      const secondClose = await runCompiled(
        ["task", "close", id, "--reason", "second reason"],
        tmpDir,
      );
      expect(secondClose.exitCode).not.toBe(0);
      expect(secondClose.stderr).toContain("already closed");

      const shown = await runCompiled(["task", "show", id, "--json"], tmpDir);
      expect(expectJson<{ closeReason: string }>(shown).closeReason).toBe("first reason");
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  it(
    "still closes the task when candidate capture fails",
    async () => {
      const created = await runCompiled(["task", "q", "candidate write fail"], tmpDir);
      expect(created.exitCode).toBe(0);
      const id = created.stdout;

      const candidatePath = join(tmpDir, ".maestro", "tasks", "candidates", `${id}.json`);
      await mkdir(candidatePath, { recursive: true });

      const closed = await runCompiled(
        ["task", "close", id, "--reason", "done"],
        tmpDir,
      );
      expect(closed.exitCode).toBe(0);
      expect(closed.stdout).toContain("Task closed:");
      expect(closed.stderr).toContain("hint capture failed");

      const shown = await runCompiled(["task", "show", id, "--json"], tmpDir);
      const task = expectJson<{ status: string; closeReason: string }>(shown);
      expect(task.status).toBe("closed");
      expect(task.closeReason).toBe("done");
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  it(
    "keeps task ready working when a candidate file is malformed",
    async () => {
      const id = (
        await runCompiled(["task", "q", "candidate reader"], tmpDir)
      ).stdout;
      expect(id).toMatch(/^tsk-[0-9a-f]{6}$/);

      const candidatesDir = join(tmpDir, ".maestro", "tasks", "candidates");
      await mkdir(candidatesDir, { recursive: true });
      await writeFile(join(candidatesDir, "broken.json"), "{bad json\n");

      const ready = await runCompiled(["task", "ready", "--json"], tmpDir);
      expect(ready.exitCode).toBe(0);
      expect(expectJson<Array<{ id: string }>>(ready).map((task) => task.id)).toEqual([id]);
    },
    SLOW_CLI_TIMEOUT_MS,
  );
});
