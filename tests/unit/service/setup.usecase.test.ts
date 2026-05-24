import { describe, expect, it, beforeEach, afterEach } from "bun:test";
import { access, mkdtemp, mkdir, readFile, rm, stat, symlink, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { runSetup } from "@/service/setup.usecase.js";
import { runDoctor } from "@/infra/usecases/run-doctor.usecase.js";
import { resolveSkillDirectoryName } from "@/shared/lib/skill-path.js";
import {
  mockConfig,
  mockRepoTaskStore,
  mockVerdictStore,
} from "../../helpers/mocks.js";
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
    const lines = new Set(gitignore.split(/\r?\n/));
    expect(gitignore).not.toContain(".maestro/launches/");
    expect(gitignore).not.toContain(".maestro/handoff/");
    // Mission sidecars (`<slug>.md`) stay tracked; the rest of the dir is
    // runtime state. The bare `.maestro/missions/` line is intentionally not
    // emitted — it would mask the negations that re-include the sidecars.
    expect(lines.has(".maestro/missions/")).toBe(false);
    expect(lines.has(".maestro/missions/*")).toBe(true);
    expect(lines.has("!.maestro/missions/*.md")).toBe(true);
    expect(lines.has("!.maestro/missions/.gitkeep")).toBe(true);
    expect(lines.has(".maestro/sessions/")).toBe(true);
    expect(lines.has(".maestro/tasks/local-history/")).toBe(true);
  });

  it("strips a stale `.maestro/missions/` blanket line on upgrade", async () => {
    await writeFile(
      join(tmpDir, ".gitignore"),
      "node_modules/\n\n# Maestro runtime state\n.maestro/missions/\n",
    );

    await runSetup(project());

    const gitignore = await readFile(join(tmpDir, ".gitignore"), "utf8");
    const lines = new Set(gitignore.split(/\r?\n/));
    expect(lines.has(".maestro/missions/")).toBe(false);
    expect(lines.has(".maestro/missions/*")).toBe(true);
    expect(lines.has("!.maestro/missions/*.md")).toBe(true);
    expect(gitignore).toContain("node_modules/");
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

  it("emits an executable init.sh at the project root on fresh setup", async () => {
    const result = await runSetup(project());
    const initPath = join(tmpDir, "init.sh");
    expect(result.created).toContain(initPath);

    const content = await readFile(initPath, "utf8");
    expect(content).toContain("maestro doctor");
    expect(content).toContain("maestro status");

    if (process.platform !== "win32") {
      const s = await stat(initPath);
      expect(s.mode & 0o111).not.toBe(0);
    }
  });

  it("does not overwrite an existing init.sh on rerun", async () => {
    const initPath = join(tmpDir, "init.sh");
    await writeFile(initPath, "#!/usr/bin/env bash\necho user-init\n");

    const result = await runSetup(project());

    expect(result.skipped).toContain(initPath);
    expect(result.created).not.toContain(initPath);
    expect(await readFile(initPath, "utf8")).toBe(
      "#!/usr/bin/env bash\necho user-init\n",
    );
  });

  // Regression: FIX-2 -- `--reset-templates` was the force flag for the
  // template step. Without an `overwritePolicy: "never"` guard on init.sh,
  // user customizations were silently overwritten despite the spec promising
  // "never overwrites init.sh". The customization here is intentionally
  // distinctive so any overwrite is loud and obvious.
  it("preserves user-customized init.sh even with --reset-templates", async () => {
    const initPath = join(tmpDir, "init.sh");
    const userScript = "#!/usr/bin/env bash\n# user-owned init\necho 'user logic here'\nexit 0\n";
    await writeFile(initPath, userScript);

    const result = await runSetup(project({ resetTemplates: true }));

    expect(result.skipped).toContain(initPath);
    expect(result.created).not.toContain(initPath);
    expect(await readFile(initPath, "utf8")).toBe(userScript);
  });

  it("seeds project-root AGENTS.md with the managed setup block on fresh setup", async () => {
    const result = await runSetup(project());
    const agentsPath = join(tmpDir, "AGENTS.md");
    expect(result.created).toContain(agentsPath);

    const content = await readFile(agentsPath, "utf8");
    expect(content).toContain("<!-- maestro-setup:start -->");
    expect(content).toContain("<!-- maestro-setup:end -->");
    expect(content).toContain("## Maestro");
    expect(content).toContain("./init.sh");
  });

  it("preserves existing AGENTS.md content and appends the managed block", async () => {
    const agentsPath = join(tmpDir, "AGENTS.md");
    const userContent = "# My Project\n\nLong-standing notes the user keeps here.\n";
    await writeFile(agentsPath, userContent);

    const result = await runSetup(project());

    const pointerStep = result.steps.find((s) => s.id === "write-project-pointers");
    const entry = pointerStep?.paths.find((p) => p.path === agentsPath);
    expect(entry?.action).toBe("overwrite");

    const content = await readFile(agentsPath, "utf8");
    expect(content).toContain("# My Project");
    expect(content).toContain("Long-standing notes the user keeps here.");
    expect(content).toContain("<!-- maestro-setup:start -->");
    expect(content.indexOf("# My Project")).toBeLessThan(
      content.indexOf("<!-- maestro-setup:start -->"),
    );
  });

  // Regression: FIX-1 -- `--reset-templates` was the force flag for bootstrap
  // templates, but the project-root pointer step routes through
  // `injectSetupBlock`, which only swaps the managed block. The original bug
  // landed when AGENTS.md was added to the template list without the
  // `managed-block` policy guard. The bite-mark: with `resetTemplates: true`,
  // user content BOTH before AND after the managed block must survive.
  it("preserves user content surrounding the managed block in AGENTS.md under --reset-templates", async () => {
    const agentsPath = join(tmpDir, "AGENTS.md");
    // Seed: user has a pre-existing AGENTS.md with content above AND below a
    // legacy managed setup block (simulating a re-setup after manual edits).
    const userPrefix = "# My Project\n\nLong-standing user notes.\n\n";
    const managedBlock = "<!-- maestro-setup:start -->\n## Maestro\n\nold pointer body.\n<!-- maestro-setup:end -->";
    const userSuffix = "\n\n## My Custom Section\n\nMore content the user owns.\n";
    await writeFile(agentsPath, userPrefix + managedBlock + userSuffix);

    await runSetup(project({ resetTemplates: true }));

    const content = await readFile(agentsPath, "utf8");
    // Both user-owned regions survive.
    expect(content).toContain("Long-standing user notes.");
    expect(content).toContain("## My Custom Section");
    expect(content).toContain("More content the user owns.");
    // Managed block is still present (idempotency).
    expect(content).toContain("<!-- maestro-setup:start -->");
    expect(content).toContain("<!-- maestro-setup:end -->");
  });

  // Category sibling for FIX-1: CLAUDE.md uses the reference-line (not block)
  // mechanism, but the same overwrite protection applies.
  it("preserves user content in CLAUDE.md under --reset-templates", async () => {
    const claudePath = join(tmpDir, "CLAUDE.md");
    const userContent = "# My CLAUDE.md\n\n@my-other-doc.md\n\nUser notes here.\n";
    await writeFile(claudePath, userContent);

    await runSetup(project({ resetTemplates: true }));

    const content = await readFile(claudePath, "utf8");
    expect(content).toContain("# My CLAUDE.md");
    expect(content).toContain("@my-other-doc.md");
    expect(content).toContain("User notes here.");
    expect(content).toContain("@AGENTS.md");
  });

  it("emits AGENTS.md with both the project-conventions template and the managed setup block on fresh setup", async () => {
    await runSetup(project());

    const content = await readFile(join(tmpDir, "AGENTS.md"), "utf8");
    expect(content).toContain("# Project Conventions");
    expect(content).toContain("<!-- maestro-setup:start -->");
    expect(content).toContain("## Maestro");
    expect(content.indexOf("# Project Conventions")).toBeLessThan(
      content.indexOf("<!-- maestro-setup:start -->"),
    );
  });

  it("preserves an existing legacy maestro block alongside the new setup block", async () => {
    const agentsPath = join(tmpDir, "AGENTS.md");
    const legacyBlock =
      "<!-- maestro:start -->\n## Cross-Agent Handoff (maestro)\n\nLegacy content.\n<!-- maestro:end -->";
    await writeFile(agentsPath, `# My Project\n\n${legacyBlock}\n`);

    await runSetup(project());

    const content = await readFile(agentsPath, "utf8");
    expect(content).toContain("<!-- maestro:start -->");
    expect(content).toContain("<!-- maestro:end -->");
    expect(content).toContain("Legacy content.");
    expect(content).toContain("<!-- maestro-setup:start -->");
    expect(content).toContain("<!-- maestro-setup:end -->");
  });

  it("does not re-inject the AGENTS.md block on rerun", async () => {
    await runSetup(project());
    const agentsPath = join(tmpDir, "AGENTS.md");
    const afterFirst = await readFile(agentsPath, "utf8");

    const second = await runSetup(project());
    expect(second.skipped).toContain(agentsPath);
    expect(second.created).not.toContain(agentsPath);
    expect(await readFile(agentsPath, "utf8")).toBe(afterFirst);
  });

  it("seeds project-root CLAUDE.md with the @AGENTS.md reference on fresh setup", async () => {
    const result = await runSetup(project());
    const claudePath = join(tmpDir, "CLAUDE.md");
    expect(result.created).toContain(claudePath);

    const content = await readFile(claudePath, "utf8");
    expect(content).toContain("@AGENTS.md");
  });

  it("preserves existing CLAUDE.md content and appends the @AGENTS.md reference", async () => {
    const claudePath = join(tmpDir, "CLAUDE.md");
    const userContent = "# My CLAUDE.md\n\n@my-other-doc.md\n";
    await writeFile(claudePath, userContent);

    const result = await runSetup(project());

    const pointerStep = result.steps.find((s) => s.id === "write-project-pointers");
    const entry = pointerStep?.paths.find((p) => p.path === claudePath);
    expect(entry?.action).toBe("overwrite");

    const content = await readFile(claudePath, "utf8");
    expect(content).toContain("@my-other-doc.md");
    expect(content).toContain("@AGENTS.md");
  });

  it("does not re-inject the CLAUDE.md reference on rerun", async () => {
    await runSetup(project());
    const claudePath = join(tmpDir, "CLAUDE.md");
    const afterFirst = await readFile(claudePath, "utf8");

    const second = await runSetup(project());
    expect(second.skipped).toContain(claudePath);
    expect(second.created).not.toContain(claudePath);
    expect(await readFile(claudePath, "utf8")).toBe(afterFirst);
  });

  it("syncs all 6 bundled maestro-* skills under .claude/skills/ and .codex/skills/", async () => {
    // Regression: setup.usecase.ts previously iterated BUILT_IN_SKILL_TEMPLATES
    // (empty `[]` since v0.100.0), leaving project-level skill directories
    // empty. The 6 shipped skills live in BUNDLED_SKILL_TEMPLATES.
    await runSetup(project());

    const expectedSkills = [
      "maestro-design",
      "maestro-handoff",
      "maestro-mission",
      "maestro-setup",
      "maestro-task",
      "maestro-verify",
    ];

    for (const skill of expectedSkills) {
      for (const root of [".claude", ".codex"]) {
        const skillFile = join(tmpDir, root, "skills", skill, "SKILL.md");
        const content = await readFile(skillFile, "utf8");
        expect(content.length).toBeGreaterThan(0);
        expect(content).toContain(`name: ${skill}`);
      }
    }
  });

  it("emitted init.sh satisfies maestro doctor's init-script dimension", async () => {
    await runSetup(project());

    const checks = await runDoctor({
      taskStore: mockRepoTaskStore(),
      verdictStore: mockVerdictStore(),
      projectDir: tmpDir,
    });

    const initCheck = checks.find((c) => c.name === "init-script");
    expect(initCheck?.status).toBe("ok");
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

  it("reports would-create for project-root pointers when files are absent", async () => {
    const result = await runSetup(project({ dryRun: true }));
    const pointerStep = result.steps.find((s) => s.id === "write-project-pointers");
    const agentsEntry = pointerStep?.paths.find((p) => p.path === join(tmpDir, "AGENTS.md"));
    const claudeEntry = pointerStep?.paths.find((p) => p.path === join(tmpDir, "CLAUDE.md"));

    expect(agentsEntry?.action).toBe("would-create");
    expect(claudeEntry?.action).toBe("would-create");
    await expect(access(join(tmpDir, "AGENTS.md"))).rejects.toThrow();
    await expect(access(join(tmpDir, "CLAUDE.md"))).rejects.toThrow();
  });

  it("reports would-overwrite for project-root pointers when files pre-exist without the block", async () => {
    await writeFile(join(tmpDir, "AGENTS.md"), "# Existing notes\n");
    await writeFile(join(tmpDir, "CLAUDE.md"), "@my-other-doc.md\n");

    const result = await runSetup(project({ dryRun: true }));
    const pointerStep = result.steps.find((s) => s.id === "write-project-pointers");
    const agentsEntry = pointerStep?.paths.find((p) => p.path === join(tmpDir, "AGENTS.md"));
    const claudeEntry = pointerStep?.paths.find((p) => p.path === join(tmpDir, "CLAUDE.md"));

    expect(agentsEntry?.action).toBe("would-overwrite");
    expect(claudeEntry?.action).toBe("would-overwrite");
    expect(await readFile(join(tmpDir, "AGENTS.md"), "utf8")).toBe("# Existing notes\n");
    expect(await readFile(join(tmpDir, "CLAUDE.md"), "utf8")).toBe("@my-other-doc.md\n");
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
