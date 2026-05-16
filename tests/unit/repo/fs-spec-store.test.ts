import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp, readFile, rm, writeFile, mkdir } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  FsSpecStore,
  parseSpecFile,
  serializeSpec,
} from "@/repo/fs-spec-store.adapter.js";
import {
  SpecAlreadyExistsError,
  SpecNotFoundError,
  SpecParseError,
} from "@/repo/spec-store.port.js";
import type { ProductSpec } from "@/types/product-spec.js";

const VALID_FRONTMATTER = `---
slug: improve-handoff-pickup-error
acceptance_criteria:
  - "Pickup of a missing handoff returns a recoverable error"
non_goals:
  - "Migrating existing handoffs"
risk_class: medium
mode: light
work_type: change-request
---
# Improve handoff pickup error path
context goes here
`;

describe("parseSpecFile", () => {
  it("round-trips a valid spec", () => {
    const spec = parseSpecFile(VALID_FRONTMATTER, "test.md");
    expect(spec.frontmatter.slug).toBe("improve-handoff-pickup-error");
    expect(spec.frontmatter.acceptance_criteria).toHaveLength(1);
    expect(spec.frontmatter.non_goals).toEqual(["Migrating existing handoffs"]);
    expect(spec.frontmatter.risk_class).toBe("medium");
    expect(spec.frontmatter.mode).toBe("light");
    expect(spec.frontmatter.work_type).toBe("change-request");
    expect(spec.body).toContain("# Improve handoff pickup error path");
  });

  it("throws SpecParseError on missing leading delimiter", () => {
    expect(() => parseSpecFile("no frontmatter\n# body", "f.md")).toThrow(SpecParseError);
  });

  it("throws SpecParseError on missing closing delimiter", () => {
    expect(() => parseSpecFile("---\nslug: x\n", "f.md")).toThrow(SpecParseError);
  });

  it("throws SpecParseError naming missing required fields", () => {
    const missingAcceptance = `---\nslug: abc-spec\nrisk_class: low\nmode: light\nwork_type: maintenance\n---\nbody`;
    let caught: unknown;
    try {
      parseSpecFile(missingAcceptance, "f.md");
    } catch (e) {
      caught = e;
    }
    expect(caught).toBeInstanceOf(SpecParseError);
    expect((caught as SpecParseError).field).toBe("acceptance_criteria");
  });

  it("throws on invalid risk_class value", () => {
    const bad = `---\nslug: abc-spec\nacceptance_criteria:\n  - one\nrisk_class: urgent\nmode: light\nwork_type: maintenance\n---\nbody`;
    let caught: unknown;
    try {
      parseSpecFile(bad, "f.md");
    } catch (e) {
      caught = e;
    }
    expect((caught as SpecParseError).field).toBe("risk_class");
  });

  it("throws on invalid slug", () => {
    const bad = `---\nslug: BadSlug\nacceptance_criteria:\n  - one\nrisk_class: low\nmode: light\nwork_type: maintenance\n---\nbody`;
    let caught: unknown;
    try {
      parseSpecFile(bad, "f.md");
    } catch (e) {
      caught = e;
    }
    expect((caught as SpecParseError).field).toBe("slug");
  });
});

describe("serializeSpec + parseSpecFile round-trip", () => {
  it("preserves all fields", () => {
    const spec: ProductSpec = {
      frontmatter: {
        slug: "alpha-bravo",
        acceptance_criteria: ["one", "two"],
        non_goals: ["non-one"],
        risk_class: "high",
        mode: "heavy",
        work_type: "initiative",
      },
      body: "# Alpha Bravo\n\nbody text",
      path: "test.md",
    };
    const text = serializeSpec(spec);
    const parsed = parseSpecFile(text, "test.md");
    expect(parsed.frontmatter).toEqual(spec.frontmatter);
    expect(parsed.body.trim()).toBe(spec.body.trim());
  });
});

describe("FsSpecStore", () => {
  let root: string;

  beforeEach(async () => {
    root = await mkdtemp(join(tmpdir(), "v2-spec-store-"));
  });

  afterEach(async () => {
    await rm(root, { recursive: true, force: true });
  });

  it("list returns empty when specs dir does not exist yet", async () => {
    const store = new FsSpecStore({ repoRoot: root });
    expect(await store.list()).toEqual([]);
  });

  it("write then read round-trips", async () => {
    const store = new FsSpecStore({ repoRoot: root });
    const spec: ProductSpec = {
      frontmatter: {
        slug: "alpha-bravo",
        acceptance_criteria: ["one"],
        non_goals: [],
        risk_class: "low",
        mode: "light",
        work_type: "maintenance",
      },
      body: "body",
      path: "irrelevant",
    };
    await store.write(spec);
    const fetched = await store.read("alpha-bravo");
    expect(fetched.frontmatter).toEqual(spec.frontmatter);
  });

  it("read throws SpecNotFoundError for missing slug", async () => {
    const store = new FsSpecStore({ repoRoot: root });
    let caught: unknown;
    try {
      await store.read("missing");
    } catch (e) {
      caught = e;
    }
    expect(caught).toBeInstanceOf(SpecNotFoundError);
  });

  it("create refuses to overwrite", async () => {
    const store = new FsSpecStore({ repoRoot: root });
    const spec: ProductSpec = {
      frontmatter: {
        slug: "dup-spec",
        acceptance_criteria: ["one"],
        non_goals: [],
        risk_class: "low",
        mode: "light",
        work_type: "maintenance",
      },
      body: "first",
      path: "irrelevant",
    };
    await store.create(spec);
    let caught: unknown;
    try {
      await store.create(spec);
    } catch (e) {
      caught = e;
    }
    expect(caught).toBeInstanceOf(SpecAlreadyExistsError);
  });

  it("list returns valid slugs only, ignoring non-.md files", async () => {
    const store = new FsSpecStore({ repoRoot: root });
    const spec: ProductSpec = {
      frontmatter: {
        slug: "valid-spec",
        acceptance_criteria: ["one"],
        non_goals: [],
        risk_class: "low",
        mode: "light",
        work_type: "maintenance",
      },
      body: "body",
      path: "irrelevant",
    };
    await store.write(spec);
    await mkdir(join(root, ".maestro/specs"), { recursive: true });
    await writeFile(join(root, ".maestro/specs/README.txt"), "junk");
    const list = await store.list();
    expect(list).toEqual(["valid-spec"]);
  });

  it("writes a file content that round-trips parse on disk", async () => {
    const store = new FsSpecStore({ repoRoot: root });
    const spec: ProductSpec = {
      frontmatter: {
        slug: "disk-trip",
        acceptance_criteria: ["one"],
        non_goals: [],
        risk_class: "medium",
        mode: "light",
        work_type: "change-request",
      },
      body: "# Hello",
      path: "irrelevant",
    };
    await store.write(spec);
    const raw = await readFile(join(root, ".maestro/specs/disk-trip.md"), "utf8");
    expect(raw.startsWith("---\n")).toBe(true);
    expect(raw).toContain("slug: disk-trip");
  });
});
