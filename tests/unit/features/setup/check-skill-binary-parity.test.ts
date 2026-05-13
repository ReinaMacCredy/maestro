import { describe, expect, it } from "bun:test";
import {
  checkSkillBinaryParity,
  renderDriftError,
} from "@/features/setup";

describe("checkSkillBinaryParity", () => {
  it("returns no findings when all skill-referenced verbs exist", () => {
    const allVerbs = new Set<string>([
      "session", "task", "evidence", "verdict", "plan", "verify",
      "session start", "session exit", "task introspect",
      "verdict request", "plan check",
    ]);
    // Add every other possible verb so nothing reports drift.
    for (const v of [
      "intake", "ralph", "review", "recover", "gc", "state", "inspect",
      "policy", "spec", "contract", "deploy", "runtime", "merge", "ci",
      "skills", "install", "update", "uninstall", "providers", "init",
      "status", "doctor", "note", "memory", "memory-ratchet", "graph",
      "handoff", "bundle", "mission", "feature", "validate", "milestone",
      "checkpoint", "principle", "reply", "qa", "mission-control",
      "qa install", "qa check", "qa modalities", "task verify",
      "task contract", "task budget", "task plan", "task claim",
      "task update", "task heartbeat", "task ready", "task show",
      "task mine", "task stuck", "task similar", "task prune",
      "task observe", "deploy gate", "deploy rollback", "verdict show",
      "verdict override", "review ack", "evidence record", "evidence list",
      "evidence show", "spec show", "spec edit", "ralph review",
      "merge auto", "ci verify", "gc doc-gardening", "gc slop-cleanup",
      "gc plan-regen", "contract sprint", "policy check", "policy pending",
      "task proof", "state since", "worktree", "worktree create",
      "memory correct", "memory recall", "memory search", "memory learn",
      "memory compile", "memory stats", "memory lint", "ratchet check",
      "ratchet promote", "graph link", "graph context", "bundle export",
      "bundle inspect", "mission show", "mission validate",
      "setup", "setup check", "setup languages",
      "memory-correct", "memory-recall", "memory-search", "memory-learn",
    ]) {
      allVerbs.add(v);
    }
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
});
