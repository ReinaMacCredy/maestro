import { describe, expect, it } from "bun:test";
import { shouldCleanupStaleWindowsBinary } from "@/index.js";

describe("startup cleanup", () => {
  it("only cleans the configured installed Windows binary path", () => {
    const previousInstallDir = process.env.MAESTRO_INSTALL_DIR;
    process.env.MAESTRO_INSTALL_DIR = "C:\\Users\\u\\bin";

    try {
      expect(shouldCleanupStaleWindowsBinary("win32", "C:\\Users\\u\\bin\\maestro.exe")).toBe(true);
      expect(shouldCleanupStaleWindowsBinary("win32", "C:\\temp\\maestro.exe")).toBe(false);
      expect(shouldCleanupStaleWindowsBinary("win32", "C:\\Users\\u\\bin\\bun.exe")).toBe(false);
      expect(shouldCleanupStaleWindowsBinary("darwin", "/usr/local/bin/maestro")).toBe(false);
    } finally {
      if (previousInstallDir === undefined) {
        delete process.env.MAESTRO_INSTALL_DIR;
      } else {
        process.env.MAESTRO_INSTALL_DIR = previousInstallDir;
      }
    }
  });
});
