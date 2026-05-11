import { describe, it, expect } from "bun:test";
import { assertNoDeprecatedVersionFlag } from "@/infra/lib/deprecated-version-flag.js";
import { MaestroError } from "@/shared/errors.js";

const argv = (...rest: string[]): readonly string[] => ["bun", "maestro", ...rest];

describe("assertNoDeprecatedVersionFlag", () => {
  it("throws on `contract show --task <id> --version <n>` (interleaved options)", () => {
    let caught: unknown;
    try {
      assertNoDeprecatedVersionFlag(argv("contract", "show", "--task", "tsk-abc", "--version", "1"));
    } catch (err) {
      caught = err;
    }
    expect(caught).toBeInstanceOf(MaestroError);
    expect(String((caught as Error).message)).toContain("contract show --version <id>");
  });

  it("throws on `verdict show --task <id> --version <n>` (interleaved options)", () => {
    let caught: unknown;
    try {
      assertNoDeprecatedVersionFlag(argv("verdict", "show", "--task", "tsk-abc", "--version", "v-1"));
    } catch (err) {
      caught = err;
    }
    expect(caught).toBeInstanceOf(MaestroError);
    expect(String((caught as Error).message)).toContain("verdict show --version <id>");
  });

  it("throws on `update --version <release>` (no preceding options)", () => {
    let caught: unknown;
    try {
      assertNoDeprecatedVersionFlag(argv("update", "--version", "0.72.0"));
    } catch (err) {
      caught = err;
    }
    expect(caught).toBeInstanceOf(MaestroError);
    expect(String((caught as Error).message)).toContain("update --version <release>");
  });

  it("does not throw on bare `maestro --version` (root version flag)", () => {
    expect(() => assertNoDeprecatedVersionFlag(argv("--version"))).not.toThrow();
  });

  it("does not throw when `--version` is followed by another flag (no value)", () => {
    expect(() =>
      assertNoDeprecatedVersionFlag(argv("contract", "show", "--version", "--task", "tsk-abc")),
    ).not.toThrow();
  });

  it("throws on any other subcommand prefix + `--version <value>` (catch-all)", () => {
    // No subcommand declares `--version` as an option (verified at v0.72.20),
    // so any verb-prefix + `--version <value>` is a trap that would otherwise
    // silently hit Commander's root handler.
    let caught: unknown;
    try {
      assertNoDeprecatedVersionFlag(argv("task", "update", "--task", "tsk-abc", "--version", "1"));
    } catch (err) {
      caught = err;
    }
    expect(caught).toBeInstanceOf(MaestroError);
    expect(String((caught as Error).message)).toContain("task update --version <value>");
  });

  it("handles `--foo=bar` interleaved option syntax", () => {
    let caught: unknown;
    try {
      assertNoDeprecatedVersionFlag(argv("contract", "show", "--task=tsk-abc", "--version", "1"));
    } catch (err) {
      caught = err;
    }
    expect(caught).toBeInstanceOf(MaestroError);
  });

  it("does not throw when `--version` appears mid-subcommand options without a value", () => {
    expect(() =>
      assertNoDeprecatedVersionFlag(argv("contract", "show", "--task", "tsk-abc", "--version")),
    ).not.toThrow();
  });

  it("throws on `task contract show --task <id> --version <n>` (L1 viewer doesn't take this flag)", () => {
    let caught: unknown;
    try {
      assertNoDeprecatedVersionFlag(
        argv("task", "contract", "show", "--task", "tsk-abc", "--version", "1"),
      );
    } catch (err) {
      caught = err;
    }
    expect(caught).toBeInstanceOf(MaestroError);
    expect(String((caught as Error).message)).toContain(
      "is not a flag on the L1 contract viewer",
    );
    // The redirect should point at the L2 verb with the new flag and value preserved.
    const hints = (caught as { hints?: readonly string[] }).hints;
    expect(hints?.some((h) => h.includes("contract show --task <id> --at-version 1"))).toBe(true);
  });
});
