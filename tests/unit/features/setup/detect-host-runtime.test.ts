import { describe, expect, it } from "bun:test";
import { mkdir, mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { detectHostRuntimes } from "@/features/setup";

describe("detectHostRuntimes", () => {
  it("returns empty when no host directories exist", async () => {
    const tmp = await mkdtemp(join(tmpdir(), "setup-detect-"));
    const result = await detectHostRuntimes(tmp);
    expect(result).toEqual([]);
  });

  it("detects claude-code when .claude exists", async () => {
    const tmp = await mkdtemp(join(tmpdir(), "setup-detect-"));
    await mkdir(join(tmp, ".claude"), { recursive: true });
    const result = await detectHostRuntimes(tmp);
    expect(result).toHaveLength(1);
    expect(result[0]?.id).toBe("claude-code");
    expect(result[0]?.hooksFile.endsWith("maestro-hooks.md")).toBe(true);
  });

  it("detects all three runtimes when all dirs exist", async () => {
    const tmp = await mkdtemp(join(tmpdir(), "setup-detect-"));
    await mkdir(join(tmp, ".claude"), { recursive: true });
    await mkdir(join(tmp, ".codex"), { recursive: true });
    await mkdir(join(tmp, ".cursor"), { recursive: true });
    const result = await detectHostRuntimes(tmp);
    const ids = result.map((r) => r.id).sort();
    expect(ids).toEqual(["claude-code", "codex", "cursor"]);
  });
});
