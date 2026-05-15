import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import type {
  ArchitectureRules,
  ArchitectureRulesPort,
} from "@/v2/repo/architecture-rules.port.js";
import {
  detectLayer,
  resolveImportTargetLayer,
  runArchitectureLints,
} from "@/v2/service/architecture-lint.usecase.js";

const DEFAULT_RULES: ArchitectureRules = {
  version: 1,
  forward_only: true,
  layers: ["types", "config", "repo", "service", "runtime", "ui"],
  cross_cutting: ["providers"],
  lint_scope: ["src/v2/**/*.ts"],
  passive_harness: {
    forbidden_patterns: ["setInterval", "setTimeout", "cron"],
  },
};

function stubPort(rules: ArchitectureRules): ArchitectureRulesPort {
  return { load: async () => rules };
}

describe("detectLayer", () => {
  it("returns the directory immediately under src/v2/", () => {
    expect(detectLayer("src/v2/service/foo.ts")).toBe("service");
    expect(detectLayer("src/v2/repo/bar.adapter.ts")).toBe("repo");
    expect(detectLayer("src/v2/providers/wire.ts")).toBe("providers");
  });

  it("returns undefined for non-v2 paths", () => {
    expect(detectLayer("src/features/handoff/x.ts")).toBeUndefined();
    expect(detectLayer("tests/unit/v2/repo/y.test.ts")).toBeUndefined();
  });
});

describe("resolveImportTargetLayer", () => {
  it("matches @/v2/<layer>/ imports", () => {
    expect(resolveImportTargetLayer("@/v2/types/x.js", "src/v2/service/u.ts")).toBe("types");
    expect(resolveImportTargetLayer("@/v2/runtime/x.js", "src/v2/repo/u.ts")).toBe("runtime");
  });

  it("resolves relative imports across layers", () => {
    expect(resolveImportTargetLayer("../types/x.js", "src/v2/service/u.ts")).toBe("types");
    expect(resolveImportTargetLayer("./y.js", "src/v2/service/u.ts")).toBe("service");
  });

  it("ignores external module specifiers", () => {
    expect(resolveImportTargetLayer("node:fs/promises", "src/v2/repo/x.ts")).toBeUndefined();
    expect(resolveImportTargetLayer("yaml", "src/v2/repo/x.ts")).toBeUndefined();
    expect(resolveImportTargetLayer("@/features/x.js", "src/v2/repo/x.ts")).toBeUndefined();
  });
});

