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

  it("ignores stale binary cleanup delete failures", async () => {
    const mod = await import("@/index.js").catch(() => ({}));
    const cleanupStaleWindowsBinary = (
      mod as {
        cleanupStaleWindowsBinary?: (
          platform?: NodeJS.Platform,
          execPath?: string,
          removeIfExistsImpl?: (path: string) => Promise<boolean>,
        ) => Promise<void>;
      }
    ).cleanupStaleWindowsBinary;
    expect(typeof cleanupStaleWindowsBinary).toBe("function");
    if (!cleanupStaleWindowsBinary) return;

    await expect(cleanupStaleWindowsBinary(
      "win32",
      "C:\\Users\\u\\bin\\maestro.exe",
      async () => {
        throw Object.assign(new Error("locked"), { code: "EPERM" });
      },
    )).resolves.toBeUndefined();
  });
});
