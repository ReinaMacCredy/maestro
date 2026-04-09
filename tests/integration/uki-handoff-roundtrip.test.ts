/**
 * UKI v5.3 handoff end-to-end round-trip integration test.
 *
 * Creates a handoff via the CLI, lists it, picks it up with --uki, then
 * runs the raw UKI string back through parseUki() and asserts the parsed
 * slots match the original input. Exercises the full compress -> persist
 * -> read -> parse chain.
 */
import { describe, it, expect, beforeEach, afterEach } from "bun:test";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { parseUki } from "../../src/lib/uki-format.js";

const CLI = [
  "bun",
  "run",
  join(import.meta.dir, "..", "..", "src", "index.ts"),
];

const SLOW_CLI_TIMEOUT_MS = 20_000;

async function run(
  args: string[],
  cwd: string,
): Promise<{ stdout: string; stderr: string; exitCode: number }> {
  const proc = Bun.spawn([...CLI, ...args], {
    stdout: "pipe",
    stderr: "pipe",
    cwd,
  });
  const [stdout, stderr] = await Promise.all([
    new Response(proc.stdout).text(),
    new Response(proc.stderr).text(),
  ]);
  const exitCode = await proc.exited;
  return { stdout: stdout.trim(), stderr: stderr.trim(), exitCode };
}

  describe("UKI handoff roundtrip", () => {
  let tmpDir: string;

  beforeEach(async () => {
    tmpDir = await mkdtemp(join(tmpdir(), "maestro-uki-it-"));
  });

  afterEach(async () => {
    await rm(tmpDir, { recursive: true, force: true });
  });

    it("create -> list -> pickup --uki -> parseUki round-trips faithfully", async () => {
    const create = await run(
      [
        "handoff",
        "create",
        "--session-core", "integration_test",
        "--summary", "Integration_test-roundtrip-low_risk",
        "--next-action", "assert_structural_equality",
          "--driver", "ci_ran",
          "--driver", "suite_exercised",
          "--decision", "use_fixture_slots",
          "--decision-basis", "keep_roundtrip_lossless",
          "--signal", "handoffs_0~1",
          "--validation", "json_green",
          "--validation", "pickup_green",
          "--artifact", "branch_feat_missionControl",
          "--artifact", "file_tests_uki_roundtrip",
          "--boundary", "no_real_work",
          "--execution-state", "tmpdir_sandbox",
          "--blind-spot", "green_tests_masked_drift",
          "--metaphor", "baton_pass_snapshot",
          "--confidence-work", "0.95",
          "--confidence-summary", "0.9",
          "--json",
      ],
      tmpDir,
    );
    expect(create.exitCode).toBe(0);
    const created = JSON.parse(create.stdout);
    expect(created.id).toMatch(/^\d{4}-\d{2}-\d{2}-\d{3}$/);
    expect(created.status).toBe("pending");
    expect(typeof created.uki).toBe("string");

    // list
    const list = await run(["handoff", "list", "--json"], tmpDir);
    expect(list.exitCode).toBe(0);
    const handoffs = JSON.parse(list.stdout);
    expect(handoffs).toHaveLength(1);
    expect(handoffs[0].id).toBe(created.id);

    // pickup --uki
    const pickup = await run(
      ["handoff", "pickup", "--id", created.id, "--uki"],
      tmpDir,
    );
    expect(pickup.exitCode).toBe(0);
    const rawUki = pickup.stdout;
    expect(rawUki.length).toBeGreaterThan(0);

      // exactly 15 pipes, 0 colons, 0 newlines
      const pipeCount = (rawUki.match(/\|/g) ?? []).length;
      expect(pipeCount).toBe(15);
    expect(rawUki.includes(":")).toBe(false);
    expect(rawUki.includes("\n")).toBe(false);

    // parseUki should deep-equal the created slots (read back from JSON)
    const parsed = parseUki(rawUki);
      expect(parsed).toEqual(created.slots);
    }, SLOW_CLI_TIMEOUT_MS);

    it("create --uki returns only the raw UKI payload", async () => {
      const create = await run(
        [
          "handoff",
          "create",
          "--session-core", "uki_only_test",
          "--summary", "Uki_only_test-direct-low_risk",
          "--next-action", "pipe_to_agent",
          "--decision-basis", "keep_create_pipelineable",
          "--validation", "compiled_green",
          "--artifact", "file_src_uki_format",
          "--confidence-work", "0.96",
          "--confidence-summary", "0.93",
          "--uki",
        ],
        tmpDir,
      );

      expect(create.exitCode).toBe(0);
      expect(create.stdout.startsWith("SESSION_CORE-uki_only_test|")).toBe(true);
      expect(create.stdout.includes("\n")).toBe(false);
      expect(create.stdout.includes("[ok] Handoff created")).toBe(false);

      const parsed = parseUki(create.stdout);
      expect(parsed.nextAction).toBe("pipe_to_agent");
      expect(parsed.decisionBasis).toEqual(["keep_create_pipelineable"]);
      expect(parsed.validationState).toEqual(["compiled_green"]);
    }, SLOW_CLI_TIMEOUT_MS);

  it("list --status filters by status", async () => {
    await run(
      [
        "handoff", "create",
        "--session-core", "filter_test",
        "--summary", "Filter_test-basic-low_risk",
        "--next-action", "verify_filter",
        "--artifact", "branch_foo",
        "--confidence-work", "0.9",
        "--json",
      ],
      tmpDir,
    );

    const pending = await run(
      ["handoff", "list", "--status", "pending", "--json"],
      tmpDir,
    );
    expect(pending.exitCode).toBe(0);
    expect(JSON.parse(pending.stdout)).toHaveLength(1);

    const pickedUp = await run(
      ["handoff", "list", "--status", "picked-up", "--json"],
      tmpDir,
    );
    expect(pickedUp.exitCode).toBe(0);
    expect(JSON.parse(pickedUp.stdout)).toEqual([]);
  }, SLOW_CLI_TIMEOUT_MS);

  it("pickup --claim transitions pending -> picked-up", async () => {
    const create = await run(
      [
        "handoff", "create",
        "--session-core", "claim_test",
        "--summary", "Claim_test-transition-low_risk",
        "--next-action", "verify_claim",
        "--artifact", "branch_bar",
        "--confidence-work", "0.9",
        "--json",
      ],
      tmpDir,
    );
    const id = JSON.parse(create.stdout).id;

    const claim = await run(
      ["handoff", "pickup", "--id", id, "--claim", "--agent", "codex", "--json"],
      tmpDir,
    );
    expect(claim.exitCode).toBe(0);
    const claimed = JSON.parse(claim.stdout);
    expect(claimed.status).toBe("picked-up");
    expect(claimed.pickedUpBy).toBe("codex");
  }, SLOW_CLI_TIMEOUT_MS);

  it("pickup with no pending handoffs throws with hints", async () => {
    const pickup = await run(["handoff", "pickup"], tmpDir);
    expect(pickup.exitCode).toBe(1);
    const output = pickup.stdout + pickup.stderr;
    expect(output.toLowerCase()).toContain("pending");
  }, SLOW_CLI_TIMEOUT_MS);

  it("create rejects CS with no confidence scope (R5)", async () => {
    const result = await run(
      [
        "handoff", "create",
        "--session-core", "no_cs",
        "--summary", "No_CS-error_test-low_risk",
        "--next-action", "verify_error",
        "--artifact", "branch_qux",
      ],
      tmpDir,
    );
    expect(result.exitCode).toBe(1);
    const output = result.stdout + result.stderr;
    expect(output.toLowerCase()).toContain("confidence");
  }, SLOW_CLI_TIMEOUT_MS);
});
