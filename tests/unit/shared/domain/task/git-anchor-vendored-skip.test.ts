import { describe, expect, it } from "bun:test";
import { isVendoredOrBuildPath } from "@/shared/domain/task/adapters/git-anchor.adapter.js";

describe("isVendoredOrBuildPath", () => {
  it("skips repo-root vendored prefixes", () => {
    expect(isVendoredOrBuildPath("node_modules/foo/index.js")).toBe(true);
    expect(isVendoredOrBuildPath("dist/bundle.js")).toBe(true);
    expect(isVendoredOrBuildPath("vendor/github.com/x/y.go")).toBe(true);
    expect(isVendoredOrBuildPath(".next/build-manifest.json")).toBe(true);
    expect(isVendoredOrBuildPath("target/debug/foo")).toBe(true);
  });

  it("does NOT skip nested look-alike paths planted as exfil lanes", () => {
    // Previous substring-match form skipped any path containing `/node_modules/`
    // anywhere — an attacker could plant a real secret at
    // `tests/fixtures/node_modules/leak.env` to slip past the scanner.
    // Anchoring to repo root closes that lane.
    expect(isVendoredOrBuildPath("tests/fixtures/node_modules/leak.env")).toBe(false);
    expect(isVendoredOrBuildPath("docs/examples/dist/snapshot.txt")).toBe(false);
    expect(isVendoredOrBuildPath("packages/foo/vendor/credentials.json")).toBe(false);
    expect(isVendoredOrBuildPath("src/__pycache__/cache.bin")).toBe(false);
  });

  it("does not skip regular source paths", () => {
    expect(isVendoredOrBuildPath("src/index.ts")).toBe(false);
    expect(isVendoredOrBuildPath("README.md")).toBe(false);
  });

  it("normalizes Windows separators", () => {
    expect(isVendoredOrBuildPath("node_modules\\foo\\index.js")).toBe(true);
    expect(isVendoredOrBuildPath("tests\\fixtures\\node_modules\\leak.env")).toBe(false);
  });
});
