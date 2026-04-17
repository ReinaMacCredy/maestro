/**
 * Local CI: auto-bump, test, build, commit, install.
 *
 * Usage:
 *   bun scripts/ci.ts             # full release pipeline
 *   bun scripts/ci.ts --dry-run   # preview version bump only
 *
 * Pipeline:
 *   1. Guard: reject dirty working tree (unstaged/staged changes)
 *   2. Auto-bump version from conventional commits
 *   3. Check feature-folder boundaries
 *   4. Run tests
 *   5. Commit version files
 *   6. Build the release artifact and install locally
 *
 * On test failure the version bump is rolled back automatically.
 */
import { join } from "node:path";
import { $ } from "bun";

const root = join(import.meta.dir, "..");
const pkgPath = join(root, "package.json");
const versionPath = join(root, "src", "shared", "version.ts");
const dryRun = process.argv.includes("--dry-run");

// ---- helpers ----

function fail(msg: string): never {
  console.error(`[!!] ${msg}`);
  process.exit(1);
}

async function restoreVersion(pkgText: string, versionText: string): Promise<void> {
  await Bun.write(pkgPath, pkgText);
  await Bun.write(versionPath, versionText);
}

async function rollbackRelease(previousHead: string): Promise<void> {
  await $`git reset --hard ${previousHead}`.cwd(root).quiet();
}

// ---- step 1: dirty guard ----

const status = (await $`git status --porcelain`.quiet()).text().trim();
if (status && !dryRun) {
  fail("Working tree is dirty. Commit or stash changes before running CI.\n" + status);
}

// ---- step 2: auto-bump ----

console.log("[-->] Analyzing commits...");
const bumpArgs = dryRun ? ["--dry-run"] : [];
const bumpResult = await Bun.spawn(["bun", "scripts/auto-bump.ts", ...bumpArgs], {
  cwd: root,
  stdout: "inherit",
  stderr: "inherit",
}).exited;

if (bumpResult !== 0) fail("Auto-bump failed.");
if (dryRun) process.exit(0);

// Re-read the bumped version
const pkg = await Bun.file(pkgPath).json();
const nextVersion: string = pkg.version;
const previousHead = (await $`git rev-parse HEAD`.quiet()).text().trim();

// Read the pre-bump version from git
const origPkgText = (await $`git show HEAD:package.json`.quiet()).text();
const origVersionText = (await $`git show HEAD:src/shared/version.ts`.quiet()).text();
const origVersion: string = JSON.parse(origPkgText).version;

if (origVersion === nextVersion) {
  // auto-bump exited 0 with no changes (no new commits)
  process.exit(0);
}

// ---- step 3: boundary check ----

console.log("\n[-->] Checking feature boundaries...");
const boundaryResult = await Bun.spawn(["bun", "run", "check:boundaries"], {
  cwd: root,
  stdout: "inherit",
  stderr: "inherit",
}).exited;

if (boundaryResult !== 0) {
  await restoreVersion(origPkgText, origVersionText);
  fail(`Feature boundary check failed. Version reverted to ${origVersion}.`);
}
console.log("[ok] Feature boundaries OK.");

// ---- step 4: test ----

console.log("\n[-->] Running tests...");
const testResult = await Bun.spawn(["bun", "run", "test"], {
  cwd: root,
  stdout: "inherit",
  stderr: "inherit",
}).exited;

if (testResult !== 0) {
  await restoreVersion(origPkgText, origVersionText);
  fail(`Tests failed. Version reverted to ${origVersion}.`);
}
console.log("[ok] Tests passed.");

// ---- step 5: commit release metadata ----

console.log("\n[-->] Committing release...");
await $`git add package.json src/shared/version.ts`.cwd(root);

const commitMsg = `chore(release): v${nextVersion}`;
await $`git commit -m ${commitMsg}`.cwd(root);
console.log(`[ok] Committed release metadata for v${nextVersion}.`);

// ---- step 6: build + install locally ----

console.log("\n[-->] Building release artifact...");
const buildResult = await Bun.spawn(["bun", "run", "build"], {
  cwd: root,
  stdout: "inherit",
  stderr: "inherit",
}).exited;

if (buildResult !== 0) {
  await rollbackRelease(previousHead);
  fail(`Release build failed. Rolled back v${nextVersion}.`);
}
console.log("[ok] Built dist/maestro from the release commit.");

console.log("\n[-->] Installing locally...");
const installResult = await Bun.spawn(["bun", "scripts/install-local.ts"], {
  cwd: root,
  stdout: "inherit",
  stderr: "inherit",
}).exited;

if (installResult !== 0) {
  await rollbackRelease(previousHead);
  fail(`Local install failed. Rolled back v${nextVersion}.`);
}

// ---- done ----

console.log(`\n[ok] Release v${nextVersion} complete.`);
console.log("     Push or merge this release commit to main to publish.");
console.log("     GitHub Actions will create the remote tag and GitHub Release automatically.");
