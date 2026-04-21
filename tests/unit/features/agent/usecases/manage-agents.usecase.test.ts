import { describe, expect, it, beforeEach, afterEach } from "bun:test";
import { mkdtemp, rm, mkdir, writeFile, readFile } from "node:fs/promises";
import { existsSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";

import { AGENT_INSTRUCTION_BLOCK } from "@/infra/domain/bootstrap-templates.js";
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

describe("manage-agents use case logic", () => {
  let tmpDir: string;

  beforeEach(async () => {
    tmpDir = await mkdtemp(join(tmpdir(), "maestro-agents-"));
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

    it("injectReference on empty string", () => {
      const result = injectReference("");
      expect(result).toBe(`${REFERENCE_LINE}\n`);
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

  describe("migration helpers (block cleanup + reference)", () => {
    it("removes old block markers then injects reference", () => {
      const content = `# Config\n\n${wrapBlock("Old maestro instructions")}\n\n## Other Section\n`;
      expect(hasBlock(content)).toBe(true);

      const cleaned = removeBlock(content)!;
      expect(cleaned).not.toBeNull();
      expect(hasBlock(cleaned)).toBe(false);

      const final = injectReference(cleaned);
      expect(hasReference(final)).toBe(true);
      expect(hasBlock(final)).toBe(false);
      expect(final).toContain("## Other Section");
    });

    it("removes legacy heading then injects reference", () => {
      const content = `# Config\n\n## Cross-Agent Handoff (maestro)\n\nOld stale commands.\nmaestro handoff-plan --to codex\n\n## Other Section\n`;

      const cleaned = removeLegacyBlock(content)!;
      expect(cleaned).not.toBeNull();
      expect(cleaned).not.toContain("handoff-plan");
      expect(cleaned).not.toContain("Cross-Agent Handoff");

      const final = injectReference(cleaned);
      expect(hasReference(final)).toBe(true);
      expect(final).toContain("## Other Section");
    });
  });

  describe("droid project-local anchoring", () => {
    it("writes MAESTRO.md and adds reference to project-local .maestro/AGENTS.md", async () => {
      const maestroDir = join(tmpDir, ".maestro");
      await mkdir(maestroDir, { recursive: true });
      await writeFile(join(maestroDir, "AGENTS.md"), "# Project config\n");

      const results = await injectAgentBlocks(tmpDir);
      const droid = results.find((r) => r.agent === "Droid CLI");

      expect(droid).toBeDefined();
      expect(droid?.action).toBe("injected");

      const config = await readFile(join(maestroDir, "AGENTS.md"), "utf8");
      expect(hasReference(config)).toBe(true);
      expect(hasBlock(config)).toBe(false);

      const ref = await readFile(join(maestroDir, REFERENCE_FILE), "utf8");
      expect(ref.trimEnd()).toBe(AGENT_INSTRUCTION_BLOCK);
    });

    it("skips when MAESTRO.md and reference already match", async () => {
      const maestroDir = join(tmpDir, ".maestro");
      await mkdir(maestroDir, { recursive: true });
      await writeFile(join(maestroDir, REFERENCE_FILE), AGENT_INSTRUCTION_BLOCK + "\n");
      await writeFile(join(maestroDir, "AGENTS.md"), `# Config\n\n${REFERENCE_LINE}\n`);

      const results = await injectAgentBlocks(tmpDir);
      const droid = results.find((r) => r.agent === "Droid CLI");

      expect(droid?.action).toBe("skipped");
    });

    it("cleans stale inline blocks even when MAESTRO.md and reference already match", async () => {
      const maestroDir = join(tmpDir, ".maestro");
      await mkdir(maestroDir, { recursive: true });
      await writeFile(join(maestroDir, REFERENCE_FILE), AGENT_INSTRUCTION_BLOCK + "\n");
      await writeFile(
        join(maestroDir, "AGENTS.md"),
        `# Config\n\n${REFERENCE_LINE}\n\n${wrapBlock("Old maestro instructions")}\n`,
      );

      const results = await injectAgentBlocks(tmpDir);
      const droid = results.find((r) => r.agent === "Droid CLI");

      expect(droid?.action).toBe("migrated");

      const config = await readFile(join(maestroDir, "AGENTS.md"), "utf8");
      expect(hasReference(config)).toBe(true);
      expect(hasBlock(config)).toBe(false);
    });

    it("updates MAESTRO.md when content differs", async () => {
      const maestroDir = join(tmpDir, ".maestro");
      await mkdir(maestroDir, { recursive: true });
      await writeFile(join(maestroDir, REFERENCE_FILE), "Old instructions\n");
      await writeFile(join(maestroDir, "AGENTS.md"), `# Config\n\n${REFERENCE_LINE}\n`);

      const results = await injectAgentBlocks(tmpDir);
      const droid = results.find((r) => r.agent === "Droid CLI");

      expect(droid?.action).toBe("updated");
      const ref = await readFile(join(maestroDir, REFERENCE_FILE), "utf8");
      expect(ref.trimEnd()).toBe(AGENT_INSTRUCTION_BLOCK);
    });

    it("migrates old block markers to MAESTRO.md + reference", async () => {
      const maestroDir = join(tmpDir, ".maestro");
      await mkdir(maestroDir, { recursive: true });
      await writeFile(
        join(maestroDir, "AGENTS.md"),
        `# Config\n\n${wrapBlock("Old maestro instructions")}\n\n## Other Section\n`,
      );

      const results = await injectAgentBlocks(tmpDir);
      const droid = results.find((r) => r.agent === "Droid CLI");

      expect(droid?.action).toBe("migrated");

      const config = await readFile(join(maestroDir, "AGENTS.md"), "utf8");
      expect(hasBlock(config)).toBe(false);
      expect(hasReference(config)).toBe(true);
      expect(config).toContain("## Other Section");

      const ref = await readFile(join(maestroDir, REFERENCE_FILE), "utf8");
      expect(ref.trimEnd()).toBe(AGENT_INSTRUCTION_BLOCK);
    });

    it("migrates legacy .factory/AGENTS.md to project-local .maestro/", async () => {
      const legacyDir = join(tmpDir, ".factory");
      const targetDir = join(tmpDir, ".maestro");
      await mkdir(legacyDir, { recursive: true });
      await writeFile(
        join(legacyDir, "AGENTS.md"),
        "# Legacy\n\n## Cross-Agent Handoff (maestro)\n\nmaestro handoff-plan --to droid\n",
      );

      const results = await injectAgentBlocks(tmpDir);
      const droid = results.find((r) => r.agent === "Droid CLI");

      expect(droid).toBeDefined();
      expect(droid?.action).toBe("migrated");

      const config = await readFile(join(targetDir, "AGENTS.md"), "utf8");
      expect(hasReference(config)).toBe(true);
      expect(config).not.toContain("handoff-plan");

      const ref = await readFile(join(targetDir, REFERENCE_FILE), "utf8");
      expect(ref.trimEnd()).toBe(AGENT_INSTRUCTION_BLOCK);
    });

    it("removes MAESTRO.md and reference from project-local .maestro/AGENTS.md", async () => {
      const maestroDir = join(tmpDir, ".maestro");
      await mkdir(maestroDir, { recursive: true });
      await writeFile(join(maestroDir, "AGENTS.md"), `# Config\n\n${REFERENCE_LINE}\n`);
      await writeFile(join(maestroDir, REFERENCE_FILE), AGENT_INSTRUCTION_BLOCK + "\n");

      const results = await removeAgentBlocks(tmpDir);
      const droid = results.find((r) => r.agent === "Droid CLI");

      expect(droid).toBeDefined();
      expect(droid?.action).toBe("removed");
      expect(existsSync(join(maestroDir, REFERENCE_FILE))).toBe(false);

      const config = await readFile(join(maestroDir, "AGENTS.md"), "utf8");
      expect(hasReference(config)).toBe(false);
    });

    it("removes old block during uninstall", async () => {
      const maestroDir = join(tmpDir, ".maestro");
      await mkdir(maestroDir, { recursive: true });
      await writeFile(join(maestroDir, "AGENTS.md"), wrapBlock(AGENT_INSTRUCTION_BLOCK));

      const results = await removeAgentBlocks(tmpDir);
      const droid = results.find((r) => r.agent === "Droid CLI");

      expect(droid?.action).toBe("removed");
      const config = await readFile(join(maestroDir, "AGENTS.md"), "utf8");
      expect(hasBlock(config)).toBe(false);
    });

    it("returns not-detected when neither project config nor legacy home paths exist", async () => {
      const fakeHome = join(tmpDir, "fake-home");
      await mkdir(fakeHome, { recursive: true });

      const results = await injectAgentBlocks(tmpDir, "all", fakeHome);
      const droid = results.find((r) => r.agent === "Droid CLI");

      expect(droid).toBeDefined();
      expect(droid?.action).toBe("not-detected");
      expect(droid?.configPath).toBe(join(tmpDir, ".maestro", REFERENCE_FILE));
    });
  });
});
