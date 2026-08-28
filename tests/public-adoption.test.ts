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
