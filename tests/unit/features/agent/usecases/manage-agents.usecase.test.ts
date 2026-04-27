import { describe, expect, it, beforeEach, afterEach } from "bun:test";
import { createHash } from "node:crypto";
import { chmod, lstat, mkdtemp, readlink, rm, mkdir, symlink, writeFile, readFile, stat } from "node:fs/promises";
import { existsSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";

import { BUNDLED_SKILL_TEMPLATES } from "@/infra/domain/bundled-skill-templates.js";
import {
  hasBlock,
  wrapBlock,
  removeBlock,
  removeLegacyBlock,
  hasReference,
  injectReference,
  removeReference,
  injectAgentBlocks,
  removeAgentBlocks,
  REFERENCE_FILE,
} from "@/features/agent";

const REFERENCE_LINE = `@${REFERENCE_FILE}`;
const BUNDLED_SKILL_NAMES = BUNDLED_SKILL_TEMPLATES.map((template) => template.name);

describe("manage-agents use case logic", () => {
  let tmpDir: string;
  let fakeHome: string;

  beforeEach(async () => {
    tmpDir = await mkdtemp(join(tmpdir(), "maestro-agents-"));
    fakeHome = join(tmpDir, "home");
    await mkdir(fakeHome, { recursive: true });
  });

  afterEach(async () => {
    await rm(tmpDir, { recursive: true, force: true });
  });

  describe("reference helpers (pure string)", () => {
    it("hasReference detects @MAESTRO.md line", () => {
      expect(hasReference(`# Config\n\n${REFERENCE_LINE}\n`)).toBe(true);
      expect(hasReference(`# Config\n\nSome content\n`)).toBe(false);
      expect(hasReference("")).toBe(false);
    });

    it("injectReference appends to content", () => {
      const result = injectReference("# Config\n\nExisting.\n");
      expect(result).toContain("# Config");
      expect(result).toContain("Existing.");
      expect(hasReference(result)).toBe(true);
    });

    it("injectReference is idempotent", () => {
      const first = injectReference("# Config\n");
      const second = injectReference(first);
      expect(first).toBe(second);
    });

    it("removeReference strips the @MAESTRO.md line", () => {
      const content = `# Config\n\n${REFERENCE_LINE}\n\n## Other\n`;
      const result = removeReference(content);
      expect(result).not.toBeNull();
      expect(hasReference(result!)).toBe(false);
      expect(result).toContain("# Config");
      expect(result).toContain("## Other");
    });

    it("removeReference returns null when no reference", () => {
      expect(removeReference("# Config\n\nNo maestro.\n")).toBeNull();
    });
  });

  describe("migration helpers (block + legacy)", () => {
    it("removes old inline block markers", () => {
      const content = `# Config\n\n${wrapBlock("Old maestro instructions")}\n\n## Other Section\n`;
      expect(hasBlock(content)).toBe(true);

      const cleaned = removeBlock(content)!;
      expect(cleaned).not.toBeNull();
      expect(hasBlock(cleaned)).toBe(false);
      expect(cleaned).toContain("## Other Section");
    });

    it("removes legacy heading section", () => {
      const content = `# Config\n\n## Cross-Agent Handoff (maestro)\n\nOld stale commands.\nmaestro handoff-plan --to codex\n\n## Other Section\n`;

      const cleaned = removeLegacyBlock(content)!;
      expect(cleaned).not.toBeNull();
      expect(cleaned).not.toContain("handoff-plan");
      expect(cleaned).not.toContain("Cross-Agent Handoff");
      expect(cleaned).toContain("## Other Section");
    });
  });

  describe("skill install (home-scoped agents)", () => {
    it("installs all bundled skills for Claude Code and Codex", async () => {
      // Pre-create both agents' config dirs so they are detected
      await mkdir(join(fakeHome, ".claude"), { recursive: true });
      await mkdir(join(fakeHome, ".codex"), { recursive: true });

      const results = await injectAgentBlocks(tmpDir, "all", fakeHome);

      const claude = results.find((r) => r.agent === "Claude Code");
      const codex = results.find((r) => r.agent === "Codex");
      expect(claude?.action).toBe("installed");
      expect(codex?.action).toBe("installed");

      for (const skillName of BUNDLED_SKILL_NAMES) {
        const claudePath = join(fakeHome, ".claude", "skills", skillName, "SKILL.md");
        const codexPath = join(fakeHome, ".codex", "skills", skillName, "SKILL.md");
        expect(existsSync(claudePath)).toBe(true);
        expect(existsSync(codexPath)).toBe(true);

        const claudeFrontmatter = await readFile(claudePath, "utf8");
        expect(claudeFrontmatter).toContain(`name: ${skillName}`);
      }

      expect(claude?.installedSkills).toEqual(BUNDLED_SKILL_NAMES);
      expect(codex?.installedSkills).toEqual(BUNDLED_SKILL_NAMES);
    });

    it("returns skipped when skills are already in sync", async () => {
      await mkdir(join(fakeHome, ".claude"), { recursive: true });
      await mkdir(join(fakeHome, ".codex"), { recursive: true });

      await injectAgentBlocks(tmpDir, "all", fakeHome);
      const second = await injectAgentBlocks(tmpDir, "all", fakeHome);

      expect(second.find((r) => r.agent === "Claude Code")?.action).toBe("skipped");
      expect(second.find((r) => r.agent === "Codex")?.action).toBe("skipped");
    });

    it("returns not-detected when agent config dir is missing", async () => {
      // fakeHome exists but no .claude/ or .codex/
      const results = await injectAgentBlocks(tmpDir, "all", fakeHome);

      expect(results.find((r) => r.agent === "Claude Code")?.action).toBe("not-detected");
      expect(results.find((r) => r.agent === "Codex")?.action).toBe("not-detected");
    });

    it("cleans up legacy MAESTRO.md and @MAESTRO.md reference and reports migrated-to-skills", async () => {
      await mkdir(join(fakeHome, ".claude"), { recursive: true });
      await writeFile(join(fakeHome, ".claude", "MAESTRO.md"), "# old injection\n");
      await writeFile(join(fakeHome, ".claude", "CLAUDE.md"), `# config\n\n${REFERENCE_LINE}\n`);
      await mkdir(join(fakeHome, ".codex"), { recursive: true });

      const results = await injectAgentBlocks(tmpDir, "all", fakeHome);

      const claude = results.find((r) => r.agent === "Claude Code");
      expect(claude?.action).toBe("migrated-to-skills");
      expect(existsSync(join(fakeHome, ".claude", "MAESTRO.md"))).toBe(false);

      const configAfter = await readFile(join(fakeHome, ".claude", "CLAUDE.md"), "utf8");
      expect(hasReference(configAfter)).toBe(false);
    });

    it("strips old inline block markers from the main config during migration", async () => {
      await mkdir(join(fakeHome, ".codex"), { recursive: true });
      await writeFile(
        join(fakeHome, ".codex", "AGENTS.md"),
        `# Codex config\n\n${wrapBlock("Old maestro instructions")}\n\n## Other\n`,
      );
      await mkdir(join(fakeHome, ".claude"), { recursive: true });

      const results = await injectAgentBlocks(tmpDir, "all", fakeHome);
      const codex = results.find((r) => r.agent === "Codex");

      expect(codex?.action).toBe("migrated-to-skills");
      const after = await readFile(join(fakeHome, ".codex", "AGENTS.md"), "utf8");
      expect(hasBlock(after)).toBe(false);
      expect(after).toContain("## Other");
    });

    it("removes stale maestro-managed skill dirs not in the bundled set", async () => {
      await mkdir(join(fakeHome, ".claude"), { recursive: true });
      await mkdir(join(fakeHome, ".codex"), { recursive: true });
      const staleDir = join(fakeHome, ".claude", "skills", "maestro-obsolete");
      await mkdir(staleDir, { recursive: true });
      await writeFile(join(staleDir, "SKILL.md"), "---\nname: maestro-obsolete\n---\n# old\n");
      // A maestro-managed skill dir carries the manifest marker.
      await writeFile(
        join(staleDir, ".maestro-bundled.json"),
        JSON.stringify({ managedBy: "maestro", skillName: "maestro-obsolete", fileHashes: {} }),
      );

      const results = await injectAgentBlocks(tmpDir, "all", fakeHome);
      expect(results.find((r) => r.agent === "Claude Code")?.action).toBe("installed");
      expect(existsSync(staleDir)).toBe(false);
    });

    it("reports installed when a later refresh only removes stale managed skill dirs", async () => {
      await mkdir(join(fakeHome, ".claude"), { recursive: true });
      await mkdir(join(fakeHome, ".codex"), { recursive: true });
      await injectAgentBlocks(tmpDir, "all", fakeHome);

      const staleDir = join(fakeHome, ".claude", "skills", "maestro-obsolete");
      await mkdir(staleDir, { recursive: true });
      await writeFile(join(staleDir, "SKILL.md"), "---\nname: maestro-obsolete\n---\n# old\n");
      await writeFile(
        join(staleDir, ".maestro-bundled.json"),
        JSON.stringify({ managedBy: "maestro", skillName: "maestro-obsolete", fileHashes: {} }),
      );

      const results = await injectAgentBlocks(tmpDir, "all", fakeHome);
      expect(results.find((r) => r.agent === "Claude Code")?.action).toBe("installed");
      expect(existsSync(staleDir)).toBe(false);
    });

    it("leaves non-maestro skill dirs untouched during stale cleanup", async () => {
      await mkdir(join(fakeHome, ".claude"), { recursive: true });
      await mkdir(join(fakeHome, ".codex"), { recursive: true });
      const userSkill = join(fakeHome, ".claude", "skills", "my-personal-skill");
      await mkdir(userSkill, { recursive: true });
      await writeFile(join(userSkill, "SKILL.md"), "---\nname: my-personal-skill\n---\n# mine\n");

      await injectAgentBlocks(tmpDir, "all", fakeHome);
      expect(existsSync(userSkill)).toBe(true);
    });

    it("leaves user-authored maestro-prefixed skill dirs untouched (no manifest marker)", async () => {
      await mkdir(join(fakeHome, ".claude"), { recursive: true });
      await mkdir(join(fakeHome, ".codex"), { recursive: true });
      // User's own skill that happens to use the maestro- prefix. No manifest.
      const userSkill = join(fakeHome, ".claude", "skills", "maestro-my-custom");
      await mkdir(userSkill, { recursive: true });
      await writeFile(join(userSkill, "SKILL.md"), "---\nname: maestro-my-custom\n---\n# mine\n");

      await injectAgentBlocks(tmpDir, "all", fakeHome);
      expect(existsSync(userSkill)).toBe(true);
      expect(existsSync(join(userSkill, "SKILL.md"))).toBe(true);
    });

    it("writes a .maestro-bundled.json manifest to each shipped skill dir", async () => {
      await mkdir(join(fakeHome, ".claude"), { recursive: true });
      await mkdir(join(fakeHome, ".codex"), { recursive: true });

      await injectAgentBlocks(tmpDir, "all", fakeHome);

      for (const skillName of BUNDLED_SKILL_NAMES) {
        const manifestPath = join(fakeHome, ".claude", "skills", skillName, ".maestro-bundled.json");
        expect(existsSync(manifestPath)).toBe(true);
        const raw = await readFile(manifestPath, "utf8");
        const manifest = JSON.parse(raw);
        expect(manifest.managedBy).toBe("maestro");
        expect(manifest.skillName).toBe(skillName);
        expect(manifest.fileHashes["SKILL.md"]).toMatch(/^[0-9a-f]{64}$/);
      }
    });

    it("preserves user edits to installed skill files across updates", async () => {
      await mkdir(join(fakeHome, ".claude"), { recursive: true });
      await mkdir(join(fakeHome, ".codex"), { recursive: true });

      // First install writes baseline files + manifest.
      await injectAgentBlocks(tmpDir, "all", fakeHome);

      // User edits maestro-task/SKILL.md.
      const userEditedPath = join(fakeHome, ".claude", "skills", "maestro-task", "SKILL.md");
      const userContent = "---\nname: maestro-task\n---\n# my custom override\n";
      await writeFile(userEditedPath, userContent);

      // Second install should preserve the edit because the manifest hash
      // differs from the on-disk hash.
      await injectAgentBlocks(tmpDir, "all", fakeHome);

      const afterContent = await readFile(userEditedPath, "utf8");
      expect(afterContent).toBe(userContent);
    });

    it("preserves user edits across repeated installs, not just one refresh", async () => {
      await mkdir(join(fakeHome, ".claude"), { recursive: true });
      await mkdir(join(fakeHome, ".codex"), { recursive: true });

      await injectAgentBlocks(tmpDir, "all", fakeHome);

      const userEditedPath = join(fakeHome, ".claude", "skills", "maestro-task", "SKILL.md");
      const userContent = "---\nname: maestro-task\n---\n# my sticky custom override\n";
      await writeFile(userEditedPath, userContent);

      await injectAgentBlocks(tmpDir, "all", fakeHome);
      expect(await readFile(userEditedPath, "utf8")).toBe(userContent);

      await injectAgentBlocks(tmpDir, "all", fakeHome);
      expect(await readFile(userEditedPath, "utf8")).toBe(userContent);
    });

    it("restores execute bits for shipped bundled scripts on reinstall", async () => {
      if (process.platform === "win32") return;

      await mkdir(join(fakeHome, ".claude"), { recursive: true });
      await mkdir(join(fakeHome, ".codex"), { recursive: true });

      await injectAgentBlocks(tmpDir, "all", fakeHome);

      const scriptPath = join(
        fakeHome,
        ".claude",
        "skills",
        "maestro-brainstorm",
        "scripts",
        "start-server.sh",
      );
      await chmod(scriptPath, 0o644);
      expect((await stat(scriptPath)).mode & 0o111).toBe(0);

      const second = await injectAgentBlocks(tmpDir, "all", fakeHome);

      expect(second.find((r) => r.agent === "Claude Code")?.action).toBe("installed");
      expect((await stat(scriptPath)).mode & 0o111).not.toBe(0);
    });

    it("removes stale manifest-owned files from still-shipped skill dirs", async () => {
      await mkdir(join(fakeHome, ".claude"), { recursive: true });
      await mkdir(join(fakeHome, ".codex"), { recursive: true });

      await injectAgentBlocks(tmpDir, "all", fakeHome);

      const staleFile = join(fakeHome, ".claude", "skills", "maestro-task", "reference", "stale.md");
      await mkdir(join(fakeHome, ".claude", "skills", "maestro-task", "reference"), { recursive: true });
      await writeFile(staleFile, "# stale\n");

      const manifestPath = join(fakeHome, ".claude", "skills", "maestro-task", ".maestro-bundled.json");
      const manifest = JSON.parse(await readFile(manifestPath, "utf8")) as {
        managedBy: string;
        skillName: string;
        installedAt?: string;
        maestroVersion?: string;
        fileHashes: Record<string, string>;
      };
      manifest.fileHashes["reference/stale.md"] = createHash("sha256").update("# stale\n").digest("hex");
      await writeFile(manifestPath, JSON.stringify(manifest, null, 2));

      await injectAgentBlocks(tmpDir, "all", fakeHome);
      expect(existsSync(staleFile)).toBe(false);
    });

    it("ignores manifest paths that escape the managed skill directory", async () => {
      await mkdir(join(fakeHome, ".claude"), { recursive: true });
      await mkdir(join(fakeHome, ".codex"), { recursive: true });

      await injectAgentBlocks(tmpDir, "all", fakeHome);

      const victimPath = join(fakeHome, ".claude", "outside.md");
      const victimContent = "# keep me\n";
      await writeFile(victimPath, victimContent);

      const manifestPath = join(fakeHome, ".claude", "skills", "maestro-task", ".maestro-bundled.json");
      const manifest = JSON.parse(await readFile(manifestPath, "utf8")) as {
        managedBy: string;
        skillName: string;
        installedAt?: string;
        maestroVersion?: string;
        fileHashes: Record<string, string>;
      };
      manifest.fileHashes["../../outside.md"] = createHash("sha256").update(victimContent).digest("hex");
      await writeFile(manifestPath, JSON.stringify(manifest, null, 2));

      await injectAgentBlocks(tmpDir, "all", fakeHome);

      expect(existsSync(victimPath)).toBe(true);
      expect(await readFile(victimPath, "utf8")).toBe(victimContent);
    });
  });

  describe("skill uninstall", () => {
    it("removes all bundled skill dirs and legacy files", async () => {
      await mkdir(join(fakeHome, ".claude"), { recursive: true });
      await mkdir(join(fakeHome, ".codex"), { recursive: true });
      await injectAgentBlocks(tmpDir, "all", fakeHome);

      const results = await removeAgentBlocks(tmpDir, "all", fakeHome);
      const claude = results.find((r) => r.agent === "Claude Code");
      const codex = results.find((r) => r.agent === "Codex");
      expect(claude?.action).toBe("removed");
      expect(codex?.action).toBe("removed");
      expect(claude?.removedSkills).toEqual(BUNDLED_SKILL_NAMES);

      for (const skillName of BUNDLED_SKILL_NAMES) {
        expect(existsSync(join(fakeHome, ".claude", "skills", skillName))).toBe(false);
        expect(existsSync(join(fakeHome, ".codex", "skills", skillName))).toBe(false);
      }
    });

    it("returns not-found when nothing is installed", async () => {
      await mkdir(join(fakeHome, ".claude"), { recursive: true });
      await mkdir(join(fakeHome, ".codex"), { recursive: true });

      const results = await removeAgentBlocks(tmpDir, "all", fakeHome);
      expect(results.find((r) => r.agent === "Claude Code")?.action).toBe("not-found");
      expect(results.find((r) => r.agent === "Codex")?.action).toBe("not-found");
    });

    it("returns not-detected when agent config dir is missing", async () => {
      const results = await removeAgentBlocks(tmpDir, "all", fakeHome);
      expect(results.find((r) => r.agent === "Claude Code")?.action).toBe("not-detected");
      expect(results.find((r) => r.agent === "Codex")?.action).toBe("not-detected");
    });

    it("sweeps manifest-bearing skill dirs that are no longer in the bundle", async () => {
      await mkdir(join(fakeHome, ".claude"), { recursive: true });
      await mkdir(join(fakeHome, ".codex"), { recursive: true });
      // Install current bundle first (writes manifests for each shipped skill).
      await injectAgentBlocks(tmpDir, "all", fakeHome);

      // Simulate a skill dropped from the bundle in a later release: manifest
      // is present (so uninstall sweeps it) but the name is not in the current
      // BUNDLED_SKILL_TEMPLATES set.
      const droppedDir = join(fakeHome, ".claude", "skills", "maestro-dropped");
      await mkdir(droppedDir, { recursive: true });
      await writeFile(join(droppedDir, "SKILL.md"), "---\nname: maestro-dropped\n---\n# old\n");
      await writeFile(
        join(droppedDir, ".maestro-bundled.json"),
        JSON.stringify({ managedBy: "maestro", skillName: "maestro-dropped", fileHashes: {} }),
      );

      const results = await removeAgentBlocks(tmpDir, "all", fakeHome);
      const claude = results.find((r) => r.agent === "Claude Code")!;
      expect(claude.action).toBe("removed");
      expect(claude.removedSkills).toContain("maestro-dropped");
      expect(existsSync(droppedDir)).toBe(false);
    });
  });

  describe("corrupt manifest tolerance", () => {
    it("treats an invalid-JSON manifest in ~/.maestro/skills/ as missing and re-writes shipped content", async () => {
      await mkdir(join(fakeHome, ".claude"), { recursive: true });
      await mkdir(join(fakeHome, ".codex"), { recursive: true });

      // Pre-seed the maestro source-of-truth tree with a corrupt manifest.
      const maestroSkillDir = join(fakeHome, ".maestro", "skills", "maestro-task");
      await mkdir(maestroSkillDir, { recursive: true });
      await writeFile(join(maestroSkillDir, ".maestro-bundled.json"), "{not valid json");
      await writeFile(join(maestroSkillDir, "SKILL.md"), "stale content");

      const results = await injectAgentBlocks(tmpDir, "all", fakeHome);
      expect(results.find((r) => r.agent === "Claude Code")?.action).toBe("installed");

      // Manifest rewritten to valid JSON in the maestro tree, content refreshed.
      const raw = await readFile(join(maestroSkillDir, ".maestro-bundled.json"), "utf8");
      const manifest = JSON.parse(raw);
      expect(manifest.managedBy).toBe("maestro");
      const skillContent = await readFile(join(maestroSkillDir, "SKILL.md"), "utf8");
      expect(skillContent).toContain("name: maestro-task");

      // Agent side reads through the symlink and sees the same fresh content.
      const agentContent = await readFile(
        join(fakeHome, ".claude", "skills", "maestro-task", "SKILL.md"),
        "utf8",
      );
      expect(agentContent).toBe(skillContent);
    });
  });

  describe("symlink layout (~/.maestro/skills as source of truth)", () => {
    it("writes skills to ~/.maestro/skills/ and links each agent's skills root into it", async () => {
      await mkdir(join(fakeHome, ".claude"), { recursive: true });
      await mkdir(join(fakeHome, ".codex"), { recursive: true });

      await injectAgentBlocks(tmpDir, "all", fakeHome);

      const maestroSkills = join(fakeHome, ".maestro", "skills");
      for (const skillName of BUNDLED_SKILL_NAMES) {
        // Source of truth is a real directory.
        const srcStats = await lstat(join(maestroSkills, skillName));
        expect(srcStats.isDirectory()).toBe(true);
        expect(srcStats.isSymbolicLink()).toBe(false);

        // Each agent's entry is a symlink/junction pointing at the maestro tree.
        for (const agentDir of [".claude", ".codex"]) {
          const linkPath = join(fakeHome, agentDir, "skills", skillName);
          const linkStats = await lstat(linkPath);
          expect(linkStats.isSymbolicLink()).toBe(true);
          const target = await readlink(linkPath);
          expect(target).toBe(join(maestroSkills, skillName));
        }
      }

      // Reading through the symlink resolves to the maestro tree's content.
      const throughLink = await readFile(
        join(fakeHome, ".claude", "skills", "maestro-task", "SKILL.md"),
        "utf8",
      );
      const direct = await readFile(join(maestroSkills, "maestro-task", "SKILL.md"), "utf8");
      expect(throughLink).toBe(direct);
    });

    it("does not duplicate manifests into the agent paths", async () => {
      await mkdir(join(fakeHome, ".claude"), { recursive: true });
      await injectAgentBlocks(tmpDir, "all", fakeHome);

      // The manifest exists at the maestro tree.
      const maestroManifest = join(fakeHome, ".maestro", "skills", "maestro-task", ".maestro-bundled.json");
      expect(existsSync(maestroManifest)).toBe(true);

      // It does NOT exist as a separate file under the agent path — the agent
      // path is a symlink and any read goes through to the maestro tree.
      const agentSkillEntry = join(fakeHome, ".claude", "skills", "maestro-task");
      const stats = await lstat(agentSkillEntry);
      expect(stats.isSymbolicLink()).toBe(true);
    });

    it("re-creates a missing symlink on the next install", async () => {
      await mkdir(join(fakeHome, ".claude"), { recursive: true });
      await injectAgentBlocks(tmpDir, "all", fakeHome);

      // User (or tooling) accidentally removes the symlink.
      const linkPath = join(fakeHome, ".claude", "skills", "maestro-task");
      await rm(linkPath);
      expect(existsSync(linkPath)).toBe(false);

      const second = await injectAgentBlocks(tmpDir, "all", fakeHome);
      expect(second.find((r) => r.agent === "Claude Code")?.action).toBe("installed");

      const linkStats = await lstat(linkPath);
      expect(linkStats.isSymbolicLink()).toBe(true);
    });

    it("repairs a symlink that points to the wrong skill within the maestro tree", async () => {
      await mkdir(join(fakeHome, ".claude"), { recursive: true });
      await injectAgentBlocks(tmpDir, "all", fakeHome);

      const linkPath = join(fakeHome, ".claude", "skills", "maestro-task");
      const wrongTarget = join(fakeHome, ".maestro", "skills", "maestro-plan");
      await rm(linkPath);
      await symlink(wrongTarget, linkPath);

      const second = await injectAgentBlocks(tmpDir, "all", fakeHome);
      expect(second.find((r) => r.agent === "Claude Code")?.action).toBe("installed");

      const target = await readlink(linkPath);
      expect(target).toBe(join(fakeHome, ".maestro", "skills", "maestro-task"));
    });

    it("leaves a user-authored symlink pointing outside the maestro tree alone", async () => {
      await mkdir(join(fakeHome, ".claude"), { recursive: true });
      // User has overridden one of our shipped skill names with their own
      // local skill (e.g. linked to their dotfiles). Don't clobber.
      const userOverrideTarget = join(tmpDir, "user-skills", "maestro-setup");
      await mkdir(userOverrideTarget, { recursive: true });
      await writeFile(join(userOverrideTarget, "SKILL.md"), "user override\n");
      await mkdir(join(fakeHome, ".claude", "skills"), { recursive: true });
      await symlink(userOverrideTarget, join(fakeHome, ".claude", "skills", "maestro-setup"));

      await injectAgentBlocks(tmpDir, "all", fakeHome);

      // User's override is preserved.
      const linkTarget = await readlink(join(fakeHome, ".claude", "skills", "maestro-setup"));
      expect(linkTarget).toBe(userOverrideTarget);
      const content = await readFile(
        join(fakeHome, ".claude", "skills", "maestro-setup", "SKILL.md"),
        "utf8",
      );
      expect(content).toBe("user override\n");

      // Other shipped skills still get linked normally.
      const planLink = await lstat(join(fakeHome, ".claude", "skills", "maestro-plan"));
      expect(planLink.isSymbolicLink()).toBe(true);
    });

    it("removes stale agent symlinks for skills no longer in the bundle", async () => {
      await mkdir(join(fakeHome, ".claude"), { recursive: true });
      await injectAgentBlocks(tmpDir, "all", fakeHome);

      // Simulate a skill dropped from the bundle in a later release: a stray
      // symlink under the agent path that points at a maestro-tree entry no
      // longer in the current set.
      const stalePath = join(fakeHome, ".claude", "skills", "maestro-dropped");
      const staleTarget = join(fakeHome, ".maestro", "skills", "maestro-dropped");
      await mkdir(staleTarget, { recursive: true });
      await symlink(staleTarget, stalePath);

      const second = await injectAgentBlocks(tmpDir, "all", fakeHome);
      expect(second.find((r) => r.agent === "Claude Code")?.action).toBe("installed");
      expect(existsSync(stalePath)).toBe(false);
    });
  });

  describe("migration from pre-redesign real-dir installs", () => {
    function shippedSkillFile(skillName: string, relativePath: string): string {
      const template = BUNDLED_SKILL_TEMPLATES.find((t) => t.name === skillName)!;
      return template.files.find((f) => f.path === relativePath)!.content;
    }

    function shippedSkillFileHash(skillName: string, relativePath: string): string {
      const content = shippedSkillFile(skillName, relativePath);
      return createHash("sha256").update(content).digest("hex");
    }

    async function seedLegacyAgentSkill(
      agentDir: string,
      skillName: string,
      overrides: Record<string, string> = {},
    ): Promise<void> {
      const skillDir = join(fakeHome, agentDir, "skills", skillName);
      await mkdir(skillDir, { recursive: true });
      const template = BUNDLED_SKILL_TEMPLATES.find((t) => t.name === skillName)!;
      const fileHashes: Record<string, string> = {};
      for (const file of template.files) {
        const onDiskPath = join(skillDir, file.path);
        await mkdir(join(skillDir, file.path, ".."), { recursive: true });
        const content = overrides[file.path] ?? file.content;
        await writeFile(onDiskPath, content);
        // Manifest always records the SHIPPED hash (pre-edit baseline) — that's
        // how the real pre-redesign installs left things.
        fileHashes[file.path] = createHash("sha256").update(file.content).digest("hex");
      }
      await writeFile(
        join(skillDir, ".maestro-bundled.json"),
        JSON.stringify({
          managedBy: "maestro",
          skillName,
          installedAt: new Date().toISOString(),
          maestroVersion: "0.0.0-legacy",
          fileHashes,
        }),
      );
    }

    it("migrates an unedited legacy real dir into a symlink", async () => {
      await mkdir(join(fakeHome, ".claude"), { recursive: true });
      await mkdir(join(fakeHome, ".codex"), { recursive: true });
      await seedLegacyAgentSkill(".claude", "maestro-task");

      const results = await injectAgentBlocks(tmpDir, "all", fakeHome);
      expect(results.find((r) => r.agent === "Claude Code")?.action).toBe("installed");

      const linkPath = join(fakeHome, ".claude", "skills", "maestro-task");
      const stats = await lstat(linkPath);
      expect(stats.isSymbolicLink()).toBe(true);
      expect(await readlink(linkPath)).toBe(join(fakeHome, ".maestro", "skills", "maestro-task"));
    });

    it("migrates user edits from legacy real dir into the maestro tree before linking", async () => {
      await mkdir(join(fakeHome, ".claude"), { recursive: true });
      const userContent = "---\nname: maestro-task\n---\n# my legacy edit\n";
      await seedLegacyAgentSkill(".claude", "maestro-task", { "SKILL.md": userContent });

      const results = await injectAgentBlocks(tmpDir, "all", fakeHome);
      const claude = results.find((r) => r.agent === "Claude Code")!;
      expect(claude.action).toBe("installed");
      expect(claude.preservedUserEdits).toContain("maestro-task/SKILL.md");

      // Edit landed in the maestro tree.
      const inMaestro = await readFile(
        join(fakeHome, ".maestro", "skills", "maestro-task", "SKILL.md"),
        "utf8",
      );
      expect(inMaestro).toBe(userContent);

      // Real dir replaced by a symlink; reading through it returns the same content.
      const linkStats = await lstat(join(fakeHome, ".claude", "skills", "maestro-task"));
      expect(linkStats.isSymbolicLink()).toBe(true);
      const throughLink = await readFile(
        join(fakeHome, ".claude", "skills", "maestro-task", "SKILL.md"),
        "utf8",
      );
      expect(throughLink).toBe(userContent);

      // Re-running install keeps the edit preserved at the source-of-truth tree.
      await injectAgentBlocks(tmpDir, "all", fakeHome);
      const stillEdited = await readFile(
        join(fakeHome, ".maestro", "skills", "maestro-task", "SKILL.md"),
        "utf8",
      );
      expect(stillEdited).toBe(userContent);
    });

    it("refuses migration when Claude and Codex have divergent edits to the same file", async () => {
      await mkdir(join(fakeHome, ".claude"), { recursive: true });
      await mkdir(join(fakeHome, ".codex"), { recursive: true });
      const claudeContent = "---\nname: maestro-task\n---\n# claude edit\n";
      const codexContent = "---\nname: maestro-task\n---\n# codex edit\n";
      await seedLegacyAgentSkill(".claude", "maestro-task", { "SKILL.md": claudeContent });
      await seedLegacyAgentSkill(".codex", "maestro-task", { "SKILL.md": codexContent });

      const results = await injectAgentBlocks(tmpDir, "all", fakeHome);

      // Claude (first in SUPPORTED_AGENTS) wins; its real dir becomes a symlink
      // and its edit lives in the maestro tree.
      const claudeLink = await lstat(join(fakeHome, ".claude", "skills", "maestro-task"));
      expect(claudeLink.isSymbolicLink()).toBe(true);
      const inMaestro = await readFile(
        join(fakeHome, ".maestro", "skills", "maestro-task", "SKILL.md"),
        "utf8",
      );
      expect(inMaestro).toBe(claudeContent);

      // Codex's real dir is left in place — divergence is loud, not silent.
      const codexEntry = await lstat(join(fakeHome, ".codex", "skills", "maestro-task"));
      expect(codexEntry.isDirectory()).toBe(true);
      expect(codexEntry.isSymbolicLink()).toBe(false);
      const codexFileStill = await readFile(
        join(fakeHome, ".codex", "skills", "maestro-task", "SKILL.md"),
        "utf8",
      );
      expect(codexFileStill).toBe(codexContent);

      const codex = results.find((r) => r.agent === "Codex")!;
      expect(codex.preservedUserEdits).toContain("maestro-task/SKILL.md");
    });

    it("ensures other agents still get clean symlinks for non-divergent skills", async () => {
      await mkdir(join(fakeHome, ".claude"), { recursive: true });
      await mkdir(join(fakeHome, ".codex"), { recursive: true });
      // Only maestro-task diverges; maestro-plan is fresh on both sides.
      await seedLegacyAgentSkill(".claude", "maestro-task", { "SKILL.md": "claude\n" });
      await seedLegacyAgentSkill(".codex", "maestro-task", { "SKILL.md": "codex\n" });

      await injectAgentBlocks(tmpDir, "all", fakeHome);

      // maestro-plan symlinks were created normally on both agents.
      for (const agentDir of [".claude", ".codex"]) {
        const planLink = await lstat(join(fakeHome, agentDir, "skills", "maestro-plan"));
        expect(planLink.isSymbolicLink()).toBe(true);
      }
    });

    it("leaves a real dir without a maestro manifest untouched and does not link over it", async () => {
      await mkdir(join(fakeHome, ".claude"), { recursive: true });
      const userDir = join(fakeHome, ".claude", "skills", "maestro-task");
      await mkdir(userDir, { recursive: true });
      await writeFile(join(userDir, "SKILL.md"), "user content");
      // No manifest file at all.

      await injectAgentBlocks(tmpDir, "all", fakeHome);

      // Real dir still there, content untouched.
      const stats = await lstat(userDir);
      expect(stats.isDirectory()).toBe(true);
      expect(stats.isSymbolicLink()).toBe(false);
      const content = await readFile(join(userDir, "SKILL.md"), "utf8");
      expect(content).toBe("user content");
    });
  });

  describe("environment overrides", () => {
    let savedCodexHome: string | undefined;
    let savedMaestroHome: string | undefined;

    beforeEach(() => {
      savedCodexHome = process.env["CODEX_HOME"];
      savedMaestroHome = process.env["MAESTRO_HOME"];
    });

    afterEach(() => {
      if (savedCodexHome === undefined) delete process.env["CODEX_HOME"];
      else process.env["CODEX_HOME"] = savedCodexHome;
      if (savedMaestroHome === undefined) delete process.env["MAESTRO_HOME"];
      else process.env["MAESTRO_HOME"] = savedMaestroHome;
    });

    it("honors CODEX_HOME for the Codex skills root", async () => {
      const codexHome = join(tmpDir, "custom-codex");
      await mkdir(codexHome, { recursive: true });
      process.env["CODEX_HOME"] = codexHome;

      // Don't pre-create ~/.codex; the override should take precedence.
      await mkdir(join(fakeHome, ".claude"), { recursive: true });

      const results = await injectAgentBlocks(tmpDir, "all", fakeHome);
      expect(results.find((r) => r.agent === "Codex")?.action).toBe("installed");

      const linkPath = join(codexHome, "skills", "maestro-task");
      const stats = await lstat(linkPath);
      expect(stats.isSymbolicLink()).toBe(true);
      expect(await readlink(linkPath)).toBe(join(fakeHome, ".maestro", "skills", "maestro-task"));
    });

    it("honors MAESTRO_HOME for the source-of-truth tree", async () => {
      const maestroHome = join(tmpDir, "custom-maestro");
      process.env["MAESTRO_HOME"] = maestroHome;

      await mkdir(join(fakeHome, ".claude"), { recursive: true });

      await injectAgentBlocks(tmpDir, "all", fakeHome);

      // Skills live under MAESTRO_HOME, not ~/.maestro.
      const expectedSrc = join(maestroHome, "skills", "maestro-task", "SKILL.md");
      expect(existsSync(expectedSrc)).toBe(true);

      const linkPath = join(fakeHome, ".claude", "skills", "maestro-task");
      const linkStats = await lstat(linkPath);
      expect(linkStats.isSymbolicLink()).toBe(true);
      expect(await readlink(linkPath)).toBe(join(maestroHome, "skills", "maestro-task"));
    });
  });

  describe("uninstall handles symlinks and legacy real dirs", () => {
    it("removes agent symlinks for current bundled skills (post-redesign install)", async () => {
      await mkdir(join(fakeHome, ".claude"), { recursive: true });
      await mkdir(join(fakeHome, ".codex"), { recursive: true });
      await injectAgentBlocks(tmpDir, "all", fakeHome);

      const results = await removeAgentBlocks(tmpDir, "all", fakeHome);
      expect(results.find((r) => r.agent === "Claude Code")?.action).toBe("removed");

      for (const skillName of BUNDLED_SKILL_NAMES) {
        expect(existsSync(join(fakeHome, ".claude", "skills", skillName))).toBe(false);
        expect(existsSync(join(fakeHome, ".codex", "skills", skillName))).toBe(false);
      }
    });

    it("leaves user-authored skill dirs and dangling symlinks (non-maestro target) alone", async () => {
      await mkdir(join(fakeHome, ".claude"), { recursive: true });
      await injectAgentBlocks(tmpDir, "all", fakeHome);

      // User-authored dir alongside the maestro symlinks.
      const userDir = join(fakeHome, ".claude", "skills", "my-personal-skill");
      await mkdir(userDir, { recursive: true });
      await writeFile(join(userDir, "SKILL.md"), "mine\n");

      // Symlink that uses the maestro- prefix but points outside the maestro tree.
      const userSymlinkSource = join(tmpDir, "elsewhere-skill");
      await mkdir(userSymlinkSource, { recursive: true });
      await symlink(userSymlinkSource, join(fakeHome, ".claude", "skills", "maestro-foreign"));

      await removeAgentBlocks(tmpDir, "all", fakeHome);

      expect(existsSync(userDir)).toBe(true);
      expect(existsSync(join(fakeHome, ".claude", "skills", "maestro-foreign"))).toBe(true);
    });
  });
});
