import { rm } from "node:fs/promises";
import { join } from "node:path";
import { removeRootBunBuildArtifacts } from "./build-lib";
import { getGitShortSha } from "./git-short-sha";

const root = join(import.meta.dir, "..");

const gitSha = await getGitShortSha(root);
const buildUnix = Math.floor(Date.now() / 1_000).toString();
const releasedAt = new Date().toISOString();
const args = [
  "bun",
  "build",
  "src/index.ts",
  "--compile",
  "--outfile",
  "dist/maestro",
  "--target",
  "bun",
  "--env=MAESTRO_BUILD_*",
];

const outExe = join(
  root,
  "dist",
  process.platform === "win32" ? "maestro.exe" : "maestro",
);

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
