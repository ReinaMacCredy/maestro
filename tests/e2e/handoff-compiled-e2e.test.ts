import {
  afterEach,
  beforeAll,
  beforeEach,
  describe,
  expect,
  it,
} from "bun:test";
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

let tmpDir: string;

// Minimum flags that satisfy the handoff validator for an execute-mode
// create. Reused across tests so they share the same baseline shape.
const BASE_CREATE_ARGS = [
  "handoff",
  "create",
  "--mode",
  "execute",
  "--session-core",
  "integration_test",
  "--summary",
  "e2e_roundtrip-low_risk",
  "--next-action",
  "verify_e2e_output",
  "--current-state",
  "execute_in_progress",
  "--decision",
  "use_fixed_fixture",
  "--artifact",
  "file_test_handoff",
  "--read-more",
  "file_test_handoff",
  "--touched-file",
  "file_test_handoff",
  "--completed",
  "fixture_written",
  "--validation",
  "json_green",
  "--boundary",
  "no_real_work",
  "--blind-spot",
  "none",
  "--metaphor",
  "baton_pass",
  "--confidence-work",
  "0.95",
  "--confidence-summary",
  "0.9",
];

beforeAll(buildCompiledCli, BUILD_TIMEOUT_MS);

beforeEach(async () => {
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-handoff-e2e-"));
  await initGitRepo(tmpDir);
});

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true });
});

describe("compiled handoff feature E2E", () => {
  it(
    "create + list + pickup round-trips through the compiled binary",
    async () => {
      const create = await runCompiled([...BASE_CREATE_ARGS, "--json"], tmpDir);
      expect(create.exitCode).toBe(0);
      const created = expectJson<{
        id: string;
        status: string;
        content: { mode: string; summary: string; nextAction: string };
        uki: string;
      }>(create);
      expect(created.id).toMatch(/^\d{4}-\d{2}-\d{2}-\d{3}$/);
      expect(created.status).toBe("pending");
      expect(created.content.mode).toBe("execute");
      expect(created.content.summary).toBe("e2e_roundtrip-low_risk");
      expect(created.content.nextAction).toBe("verify_e2e_output");
      expect(created.uki).toContain("MODE-execute");
      expect(created.uki).toContain("SUMMARY-e2e_roundtrip-low_risk");
      expect(created.uki).toContain("NEXT_ACTION-verify_e2e_output");

      const list = await runCompiled(["handoff", "list", "--json"], tmpDir);
      const listed = expectJson<Array<{ id: string; status: string }>>(list);
      expect(listed.length).toBe(1);
      expect(listed[0]?.id).toBe(created.id);
      expect(listed[0]?.status).toBe("pending");

      // pickup without --claim is read-only — must not mutate status.
      const peek = await runCompiled(
        ["handoff", "pickup", "--id", created.id, "--json"],
        tmpDir,
      );
      const peeked = expectJson<{ id: string; status: string }>(peek);
      expect(peeked.id).toBe(created.id);
      expect(peeked.status).toBe("pending");

      const claim = await runCompiled(
        [
          "handoff",
          "pickup",
          "--id",
          created.id,
          "--claim",
          "--agent",
          "alice",
          "--json",
        ],
        tmpDir,
      );
      const claimed = expectJson<{ status: string; pickedUpBy?: string }>(claim);
      expect(claimed.status).toBe("picked-up");

      const listPicked = await runCompiled(
        ["handoff", "list", "--status", "picked-up", "--json"],
        tmpDir,
      );
      const pickedList = expectJson<Array<{ id: string; status: string }>>(
        listPicked,
      );
      expect(pickedList.length).toBe(1);
      expect(pickedList[0]?.id).toBe(created.id);

      const listPending = await runCompiled(
        ["handoff", "list", "--status", "pending", "--json"],
        tmpDir,
      );
      const pendingList = expectJson<unknown[]>(listPending);
      expect(pendingList.length).toBe(0);
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  it(
    "create --uki emits only the raw UKI transfer string",
    async () => {
      const result = await runCompiled([...BASE_CREATE_ARGS, "--uki"], tmpDir);
      expect(result.exitCode).toBe(0);
      expect(result.stdout).toContain("MODE-execute");
      expect(result.stdout).toContain("CS-work_0.95~summary_0.9");
      expect(result.stdout).not.toContain("\"id\":");
      expect(result.stdout).not.toContain("Status:");
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  it(
    "create --uki rejects missing confidence scores",
    async () => {
      const result = await runCompiled(
        [
          "handoff",
          "create",
          "--mode",
          "execute",
          "--session-core",
          "x",
          "--summary",
          "x-x-low_risk",
          "--next-action",
          "x",
          "--current-state",
          "executing",
          "--decision",
          "d",
          "--artifact",
          "a",
          "--read-more",
          "a",
          "--touched-file",
          "a",
          "--completed",
          "done",
          "--validation",
          "ok",
          "--boundary",
          "no",
          "--blind-spot",
          "none",
          "--metaphor",
          "m",
          "--uki",
        ],
        tmpDir,
      );
      expect(result.exitCode).not.toBe(0);
      const combined = `${result.stdout}\n${result.stderr}`;
      expect(combined).toContain("confidence-work");
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  it(
    "create --paste emits an agent-ready prompt plus the UKI packet",
    async () => {
      const result = await runCompiled(
        [...BASE_CREATE_ARGS, "--paste"],
        tmpDir,
      );
      expect(result.exitCode).toBe(0);
      expect(result.stdout).toContain("MODE-execute");
      // Loose length check rather than exact wording — preamble copy is
      // implementation detail and not worth coupling tests to.
      expect(result.stdout.length).toBeGreaterThan(100);
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  it(
    "list returns empty array on a fresh workspace",
      async () => {
        const result = await runCompiled(["handoff", "list", "--json"], tmpDir);
        expect(result.exitCode).toBe(0);
        expect(expectJson<unknown[]>(result)).toEqual([]);
      },
    SLOW_CLI_TIMEOUT_MS,
  );

  it(
    "pickup with no pending handoff returns a clear error",
    async () => {
      const result = await runCompiled(["handoff", "pickup"], tmpDir);
      expect(result.exitCode).not.toBe(0);
      // Loose check — exact wording is implementation detail, just verify
      // the user got SOMETHING on stderr rather than an empty failure.
      expect(result.stderr.length + result.stdout.length).toBeGreaterThan(0);
    },
    SLOW_CLI_TIMEOUT_MS,
  );
});
