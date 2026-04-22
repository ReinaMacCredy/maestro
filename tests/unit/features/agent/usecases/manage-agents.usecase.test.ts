import { describe, expect, it, beforeEach, afterEach } from "bun:test";
import { mkdtemp, rm, mkdir, writeFile, readFile } from "node:fs/promises";
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

    it("removes stale maestro-* skill dirs not in the bundled set", async () => {
      await mkdir(join(fakeHome, ".claude"), { recursive: true });
      await mkdir(join(fakeHome, ".codex"), { recursive: true });
      const staleDir = join(fakeHome, ".claude", "skills", "maestro-obsolete");
      await mkdir(staleDir, { recursive: true });
      await writeFile(join(staleDir, "SKILL.md"), "---\nname: maestro-obsolete\n---\n# old\n");

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
  });
});
