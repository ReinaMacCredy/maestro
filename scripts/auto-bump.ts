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
import { splitCommitMessages, summarizeCommitBumps } from "./auto-bump-lib";
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

const raw = (await $`git log ${baseRef}..HEAD --format=%B%x00`.quiet()).text();
const messages = splitCommitMessages(raw);

if (messages.length === 0) {
  console.log(`[ok] No new commits since v${currentVersion}. Nothing to bump.`);
  process.exit(0);
}

// --- Determine bump level (MAJOR.MINOR.PATCH scheme) ---
// x bumps on feat or BREAKING CHANGE (feature slot)
// y bumps on everything else (patch slot)
// Major bumps are intentional and not driven by auto-bump.

const { bump, featureCount: featCount, patchCount } = summarizeCommitBumps(messages);

// --- Compute next version ---

const { major: m, feature: x, patch: y } = parseReleaseVersion(currentVersion);
const nextVersion = bump === "feature"
  ? `${m}.${x + 1}.0`
  : `${m}.${x}.${y + 1}`;

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
