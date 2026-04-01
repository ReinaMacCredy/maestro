import { join } from "node:path";
import { getGitShortSha } from "./git-short-sha";

const root = join(import.meta.dir, "..");

const gitSha = await getGitShortSha(root);
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

const build = Bun.spawn(args, {
  cwd: root,
  stdout: "inherit",
  stderr: "inherit",
  env: gitSha
    ? {
      ...process.env,
      MAESTRO_BUILD_GIT_SHA: gitSha,
    }
    : process.env,
});

process.exit(await build.exited);
