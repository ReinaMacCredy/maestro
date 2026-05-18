import { describe, expect, it } from "bun:test";
import {
  checkSkillBinaryParity,
  renderDriftError,
} from "@/service/skill-binary-parity.js";
import { collectKnownVerbs } from "@/service/known-verbs.js";
import { program } from "@/index.js";

describe("checkSkillBinaryParity", () => {
  it("returns no findings when verbs are collected from the real Commander tree", () => {
    // Walk the actual program in src/index.ts so the test cannot drift away
    // from what the binary actually exposes. If a skill (SKILL.md or any
    // reference/*.md) references a dead verb, this test fails and surfaces
    // the exact skill+verb pair.
    const knownVerbs = collectKnownVerbs(program);
    const report = checkSkillBinaryParity({ knownVerbs });
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
