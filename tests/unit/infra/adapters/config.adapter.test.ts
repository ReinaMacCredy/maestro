import { describe, expect, it, beforeEach, afterEach } from "bun:test";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { YamlConfigAdapter } from "@/infra/adapters/config.adapter.js";
import { ensureDir } from "@/shared/lib/fs.js";

let tmpDir: string;
const config = new YamlConfigAdapter();

beforeEach(async () => {
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-test-"));
});

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true });
});

describe("YamlConfigAdapter", () => {
  describe("exists", () => {
    it("returns false when no config file exists", async () => {
      const result = await config.exists("project", tmpDir);
      expect(result).toBe(false);
    });

    it("returns true after writing config", async () => {
      await config.write("project", tmpDir, { defaultAgent: "codex" });
      const result = await config.exists("project", tmpDir);
      expect(result).toBe(true);
    });
  });

  describe("write and load", () => {
    it("round-trips config through yaml", async () => {
      const input = {
        defaultAgent: "codex" as const,
        sourceRepo: "git@github.com:example/repo.git",
      };
      await config.write("project", tmpDir, input);
      const loaded = await config.load(tmpDir);
      expect(loaded.defaultAgent).toBe("codex");
      expect(loaded.sourceRepo).toBe("git@github.com:example/repo.git");
    });

    it("creates .maestro directory for project scope", async () => {
      await config.write("project", tmpDir, {});
      const file = Bun.file(join(tmpDir, ".maestro", "config.yaml"));
      expect(await file.exists()).toBe(true);
    });
  });

  describe("load with defaults", () => {
    it("returns default config when no files exist", async () => {
      const loaded = await config.load(tmpDir);
      expect(loaded.sessionDetection?.enabled).toBe(true);
      expect(loaded.sessionDetection?.agents).toContain("claude-code");
      expect(loaded.contracts).toEqual({
        default: "prompt",
        strict: false,
        overlapPolicy: "fail",
        rebaseFallback: "best-effort",
        staleReclaimContractPolicy: "inherit",
      });
    });

    it("merges project config over defaults", async () => {
      await config.write("project", tmpDir, {
        defaultAgent: "gemini",
        sessionDetection: { enabled: false, agents: [] },
      });
      const loaded = await config.load(tmpDir);
      expect(loaded.defaultAgent).toBe("gemini");
      expect(loaded.sessionDetection?.enabled).toBe(false);
    });

    it("merges nested contracts config over defaults", async () => {
      await config.write("project", tmpDir, {
        contracts: {
          strict: true,
          overlapPolicy: "annotate",
        },
      });

      const loaded = await config.load(tmpDir);

      expect(loaded.contracts).toEqual({
        default: "prompt",
        strict: true,
        overlapPolicy: "annotate",
        rebaseFallback: "best-effort",
        staleReclaimContractPolicy: "inherit",
      });
    });

    it("throws when yaml is malformed", async () => {
      await ensureDir(join(tmpDir, ".maestro"));
      await writeFile(join(tmpDir, ".maestro", "config.yaml"), "execution: [broken");

      await expect(config.load(tmpDir)).rejects.toThrow("Cannot load Maestro config due to YAML errors");
    });
  });
});
