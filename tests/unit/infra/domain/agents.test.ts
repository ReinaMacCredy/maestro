import { describe, expect, it } from "bun:test";
import { tmpdir, homedir } from "node:os";
import { join } from "node:path";
import { SUPPORTED_AGENTS, agentConfigPath, agentSkillsRoot } from "@/infra/domain/agents.js";

describe("agent config specs", () => {
  it("ships runtime providers plus the shared AgentSkills target", () => {
    const slugs = SUPPORTED_AGENTS.map((a) => a.slug).sort();
    expect(slugs).toEqual(["agentskills", "claude-code", "codex", "hermes"]);
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

  it("resolves Hermes config and skill roots", () => {
    const hermes = SUPPORTED_AGENTS.find((agent) => agent.slug === "hermes")!;
    expect(agentConfigPath(hermes, tmpdir(), homedir())).toBe(
      join(homedir(), ".hermes", "config.yaml"),
    );
    expect(agentSkillsRoot(hermes, tmpdir(), homedir())).toBe(
      join(homedir(), ".hermes", "skills", "maestro"),
    );
  });

  it("resolves the shared AgentSkills root", () => {
    const agentSkills = SUPPORTED_AGENTS.find((agent) => agent.slug === "agentskills")!;
    expect(agentSkills.runtime).toBe(false);
    expect(agentSkills.skillTarget).toBe(true);
    expect(agentSkillsRoot(agentSkills, tmpdir(), homedir())).toBe(
      join(homedir(), ".agents", "skills"),
    );
  });
});
