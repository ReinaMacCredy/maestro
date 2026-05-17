import { describe, expect, it } from "bun:test";
import {
  checkSkillBinaryParity,
  renderDriftError,
} from "@/service/skill-binary-parity.js";

describe("checkSkillBinaryParity", () => {
  it("returns no findings when all skill-referenced verbs exist (full paths)", () => {
    // The parity check now validates the *full* verb path (e.g. "setup
    // migrate-v2"), not just the head. src/index.ts populates knownVerbs by
    // walking the Commander tree, which adds both leaf names and full paths.
    // Mirror that here: enumerate the real verbs/subverbs the bundled SKILLs
    // reference. When a subverb is added or removed, update this set.
    const allVerbs = new Set<string>([
      // Top-level / hot-path aliases.
      "evidence", "contract", "task", "spec", "plan", "verdict",
      "policy", "ci", "merge", "deploy", "runtime", "review",
      "worktree", "setup", "handoff", "bundle", "skills", "mcp",
      "recover", "gc", "principle", "init", "status", "doctor",
      "install", "update", "uninstall", "providers", "reply",
      "mission-control", "mission", "claim", "block", "abandon", "ship", "verify",
      "intake", "qa", "note", "inspect", "project",
      // Full subverb paths referenced by skills.
      "ci verify",
      "contract amend", "contract show",
      "evidence list", "evidence record",
      "gc slop-cleanup",
      "mission cancel", "mission decompose", "mission new", "mission show",
      "plan check",
      "policy check",
      "principle promote",
      "setup check",
      "spec grill", "spec new", "spec validate",
      "task abandon", "task block", "task budget", "task claim",
      "task from-spec", "task get", "task list", "task observe",
      "task ship", "task verify",
      "verdict request", "verdict show",
      "worktree create",
    ]);
    const report = checkSkillBinaryParity({ knownVerbs: allVerbs });
    expect(report.skillsChecked).toBeGreaterThan(0);
    expect(report.findings).toEqual([]);
  });

  it("reports drift when skill references a missing verb", () => {
    const report = checkSkillBinaryParity({ knownVerbs: new Set<string>() });
    expect(report.findings.length).toBeGreaterThan(0);
    for (const f of report.findings) {
      expect(f.status).toBe("missing-in-binary");
    }
  });
});

describe("renderDriftError", () => {
  it("includes verb, version, and remediation", () => {
    const msg = renderDriftError(
      { skill: "maestro-setup", verb: "setup", status: "missing-in-binary" },
      "1.2.3",
    );
    expect(msg).toContain('"maestro setup"');
    expect(msg).toContain("v1.2.3");
    expect(msg).toContain("maestro update");
  });

  it("matches the exact contract message format", () => {
    const msg = renderDriftError(
      { skill: "maestro-task", verb: "task observe", status: "missing-in-binary" },
      "9.9.9",
    );
    expect(msg).toBe(
      'Skill expects "maestro task observe"; binary v9.9.9 does not have it. ' +
      'Run "maestro update" or downgrade the skill bundle.',
    );
  });
});
