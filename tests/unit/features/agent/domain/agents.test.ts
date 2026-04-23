import { describe, expect, it } from "bun:test";
import { tmpdir, homedir } from "node:os";
import { join } from "node:path";
import { SUPPORTED_AGENTS, agentConfigPath, agentSkillsRoot } from "@/features/agent";

describe("agent config specs", () => {
  it("ships Claude Code and Codex entries only", () => {
    const slugs = SUPPORTED_AGENTS.map((a) => a.slug).sort();
    expect(slugs).toEqual(["claude-code", "codex"]);
    // droid and gemini were removed; the comment in agents.ts notes they
    // will be re-added once skill support lands in those CLIs.
  });

  it("resolves Claude Code config to ~/.claude/CLAUDE.md", () => {
    const claude = SUPPORTED_AGENTS.find((agent) => agent.slug === "claude-code")!;
    expect(claude.configDir).toBe(".claude");
    expect(claude.configFile).toBe("CLAUDE.md");
    expect(agentConfigPath(claude, tmpdir(), homedir())).toBe(
      join(homedir(), ".claude", "CLAUDE.md"),
    );
  });

  it("resolves Codex config to ~/.codex/AGENTS.md", () => {
    const codex = SUPPORTED_AGENTS.find((agent) => agent.slug === "codex")!;
    expect(codex.configDir).toBe(".codex");
    expect(codex.configFile).toBe("AGENTS.md");
    expect(agentConfigPath(codex, tmpdir(), homedir())).toBe(
      join(homedir(), ".codex", "AGENTS.md"),
    );
  });

  it("resolves agent skills root to ~/<configDir>/skills", () => {
    const claude = SUPPORTED_AGENTS.find((agent) => agent.slug === "claude-code")!;
    expect(agentSkillsRoot(claude, tmpdir(), homedir())).toBe(
      join(homedir(), ".claude", "skills"),
    );
  });
});
