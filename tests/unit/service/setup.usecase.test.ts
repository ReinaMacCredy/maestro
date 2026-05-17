import { describe, expect, it, beforeEach, afterEach } from "bun:test";
import { access, mkdtemp, mkdir, readFile, rm, symlink, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { runSetup } from "@/service/setup.usecase.js";
import { resolveSkillDirectoryName } from "@/shared/lib/skill-path.js";
import { mockConfig } from "../../helpers/mocks.js";
import { DEFAULT_PRINCIPLES } from "@/service/default-principles.js";

let tmpDir: string;

beforeEach(async () => {
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-setup-"));
});

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true });
});

function project(overrides: Partial<Parameters<typeof runSetup>[0]> = {}) {
  return {
    config: overrides.config ?? mockConfig(),
    dir: overrides.dir ?? tmpDir,
    global: false,
    noGitOk: true,
    ...overrides,
  } as Parameters<typeof runSetup>[0];
}

describe("runSetup (project scope)", () => {
  it("creates project .maestro directory", async () => {
    const result = await runSetup(project());
    expect(result.scope).toBe("project");
    expect(result.ok).toBe(true);
    expect(result.created.length).toBeGreaterThan(0);
    expect(result.created.some((p) => p.includes(".maestro"))).toBe(true);
  });

  it("creates the missions templates directory with a .gitkeep", async () => {
    const result = await runSetup(project());
    expect(result.created).toContain(join(tmpDir, ".maestro", "templates", "missions"));
    expect(result.created).toContain(
      join(tmpDir, ".maestro", "templates", "missions", ".gitkeep"),
    );
  });

  it("does not create a project-local handoff directory", async () => {
    const result = await runSetup(project());
    expect(result.created).not.toContain(join(tmpDir, ".maestro", "launches"));
    expect(result.created).not.toContain(join(tmpDir, ".maestro", "handoff"));
    expect(result.created).toContain(join(tmpDir, ".maestro", "bootstrap", "services.yaml"));
    expect(result.created).toContain(join(tmpDir, ".maestro", "AGENTS.md"));
  });

  it("does not overwrite existing config", async () => {
    let writeCount = 0;
    const config = mockConfig({
      exists: async () => true,
      write: async () => {
        writeCount++;
      },
    });
    const result = await runSetup(project({ config }));
    expect(writeCount).toBe(0);
    expect(result.skipped).toContain(join(tmpDir, ".maestro", "config.yaml"));
  });

  it("writes config when none exists", async () => {
    let written = false;
    const config = mockConfig({
      exists: async () => false,
      write: async () => {
        written = true;
      },
    });
    await runSetup(project({ config }));
    expect(written).toBe(true);
  });

  it("skips existing bootstrap files by default", async () => {
    const agentsPath = join(tmpDir, ".maestro", "AGENTS.md");
    await mkdir(join(tmpDir, ".maestro"), { recursive: true });
    await Bun.write(agentsPath, "keep me\n");

    const result = await runSetup(project());

    expect(result.skipped).toContain(agentsPath);
    expect(await readFile(agentsPath, "utf8")).toBe("keep me\n");
  });

  it("replaces existing bootstrap files when confirmed", async () => {
    const agentsPath = join(tmpDir, ".maestro", "AGENTS.md");
    await mkdir(join(tmpDir, ".maestro"), { recursive: true });
    await Bun.write(agentsPath, "old content\n");

    const result = await runSetup(
      project({ confirmReplace: async (path) => path === agentsPath }),
    );

    expect(result.created).toContain(agentsPath);
    expect(await readFile(agentsPath, "utf8")).toContain("Maestro Project Bootstrap");
  });

  it("does not re-report existing directories on rerun", async () => {
    const config = mockConfig();
    await runSetup(project({ config }));
    const second = await runSetup(project({ config }));

    expect(second.created).toEqual([]);
    expect(second.skipped).toContain(join(tmpDir, ".maestro", "config.yaml"));
  });

  it("scaffolds gitignore entries for runtime state", async () => {
    await runSetup(project());

    const gitignore = await readFile(join(tmpDir, ".gitignore"), "utf8");
    expect(gitignore).not.toContain(".maestro/launches/");
    expect(gitignore).not.toContain(".maestro/handoff/");
    expect(gitignore).toContain(".maestro/missions/");
    expect(gitignore).toContain(".maestro/sessions/");
    expect(gitignore).toContain(".maestro/tasks/local-history/");
  });

  it("rejects symlinked .maestro paths that escape the project root", async () => {
    const outsideDir = join(tmpDir, "outside");
    await mkdir(outsideDir, { recursive: true });
    await symlink(outsideDir, join(tmpDir, ".maestro"));

    await expect(runSetup(project())).rejects.toThrow(
      "Refusing to initialize through symlinked path",
    );
  });

  it("rejects symlinked project roots", async () => {
    const realRoot = join(tmpDir, "real-project");
    const linkRoot = join(tmpDir, "project-link");
    await mkdir(realRoot, { recursive: true });
    await symlink(realRoot, linkRoot);

    await expect(runSetup(project({ dir: linkRoot }))).rejects.toThrow(
      "Refusing to initialize through symlinked project root",
    );
  });

  it("creates docs/principles/<slug>.md with default principles on fresh setup", async () => {
    const result = await runSetup(project());

    const principlesDir = join(tmpDir, "docs", "principles");
    for (const principle of DEFAULT_PRINCIPLES) {
      const principleFile = join(principlesDir, `${principle.slug}.md`);
      expect(result.created).toContain(principleFile);
      const content = await readFile(principleFile, "utf8");
      expect(content).toBe(principle.content);
    }
  });

  it("does not overwrite existing docs/principles/<slug>.md on re-setup", async () => {
    const principlesDir = join(tmpDir, "docs", "principles");
    await mkdir(principlesDir, { recursive: true });
    const firstPrinciple = DEFAULT_PRINCIPLES[0]!;
    const principleFile = join(principlesDir, `${firstPrinciple.slug}.md`);
    await writeFile(principleFile, "# custom content\n");

    const result = await runSetup(project());

    expect(result.skipped).toContain(principleFile);
    expect(result.created).not.toContain(principleFile);

    const content = await readFile(principleFile, "utf8");
    expect(content).toBe("# custom content\n");
  });

  it("removes stale synced maestro skills only with --resync-skills", async () => {
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

    // Default run (no --resync-skills): stale dirs persist
    await runSetup(project());
    await access(staleClaudeSkillPath);
    await access(staleCodexSkillPath);

    // With --resync-skills: stale dirs removed, custom untouched
    await runSetup(project({ resyncSkills: true }));
    await expect(access(staleClaudeSkillPath)).rejects.toThrow();
    await expect(access(staleCodexSkillPath)).rejects.toThrow();
    expect(await readFile(customSkillPath, "utf8")).toBe("# keep me\n");
  });

  it("creates .maestro/policies/owners.yaml on fresh setup", async () => {
    const result = await runSetup(project());
    const ownersPath = join(tmpDir, ".maestro", "policies", "owners.yaml");
    expect(result.created).toContain(ownersPath);

    const content = await readFile(ownersPath, "utf8");
    expect(content).toContain("policy_approver");
    expect(content).toContain("ratchet_approver");
    expect(content).toContain("sensitive_waiver");
  });

  it("does not overwrite existing .maestro/policies/owners.yaml on re-setup", async () => {
    const ownersPath = join(tmpDir, ".maestro", "policies", "owners.yaml");
    await mkdir(join(tmpDir, ".maestro", "policies"), { recursive: true });
    await writeFile(ownersPath, "policy_approver:\n  - \"@customowner\"\n");

    const result = await runSetup(project());

    expect(result.skipped).toContain(ownersPath);
    expect(result.created).not.toContain(ownersPath);
    expect(await readFile(ownersPath, "utf8")).toBe("policy_approver:\n  - \"@customowner\"\n");
  });

  it("creates .maestro/policies/sensitive-paths.yaml on fresh setup", async () => {
    const result = await runSetup(project());
    const policiesPath = join(tmpDir, ".maestro", "policies", "sensitive-paths.yaml");
    expect(result.created).toContain(policiesPath);

    const content = await readFile(policiesPath, "utf8");
    expect(content).toContain("paths:");
    expect(content).toContain("src/auth/**");
    expect(content).toContain("bun.lock");
  });

  it("creates .maestro/policies/risk.yaml on fresh setup", async () => {
    const result = await runSetup(project());
    const riskPath = join(tmpDir, ".maestro", "policies", "risk.yaml");
    expect(result.created).toContain(riskPath);

    const content = await readFile(riskPath, "utf8");
    expect(content).toContain("diff-intersects-sensitive-security");
    expect(content).toContain("critical");
    expect(content).toContain("diff-docs-only");
    expect(content).toContain("low");
  });

  it("creates .maestro/policies/autopilot.yaml on fresh setup", async () => {
    const result = await runSetup(project());
    const autopilotPath = join(tmpDir, ".maestro", "policies", "autopilot.yaml");
    expect(result.created).toContain(autopilotPath);

    const content = await readFile(autopilotPath, "utf8");
    expect(content).toContain("auto_merge_allowed");
    expect(content).toContain("required_witness_level");
    expect(content).toContain("witnessed-by-maestro");
  });

  it("creates .maestro/policies/release.yaml on fresh setup", async () => {
    const result = await runSetup(project());
    const releasePath = join(tmpDir, ".maestro", "policies", "release.yaml");
    expect(result.created).toContain(releasePath);

    const content = await readFile(releasePath, "utf8");
    expect(content).toContain("require_signed_commits");
    expect(content).toContain("require_proof_map_complete");
  });
});

