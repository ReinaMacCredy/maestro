import { describe, expect, it, beforeEach, afterEach } from "bun:test";
import { mkdtemp, rm, mkdir, writeFile, readFile } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";

// We can't easily mock homedir() in the use case since it uses os.homedir() directly.
// Instead, test the underlying block functions + do a focused integration test
// by calling inject/remove on real temp files via the lib functions.
import { AGENT_INSTRUCTION_BLOCK } from "../../../src/domain/defaults.js";
import { renderTemplate } from "../../../src/lib/template.js";
import {
  hasBlock,
  extractBlock,
  injectBlock,
  replaceBlock,
  removeBlock,
  removeLegacyBlock,
  wrapBlock,
} from "../../../src/lib/agent-block.js";
import {
  injectAgentBlocks,
  removeAgentBlocks,
} from "../../../src/usecases/manage-agents.usecase.js";

describe("manage-agents use case logic", () => {
  let tmpDir: string;
  let homeDir: string;
  let originalHome: string | undefined;

  beforeEach(async () => {
    tmpDir = await mkdtemp(join(tmpdir(), "maestro-agents-"));
    homeDir = await mkdtemp(join(tmpdir(), "maestro-agents-home-"));
    originalHome = process.env.HOME;
    process.env.HOME = homeDir;
  });

  afterEach(async () => {
    process.env.HOME = originalHome;
    await rm(tmpDir, { recursive: true, force: true });
    await rm(homeDir, { recursive: true, force: true });
  });

  const rendered = renderTemplate(AGENT_INSTRUCTION_BLOCK, { agent: "codex" });

  describe("inject flow", () => {
    it("injects into empty file", async () => {
      const path = join(tmpDir, "AGENTS.md");
      await writeFile(path, "");

      const content = await Bun.file(path).text();
      const result = injectBlock(content, rendered);
      await writeFile(path, result);

      const final = await Bun.file(path).text();
      expect(hasBlock(final)).toBe(true);
      expect(final).toContain("maestro handoff-pickup --claim --agent codex");
    });

    it("injects into existing config", async () => {
      const path = join(tmpDir, "CLAUDE.md");
      await writeFile(path, "# My Config\n\nExisting content here.\n");

      const content = await Bun.file(path).text();
      const result = injectBlock(content, rendered);
      await writeFile(path, result);

      const final = await Bun.file(path).text();
      expect(final).toContain("# My Config");
      expect(final).toContain("Existing content here.");
      expect(hasBlock(final)).toBe(true);
    });

    it("skips when block already matches", () => {
      const content = `# Config\n\n${wrapBlock(rendered)}\n`;
      expect(hasBlock(content)).toBe(true);
      expect(extractBlock(content)).toBe(rendered);
    });

    it("updates when block content differs", async () => {
      const path = join(tmpDir, "CLAUDE.md");
      const oldBlock = wrapBlock("Old maestro instructions");
      await writeFile(path, `# Config\n\n${oldBlock}\n`);

      const content = await Bun.file(path).text();
      const result = replaceBlock(content, rendered);
      expect(result).not.toBeNull();
      await writeFile(path, result!);

      const final = await Bun.file(path).text();
      expect(final).toContain("maestro handoff-pickup");
      expect(final).not.toContain("Old maestro instructions");
    });
  });

  describe("legacy migration flow", () => {
    it("replaces unmarked legacy block with marked one", async () => {
      const path = join(tmpDir, "CLAUDE.md");
      const legacy = `# Config\n\n## Cross-Agent Handoff (maestro)\n\nOld stale commands here.\nmaestro handoff-plan --to codex\n\n## Other Section\n`;
      await writeFile(path, legacy);

      const content = await Bun.file(path).text();
      const cleaned = removeLegacyBlock(content);
      expect(cleaned).not.toBeNull();

      const result = injectBlock(cleaned!, rendered);
      await writeFile(path, result);

      const final = await Bun.file(path).text();
      expect(final).not.toContain("handoff-plan");
      expect(final).toContain("maestro handoff-pickup");
      expect(hasBlock(final)).toBe(true);
      expect(final).toContain("## Other Section");
    });
  });

  describe("remove flow", () => {
    it("removes block from file", async () => {
      const path = join(tmpDir, "CLAUDE.md");
      const content = `# Config\n\n${wrapBlock(rendered)}\n\n## Other\n`;
      await writeFile(path, content);

      const existing = await Bun.file(path).text();
      const result = removeBlock(existing);
      expect(result).not.toBeNull();
      await writeFile(path, result!);

      const final = await Bun.file(path).text();
      expect(hasBlock(final)).toBe(false);
      expect(final).toContain("# Config");
      expect(final).toContain("## Other");
    });

    it("reports not-found when no block", () => {
      const content = "# Config\n\nNo maestro here.\n";
      expect(hasBlock(content)).toBe(false);
      expect(removeBlock(content)).toBeNull();
    });
  });

  describe("template rendering", () => {
    it("renders agent flag into template", () => {
      const claude = renderTemplate(AGENT_INSTRUCTION_BLOCK, { agent: "claude" });
      expect(claude).toContain("--agent claude");

      const codex = renderTemplate(AGENT_INSTRUCTION_BLOCK, { agent: "codex" });
      expect(codex).toContain("--agent codex");

      const gemini = renderTemplate(AGENT_INSTRUCTION_BLOCK, { agent: "gemini" });
      expect(gemini).toContain("--agent gemini");
    });
  });

  describe("droid project-local anchoring", () => {
    it("injects the droid block into project-local .maestro/AGENTS.md", async () => {
      const path = join(tmpDir, ".maestro", "AGENTS.md");
      await mkdir(join(tmpDir, ".maestro"), { recursive: true });
      await writeFile(path, "# Project config\n");

      const results = await injectAgentBlocks(tmpDir);
      const droid = results.find((result) => result.agent === "Droid CLI");

      expect(droid).toBeDefined();
      expect(droid?.action).toBe("injected");
      expect(droid?.configPath).toBe(path);
      expect(await readFile(path, "utf8")).toContain("maestro handoff-pickup --claim --agent droid");
    });

    it("migrates legacy project .factory/AGENTS.md into project-local .maestro/AGENTS.md", async () => {
      const legacyPath = join(tmpDir, ".factory", "AGENTS.md");
      const targetPath = join(tmpDir, ".maestro", "AGENTS.md");
      await mkdir(join(tmpDir, ".factory"), { recursive: true });
      await writeFile(
        legacyPath,
        "# Legacy config\n\n## Cross-Agent Handoff (maestro)\n\nOld stale commands here.\nmaestro handoff-plan --to droid\n",
      );

      const results = await injectAgentBlocks(tmpDir);
      const droid = results.find((result) => result.agent === "Droid CLI");

      expect(droid).toBeDefined();
      expect(droid?.action).toBe("migrated");
      expect(droid?.configPath).toBe(targetPath);
      expect(await readFile(targetPath, "utf8")).toContain("maestro handoff-pickup --claim --agent droid");
      expect(await readFile(targetPath, "utf8")).not.toContain("handoff-plan");
    });

    it("removes the droid block from project-local .maestro/AGENTS.md", async () => {
      const path = join(tmpDir, ".maestro", "AGENTS.md");
      await mkdir(join(tmpDir, ".maestro"), { recursive: true });
      await writeFile(path, wrapBlock(renderTemplate(AGENT_INSTRUCTION_BLOCK, { agent: "droid" })));

      const results = await removeAgentBlocks(tmpDir);
      const droid = results.find((result) => result.agent === "Droid CLI");

      expect(droid).toBeDefined();
      expect(droid?.action).toBe("removed");
      expect(droid?.configPath).toBe(path);
      expect(await readFile(path, "utf8")).not.toContain("maestro handoff-pickup --claim --agent droid");
    });
  });
});
