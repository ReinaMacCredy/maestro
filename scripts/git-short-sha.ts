import { $ } from "bun";

export async function getGitShortSha(cwd: string): Promise<string | undefined> {
  try {
    return (await $`git rev-parse --short=7 HEAD`.cwd(cwd).quiet()).text().trim() || undefined;
  } catch {
    return undefined;
  }
}
