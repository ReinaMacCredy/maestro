import { describe, it, expect } from "bun:test";
import { readJson, writeJson } from "@/shared/lib/fs.js";
import { parseYaml, stringifyYaml } from "@/shared/lib/yaml.js";
import { mapWithConcurrency } from "@/shared/lib/concurrency.js";
import { output } from "@/shared/lib/output.js";
import { paginate } from "@/features/mcp/server/pagination.js";
import { ok, fail } from "@/features/mcp/server/errors.js";
import { cached, makeEntry } from "@/tui/state/snapshot-poll-cache.js";
import { withFileLock } from "@/shared/lib/fs-lock.js";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

describe("Generic type parameter constraints", () => {
  describe("readJson and writeJson", () => {
    it("handles valid JSON types", async () => {
      const tmpDir = await mkdtemp(join(tmpdir(), "maestro-test-"));
      try {
        const testPath = join(tmpDir, "test.json");
        
        // Should work with objects
        await writeJson(testPath, { foo: "bar", num: 42 });
        const obj = await readJson<{ foo: string; num: number }>(testPath);
        expect(obj).toEqual({ foo: "bar", num: 42 });
        
        // Should work with arrays
        await writeJson(testPath, [1, 2, 3]);
        const arr = await readJson<number[]>(testPath);
        expect(arr).toEqual([1, 2, 3]);
        
        // Should work with primitives
        await writeJson(testPath, "string");
        const str = await readJson<string>(testPath);
        expect(str).toBe("string");
      } finally {
        await rm(tmpDir, { recursive: true, force: true });
      }
    });
  });

  describe("parseYaml and stringifyYaml", () => {
    it("handles valid YAML types", () => {
      // Should work with objects
      const obj = parseYaml<{ foo: string }>("foo: bar");
      expect(obj).toEqual({ foo: "bar" });
      
      // Should work with arrays
      const arr = parseYaml<string[]>("- a\n- b\n- c");
      expect(arr).toEqual(["a", "b", "c"]);
      
      // Should roundtrip
      const data = { nested: { value: 42 } };
      const yaml = stringifyYaml(data);
      const parsed = parseYaml<typeof data>(yaml);
      expect(parsed).toEqual(data);
    });
  });

  describe("mapWithConcurrency", () => {
    it("transforms items with proper types", async () => {
      const numbers = [1, 2, 3, 4, 5];
      const strings = await mapWithConcurrency(numbers, 2, async (n) => `num-${n}`);
      expect(strings).toEqual(["num-1", "num-2", "num-3", "num-4", "num-5"]);
    });
  });

  describe("output", () => {
    it("formats any type with a formatter function", () => {
      const data = { id: "test", value: 42 };
      const formatter = (d: typeof data) => [`ID: ${d.id}`, `Value: ${d.value}`];
      
      // Should not throw
      expect(() => output(false, data, formatter)).not.toThrow();
    });
  });

  describe("paginate", () => {
    it("paginates any array type", () => {
      const items = [1, 2, 3, 4, 5];
      const page = paginate(items, 2, 0);
      expect(page.items).toEqual([1, 2]);
      expect(page.pagination.total).toBe(5);
    });
  });

  describe("MCP result types", () => {
    it("wraps any success type", () => {
      const result = ok({ id: "test", value: 42 });
      expect(result.ok).toBe(true);
      if (result.ok) {
        expect(result.data).toEqual({ id: "test", value: 42 });
      }
    });

    it("wraps failure with error", () => {
      const result = fail("TEST_ERROR", "Test failed", ["Try again"]);
      expect(result.ok).toBe(false);
      if (!result.ok) {
        expect(result.error.code).toBe("TEST_ERROR");
      }
    });
  });

  describe("cache helpers", () => {
    it("caches any type", () => {
      const entry = makeEntry({ value: 42 }, 1000);
      expect(entry.value).toEqual({ value: 42 });
      expect(entry.expiresAt).toBeGreaterThan(Date.now());
      
      const retrieved = cached(entry);
      expect(retrieved).toEqual({ value: 42 });
    });
  });

  describe("withFileLock", () => {
    it("returns any type from callback", async () => {
      const tmpDir = await mkdtemp(join(tmpdir(), "maestro-test-"));
      try {
        const lockPath = join(tmpDir, "test.lock");
        
        const result = await withFileLock(
          {
            lockPath,
            staleMs: 5000,
            timeoutMs: 1000,
            initialRetryDelayMs: 10,
            maxRetryDelayMs: 100,
            timeoutMessage: "Lock timeout",
            timeoutHints: [],
          },
          async () => ({ computed: "value", num: 42 }),
        );
        
        expect(result).toEqual({ computed: "value", num: 42 });
      } finally {
        await rm(tmpDir, { recursive: true, force: true });
      }
    });
  });
});
