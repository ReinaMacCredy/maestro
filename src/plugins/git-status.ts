// Counts modified tracked files; untracked files are ignored because fixtures,
// scratch notes and build output are not a deliverable left unlanded.
export async function modifiedTrackedFiles(cwd: string): Promise<number> {
  try {
    const git = Bun.spawn(["git", "-C", cwd, "status", "--porcelain", "--untracked-files=no"], {
      stderr: "ignore",
      stdout: "pipe",
    });
    const [stdout, exitCode] = await Promise.all([new Response(git.stdout).text(), git.exited]);
    if (exitCode !== 0) return 0;
    return stdout.split("\n").filter((line) => line.trim() !== "").length;
  } catch {
    return 0;
  }
}

// A commit reference has at least one hex letter; a date or a count does not.
export function namesCommit(text: string): boolean {
  return /\b(?=[0-9]*[a-f])[0-9a-f]{7,40}\b/.test(text);
}
