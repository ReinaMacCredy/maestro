import { describe, expect, it } from "bun:test";
import { shouldCleanupStaleWindowsBinary } from "@/index.js";

describe("startup cleanup", () => {
  it("only cleans stale binaries for compiled Windows maestro executables", () => {
    expect(shouldCleanupStaleWindowsBinary("win32", "C:\\tools\\maestro.exe")).toBe(true);
    expect(shouldCleanupStaleWindowsBinary("win32", "C:\\tools\\bun.exe")).toBe(false);
    expect(shouldCleanupStaleWindowsBinary("darwin", "/usr/local/bin/maestro")).toBe(false);
  });
});