describe("runArchitectureLints", () => {
  let repoRoot: string;

  beforeEach(async () => {
    repoRoot = await mkdtemp(join(tmpdir(), "maestro-arch-lint-"));
    await mkdir(join(repoRoot, "src/v2/types"), { recursive: true });
    await mkdir(join(repoRoot, "src/v2/service"), { recursive: true });
    await mkdir(join(repoRoot, "src/v2/runtime"), { recursive: true });
    await mkdir(join(repoRoot, "src/v2/providers"), { recursive: true });
  });

  afterEach(async () => {
    await rm(repoRoot, { recursive: true, force: true });
  });

  it("returns no violations for forward-only imports", async () => {
    await writeFile(
      join(repoRoot, "src/v2/types/t.ts"),
      `export const X = 1;\n`,
    );
    await writeFile(
      join(repoRoot, "src/v2/service/s.ts"),
      `import { X } from "@/v2/types/t.js";\nexport const Y = X;\n`,
    );

    const report = await runArchitectureLints({
      repoRoot,
      rulesPort: stubPort(DEFAULT_RULES),
    });
    expect(report.violations).toEqual([]);
    expect(report.filesScanned).toBeGreaterThanOrEqual(2);
  });

  it("flags a backward import (service -> runtime)", async () => {
    await writeFile(
      join(repoRoot, "src/v2/runtime/r.ts"),
      `export const A = 1;\n`,
    );
    await writeFile(
      join(repoRoot, "src/v2/service/s.ts"),
      `import { A } from "@/v2/runtime/r.js";\nexport const B = A;\n`,
    );

    const report = await runArchitectureLints({
      repoRoot,
      rulesPort: stubPort(DEFAULT_RULES),
    });
    const layerViolations = report.violations.filter((v) => v.rule_id === "layer-order");
    expect(layerViolations).toHaveLength(1);
    expect(layerViolations[0]).toMatchObject({
      rule_id: "layer-order",
      severity: "error",
      file: "src/v2/service/s.ts",
    });
    expect(layerViolations[0].message).toContain("runtime");
  });

  it("allows providers (cross-cutting) to import from any layer", async () => {
    await writeFile(
      join(repoRoot, "src/v2/types/t.ts"),
      `export const X = 1;\n`,
    );
    await writeFile(
      join(repoRoot, "src/v2/runtime/r.ts"),
      `export const A = 1;\n`,
    );
    await writeFile(
      join(repoRoot, "src/v2/providers/wire.ts"),
      `import { X } from "@/v2/types/t.js";\nimport { A } from "@/v2/runtime/r.js";\nexport const Y = X + A;\n`,
    );

    const report = await runArchitectureLints({
      repoRoot,
      rulesPort: stubPort(DEFAULT_RULES),
    });
    expect(report.violations.filter((v) => v.rule_id === "layer-order")).toEqual([]);
  });

  it("allows any layer to import from cross-cutting (providers)", async () => {
    await writeFile(
      join(repoRoot, "src/v2/providers/wire.ts"),
      `export const W = 1;\n`,
    );
    await writeFile(
      join(repoRoot, "src/v2/runtime/r.ts"),
      `import { W } from "@/v2/providers/wire.js";\nexport const Z = W;\n`,
    );

    const report = await runArchitectureLints({
      repoRoot,
      rulesPort: stubPort(DEFAULT_RULES),
    });
    expect(report.violations.filter((v) => v.rule_id === "layer-order")).toEqual([]);
  });

  it("flags a forbidden passive-harness pattern", async () => {
    await writeFile(
      join(repoRoot, "src/v2/service/s.ts"),
      `export function tick() { setInterval(() => null, 1000); }\n`,
    );

    const report = await runArchitectureLints({
      repoRoot,
      rulesPort: stubPort(DEFAULT_RULES),
    });
    const passive = report.violations.filter((v) => v.rule_id === "passive-harness");
    expect(passive).toHaveLength(1);
    expect(passive[0]).toMatchObject({
      rule_id: "passive-harness",
      severity: "error",
      file: "src/v2/service/s.ts",
    });
    expect(passive[0].line).toBe(1);
  });

  it("uses word-boundary matching so substring hits do not trigger", async () => {
    await writeFile(
      join(repoRoot, "src/v2/service/s.ts"),
      `export const cronDescriptor = "not a cron job tool";\n`,
    );

    const report = await runArchitectureLints({
      repoRoot,
      rulesPort: stubPort(DEFAULT_RULES),
    });
    const passive = report.violations.filter((v) => v.rule_id === "passive-harness");
    expect(passive).toHaveLength(1);
    expect(passive[0].line).toBe(1);
  });

  it("skips test files even when they live under the scoped glob", async () => {
    await writeFile(
      join(repoRoot, "src/v2/service/s.test.ts"),
      `import { describe } from "bun:test";\nsetInterval(() => null, 1);\n`,
    );

    const report = await runArchitectureLints({
      repoRoot,
      rulesPort: stubPort(DEFAULT_RULES),
    });
    expect(report.violations).toEqual([]);
  });

  it("honors lint_scope (only files matching the glob are scanned)", async () => {
    await mkdir(join(repoRoot, "src/v1/legacy"), { recursive: true });
    await writeFile(
      join(repoRoot, "src/v1/legacy/old.ts"),
      `setInterval(() => null, 100);\n`,
    );
    await writeFile(
      join(repoRoot, "src/v2/service/s.ts"),
      `export const Y = 1;\n`,
    );

    const report = await runArchitectureLints({
      repoRoot,
      rulesPort: stubPort(DEFAULT_RULES),
    });
    expect(report.violations).toEqual([]);
  });

  it("falls back to src/v2/**/*.ts default when rules.lint_scope is empty", async () => {
    await writeFile(
      join(repoRoot, "src/v2/service/s.ts"),
      `export const X = 1;\n`,
    );

    const noScope: ArchitectureRules = { ...DEFAULT_RULES, lint_scope: [] };
    const report = await runArchitectureLints({
      repoRoot,
      rulesPort: stubPort(noScope),
    });
    expect(report.filesScanned).toBeGreaterThanOrEqual(1);
  });
});
