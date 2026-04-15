import {
  afterEach,
  beforeAll,
  beforeEach,
  describe,
  expect,
  it,
} from "bun:test";
import { mkdtemp, readFile, rm } from "node:fs/promises";
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
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-notes-e2e-"));
  await initGitRepo(tmpDir);
});

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true });
});

describe("compiled notes feature E2E", () => {
  it(
    "appends a note and persists it to .maestro/notes.json",
    async () => {
      const result = await runCompiled(
        ["note", "--content", "rerun doctor after init"],
        tmpDir,
      );
      expect(result.exitCode).toBe(0);
      expect(result.stdout).toContain("Note saved");
      expect(result.stdout).toContain("rerun doctor after init");

      const raw = await readFile(
        join(tmpDir, ".maestro", "notes.json"),
        "utf8",
      );
      const notes: Array<{
        timestamp: string;
        content: string;
        git_branch: string;
      }> = JSON.parse(raw);
      expect(notes.length).toBe(1);
      expect(notes[0]?.content).toBe("rerun doctor after init");
      expect(notes[0]?.git_branch).toBe("main");
      expect(notes[0]?.timestamp).toMatch(/^\d{4}-\d{2}-\d{2}T/);
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  it(
    "lists multiple notes in append order",
    async () => {
      const first = await runCompiled(
        ["note", "--content", "first entry"],
        tmpDir,
      );
      expect(first.exitCode).toBe(0);

      const second = await runCompiled(
        ["note", "--content", "second entry"],
        tmpDir,
      );
      expect(second.exitCode).toBe(0);

      const list = await runCompiled(["note", "--list", "--json"], tmpDir);
      expect(list.exitCode).toBe(0);

      const notes = expectJson<
        Array<{ content: string; git_branch: string }>
      >(list);
      expect(notes.length).toBe(2);
      expect(notes[0]?.content).toBe("first entry");
      expect(notes[1]?.content).toBe("second entry");
      expect(notes[0]?.git_branch).toBe("main");
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  it(
    "reports empty state cleanly when no notes exist",
    async () => {
      const list = await runCompiled(["note", "--list"], tmpDir);
      expect(list.exitCode).toBe(0);
      expect(list.stdout).toContain("No notes found");

        const listJson = await runCompiled(["note", "--list", "--json"], tmpDir);
        expect(listJson.exitCode).toBe(0);
        expect(expectJson<unknown[]>(listJson)).toEqual([]);
      },
    SLOW_CLI_TIMEOUT_MS,
  );

  it(
    "rejects --content and --list used together",
    async () => {
      const result = await runCompiled(
        ["note", "--content", "x", "--list"],
        tmpDir,
      );
      expect(result.exitCode).not.toBe(0);
      expect(result.stderr).toContain("cannot be used together");
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  it(
    "rejects calls with neither --content nor --list",
    async () => {
      const result = await runCompiled(["note"], tmpDir);
      expect(result.exitCode).not.toBe(0);
      expect(result.stderr).toContain("--content is required");
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  it(
    "carries branch info when run on a non-main branch",
    async () => {
      const branchProc = Bun.spawn(["git", "checkout", "-b", "feature/x"], {
        cwd: tmpDir,
        stdout: "pipe",
        stderr: "pipe",
      });
      await branchProc.exited;

      const result = await runCompiled(
        ["note", "--content", "on a branch"],
        tmpDir,
      );
      expect(result.exitCode).toBe(0);
      expect(result.stdout).toContain("feature/x");
    },
    SLOW_CLI_TIMEOUT_MS,
  );
});