describe("runSetup (v1 hard-delete)", () => {
  it("deletes v1 paths it finds", async () => {
    await mkdir(join(tmpDir, ".maestro", "memory", "corrections"), { recursive: true });
    await mkdir(join(tmpDir, ".maestro", "memory", "learnings"), { recursive: true });
    await writeFile(join(tmpDir, ".maestro", ".migrated-v2.json"), "{}\n");
    await mkdir(join(tmpDir, ".factory"), { recursive: true });

    const result = await runSetup(project());

    await expect(access(join(tmpDir, ".maestro", "memory", "corrections"))).rejects.toThrow();
    await expect(access(join(tmpDir, ".maestro", "memory", "learnings"))).rejects.toThrow();
    await expect(access(join(tmpDir, ".maestro", ".migrated-v2.json"))).rejects.toThrow();
    await expect(access(join(tmpDir, ".factory"))).rejects.toThrow();

    const deleteStep = result.steps.find((s) => s.id === "delete-v1");
    expect(deleteStep?.status).toBe("changed");
    expect(deleteStep?.paths.length).toBe(4);
  });

  it("reports no-op when v1 paths are absent", async () => {
    const result = await runSetup(project());
    const detectStep = result.steps.find((s) => s.id === "detect-v1");
    expect(detectStep?.status).toBe("ok");
  });
});

