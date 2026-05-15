import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp, mkdir, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  FsPrinciplesStore,
  parsePrincipleFile,
} from "@/v2/repo/fs-principles-store.adapter.js";
import { PrincipleParseError } from "@/v2/types/principle.js";

const VALID = `# slug-here

## Rule

Use the helper.

## Rationale

Because duplication drifts.

## Scan Command

rg -n "duplicate"

## Fix Recipe

Move it into shared.
`;

describe("parsePrincipleFile", () => {
  it("parses every required section", () => {
    const p = parsePrincipleFile(VALID, "prefer-shared-utils", "x.md");
    expect(p.slug).toBe("prefer-shared-utils");
    expect(p.rule).toBe("Use the helper.");
    expect(p.rationale).toBe("Because duplication drifts.");
    expect(p.scan_command).toBe('rg -n "duplicate"');
    expect(p.fix_recipe).toBe("Move it into shared.");
  });

  it("throws when a required section is missing", () => {
    const missing = `# x\n\n## Rule\n\nx\n\n## Rationale\n\ny\n`;
    expect(() => parsePrincipleFile(missing, "x", "x.md")).toThrow(PrincipleParseError);
  });

  it("ignores unknown headings", () => {
    const extra = `${VALID}\n## Notes\n\nignored\n`;
    const p = parsePrincipleFile(extra, "x", "x.md");
    expect(p.rule).toBe("Use the helper.");
  });
});

describe("FsPrinciplesStore", () => {
  let dir: string;
  let store: FsPrinciplesStore;

  beforeEach(async () => {
    dir = await mkdtemp(join(tmpdir(), "principles-"));
    store = new FsPrinciplesStore({ repoRoot: dir });
  });

  afterEach(async () => {
    await rm(dir, { recursive: true, force: true });
  });

  it("returns empty list when docs/principles is missing", async () => {
    expect(await store.list()).toEqual([]);
  });

  it("get() returns undefined when slug not present", async () => {
    expect(await store.get("nope")).toBeUndefined();
  });

  it("exists() returns true even when file is unparseable", async () => {
    const principlesDir = join(dir, "docs/principles");
    await mkdir(principlesDir, { recursive: true });
    await writeFile(join(principlesDir, "stub.md"), "not a principle", "utf8");
    expect(await store.exists("stub")).toBe(true);
    expect(await store.exists("missing")).toBe(false);
  });

  it("exists() rejects invalid slug without touching disk", async () => {
    expect(await store.exists("Bad_Slug!")).toBe(false);
  });

  it("round-trips write/get", async () => {
    await store.write("rule-one", VALID);
    const p = await store.get("rule-one");
    expect(p?.slug).toBe("rule-one");
    expect(p?.rule).toBe("Use the helper.");
  });

  it("list() reads all valid principles", async () => {
    await store.write("rule-one", VALID);
    await store.write("rule-two", VALID);
    const all = await store.list();
    expect(all.map((p) => p.slug).sort()).toEqual(["rule-one", "rule-two"]);
  });

  it("list() excludes legacy/ subdirectory", async () => {
    await store.write("real-rule", VALID);
    const principlesDir = join(dir, "docs/principles");
    await mkdir(join(principlesDir, "legacy"), { recursive: true });
    await writeFile(join(principlesDir, "legacy", "old-correction.md"), VALID, "utf8");
    const all = await store.list();
    expect(all.map((p) => p.slug)).toEqual(["real-rule"]);
  });

  it("write() rejects invalid slug", async () => {
    await expect(store.write("Bad_Slug!", VALID)).rejects.toThrow(PrincipleParseError);
  });

  it("get() throws PrincipleParseError on malformed file", async () => {
    const principlesDir = join(dir, "docs/principles");
    await mkdir(principlesDir, { recursive: true });
    await writeFile(join(principlesDir, "broken.md"), "no sections", "utf8");
    await expect(store.get("broken")).rejects.toThrow(PrincipleParseError);
  });
});
