import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  ArchitectureRulesNotFoundError,
  ArchitectureRulesParseError,
} from "@/v2/repo/architecture-rules.port.js";
import {
  YamlArchitectureRules,
  parseArchitectureRules,
} from "@/v2/repo/yaml-architecture-rules.adapter.js";

const VALID_YAML = `version: 1
forward_only: true
layers:
  - types
  - config
  - repo
  - service
  - runtime
  - ui
cross_cutting:
  - providers
lint_scope:
  - "src/v2/**/*.ts"
passive_harness:
  forbidden_patterns:
    - setInterval
    - setTimeout
    - cron
    - daemon
`;

describe("parseArchitectureRules", () => {
  it("round-trips a valid rules document", () => {
    const rules = parseArchitectureRules(VALID_YAML, "docs/architecture.yaml");
    expect(rules.version).toBe(1);
    expect(rules.forward_only).toBe(true);
    expect(rules.layers).toEqual(["types", "config", "repo", "service", "runtime", "ui"]);
    expect(rules.cross_cutting).toEqual(["providers"]);
    expect(rules.lint_scope).toEqual(["src/v2/**/*.ts"]);
    expect(rules.passive_harness.forbidden_patterns).toEqual([
      "setInterval",
      "setTimeout",
      "cron",
      "daemon",
    ]);
  });

  it("defaults cross_cutting, lint_scope, and forbidden_patterns to empty arrays when omitted", () => {
    const minimal = `version: 1
forward_only: false
layers:
  - types
`;
    const rules = parseArchitectureRules(minimal, "minimal.yaml");
    expect(rules.cross_cutting).toEqual([]);
    expect(rules.lint_scope).toEqual([]);
    expect(rules.passive_harness.forbidden_patterns).toEqual([]);
  });

  it("rejects missing version", () => {
    const yaml = `forward_only: true
layers: [a]
`;
    expect(() => parseArchitectureRules(yaml, "x.yaml")).toThrow(ArchitectureRulesParseError);
  });

  it("rejects non-1 version", () => {
    const yaml = `version: 2
forward_only: true
layers: [a]
`;
    expect(() => parseArchitectureRules(yaml, "x.yaml")).toThrow(/requires version: 1/);
  });

  it("rejects non-boolean forward_only", () => {
    const yaml = `version: 1
forward_only: "yes"
layers: [a]
`;
    expect(() => parseArchitectureRules(yaml, "x.yaml")).toThrow(/forward_only/);
  });

  it("rejects empty layers array", () => {
    const yaml = `version: 1
forward_only: true
layers: []
`;
    expect(() => parseArchitectureRules(yaml, "x.yaml")).toThrow(/layers/);
  });

  it("rejects non-array cross_cutting", () => {
    const yaml = `version: 1
forward_only: true
layers: [a]
cross_cutting: "providers"
`;
    expect(() => parseArchitectureRules(yaml, "x.yaml")).toThrow(/cross_cutting/);
  });

  it("rejects non-array lint_scope", () => {
    const yaml = `version: 1
forward_only: true
layers: [a]
lint_scope: "src/**"
`;
    expect(() => parseArchitectureRules(yaml, "x.yaml")).toThrow(/lint_scope/);
  });

  it("rejects non-array passive_harness.forbidden_patterns", () => {
    const yaml = `version: 1
forward_only: true
layers: [a]
passive_harness:
  forbidden_patterns: "setInterval"
`;
    expect(() => parseArchitectureRules(yaml, "x.yaml")).toThrow(/forbidden_patterns/);
  });

  it("rejects invalid YAML", () => {
    expect(() => parseArchitectureRules("version: 1\n  : invalid", "x.yaml")).toThrow(
      ArchitectureRulesParseError,
    );
  });

  it("rejects a non-mapping root", () => {
    expect(() => parseArchitectureRules("- 1\n- 2\n", "x.yaml")).toThrow(/must be a YAML mapping/);
  });
});

describe("YamlArchitectureRules", () => {
  let tmpDir: string;

  beforeEach(async () => {
    tmpDir = await mkdtemp(join(tmpdir(), "maestro-arch-rules-"));
  });

  afterEach(async () => {
    await rm(tmpDir, { recursive: true, force: true });
  });

  it("loads rules from docs/architecture.yaml by default", async () => {
    await mkdir(join(tmpDir, "docs"), { recursive: true });
    await writeFile(join(tmpDir, "docs/architecture.yaml"), VALID_YAML, "utf8");

    const port = new YamlArchitectureRules({ repoRoot: tmpDir });
    const rules = await port.load();
    expect(rules.layers).toContain("repo");
  });

  it("loads from a custom relPath", async () => {
    await mkdir(join(tmpDir, "config"), { recursive: true });
    await writeFile(join(tmpDir, "config/arch.yaml"), VALID_YAML, "utf8");

    const port = new YamlArchitectureRules({ repoRoot: tmpDir, relPath: "config/arch.yaml" });
    const rules = await port.load();
    expect(rules.forward_only).toBe(true);
  });

  it("throws ArchitectureRulesNotFoundError when the file is missing", async () => {
    const port = new YamlArchitectureRules({ repoRoot: tmpDir });
    await expect(port.load()).rejects.toThrow(ArchitectureRulesNotFoundError);
  });

  it("propagates parse errors with the absolute path in the message", async () => {
    await mkdir(join(tmpDir, "docs"), { recursive: true });
    await writeFile(join(tmpDir, "docs/architecture.yaml"), "version: 2\n", "utf8");

    const port = new YamlArchitectureRules({ repoRoot: tmpDir });
    await expect(port.load()).rejects.toThrow(/architecture\.yaml/);
  });
});
