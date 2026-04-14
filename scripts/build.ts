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

let exitCode = 1;

await removeRootBunBuildArtifacts(root);

try {
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

  exitCode = await build.exited;
} finally {
  await removeRootBunBuildArtifacts(root);
}

process.exit(exitCode);
