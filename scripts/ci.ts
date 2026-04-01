/**
 * Local CI: auto-bump, test, build, commit, tag, install.
 *
 * Usage:
 *   bun scripts/ci.ts             # full release pipeline
 *   bun scripts/ci.ts --dry-run   # preview version bump only
 *
 * Pipeline:
 *   1. Guard: reject dirty working tree (unstaged/staged changes)
 *   2. Auto-bump version from conventional commits
 *   3. Run tests
 *   4. Build compiled binary
 *   5. Commit version files + tag
 *   6. Install binary locally
 *
 * On test failure the version bump is rolled back automatically.
 */
import { join } from "node:path";
import { $ } from "bun";
import { writeVersionArtifacts } from "./version-file";

const root = join(import.meta.dir, "..");
const pkgPath = join(root, "package.json");
const versionPath = join(root, "src", "version.ts");
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

// Read the pre-bump version from git
const origPkgText = (await $`git show HEAD:package.json`.quiet()).text();
const origVersionText = (await $`git show HEAD:src/version.ts`.quiet()).text();
const origVersion: string = JSON.parse(origPkgText).version;

if (origVersion === nextVersion) {
  // auto-bump exited 0 with no changes (no new commits)
  process.exit(0);
}

// ---- step 3: test ----

console.log("\n[-->] Running tests...");
const testResult = await Bun.spawn(["bun", "test"], {
  cwd: root,
  stdout: "inherit",
  stderr: "inherit",
}).exited;

if (testResult !== 0) {
  await restoreVersion(origPkgText, origVersionText);
  fail(`Tests failed. Version reverted to ${origVersion}.`);
}
console.log("[ok] Tests passed.");

// ---- step 4: build ----

console.log("\n[-->] Building...");
const buildResult = await Bun.spawn(["bun", "run", "build"], {
  cwd: root,
  stdout: "inherit",
  stderr: "inherit",
}).exited;

if (buildResult !== 0) {
  await restoreVersion(origPkgText, origVersionText);
  fail(`Build failed. Version reverted to ${origVersion}.`);
}
console.log("[ok] Built dist/maestro.");

// ---- step 5: commit + tag ----

console.log("\n[-->] Committing release...");
await $`git add package.json src/version.ts`.cwd(root);

const commitMsg = `chore(release): v${nextVersion}`;
await $`git commit -m ${commitMsg}`.cwd(root);
await $`git tag ${"v" + nextVersion}`.cwd(root);
console.log(`[ok] Committed and tagged v${nextVersion}.`);

// ---- step 6: install locally ----

console.log("\n[-->] Installing locally...");
const installResult = await Bun.spawn(["bash", "scripts/install-local.sh", "./dist/maestro"], {
  cwd: root,
  stdout: "inherit",
  stderr: "inherit",
}).exited;

if (installResult !== 0) fail("Local install failed.");

// ---- done ----

console.log(`\n[ok] Release v${nextVersion} complete.`);
console.log(`     Run 'git push && git push --tags' to publish.`);
