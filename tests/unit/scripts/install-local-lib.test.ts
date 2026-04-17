import { describe, expect, it } from "bun:test";

describe("readInstalledVersion", () => {
  it("rejects binaries that print no version output", async () => {
    const mod = await import("../../../scripts/install-local-lib").catch(() => ({}));
    const readInstalledVersion = (
      mod as {
        readInstalledVersion?: (
          bin: string,
          spawnSyncImpl?: (argv: string[], options: { stdout: "pipe"; stderr: "pipe" }) => {
            exitCode: number;
            stdout: Uint8Array;
            stderr: Uint8Array;
          },
        ) => string;
      }
    ).readInstalledVersion;
    expect(typeof readInstalledVersion).toBe("function");
    if (!readInstalledVersion) return;

    expect(() => readInstalledVersion("/tmp/fake-bin", () => ({
      exitCode: 0,
      stdout: new Uint8Array(),
      stderr: new Uint8Array(),
    }))).toThrow("Installed binary did not print a version");
  });

  it("rejects binaries that fail version verification", async () => {
    const mod = await import("../../../scripts/install-local-lib").catch(() => ({}));
    const readInstalledVersion = (
      mod as {
        readInstalledVersion?: (
          bin: string,
          spawnSyncImpl?: (argv: string[], options: { stdout: "pipe"; stderr: "pipe" }) => {
            exitCode: number;
            stdout: Uint8Array;
            stderr: Uint8Array;
          },
        ) => string;
      }
    ).readInstalledVersion;
    expect(typeof readInstalledVersion).toBe("function");
    if (!readInstalledVersion) return;

    expect(() => readInstalledVersion("/tmp/fake-bin", () => ({
      exitCode: 1,
      stdout: new Uint8Array(),
      stderr: new TextEncoder().encode("boom"),
    }))).toThrow("Installed binary failed version verification");
  });
});
