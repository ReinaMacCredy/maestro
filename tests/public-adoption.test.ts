import { expect, test } from "bun:test";
import { join } from "node:path";

const root = join(import.meta.dir, "..");

async function repoFile(name: string): Promise<string> {
  const file = Bun.file(join(root, name));
  expect({ file: name, exists: await file.exists() }).toEqual({ file: name, exists: true });
  return file.text();
}

test("502 [lint] LICENSE carries the MIT grant owned by ReinaMacCredy", async () => {
  const license = await repoFile("LICENSE");
  expect(license).toContain("MIT License");
  expect(license).toContain("Copyright (c) 2026 ReinaMacCredy");
  expect(license).toContain("without restriction");
  expect(license).toContain('THE SOFTWARE IS PROVIDED "AS IS"');
});

test("503 [lint] SECURITY.md names a private report channel and never a public one", async () => {
  const security = await repoFile("SECURITY.md");
  expect(security).toContain("security/advisories/new");
  expect(security).toContain("Do not open a public issue");
  expect(security).toContain("## Supported versions");
  // The boundaries an external report is measured against must stay named.
  for (const scope of ["MAESTRO_READ_ONLY=1", "mutates: false", "scripts/install.sh"]) {
    expect(security).toContain(scope);
  }
});

test("504 [lint] CONTRIBUTING.md states the toolchain and every gate CI enforces", async () => {
  const contributing = await repoFile("CONTRIBUTING.md");
  const ci = await repoFile(".github/workflows/ci.yml");
  expect(contributing).toContain("bun test");
  expect(contributing).toContain("bunx tsc --noEmit");
  // Each named CI gate is a promise to a contributor; a renamed gate must not
  // leave the promise pointing at nothing.
  for (const gate of ci.matchAll(/name: (A\d) ([^\n]+)/g)) {
    expect(contributing).toContain(gate[1] as string);
  }
  expect(contributing).toContain("SECURITY.md");
  expect(contributing).toContain("LICENSE");
});

test("505 [lint] README points an external reader at all three governance files", async () => {
  const readme = await repoFile("README.md");
  for (const file of ["CONTRIBUTING.md", "SECURITY.md", "LICENSE"]) {
    expect(readme).toContain(file);
  }
});

test("506 the desktop data layer spawns every maestro verb under MAESTRO_READ_ONLY=1", async () => {
  // The desktop polls each configured repository once a second while agents are
  // working in it. Without this env the poll writes a session row, heartbeats
  // liveness, and loads that repository's plugins, so watching a store would
  // change it.
  const data = await repoFile("apps/desktop/src-tauri/src/data.rs");
  const spawn = data.match(/fn run_verb\([\s\S]*?\n}/)?.[0];
  expect(spawn).toBeString();
  expect(spawn).toContain('.env("MAESTRO_READ_ONLY", "1")');
});

test("516 [lint] PR CI builds every tree the repository ships, not only the root", async () => {
  const ci = await repoFile(".github/workflows/ci.yml");
  const jobs = [...ci.slice(ci.indexOf("\njobs:")).matchAll(/^ {2}([a-z][a-z-]*):$/gm)]
    .map((match) => match[1] as string);
  expect(jobs).toEqual(["verify", "desktop", "desktop-rust", "site"]);

  const desktop = ci.slice(ci.indexOf("\n  desktop:"), ci.indexOf("\n  desktop-rust:"));
  // apps/desktop's tsconfig sets types: ["bun"], resolved from the ROOT install.
  // Installing only the desktop's own dependencies fails every run with TS2688
  // before reaching desktop code, which is a gate people learn to ignore.
  expect(desktop).toContain("bun install --frozen-lockfile && cd apps/desktop");
  expect(desktop).toContain("bun run build");
  expect(desktop).toContain("bun test");

  const rust = ci.slice(ci.indexOf("\n  desktop-rust:"), ci.indexOf("\n  site:"));
  expect(rust).toContain("cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml");
  // tauri-nspanel is macOS-only, so an ubuntu check would pass on a
  // configuration nobody ships.
  expect(rust).toContain("runs-on: macos-latest");

  const site = ci.slice(ci.indexOf("\n  site:"));
  expect(site).toContain("cd site && bun run build");
  expect(site).toContain("playwright install --with-deps chromium");
});
