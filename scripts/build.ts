import { join } from "node:path";
import { $ } from "bun";

const root = join(import.meta.dir, "..");

async function getGitShortSha(cwd: string): Promise<string | undefined> {
  try {
    return (await $`git rev-parse --short=7 HEAD`.cwd(cwd).quiet()).text().trim() || undefined;
  } catch {
    return undefined;
  }
}

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
