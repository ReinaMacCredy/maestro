import { afterEach, beforeEach, describe, expect, it } from "bun:test";
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

  it("create execute -> list -> pickup default UKI -> parseUki round-trips", async () => {
    const create = await run(
      [
        "handoff",
        "create",
        "--mode", "execute",
        "--session-core", "integration_test",
        "--summary", "Integration_test-roundtrip-low_risk",
        "--next-action", "assert_structural_equality",
        "--current-state", "execute_in_progress",
        "--decision", "use_fixture_content",
        "--signal", "handoffs_0_1",
        "--artifact", "branch_feat_handoff_rebuild",
        "--artifact", "file_tests_uki_roundtrip",
        "--read-more", "file_tests_uki_roundtrip",
        "--touched-file", "file_tests_uki_roundtrip",
        "--completed", "structured_content_saved",
        "--validation", "json_green",
        "--validation", "pickup_green",
        "--boundary", "no_real_work",
        "--blind-spot", "green_tests_masked_contract_drift",
        "--metaphor", "baton_pass_snapshot",
        "--confidence-work", "0.95",
        "--confidence-summary", "0.9",
        "--json",
      ],
      tmpDir,
    );

    expect(create.exitCode).toBe(0);
    const created = JSON.parse(create.stdout);
    expect(created.status).toBe("pending");
    expect(created.content.mode).toBe("execute");

    const listed = await run(["handoff", "list", "--json"], tmpDir);
    expect(listed.exitCode).toBe(0);
    expect(JSON.parse(listed.stdout)).toHaveLength(1);

    const pickup = await run(["handoff", "pickup", "--id", created.id], tmpDir);
    expect(pickup.exitCode).toBe(0);
    const parsed = parseUki(pickup.stdout);
    expect(parsed).toEqual(created.content);
  }, SLOW_CLI_TIMEOUT_MS);

  it("create --mode plan --uki returns only the raw UKI payload", async () => {
    const create = await run(
      [
        "handoff",
        "create",
        "--mode", "plan",
        "--session-core", "plan_handoff",
        "--summary", "Plan_handoff-saved-and_ready-low_risk",
        "--next-action", "start_execute_mode",
        "--decision", "save_reference_plan",
        "--artifact", "file_plan_md",
        "--read-more", "plan_md",
        "--plan-path-item", "plan_md",
        "--maestro-sync", "mission_created",
        "--confidence-work", "0.96",
        "--uki",
      ],
      tmpDir,
    );

    expect(create.exitCode).toBe(0);
    expect(create.stdout.startsWith("MODE-plan|")).toBe(true);
    expect(create.stdout.includes("[ok] Handoff created")).toBe(false);

    const parsed = parseUki(create.stdout);
    expect(parsed.mode).toBe("plan");
    expect(parsed.readMore).toEqual(["plan_md"]);
  }, SLOW_CLI_TIMEOUT_MS);

  it("pickup --json returns the structured record while pickup defaults to UKI", async () => {
    const create = await run(
      [
        "handoff",
        "create",
        "--mode", "execute",
        "--session-core", "claim_test",
        "--summary", "Claim_test-transition-low_risk",
        "--next-action", "verify_claim",
        "--artifact", "branch_bar",
        "--read-more", "branch_bar",
        "--completed", "handoff_created",
        "--validation", "unit_green",
        "--confidence-work", "0.9",
        "--json",
      ],
      tmpDir,
    );

    const id = JSON.parse(create.stdout).id;

    const pickupUki = await run(["handoff", "pickup", "--id", id], tmpDir);
    expect(pickupUki.exitCode).toBe(0);
    expect(pickupUki.stdout.startsWith("MODE-execute|")).toBe(true);

    const pickupJson = await run(
      ["handoff", "pickup", "--id", id, "--claim", "--agent", "codex", "--json"],
      tmpDir,
    );
    expect(pickupJson.exitCode).toBe(0);
    const claimed = JSON.parse(pickupJson.stdout);
      expect(claimed.status).toBe("picked-up");
      expect(claimed.pickedUpBy).toBe("codex");
    }, SLOW_CLI_TIMEOUT_MS);

  it("prefers explicit --uki over the root --json flag on pickup", async () => {
    const create = await run(
      [
        "handoff",
        "create",
        "--mode", "execute",
        "--session-core", "pickup_flag_precedence",
        "--summary", "Pickup_flag_precedence-created-low_risk",
        "--next-action", "return_raw_uki",
        "--artifact", "branch_flag_precedence",
        "--read-more", "branch_flag_precedence",
        "--completed", "handoff_created",
        "--validation", "unit_green",
        "--confidence-work", "0.9",
        "--json",
      ],
      tmpDir,
    );

    expect(create.exitCode).toBe(0);
    const created = JSON.parse(create.stdout);

    const pickup = await run(
      ["--json", "handoff", "pickup", "--id", created.id, "--uki"],
      tmpDir,
    );

    expect(pickup.exitCode).toBe(0);
    expect(pickup.stdout.startsWith("MODE-execute|")).toBe(true);
    expect(() => JSON.parse(pickup.stdout)).toThrow();
  }, SLOW_CLI_TIMEOUT_MS);

  it("auto-populates execute read-more and artifacts in a clean repo", async () => {
    const init = Bun.spawn(["git", "init"], {
      cwd: tmpDir,
      stdout: "pipe",
      stderr: "pipe",
    });
    await init.exited;

    const branch = Bun.spawn(["git", "checkout", "-b", "feat/handoff-clean"], {
      cwd: tmpDir,
      stdout: "pipe",
      stderr: "pipe",
    });
    await branch.exited;

    const create = await run(
      [
        "handoff",
        "create",
        "--mode", "execute",
        "--session-core", "clean_repo_execute",
        "--summary", "Clean_repo_execute-created-low_risk",
        "--next-action", "inspect_auto_context",
        "--completed", "handoff_created",
        "--validation", "unit_green",
        "--confidence-work", "0.9",
        "--json",
      ],
      tmpDir,
    );

    expect(create.exitCode).toBe(0);
    const created = JSON.parse(create.stdout);
    expect(created.content.readMore.length).toBeGreaterThan(0);
    expect(created.content.artifacts).toContain("branch_feat_handoff_clean");
  }, SLOW_CLI_TIMEOUT_MS);

  it("auto-populates plan read-more from known plan paths in a clean repo", async () => {
    await Bun.write(join(tmpDir, "PLAN.md"), "# plan\n");
    const init = Bun.spawn(["git", "init"], {
      cwd: tmpDir,
      stdout: "pipe",
      stderr: "pipe",
    });
    await init.exited;

    const create = await run(
      [
        "handoff",
        "create",
        "--mode", "plan",
        "--session-core", "clean_repo_plan",
        "--summary", "Clean_repo_plan-created-low_risk",
        "--next-action", "inspect_plan_auto_context",
        "--decision", "save_plan_reference",
        "--confidence-work", "0.9",
        "--json",
      ],
      tmpDir,
    );

    expect(create.exitCode).toBe(0);
    const created = JSON.parse(create.stdout);
    expect(created.content.readMore).toContain("plan_md");
    expect(created.content.planPaths).toContain("plan_md");
  }, SLOW_CLI_TIMEOUT_MS);

  it("pickup canonicalizes legacy v5.3 records to v5.4 UKI output", async () => {
    await Bun.$`mkdir -p ${join(tmpDir, ".maestro", "handoffs")}`.quiet();
    await Bun.write(
      join(tmpDir, ".maestro", "handoffs", "2026-04-09-123.json"),
      JSON.stringify({
        id: "2026-04-09-123",
        version: "5.3",
        timestamp: "2026-04-09T00:00:00.000Z",
        status: "pending",
        agent: "codex",
        sessionId: "legacy-v53",
        uki:
          "SESSION_CORE-legacy_record"
          + "|CAUSAL_DRIVERS-upgrade_path"
          + "|DIVERGENCES-NONE"
          + "|KEY_DECISIONS-keep_pickup_safe"
          + "|DECISION_BASIS-safe_upgrade_path"
          + "|SIGNAL_DELTA-handoffs_1_2"
          + "|VALIDATION_STATE-unit_green"
          + "|EXECUTION_STATE-legacy_tmpdir"
          + "|BOUNDARY_STATE-NONE"
          + "|NEXT_ACTION-review_upgrade"
          + "|ARTIFACTS-branch_main-file_src_lib_uki_format_ts"
          + "|STANCE_COLLAPSE-NONE_DETECTED_LOW_FRICTION"
          + "|CS-work_0.8"
          + "|SUMMARY-Legacy_record-normalized-low_risk",
      }, null, 2),
    );

    const pickup = await run(["handoff", "pickup", "--id", "2026-04-09-123"], tmpDir);
    expect(pickup.exitCode).toBe(0);
    expect(pickup.stdout.startsWith("MODE-execute|")).toBe(true);
    expect(pickup.stdout).toContain("|READ_MORE-file_src_lib_uki_format_ts|");
  }, SLOW_CLI_TIMEOUT_MS);

  it("errors cleanly when required mode is missing or no pending handoff exists", async () => {
    const missingMode = await run(
      [
        "handoff",
        "create",
        "--session-core", "missing_mode",
        "--summary", "Missing_mode-error-low_risk",
        "--next-action", "verify_error",
        "--artifact", "branch_main",
        "--read-more", "branch_main",
        "--confidence-work", "0.9",
      ],
      tmpDir,
    );
    expect(missingMode.exitCode).toBe(1);
    expect((missingMode.stdout + missingMode.stderr).toLowerCase()).toContain("mode");

    const noPending = await run(["handoff", "pickup"], tmpDir);
    expect(noPending.exitCode).toBe(1);
    expect((noPending.stdout + noPending.stderr).toLowerCase()).toContain("pending");
  }, SLOW_CLI_TIMEOUT_MS);
});
