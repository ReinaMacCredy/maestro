import { describe, expect, it } from "bun:test";
import { mkdir, mkdtemp, readFile, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { installRuntimeHooks } from "@/features/setup";

describe("installRuntimeHooks", () => {
  it("installs hooks into detected runtime dirs", async () => {
    const tmp = await mkdtemp(join(tmpdir(), "setup-hooks-"));
    await mkdir(join(tmp, ".claude"), { recursive: true });
    const results = await installRuntimeHooks(tmp);
    expect(results).toHaveLength(1);
    expect(results[0]?.runtime).toBe("claude-code");
    expect(results[0]?.status).toBe("installed");
    const content = await readFile(join(tmp, ".claude/settings.json"), "utf8");
    expect(content).toContain("maestro-managed: session hooks");
    expect(content).toContain('maestro session start "$TASK_ID"');
    expect(content).toContain('maestro session exit "$TASK_ID"');
  });

  it("is idempotent — re-running reports already-present", async () => {
    const tmp = await mkdtemp(join(tmpdir(), "setup-hooks-"));
    await mkdir(join(tmp, ".codex"), { recursive: true });
    await installRuntimeHooks(tmp);
    const second = await installRuntimeHooks(tmp);
    expect(second[0]?.status).toBe("already-present");
  });

  it("preserves prior settings when merging", async () => {
    const tmp = await mkdtemp(join(tmpdir(), "setup-hooks-"));
    await mkdir(join(tmp, ".cursor"), { recursive: true });
    await writeFile(join(tmp, ".cursor/settings.json"), "// pre-existing user setting\n", "utf8");
    await installRuntimeHooks(tmp);
    const content = await readFile(join(tmp, ".cursor/settings.json"), "utf8");
    expect(content).toContain("pre-existing user setting");
    expect(content).toContain("maestro-managed: session hooks");
  });

  it("returns empty when no runtimes detected", async () => {
    const tmp = await mkdtemp(join(tmpdir(), "setup-hooks-empty-"));
    const results = await installRuntimeHooks(tmp);
    expect(results).toEqual([]);
  });
});
