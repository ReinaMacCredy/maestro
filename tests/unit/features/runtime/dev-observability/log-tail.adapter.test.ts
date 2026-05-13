import { describe, it, expect } from "bun:test";
import { writeFile, mkdtemp, rm } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { LogTailAdapter } from "@/features/runtime/index.js";

async function withTmpFile(contents: string, fn: (path: string) => Promise<void>): Promise<void> {
  const dir = await mkdtemp(join(tmpdir(), "log-tail-"));
  const path = join(dir, "app.log");
  await writeFile(path, contents, "utf8");
  try {
    await fn(path);
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
}

describe("LogTailAdapter", () => {
  it("returns trailing lines bounded by lines parameter", async () => {
    await withTmpFile("a\nb\nc\nd\n", async (path) => {
      const tail = await new LogTailAdapter(path).tailLogs(undefined, 2);
      expect(tail.lines.map((l) => l.text)).toEqual(["c", "d"]);
      expect(tail.source).toBe(`file:${path}`);
    });
  });

  it("applies substring filter before bounding by lines", async () => {
    await withTmpFile("info: a\nerror: b\ninfo: c\nerror: d\n", async (path) => {
      const tail = await new LogTailAdapter(path).tailLogs("error", 5);
      expect(tail.lines.map((l) => l.text)).toEqual(["error: b", "error: d"]);
    });
  });

  it("defaults the line cap when none is passed", async () => {
    const big = Array.from({ length: 250 }, (_, i) => `line${i}`).join("\n");
    await withTmpFile(big, async (path) => {
      const tail = await new LogTailAdapter(path).tailLogs(undefined);
      expect(tail.lines.length).toBe(100);
      expect(tail.lines.at(-1)?.text).toBe("line249");
    });
  });

  it("reads the file path from MAESTRO_DEV_LOG_FILE when no arg is given", async () => {
    await withTmpFile("x\ny\n", async (path) => {
      const adapter = new LogTailAdapter(undefined, { MAESTRO_DEV_LOG_FILE: path });
      const tail = await adapter.tailLogs(undefined, 1);
      expect(tail.lines.map((l) => l.text)).toEqual(["y"]);
    });
  });

  it("throws when no path is resolvable", () => {
    expect(() => new LogTailAdapter(undefined, {})).toThrow(/MAESTRO_DEV_LOG_FILE/);
  });

  it("rejects queryMetric to surface adapter scope", async () => {
    await withTmpFile("a\n", async (path) => {
      const adapter = new LogTailAdapter(path);
      await expect(adapter.queryMetric()).rejects.toThrow(/does not support queryMetric/);
    });
  });
});
