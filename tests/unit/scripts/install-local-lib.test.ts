import { describe, expect, it } from "bun:test";
import { readInstalledVersion } from "../../../scripts/install-local-lib";

describe("readInstalledVersion", () => {
  it("rejects binaries that print no version output", () => {
    expect(() => readInstalledVersion("/tmp/fake-bin", () => ({
      exitCode: 0,
      stdout: new Uint8Array(),
      stderr: new Uint8Array(),
    }))).toThrow("Installed binary did not print a version");
  });

  it("rejects binaries that fail version verification", () => {
    expect(() => readInstalledVersion("/tmp/fake-bin", () => ({
      exitCode: 1,
      stdout: new Uint8Array(),
      stderr: new TextEncoder().encode("boom"),
    }))).toThrow("Installed binary failed version verification");
  });
});