describe("runSetup (plans -> missions migration)", () => {
  it("renames .maestro/plans/plans.jsonl to .maestro/missions/missions.jsonl", async () => {
    const plansDir = join(tmpDir, ".maestro", "plans");
    await mkdir(plansDir, { recursive: true });
    await writeFile(join(plansDir, "plans.jsonl"), '{"id":"pln-1"}\n');

    const result = await runSetup(project());

    const moved = join(tmpDir, ".maestro", "missions", "missions.jsonl");
    expect(await readFile(moved, "utf8")).toBe('{"id":"pln-1"}\n');
    await expect(access(plansDir)).rejects.toThrow();

    const step = result.steps.find((s) => s.id === "migrate-plans-to-missions");
    expect(step?.status).toBe("changed");
  });

  it("renames plans.v2.jsonl to missions.v2.jsonl", async () => {
    const plansDir = join(tmpDir, ".maestro", "plans");
    await mkdir(plansDir, { recursive: true });
    await writeFile(join(plansDir, "plans.v2.jsonl"), '{"id":"pln-1"}\n');

    await runSetup(project());

    const moved = join(tmpDir, ".maestro", "missions", "missions.v2.jsonl");
    expect(await readFile(moved, "utf8")).toBe('{"id":"pln-1"}\n');
  });

  it("aborts when .maestro/missions/ already has v2 data", async () => {
    await mkdir(join(tmpDir, ".maestro", "plans"), { recursive: true });
    await writeFile(join(tmpDir, ".maestro", "plans", "plans.jsonl"), '{"id":"pln-1"}\n');
    await mkdir(join(tmpDir, ".maestro", "missions"), { recursive: true });
    await writeFile(
      join(tmpDir, ".maestro", "missions", "missions.jsonl"),
      '{"id":"pln-2"}\n',
    );

    const result = await runSetup(project());
    expect(result.ok).toBe(false);
    const step = result.steps.find((s) => s.id === "migrate-plans-to-missions");
    expect(step?.status).toBe("error");
  });
});

describe("runSetup (dry-run)", () => {
  it("does not mutate the filesystem", async () => {
    const result = await runSetup(project({ dryRun: true }));
    expect(result.dryRun).toBe(true);

    await expect(access(join(tmpDir, ".maestro"))).rejects.toThrow();
    await expect(access(join(tmpDir, ".gitignore"))).rejects.toThrow();
  });

  it("reports would-create entries for new paths", async () => {
    const result = await runSetup(project({ dryRun: true }));
    const allActions = result.steps.flatMap((s) => s.paths.map((p) => p.action));
    expect(allActions).toContain("would-create");
  });
});

describe("runSetup (global)", () => {
  it("keeps global setup minimal", async () => {
    let writeCount = 0;
    const config = mockConfig({
      exists: async () => false,
      write: async () => {
        writeCount++;
      },
    });

    const result = await runSetup({ config, dir: tmpDir, global: true });

    expect(result.scope).toBe("global");
    expect(writeCount).toBe(1);
    expect(result.created.some((path) => path.includes("bootstrap"))).toBe(false);
  });
});

describe("runSetup (git guard)", () => {
  it("rejects setup outside a git repo without --no-git-ok", async () => {
    const result = await runSetup({
      config: mockConfig(),
      dir: tmpDir,
      global: false,
    });
    expect(result.ok).toBe(false);
    expect(result.steps[0]?.id).toBe("guard-git");
  });

  it("allows setup with --no-git-ok", async () => {
    const result = await runSetup({
      config: mockConfig(),
      dir: tmpDir,
      global: false,
      noGitOk: true,
    });
    expect(result.ok).toBe(true);
  });

  it("allows setup when .git is present", async () => {
    await mkdir(join(tmpDir, ".git"), { recursive: true });
    const result = await runSetup({
      config: mockConfig(),
      dir: tmpDir,
      global: false,
    });
    expect(result.ok).toBe(true);
  });
});
