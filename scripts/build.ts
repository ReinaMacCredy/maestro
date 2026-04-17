import { rm } from "node:fs/promises";
import { join } from "node:path";
import { removeRootBunBuildArtifacts } from "./build-lib";
import { getGitShortSha } from "./git-short-sha";

const root = join(import.meta.dir, "..");

// Keep the embedded built-in skill templates in sync with skills/built-in/
// before compile. The binary ships the embedded copy; drift here would mean
// users get stale skills on `maestro init`.
const syncSkills = Bun.spawn(["bun", join(root, "scripts", "sync-built-in-skills.ts")], {
  cwd: root,
  stdout: "inherit",
  stderr: "inherit",
});
if ((await syncSkills.exited) !== 0) {
  process.exit(1);
}

const gitSha = await getGitShortSha(root);
const buildUnix = Math.floor(Date.now() / 1_000).toString();
const releasedAt = new Date().toISOString();
// Bun's `--compile --outfile` appends `.exe` implicitly on Windows, but
// make it explicit so both the build arg and the post-build verification
// path agree without depending on that implicit behavior.
const outfileName = process.platform === "win32" ? "maestro.exe" : "maestro";
const args = [
  "bun",
  "build",
  "src/index.ts",
  "--compile",
  "--outfile",
  `dist/${outfileName}`,
  "--target",
  "bun",
  "--env=MAESTRO_BUILD_*",
];

const outExe = join(root, "dist", outfileName);

// Windows can fail Bun's final "move executable" rename with EPERM when the
// previous dist/maestro.exe is still held by a file lock (prior CI step,
// antivirus handle, or a finished child process that hasn't fully released).
// Pre-deleting makes room for the rename; retrying soaks up transient locks.
async function runBuild(): Promise<number> {
  const build = Bun.spawn(args, {
    cwd: root,
    stdout: "inherit",
    stderr: "inherit",
    env: {
      ...process.env,
      MAESTRO_BUILD_UNIX: buildUnix,
      MAESTRO_BUILD_RELEASED_AT: releasedAt,
      ...(gitSha ? { MAESTRO_BUILD_GIT_SHA: gitSha } : {}),
    },
  });
  return await build.exited;
}

let exitCode = 1;

await removeRootBunBuildArtifacts(root);

try {
  const maxAttempts = process.platform === "win32" ? 3 : 1;
  for (let attempt = 1; attempt <= maxAttempts; attempt++) {
    try {
      await rm(outExe, { force: true });
    } catch {
      // best-effort; bun will still try to overwrite
    }
    exitCode = await runBuild();
    if (exitCode === 0) break;
    if (attempt < maxAttempts) {
      await new Promise((r) => setTimeout(r, 300 * attempt));
    }
  }
} finally {
  await removeRootBunBuildArtifacts(root);
}

process.exit(exitCode);
