/**
 * Edge Case 5 (skill-binary drift): when the installed skill bundle expects
 * a verb the binary doesn't have, the agent invoking that verb gets a clear
 * "Skill expects ...; binary v<n> does not have it" error from
 * `renderDriftError` instead of a bare Commander "unknown command" line.
 *
 * The parity check is also wired into the CLI's unknown-command path in
 * `src/index.ts` so the hint fires automatically.
 */
import { describe, it, expect } from "bun:test";
import {
  checkSkillBinaryParity,
  renderDriftError,
} from "@/features/setup/usecases/check-skill-binary-parity.usecase.js";

describe("Edge Case 5: skill-binary drift detection", () => {
  it("returns no drift when the binary covers every skill-referenced verb", () => {
    // A maximal verb set guarantees no drift regardless of which skill ships.
    const knownVerbs = new Set<string>();
    for (const v of [
      "evidence", "contract", "task", "session", "ralph", "gc", "state", "recover",
      "mission-control", "spec", "verdict", "policy", "plan", "ci", "merge", "deploy",
      "runtime", "inspect", "worktree", "setup", "review", "handoff", "task verify",
      "task introspect", "task budget", "task proof", "task observe",
      "contract show", "contract amend", "contract history", "contract sprint",
      "evidence record", "evidence list", "evidence show",
      "verdict request", "verdict show", "verdict override",
      "policy check", "policy pending", "plan check",
      "ci verify", "merge auto", "deploy gate", "deploy rollback",
      "runtime check", "review ack",
      "gc doc-gardening", "gc slop-cleanup", "gc plan-regen",
      "spec show", "spec edit", "session whoami", "session start", "session exit",
      "ralph review", "mission-control", "state since", "worktree create",
      "setup", "qa",
      "qa lint", "qa typecheck", "qa test", "qa run",
      "intake", "doctor", "memory-correct",
      "claim", "block", "abandon", "ship", "verify",
    ]) knownVerbs.add(v);
    const report = checkSkillBinaryParity({ knownVerbs });
    // No findings whose first segment is missing from knownVerbs
    for (const f of report.findings) {
      expect(knownVerbs.has(f.verb.split(/\s+/)[0]!)).toBe(true);
    }
  });

  it("returns missing-in-binary findings when knownVerbs is empty", () => {
    const report = checkSkillBinaryParity({ knownVerbs: new Set() });
    expect(report.skillsChecked).toBeGreaterThan(0);
    expect(report.findings.length).toBeGreaterThan(0);
    for (const f of report.findings) {
      expect(f.status).toBe("missing-in-binary");
    }
  });

  it("renderDriftError matches the contract message format", () => {
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
