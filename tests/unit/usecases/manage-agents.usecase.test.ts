import { describe, expect, it, beforeEach, afterEach } from "bun:test";
import { mkdtemp, rm, mkdir, writeFile, readFile } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir, homedir } from "node:os";

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

describe("manage-agents use case logic", () => {
  let tmpDir: string;

  beforeEach(async () => {
    tmpDir = await mkdtemp(join(tmpdir(), "maestro-agents-"));
  });

  afterEach(async () => {
    await rm(tmpDir, { recursive: true, force: true });
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
});
