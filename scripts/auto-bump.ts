/**
 * Auto-bump version based on conventional commits since last release.
 *
 * Analyzes git log for commit prefixes:
 *   BREAKING CHANGE / feat!:  -> major
 *   feat(...)                 -> minor
 *   everything else           -> patch
 *
 * Usage:
 *   bun scripts/auto-bump.ts             # bump and write files
 *   bun scripts/auto-bump.ts --dry-run   # preview without writing
 *
 * Exits 0 with no changes if there are no new commits.
 */
import { join } from "node:path";
import { $ } from "bun";
import { parseReleaseVersion, writeVersionArtifacts } from "./version-file";

const root = join(import.meta.dir, "..");
const pkgPath = join(root, "package.json");
const versionPath = join(root, "src", "shared", "version.ts");
const dryRun = process.argv.includes("--dry-run");

// --- Resolve base reference (tag > release commit > repo root) ---

const pkg = await Bun.file(pkgPath).json();
const currentVersion: string = pkg.version;
const tagName = `v${currentVersion}`;

let baseRef: string;
try {
  await $`git rev-parse --verify ${tagName}^{commit}`.quiet();
  baseRef = tagName;
} catch {
  const grepPattern = "chore(release)";
  const hash = (await $`git log --format=%H --grep=${grepPattern} -1`.quiet()).text().trim();
  if (hash) {
    baseRef = hash;
  } else {
    baseRef = (await $`git rev-list --max-parents=0 HEAD`.quiet()).text().trim().split("\n")[0];
  }
}

// --- Collect commits since base ---

const raw = (await $`git log ${baseRef}..HEAD --format=%s`.quiet()).text().trim();
const messages = raw ? raw.split("\n") : [];

if (messages.length === 0) {
  console.log(`[ok] No new commits since v${currentVersion}. Nothing to bump.`);
  process.exit(0);
}

// --- Determine bump level (0.x.y scheme) ---
// x bumps on feat or BREAKING CHANGE (feature slot)
// y bumps on everything else (patch slot)

const BREAKING = /BREAKING[ -]CHANGE|^[a-z]+(\(.+\))?!:/;
const FEAT = /^feat(\(.+\))?[!:]/;

let bump: "feature" | "patch" = "patch";
for (const msg of messages) {
  if (BREAKING.test(msg) || FEAT.test(msg)) {
    bump = "feature";
    break;
  }
}

// --- Compute next version (0.x.y) ---

const { feature: x, patch: y } = parseReleaseVersion(currentVersion);
const nextVersion = bump === "feature"
  ? `0.${x + 1}.0`
  : `0.${x}.${y + 1}`;

const featCount = messages.filter((m) => FEAT.test(m) || BREAKING.test(m)).length;
const patchCount = messages.length - featCount;

console.log(`[-->] ${currentVersion} -> ${nextVersion} (${bump})`);
console.log(`     ${messages.length} commits: ${featCount} feature, ${patchCount} patch`);

if (dryRun) {
  console.log("[--] Dry run -- no files written.");
  process.exit(0);
}

// --- Write version files ---

await writeVersionArtifacts({
  cwd: root,
  pkgPath,
  versionPath,
  pkg,
  version: nextVersion,
});

console.log("[ok] Version files updated.");
