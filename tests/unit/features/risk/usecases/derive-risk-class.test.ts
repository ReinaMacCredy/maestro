import { describe, it, expect } from "bun:test";
import { deriveRiskClassFromDiff } from "@/features/risk/usecases/derive-risk-class.js";
import type { DerivedRiskInput } from "@/features/risk/domain/types.js";
import type { RiskPolicy } from "@/features/policy/index.js";

function makeInput(changedPaths: string[], overrides?: Partial<DerivedRiskInput>): DerivedRiskInput {
  return {
    changedPaths,
    addedLines: [],
    ...overrides,
  };
}

describe("deriveRiskClassFromDiff", () => {
  it("sensitive path → critical", () => {
    const result = deriveRiskClassFromDiff(
      makeInput(["src/auth/secrets.ts"], {
        sensitivePathsPolicy: ["src/auth/**"],
      }),
    );
    expect(result.class).toBe("critical");
    expect(result.matchedRow.signal).toBe("diff-intersects-sensitive-security");
  });

  it("package.json only → high (dependency manifest)", () => {
    const result = deriveRiskClassFromDiff(makeInput(["package.json"]));
    expect(result.class).toBe("high");
    expect(result.matchedRow.signal).toBe("diff-modifies-dependency-manifests");
  });

  it("bun.lock → high (dependency manifest)", () => {
    const result = deriveRiskClassFromDiff(makeInput(["bun.lock"]));
    expect(result.class).toBe("high");
    expect(result.matchedRow.signal).toBe("diff-modifies-dependency-manifests");
  });

  it("migration file → high", () => {
    const result = deriveRiskClassFromDiff(
      makeInput(["migrations/2026_01_01_init.sql"], {
        migrationPaths: ["migrations/**"],
      }),
    );
    expect(result.class).toBe("high");
    expect(result.matchedRow.signal).toBe("diff-modifies-migrations");
  });

  it("migration file with default paths → high", () => {
    const result = deriveRiskClassFromDiff(makeInput(["db/migrations/001_add_users.sql"]));
    expect(result.class).toBe("high");
    expect(result.matchedRow.signal).toBe("diff-modifies-migrations");
  });

  it(".github/workflows/ci.yml → high", () => {
    const result = deriveRiskClassFromDiff(makeInput([".github/workflows/ci.yml"]));
    expect(result.class).toBe("high");
    expect(result.matchedRow.signal).toBe("diff-modifies-ci-workflows");
  });

  it(".maestro/policies/owners.yaml → high", () => {
    const result = deriveRiskClassFromDiff(makeInput([".maestro/policies/owners.yaml"]));
    expect(result.class).toBe("high");
    expect(result.matchedRow.signal).toBe("diff-modifies-policy-files");
  });

  it("tsconfig.json → medium (build config)", () => {
    const result = deriveRiskClassFromDiff(makeInput(["tsconfig.json"]));
    expect(result.class).toBe("medium");
    expect(result.matchedRow.signal).toBe("diff-modifies-build-config");
  });

  it("source file → medium (source-only default)", () => {
    const result = deriveRiskClassFromDiff(makeInput(["src/foo.ts"]));
    expect(result.class).toBe("medium");
    expect(result.matchedRow.signal).toBe("diff-source-only");
  });

  it("README.md only → low (docs-only)", () => {
    const result = deriveRiskClassFromDiff(makeInput(["README.md"]));
    expect(result.class).toBe("low");
    expect(result.matchedRow.signal).toBe("diff-docs-only");
  });

  it("multiple docs-only files → low", () => {
    const result = deriveRiskClassFromDiff(makeInput(["README.md", "docs/guide.md", "CHANGELOG.md"]));
    expect(result.class).toBe("low");
    expect(result.matchedRow.signal).toBe("diff-docs-only");
  });

  it("mix of docs and source → medium (not docs-only)", () => {
    const result = deriveRiskClassFromDiff(makeInput(["README.md", "src/foo.ts"]));
    expect(result.class).toBe("medium");
    // source-only row fires before docs-only
    expect(result.matchedRow.signal).toBe("diff-source-only");
  });

  it("sensitive path wins over manifest when both present", () => {
    const result = deriveRiskClassFromDiff(
      makeInput(["package.json", "src/auth/login.ts"], {
        sensitivePathsPolicy: ["src/auth/**"],
      }),
    );
    expect(result.class).toBe("critical");
    expect(result.matchedRow.signal).toBe("diff-intersects-sensitive-security");
  });

  it("no match for sensitive paths when policy is empty → falls through to manifest", () => {
    const result = deriveRiskClassFromDiff(
      makeInput(["package.json", "src/auth/login.ts"], {
        sensitivePathsPolicy: [],
      }),
    );
    expect(result.class).toBe("high");
    expect(result.matchedRow.signal).toBe("diff-modifies-dependency-manifests");
  });

  it("custom RiskPolicy overrides default rows", () => {
    const customPolicy: RiskPolicy = {
      kind: "risk",
      id: "custom",
      version: "custom-1",
      rows: [
        {
          signal: "diff-docs-only",
          derivedClass: "medium",
          description: "Override: docs are medium in this project",
        },
      ],
    };
    const result = deriveRiskClassFromDiff(makeInput(["README.md"]), customPolicy);
    expect(result.class).toBe("medium");
    expect(result.matchedRow.signal).toBe("diff-docs-only");
  });
});
