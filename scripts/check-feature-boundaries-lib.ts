import * as path from "node:path";
import { fileURLToPath } from "node:url";
import { Glob, Transpiler } from "bun";

export interface Violation {
  readonly file: string;
  readonly ownFeature: string;
  readonly importSpec: string;
  readonly otherFeature: string;
}

const FEATURE_FILE_GLOBS = [
  "src/features/**/*.ts",
  "src/features/**/*.tsx",
  "src/features/**/*.mts",
  "src/features/**/*.cts",
] as const;

const ALLOWED_CROSS_FEATURE: readonly string[] = [
  "src/services.ts",
  "src/index.ts",
  "src/tui/state/snapshot.ts",
  "src/infra/commands/mission-control.command.ts",
] as const;

const PUBLIC_SURFACE_RE = /^(?:index\.(?:[cm]?tsx?|js))?$/;
const FEATURE_IMPORT_RE = /(?:^|\/)features\/([^/]+)(?:\/(.+))?$/;
const transpiler = new Transpiler({ loader: "tsx" });

function toPosixPath(p: string): string {
  return p.replace(/\\/g, "/");
}

function featureFromPath(relPath: string): string | undefined {
  return toPosixPath(relPath).match(/^src\/features\/([^/]+)\//)?.[1];
}

function canonicalizeSpec(fileRelPath: string, spec: string): string {
  if (!spec.startsWith(".")) return spec;
  const posixRel = toPosixPath(fileRelPath);
  return path.posix.normalize(path.posix.join(path.posix.dirname(posixRel), spec));
}

function isPublicSurfaceImport(subPath: string | undefined): boolean {
  return subPath === undefined || PUBLIC_SURFACE_RE.test(subPath);
}

export function resolveBoundaryCheckRoot(metaUrl: string): string {
  // Normalize to POSIX separators so downstream path joins and test
  // assertions behave identically on Windows and Unix.
  return toPosixPath(fileURLToPath(new URL("../", metaUrl)));
}

export function findCrossFeatureImportViolation(
  fileRelPath: string,
  spec: string,
): Violation | undefined {
  const ownFeature = featureFromPath(fileRelPath);
  if (!ownFeature) return undefined;

  const canonical = canonicalizeSpec(fileRelPath, spec);
  const match = canonical.match(FEATURE_IMPORT_RE);
  if (!match) return undefined;

  const otherFeature = match[1];
  const subPath = match[2];

  if (!otherFeature || otherFeature === ownFeature) return undefined;
  if (isPublicSurfaceImport(subPath)) return undefined;

  return {
    file: fileRelPath,
    ownFeature,
    importSpec: spec,
    otherFeature,
  };
}

export async function scanFeatureBoundaryViolations(root: string): Promise<Violation[]> {
  const violations: Violation[] = [];

  for (const pattern of FEATURE_FILE_GLOBS) {
    const featureGlob = new Glob(pattern);
    for await (const relPath of featureGlob.scan({ cwd: root })) {
      if (ALLOWED_CROSS_FEATURE.includes(toPosixPath(relPath))) continue;

      const text = await Bun.file(path.join(root, relPath)).text();
      const imports = transpiler.scanImports(text);

      for (const { path: spec } of imports) {
        const violation = findCrossFeatureImportViolation(
          toPosixPath(relPath),
          spec,
        );
        if (violation) {
          violations.push(violation);
        }
      }
    }
  }

  return violations;
}
