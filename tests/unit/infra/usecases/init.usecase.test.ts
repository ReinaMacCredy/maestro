import { describe, expect, it, beforeEach, afterEach } from "bun:test";
import { access, mkdtemp, mkdir, readFile, rm, symlink, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { initMaestro } from "@/infra/usecases/init.usecase.js";
import { resolveSkillDirectoryName } from "@/shared/lib/skill-path.js";
import { mockConfig } from "../../../helpers/mocks.js";
import { DEFAULT_PRINCIPLES } from "@/features/mission";

let tmpDir: string;

beforeEach(async () => {
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-init-"));
});

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true });
});

describe("initMaestro", () => {
  it("creates project .maestro directory", async () => {
    const config = mockConfig();
    const result = await initMaestro(config, { global: false, dir: tmpDir });
    expect(result.scope).toBe("project");
    expect(result.created.length).toBeGreaterThan(0);
    expect(result.bootstrapGenerated).toBe(true);

    // Directory created (ensureDir was called)
    expect(result.created.some((p) => p.includes(".maestro"))).toBe(true);
  });

  it("does not create a project-local handoff directory (handoffs live globally at ~/.maestro/handoff/)", async () => {
    const config = mockConfig();
    const result = await initMaestro(config, { global: false, dir: tmpDir });
    expect(result.created).not.toContain(join(tmpDir, ".maestro", "launches"));
    expect(result.created).not.toContain(join(tmpDir, ".maestro", "handoff"));
    expect(result.created).toContain(join(tmpDir, ".maestro", "bootstrap", "services.yaml"));
    expect(result.created).toContain(join(tmpDir, ".maestro", "AGENTS.md"));
  });

  it("does not overwrite existing config", async () => {
    let writeCount = 0;
    const config = mockConfig({
      exists: async () => true,
      write: async () => { writeCount++; },
    });
    const result = await initMaestro(config, { global: false, dir: tmpDir });
    expect(writeCount).toBe(0);
    expect(result.skipped).toContain(join(tmpDir, ".maestro", "config.yaml"));
  });

  it("writes config when none exists", async () => {
    let written = false;
    const config = mockConfig({
      exists: async () => false,
      write: async () => { written = true; },
    });
    await initMaestro(config, { global: false, dir: tmpDir });
    expect(written).toBe(true);
  });

  it("skips existing bootstrap files by default", async () => {
    const config = mockConfig();
    const agentsPath = join(tmpDir, ".maestro", "AGENTS.md");
    await Bun.write(agentsPath, "keep me\n");

    const result = await initMaestro(config, { global: false, dir: tmpDir });

    expect(result.skipped).toContain(agentsPath);
    expect(await readFile(agentsPath, "utf8")).toBe("keep me\n");
  });

  it("replaces existing bootstrap files when confirmed", async () => {
    const config = mockConfig();
    const agentsPath = join(tmpDir, ".maestro", "AGENTS.md");
    await Bun.write(agentsPath, "old content\n");

    const result = await initMaestro(config, {
      global: false,
      dir: tmpDir,
      confirmReplace: async (path) => path === agentsPath,
    });

    expect(result.created).toContain(agentsPath);
    expect(await readFile(agentsPath, "utf8")).toContain("Maestro Project Bootstrap");
  });

  it("keeps global init minimal", async () => {
    let writeCount = 0;
    const config = mockConfig({
      exists: async () => false,
      write: async () => { writeCount++; },
    });

    const result = await initMaestro(config, { global: true, dir: tmpDir });

    expect(result.scope).toBe("global");
    expect(result.bootstrapGenerated).toBe(false);
    expect(writeCount).toBe(1);
    expect(result.created.some((path) => path.includes("bootstrap"))).toBe(false);
  });

  it("does not re-report existing directories on rerun", async () => {
    const config = mockConfig();

    await initMaestro(config, { global: false, dir: tmpDir });
    const second = await initMaestro(config, { global: false, dir: tmpDir });

    expect(second.created).toEqual([]);
    expect(second.skipped).toContain(join(tmpDir, ".maestro", "config.yaml"));
  });

  it("migrates legacy .factory bootstrap files into .maestro", async () => {
    const config = mockConfig();
    await mkdir(join(tmpDir, ".factory", "library"), { recursive: true });
    await writeFile(
      join(tmpDir, ".factory", "services.yaml"),
      "commands:\n  test: echo legacy-test\nservices: {}\n",
    );
    await writeFile(
      join(tmpDir, ".factory", "library", "architecture.md"),
      "# Legacy Architecture\n",
    );

    const result = await initMaestro(config, { global: false, dir: tmpDir });

    expect(result.created).toContain(join(tmpDir, ".maestro", "bootstrap", "services.yaml"));
    expect(await readFile(join(tmpDir, ".maestro", "bootstrap", "services.yaml"), "utf8")).toContain(
      "legacy-test",
    );
    expect(await readFile(join(tmpDir, ".maestro", "bootstrap", "library", "architecture.md"), "utf8")).toContain(
      "# Legacy Architecture",
    );
  });

  it("scaffolds gitignore entries for runtime state", async () => {
    const config = mockConfig();

    await initMaestro(config, { global: false, dir: tmpDir });

    const gitignore = await readFile(join(tmpDir, ".gitignore"), "utf8");
    expect(gitignore).not.toContain(".maestro/launches/");
    expect(gitignore).not.toContain(".maestro/handoff/");
    expect(gitignore).toContain(".maestro/missions/");
    expect(gitignore).toContain(".maestro/sessions/");
    expect(gitignore).toContain(".maestro/tasks/local-history/");
  });

  it("rejects symlinked .maestro paths that escape the project root", async () => {
    const config = mockConfig();
    const outsideDir = join(tmpDir, "outside");

    await mkdir(outsideDir, { recursive: true });
    await symlink(outsideDir, join(tmpDir, ".maestro"));

    await expect(initMaestro(config, { global: false, dir: tmpDir })).rejects.toThrow(
      "Refusing to initialize through symlinked path",
    );
  });

  it("rejects symlinked project roots", async () => {
    const config = mockConfig();
    const realRoot = join(tmpDir, "real-project");
    const linkRoot = join(tmpDir, "project-link");

    await mkdir(realRoot, { recursive: true });
    await symlink(realRoot, linkRoot);

    await expect(initMaestro(config, { global: false, dir: linkRoot })).rejects.toThrow(
      "Refusing to initialize through symlinked project root",
    );
  });

  it("creates principles.jsonl with default principles on fresh init", async () => {
    const config = mockConfig();
    const result = await initMaestro(config, { global: false, dir: tmpDir });

    const principlesPath = join(tmpDir, ".maestro", "principles.jsonl");
    expect(result.created).toContain(principlesPath);

    const raw = await readFile(principlesPath, "utf8");
    const lines = raw.trim().split("\n");
    expect(lines).toHaveLength(DEFAULT_PRINCIPLES.length);

    const parsed = lines.map((line) => JSON.parse(line));
    expect(parsed[0].id).toBe("think-before-coding");
    expect(parsed[3].id).toBe("goal-driven-execution");
  });

  it("does not overwrite existing principles.jsonl on re-init", async () => {
    const config = mockConfig();
    const principlesPath = join(tmpDir, ".maestro", "principles.jsonl");
    await mkdir(join(tmpDir, ".maestro"), { recursive: true });
    await writeFile(principlesPath, '{"id":"custom"}\n');

    const result = await initMaestro(config, { global: false, dir: tmpDir });

    expect(result.skipped).toContain(principlesPath);
    expect(result.created).not.toContain(principlesPath);

    const raw = await readFile(principlesPath, "utf8");
    expect(raw).toBe('{"id":"custom"}\n');
  });

  it("syncs built-in maestro skills into project-local claude and codex folders", async () => {
    const config = mockConfig();
    const agentBaseDir = resolveSkillDirectoryName("maestro:agent-base");

    const result = await initMaestro(config, { global: false, dir: tmpDir });

    const claudeSkillPath = join(tmpDir, ".claude", "skills", agentBaseDir, "SKILL.md");
    const codexSkillPath = join(tmpDir, ".codex", "skills", agentBaseDir, "SKILL.md");

    expect(result.created).toContain(claudeSkillPath);
    expect(result.created).toContain(codexSkillPath);
    expect(await readFile(claudeSkillPath, "utf8")).toContain("# Agent Base Procedures");
    expect(await readFile(codexSkillPath, "utf8")).toContain("# Agent Base Procedures");
  });

  it("overwrites existing synced maestro skills with the shipped version", async () => {
    const config = mockConfig();
    const agentBaseDir = resolveSkillDirectoryName("maestro:agent-base");
    const claudeSkillPath = join(tmpDir, ".claude", "skills", agentBaseDir, "SKILL.md");

    await mkdir(join(tmpDir, ".claude", "skills", agentBaseDir), { recursive: true });
    await writeFile(claudeSkillPath, "# old agent base\n");

    const result = await initMaestro(config, { global: false, dir: tmpDir });

    expect(result.created).toContain(claudeSkillPath);
    expect(await readFile(claudeSkillPath, "utf8")).toContain("# Agent Base Procedures");
  });

  it("removes stale synced maestro skills without touching non-maestro skills", async () => {
    const config = mockConfig();
    const staleSkillDir = resolveSkillDirectoryName("maestro:obsolete");
    const staleClaudeSkillPath = join(tmpDir, ".claude", "skills", staleSkillDir, "SKILL.md");
    const staleCodexSkillPath = join(tmpDir, ".codex", "skills", staleSkillDir, "SKILL.md");
    const customSkillPath = join(tmpDir, ".claude", "skills", "custom-skill", "SKILL.md");

    await mkdir(join(tmpDir, ".claude", "skills", staleSkillDir), { recursive: true });
    await mkdir(join(tmpDir, ".codex", "skills", staleSkillDir), { recursive: true });
    await mkdir(join(tmpDir, ".claude", "skills", "custom-skill"), { recursive: true });
    await writeFile(staleClaudeSkillPath, "# old skill\n");
    await writeFile(staleCodexSkillPath, "# old skill\n");
    await writeFile(customSkillPath, "# keep me\n");

    await initMaestro(config, { global: false, dir: tmpDir });

    await expect(access(staleClaudeSkillPath)).rejects.toThrow();
    await expect(access(staleCodexSkillPath)).rejects.toThrow();
      expect(await readFile(customSkillPath, "utf8")).toBe("# keep me\n");
    });

    it.skipIf(process.platform === "win32")(
      "removes legacy colon-named synced maestro skills during re-init",
      async () => {
        // Windows reserves `:` as the drive separator and rejects it in filenames
        // (ENOTDIR on mkdir). The legacy colon-named skill directories could only
        // ever have existed on POSIX, so the re-init cleanup is unreachable there.
        const config = mockConfig();
        const legacyClaudeSkillPath = join(tmpDir, ".claude", "skills", "maestro:obsolete", "SKILL.md");
        const legacyCodexSkillPath = join(tmpDir, ".codex", "skills", "maestro:obsolete", "SKILL.md");

        await mkdir(join(tmpDir, ".claude", "skills", "maestro:obsolete"), { recursive: true });
        await mkdir(join(tmpDir, ".codex", "skills", "maestro:obsolete"), { recursive: true });
        await writeFile(legacyClaudeSkillPath, "# old skill\n");
        await writeFile(legacyCodexSkillPath, "# old skill\n");

        await initMaestro(config, { global: false, dir: tmpDir });

        await expect(access(legacyClaudeSkillPath)).rejects.toThrow();
        await expect(access(legacyCodexSkillPath)).rejects.toThrow();
      },
    );

  it("creates .maestro/policies/owners.yaml on fresh init", async () => {
    const config = mockConfig();
    const result = await initMaestro(config, { global: false, dir: tmpDir });

    const ownersPath = join(tmpDir, ".maestro", "policies", "owners.yaml");
    expect(result.created).toContain(ownersPath);

    const content = await readFile(ownersPath, "utf8");
    expect(content).toContain("policy_approver");
    expect(content).toContain("ratchet_approver");
    expect(content).toContain("sensitive_waiver");
  });

  it("does not overwrite existing .maestro/policies/owners.yaml on re-init", async () => {
    const config = mockConfig();
    const ownersPath = join(tmpDir, ".maestro", "policies", "owners.yaml");
    await mkdir(join(tmpDir, ".maestro", "policies"), { recursive: true });
    await writeFile(ownersPath, "policy_approver:\n  - \"@customowner\"\n");

    const result = await initMaestro(config, { global: false, dir: tmpDir });

    expect(result.skipped).toContain(ownersPath);
    expect(result.created).not.toContain(ownersPath);
    expect(await readFile(ownersPath, "utf8")).toBe("policy_approver:\n  - \"@customowner\"\n");
  });

  it("creates .maestro/policies/sensitive-paths.yaml on fresh init", async () => {
    const config = mockConfig();
    const result = await initMaestro(config, { global: false, dir: tmpDir });

    const policiesPath = join(tmpDir, ".maestro", "policies", "sensitive-paths.yaml");
    expect(result.created).toContain(policiesPath);

    const content = await readFile(policiesPath, "utf8");
    expect(content).toContain("sensitive_paths");
    expect(content).toContain("src/auth/**");
    expect(content).toContain("bun.lock");
  });

  it("does not overwrite existing .maestro/policies/sensitive-paths.yaml on re-init", async () => {
    const config = mockConfig();
    const policiesPath = join(tmpDir, ".maestro", "policies", "sensitive-paths.yaml");
    await mkdir(join(tmpDir, ".maestro", "policies"), { recursive: true });
    await writeFile(policiesPath, "sensitive_paths:\n  - \"custom/**\"\n");

    const result = await initMaestro(config, { global: false, dir: tmpDir });

    expect(result.skipped).toContain(policiesPath);
    expect(result.created).not.toContain(policiesPath);
    expect(await readFile(policiesPath, "utf8")).toBe("sensitive_paths:\n  - \"custom/**\"\n");
  });

  it("creates .maestro/policies/risk.yaml on fresh init", async () => {
    const config = mockConfig();
    const result = await initMaestro(config, { global: false, dir: tmpDir });

    const riskPath = join(tmpDir, ".maestro", "policies", "risk.yaml");
    expect(result.created).toContain(riskPath);

    const content = await readFile(riskPath, "utf8");
    expect(content).toContain("diff-intersects-sensitive-security");
    expect(content).toContain("critical");
    expect(content).toContain("diff-docs-only");
    expect(content).toContain("low");
  });

  it("does not overwrite existing .maestro/policies/risk.yaml on re-init", async () => {
    const config = mockConfig();
    const riskPath = join(tmpDir, ".maestro", "policies", "risk.yaml");
    await mkdir(join(tmpDir, ".maestro", "policies"), { recursive: true });
    await writeFile(riskPath, "rows: []\n");

    const result = await initMaestro(config, { global: false, dir: tmpDir });

    expect(result.skipped).toContain(riskPath);
    expect(result.created).not.toContain(riskPath);
    expect(await readFile(riskPath, "utf8")).toBe("rows: []\n");
  });

  it("creates .maestro/policies/autopilot.yaml on fresh init", async () => {
    const config = mockConfig();
    const result = await initMaestro(config, { global: false, dir: tmpDir });

    const autopilotPath = join(tmpDir, ".maestro", "policies", "autopilot.yaml");
    expect(result.created).toContain(autopilotPath);

    const content = await readFile(autopilotPath, "utf8");
    expect(content).toContain("auto_merge_allowed");
    expect(content).toContain("required_witness_level");
    expect(content).toContain("witnessed-by-maestro");
  });

  it("does not overwrite existing .maestro/policies/autopilot.yaml on re-init", async () => {
    const config = mockConfig();
    const autopilotPath = join(tmpDir, ".maestro", "policies", "autopilot.yaml");
    await mkdir(join(tmpDir, ".maestro", "policies"), { recursive: true });
    await writeFile(autopilotPath, "auto_merge_allowed:\n  low: true\n");

    const result = await initMaestro(config, { global: false, dir: tmpDir });

    expect(result.skipped).toContain(autopilotPath);
    expect(result.created).not.toContain(autopilotPath);
    expect(await readFile(autopilotPath, "utf8")).toBe("auto_merge_allowed:\n  low: true\n");
  });

  it("creates .maestro/policies/release.yaml on fresh init", async () => {
    const config = mockConfig();
    const result = await initMaestro(config, { global: false, dir: tmpDir });

    const releasePath = join(tmpDir, ".maestro", "policies", "release.yaml");
    expect(result.created).toContain(releasePath);

    const content = await readFile(releasePath, "utf8");
    expect(content).toContain("require_signed_commits");
    expect(content).toContain("require_proof_map_complete");
  });

  it("does not overwrite existing .maestro/policies/release.yaml on re-init", async () => {
    const config = mockConfig();
    const releasePath = join(tmpDir, ".maestro", "policies", "release.yaml");
    await mkdir(join(tmpDir, ".maestro", "policies"), { recursive: true });
    await writeFile(releasePath, "require_signed_commits: true\n");

    const result = await initMaestro(config, { global: false, dir: tmpDir });

    expect(result.skipped).toContain(releasePath);
    expect(result.created).not.toContain(releasePath);
    expect(await readFile(releasePath, "utf8")).toBe("require_signed_commits: true\n");
  });
  });
