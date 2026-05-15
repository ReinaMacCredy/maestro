import { describe, expect, it } from "bun:test";
import {
  classifyWorkType,
  detectHarnessImpact,
  generateNextSteps,
} from "@/features/intake/index.js";

const existing = (set: ReadonlySet<string>) => (p: string) => set.has(p);

describe("classifyWorkType", () => {
  it("returns harness-improvement for .maestro/ paths", () => {
    const w = classifyWorkType(
      { intendedPaths: [".maestro/policies/risk.yaml"] },
      { allFlags: [], pathExists: existing(new Set([".maestro/policies/risk.yaml"])) },
    );
    expect(w).toBe("harness-improvement");
  });

  it("returns harness-improvement for skills/ paths", () => {
    const w = classifyWorkType(
      { intendedPaths: ["skills/bundled/maestro-intake/SKILL.md"] },
      { allFlags: [], pathExists: existing(new Set()) },
    );
    expect(w).toBe("harness-improvement");
  });

  it("returns harness-improvement for policies/ paths", () => {
    const w = classifyWorkType(
      { intendedPaths: ["policies/risk.yaml"] },
      { allFlags: [], pathExists: existing(new Set()) },
    );
    expect(w).toBe("harness-improvement");
  });

  it("returns harness-improvement for hooks/ paths", () => {
    const w = classifyWorkType(
      { intendedPaths: ["hooks/session-start.ts"] },
      { allFlags: [], pathExists: existing(new Set()) },
    );
    expect(w).toBe("harness-improvement");
  });

  it("returns initiative when multi-domain flag is set", () => {
    const w = classifyWorkType(
      { intendedPaths: ["src/features/auth/login.ts"] },
      { allFlags: ["multi-domain"], pathExists: existing(new Set(["src/features/auth/login.ts"])) },
    );
    expect(w).toBe("initiative");
  });

  it("returns initiative when paths span 3+ top-level dirs", () => {
    const w = classifyWorkType(
      {
        intendedPaths: ["src/features/auth/x.ts", "scripts/build.ts", "tests/unit/foo.test.ts"],
      },
      {
        allFlags: [],
        pathExists: existing(
          new Set(["src/features/auth/x.ts", "scripts/build.ts", "tests/unit/foo.test.ts"]),
        ),
      },
    );
    expect(w).toBe("initiative");
  });

  it("returns maintenance for package.json + lockfile only", () => {
    const w = classifyWorkType(
      { intendedPaths: ["package.json", "bun.lock"] },
      { allFlags: [], pathExists: existing(new Set(["package.json", "bun.lock"])) },
    );
    expect(w).toBe("maintenance");
  });

  it("returns maintenance for .github/workflows", () => {
    const w = classifyWorkType(
      { intendedPaths: [".github/workflows/ci.yml"] },
      { allFlags: [], pathExists: existing(new Set([".github/workflows/ci.yml"])) },
    );
    expect(w).toBe("maintenance");
  });

  it("returns new-spec when no path exists", () => {
    const w = classifyWorkType(
      { intendedPaths: ["src/features/new-thing/index.ts"] },
      { allFlags: [], pathExists: existing(new Set()) },
    );
    expect(w).toBe("new-spec");
  });

  it("returns spec-slice when all paths share one feature root and exist", () => {
    const w = classifyWorkType(
      {
        intendedPaths: [
          "src/features/intake/domain/types.ts",
          "src/features/intake/usecases/classify-intake.usecase.ts",
        ],
      },
      {
        allFlags: [],
        pathExists: existing(
          new Set([
            "src/features/intake/domain/types.ts",
            "src/features/intake/usecases/classify-intake.usecase.ts",
          ]),
        ),
      },
    );
    expect(w).toBe("spec-slice");
  });

  it("returns change-request for existing paths spanning multiple feature roots", () => {
    const w = classifyWorkType(
      {
        intendedPaths: [
          "src/features/intake/domain/types.ts",
          "src/features/task/domain/task-types.ts",
        ],
      },
      {
        allFlags: [],
        pathExists: existing(
          new Set([
            "src/features/intake/domain/types.ts",
            "src/features/task/domain/task-types.ts",
          ]),
        ),
      },
    );
    expect(w).toBe("change-request");
  });

  it("returns change-request when paths are empty", () => {
    const w = classifyWorkType(
      { intendedPaths: [] },
      { allFlags: [], pathExists: existing(new Set()) },
    );
    expect(w).toBe("change-request");
  });

  it("honors declaredWorkType override", () => {
    const w = classifyWorkType(
      {
        intendedPaths: [".maestro/policies/risk.yaml"],
        declaredWorkType: "maintenance",
      },
      { allFlags: [], pathExists: existing(new Set()) },
    );
    expect(w).toBe("maintenance");
  });

  it("first-match wins: harness path beats initiative even with multi-domain", () => {
    const w = classifyWorkType(
      { intendedPaths: [".maestro/foo.md", "src/x.ts", "scripts/y.ts"] },
      { allFlags: ["multi-domain"], pathExists: existing(new Set()) },
    );
    expect(w).toBe("harness-improvement");
  });
});

describe("detectHarnessImpact", () => {
  it("is true when any path is under .maestro/", () => {
    expect(detectHarnessImpact([".maestro/x.md", "src/foo.ts"])).toBe(true);
  });

  it("is true for skills/, policies/, hooks/", () => {
    expect(detectHarnessImpact(["skills/built-in/foo/SKILL.md"])).toBe(true);
    expect(detectHarnessImpact(["policies/owners.yaml"])).toBe(true);
    expect(detectHarnessImpact(["hooks/tool-call.ts"])).toBe(true);
  });

  it("is false for product-only paths", () => {
    expect(detectHarnessImpact(["src/features/foo/x.ts", "tests/unit/foo.test.ts"])).toBe(false);
  });

  it("is false for empty paths", () => {
    expect(detectHarnessImpact([])).toBe(false);
  });
});

describe("generateNextSteps", () => {
  it("returns the right string for spec-slice + normal", () => {
    expect(generateNextSteps("spec-slice", "normal")).toBe(
      "Create task, reference parent spec",
    );
  });

  it("covers all 18 combinations", () => {
    const workTypes = [
      "new-spec",
      "spec-slice",
      "change-request",
      "initiative",
      "maintenance",
      "harness-improvement",
    ] as const;
    const lanes = ["tiny", "normal", "high-risk"] as const;
    for (const w of workTypes) {
      for (const l of lanes) {
        const out = generateNextSteps(w, l);
        expect(typeof out).toBe("string");
        expect(out.length).toBeGreaterThan(0);
      }
    }
  });

  it("returns the threat-model variant for high-risk new-spec", () => {
    expect(generateNextSteps("new-spec", "high-risk")).toContain("threat model");
  });

  it("returns the harness-delta evidence hint for harness-improvement + normal", () => {
    expect(generateNextSteps("harness-improvement", "normal")).toContain(
      "harness-delta",
    );
  });
});
