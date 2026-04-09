/**
 * Feature-folder boundary check.
 *
 * Rule: files under src/features/<own>/ may NOT deep-import from another
 * feature's internals. Cross-feature imports must go through the public
 * surface (@/features/<other> which resolves to <other>/index.ts).
 *
 * A "deep cross-feature import" is any import whose specifier matches
 *   features/<other>/(adapters|usecases|domain|ports|lib|commands)/...
 * where <other> != <own>. Imports shaped like `@/features/<other>` with
 * nothing after are public-surface imports and are always allowed.
 *
 * Exempt files: the composition roots and read-only cross-feature
 * aggregators in ALLOWED_CROSS_FEATURE are checked but never raise
 * violations (they legitimately glue features together).
 *
 * Feature-specific exceptions: FEATURE_EXCEPTIONS lets a named feature
 * import from a named sibling. Reserved for Phase 6 (worker --> mission,
 * memory). Empty in Phase 0.
 *
 * Phase 0 note: src/features/ does not exist yet, so the glob matches
 * zero files and the script exits clean.
 *
 * Usage: bun scripts/check-feature-boundaries.ts
 * Exits 0 on success, 1 on any violation.
 */
import { Glob } from "bun";

/** Files that may legitimately cross feature boundaries (checked but exempt). */
const ALLOWED_CROSS_FEATURE: readonly string[] = [
  "src/services.ts",
  "src/index.ts",
  "src/tui/state/snapshot.ts",
];

/** Per-feature sibling exceptions (reserved; Phase 6 will add worker). */
const FEATURE_EXCEPTIONS: Record<string, readonly string[]> = {};

const ROOT = new URL("../", import.meta.url).pathname;
const IMPORT_RE = /from\s+["']([^"']+)["']/g;
const DEEP_IMPORT_RE =
  /features\/([^/]+)\/(adapters|usecases|domain|ports|lib|commands)/;

interface Violation {
  readonly file: string;
  readonly ownFeature: string;
  readonly importSpec: string;
  readonly otherFeature: string;
}

function featureFromPath(relPath: string): string | undefined {
  const match = relPath.match(/^src\/features\/([^/]+)\//);
  return match?.[1];
}

const violations: Violation[] = [];
const featureGlob = new Glob("src/features/*/**/*.ts");

for await (const relPath of featureGlob.scan({ cwd: ROOT })) {
  if (ALLOWED_CROSS_FEATURE.includes(relPath)) continue;

  const ownFeature = featureFromPath(relPath);
  if (!ownFeature) continue;

  const text = await Bun.file(ROOT + relPath).text();
  const exceptions = FEATURE_EXCEPTIONS[ownFeature] ?? [];

  for (const match of text.matchAll(IMPORT_RE)) {
    const spec = match[1];
    if (!spec) continue;
    const deep = spec.match(DEEP_IMPORT_RE);
    if (!deep) continue;
    const otherFeature = deep[1];
    if (!otherFeature || otherFeature === ownFeature) continue;
    if (exceptions.includes(otherFeature)) continue;
    violations.push({ file: relPath, ownFeature, importSpec: spec, otherFeature });
  }
}

if (violations.length === 0) {
  console.log("Feature boundaries OK");
  process.exit(0);
}

console.error("[!] Feature boundary violations:\n");
for (const v of violations) {
  console.error(`  ${v.file}`);
  console.error(`    feature '${v.ownFeature}' deep-imports from '${v.otherFeature}'`);
  console.error(`    spec: ${v.importSpec}`);
  console.error("");
}
console.error(
  `Total: ${violations.length} violation(s). Cross-feature imports must go ` +
    `through the public surface (@/features/<name>) which resolves to index.ts.`,
);
process.exit(1);
