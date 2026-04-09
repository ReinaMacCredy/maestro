/**
 * Feature-folder boundary check.
 *
 * RULE (final form, Phase 8):
 *   Files under src/features/<own>/ may NOT deep-import from another
 *   feature's internals. Cross-feature imports MUST go through the
 *   public surface (`@/features/<other>`), which resolves to that
 *   feature's index.ts.
 *
 * A "deep cross-feature import" is any import whose target path matches
 *   features/<other>/(adapters|usecases|domain|ports|lib|commands)/...
 * where <other> != <own>. Two forms are checked:
 *
 *   1. Alias form: `@/features/<other>/usecases/foo.js`
 *      Matched directly against DEEP_IMPORT_RE on the spec string.
 *
 *   2. Relative form: `../../<other>/usecases/foo.js`
 *      The spec starts with `./` or `../`, so we resolve it against the
 *      importing file's directory to a repo-absolute path, then apply
 *      DEEP_IMPORT_RE against the resolved path. Without this step,
 *      relative imports escape detection because the literal substring
 *      "features/" is not present in the spec itself.
 *
 * Public-surface imports (`@/features/<other>` with nothing after, or
 * `../../<other>/index.js`) resolve to the feature's index.ts and never
 * match DEEP_IMPORT_RE. These are the happy path.
 *
 * EXEMPT FILES (checked but always allowed, if ever walked):
 *   - src/services.ts                           composition root
 *   - src/index.ts                              commander root
 *   - src/tui/state/snapshot.ts                 read-only dashboard aggregator
 *   - src/infra/commands/mission-control.command.ts
 *                                               cross-feature dashboard view
 * These all live OUTSIDE `src/features/*`, so the glob never walks them.
 * The list is retained as a defensive contract: if any of them ever moves
 * under `src/features/`, the exemption survives the move.
 *
 * FEATURE-SPECIFIC EXCEPTIONS (FEATURE_EXCEPTIONS):
 *   - worker: ["mission", "memory"]
 *     The worker feature legitimately composes worker prompts using
 *     mission state and memory recall. The happy path is still through
 *     their public surfaces; this exception covers any deep imports that
 *     may appear in internal reuses. No other feature has exceptions.
 *     Extending this map should require explicit review and an AGENTS.md
 *     update -- see the Feature-Folder Layout section.
 *
 * Usage:
 *   bun scripts/check-feature-boundaries.ts
 * Exits 0 on success, 1 on any violation.
 */
import * as path from "node:path";
import { Glob } from "bun";

/** Files that may legitimately cross feature boundaries (checked but exempt). */
const ALLOWED_CROSS_FEATURE: readonly string[] = [
  "src/services.ts",
  "src/index.ts",
  "src/tui/state/snapshot.ts",
  "src/infra/commands/mission-control.command.ts",
];

/**
 * Per-feature sibling exceptions.
 *
 * Worker legitimately imports from mission and memory to compose worker
 * prompts. The happy path goes through their public surfaces
 * (`@/features/mission`, `@/features/memory`) which never match
 * DEEP_IMPORT_RE anyway; this exception covers deep imports that may
 * occur in test fixtures or future internal reuses.
 */
const FEATURE_EXCEPTIONS: Record<string, readonly string[]> = {
  worker: ["mission", "memory"],
};

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

/**
 * Resolve an import spec to a repo-relative path for deep-import detection.
 *
 * - Alias form (`@/...`, `~/...`, bare module, etc.): returned as-is so
 *   DEEP_IMPORT_RE can match `features/<other>/<layer>` directly.
 * - Relative form (`./foo`, `../bar`): joined with the importing file's
 *   directory and normalized via `path.posix.normalize`, yielding a
 *   repo-relative path like `src/features/ratchet/usecases/foo.js` that
 *   DEEP_IMPORT_RE can match. Without this step, a relative hop that
 *   escapes the current feature directory slips past the check because
 *   the literal substring `features/` is absent from the spec itself.
 */
function canonicalizeSpec(fileRelPath: string, spec: string): string {
  if (!spec.startsWith(".")) return spec;
  const fileDir = path.posix.dirname(fileRelPath);
  return path.posix.normalize(path.posix.join(fileDir, spec));
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
    const canonical = canonicalizeSpec(relPath, spec);
    const deep = canonical.match(DEEP_IMPORT_RE);
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
