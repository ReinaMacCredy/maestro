import { describe, expect, it } from "bun:test";
import { findCrossFeatureImportViolation } from "../../../scripts/check-feature-boundaries-lib.ts";

describe("findCrossFeatureImportViolation", () => {
  it("allows public-surface imports across features", () => {
    expect(
      findCrossFeatureImportViolation(
        "src/features/worker/usecases/example.ts",
        "@/features/mission",
      ),
    ).toBeUndefined();

    expect(
      findCrossFeatureImportViolation(
        "src/features/worker/usecases/example.ts",
        "../../mission/index.js",
      ),
    ).toBeUndefined();
  });

  it("flags deep imports into direct and nested mission internals", () => {
    expect(
      findCrossFeatureImportViolation(
        "src/features/worker/usecases/example.ts",
        "@/features/mission/usecases/mission-report.usecase.js",
      ),
    ).toMatchObject({
      ownFeature: "worker",
      otherFeature: "mission",
      importSpec: "@/features/mission/usecases/mission-report.usecase.js",
    });

    expect(
      findCrossFeatureImportViolation(
        "src/features/worker/usecases/example.ts",
        "@/features/mission/feature/usecases/feature-lifecycle.usecase.js",
      ),
    ).toMatchObject({
      ownFeature: "worker",
      otherFeature: "mission",
      importSpec: "@/features/mission/feature/usecases/feature-lifecycle.usecase.js",
    });
  });

  it("does not exempt worker deep imports into memory internals", () => {
    expect(
      findCrossFeatureImportViolation(
        "src/features/worker/usecases/example.ts",
        "@/features/memory/domain/memory-types.js",
      ),
    ).toMatchObject({
      ownFeature: "worker",
      otherFeature: "memory",
      importSpec: "@/features/memory/domain/memory-types.js",
    });
  });

  it("allows same-feature internal imports", () => {
    expect(
      findCrossFeatureImportViolation(
        "src/features/mission/usecases/example.ts",
        "../feature/usecases/feature-lifecycle.usecase.js",
      ),
    ).toBeUndefined();
  });
});
