import { describe, expect, it } from "bun:test";
import { isMaestroSubstratePath } from "@/shared/lib/maestro-substrate-paths.js";

describe("isMaestroSubstratePath", () => {
  it("recognises the .maestro root and any path beneath it", () => {
    expect(isMaestroSubstratePath(".maestro")).toBe(true);
    expect(isMaestroSubstratePath(".maestro/tasks/tasks.jsonl")).toBe(true);
    expect(isMaestroSubstratePath(".maestro/runs/tsk-1/state.json")).toBe(true);
  });

  it("recognises bundled maestro: skill bundles under .claude/skills/ in both encodings", () => {
    expect(isMaestroSubstratePath(".claude/skills/maestro:agent-base/SKILL.md")).toBe(true);
    expect(isMaestroSubstratePath(".claude/skills/maestro%3Aagent-base/SKILL.md")).toBe(true);
    expect(isMaestroSubstratePath(".claude/skills/maestro%3Ablueprint/references/css-patterns.md"))
      .toBe(true);
  });

  it("recognises bundled maestro: skill bundles under .codex/skills/", () => {
    expect(isMaestroSubstratePath(".codex/skills/maestro:agent-base/SKILL.md")).toBe(true);
    expect(isMaestroSubstratePath(".codex/skills/maestro%3Aconduct/reference/brief-templates.md"))
      .toBe(true);
  });

  it("does NOT exempt user-authored skills outside the maestro: namespace", () => {
    // A team's own project-local skill must remain in scope — it's user code.
    expect(isMaestroSubstratePath(".claude/skills/my-team/SKILL.md")).toBe(false);
    expect(isMaestroSubstratePath(".codex/skills/internal-codegen/SKILL.md")).toBe(false);
  });

  it("does NOT exempt unrelated substrate-adjacent files", () => {
    expect(isMaestroSubstratePath(".gitignore")).toBe(false);
    expect(isMaestroSubstratePath(".claude/AGENTS.md")).toBe(false);
    expect(isMaestroSubstratePath(".codex/config.toml")).toBe(false);
    expect(isMaestroSubstratePath("src/lib/parse.ts")).toBe(false);
  });

  it("does NOT match paths that merely contain '.maestro' as a substring", () => {
    expect(isMaestroSubstratePath("docs/.maestrorc")).toBe(false);
    expect(isMaestroSubstratePath("vendor/.maestro/file.txt")).toBe(false);
  });
});
