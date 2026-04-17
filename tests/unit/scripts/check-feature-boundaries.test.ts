import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { pathToFileURL } from "node:url";
import {
  findCrossFeatureImportViolation,
  resolveBoundaryCheckRoot,
  scanFeatureBoundaryViolations,
} from "../../../scripts/check-feature-boundaries-lib";

let tmpDir: string;

beforeEach(async () => {
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-boundaries-"));
});

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true });
});

describe("findCrossFeatureImportViolation", () => {
  it("resolves script roots as decoded filesystem paths instead of raw URL pathnames", () => {
    const rootDir = join(tmpDir, "repo with spaces");
    const scriptUrl = pathToFileURL(join(rootDir, "scripts", "check-feature-boundaries.ts")).href;

    expect(resolveBoundaryCheckRoot(scriptUrl)).toBe(`${rootDir}/`);
  });

  it("allows public-surface imports across features", () => {
    expect(
      findCrossFeatureImportViolation(
        "src/features/agent/usecases/example.ts",
        "@/features/mission",
      ),
    ).toBeUndefined();

    expect(
      findCrossFeatureImportViolation(
        "src/features/agent/usecases/example.ts",
        "../../mission/index.js",
      ),
    ).toBeUndefined();
  });

  it("flags deep imports into direct and nested mission internals", () => {
    expect(
      findCrossFeatureImportViolation(
        "src/features/agent/usecases/example.ts",
        "@/features/mission/usecases/mission-report.usecase.js",
      ),
    ).toMatchObject({
      ownFeature: "agent",
      otherFeature: "mission",
      importSpec: "@/features/mission/usecases/mission-report.usecase.js",
    });

    expect(
      findCrossFeatureImportViolation(
        "src/features/agent/usecases/example.ts",
        "@/features/mission/feature/usecases/feature-lifecycle.usecase.js",
      ),
    ).toMatchObject({
      ownFeature: "agent",
      otherFeature: "mission",
      importSpec: "@/features/mission/feature/usecases/feature-lifecycle.usecase.js",
    });
  });

  it("does not exempt agent deep imports into memory internals", () => {
    expect(
      findCrossFeatureImportViolation(
        "src/features/agent/usecases/example.ts",
        "@/features/memory/domain/memory-types.js",
      ),
    ).toMatchObject({
      ownFeature: "agent",
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

  it("scans feature files on disk and ignores exempt cross-feature aggregators", async () => {
    const agentDir = join(tmpDir, "src", "features", "agent", "usecases");
    const graphDir = join(tmpDir, "src", "features", "graph");
    const tuiDir = join(tmpDir, "src", "tui", "state");
    await mkdir(agentDir, { recursive: true });
    await mkdir(graphDir, { recursive: true });
    await mkdir(tuiDir, { recursive: true });

    await writeFile(
      join(agentDir, "bad-import.ts"),
      'import { x } from "@/features/mission/usecases/mission-report.usecase.js";\n',
    );
    await writeFile(join(graphDir, "index.ts"), 'export * from "./services.js";\n');
    await writeFile(
      join(tuiDir, "snapshot.ts"),
      'import { memory } from "@/features/memory/usecases/memory-recall.usecase.js";\n',
    );

    const violations = await scanFeatureBoundaryViolations(tmpDir);

    expect(violations).toEqual([
      {
        file: "src/features/agent/usecases/bad-import.ts",
        ownFeature: "agent",
        importSpec: "@/features/mission/usecases/mission-report.usecase.js",
        otherFeature: "mission",
      },
    ]);
  });
});
