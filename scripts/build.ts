import { join } from "node:path";
import { $ } from "bun";

const root = join(import.meta.dir, "..");

async function getGitShortSha(cwd: string): Promise<string> {
  try {
    return (await $`git rev-parse --short=7 HEAD`.cwd(cwd).quiet()).text().trim() || "unknown";
  } catch {
    return "unknown";
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
  "--env=inline",
];

const build = Bun.spawn(args, {
  cwd: root,
  stdout: "inherit",
  stderr: "inherit",
  env: {
    ...process.env,
    MAESTRO_BUILD_GIT_SHA: gitSha,
  },
});

process.exit(await build.exited);
