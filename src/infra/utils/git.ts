import simpleGit from 'simple-git';

export interface GitAuditSummary {
  baseCommit?: string;
  headCommit: string;
  dirtyWorkingTree: boolean;
  changedFilesSinceBase: string[];
  uncommittedFiles: string[];
}

function parseNameOnly(output: string): string[] {
  return output
    .split('\n')
    .map((line) => line.trim())
    .filter(Boolean);
}

export async function getHeadCommit(projectRoot: string): Promise<string> {
  const git = simpleGit(projectRoot);
  return (await git.revparse(['HEAD'])).trim();
}

/**
 * Get files changed since a given ISO timestamp. Combines committed and uncommitted changes.
 * Best-effort: returns [] on any failure (no git repo, detached HEAD, etc.).
 */
export async function getChangedFilesSince(
  projectRoot: string,
  sinceISO?: string,
): Promise<string[]> {
  try {
    const git = simpleGit(projectRoot);
    const files = new Set<string>();

    // Committed files since timestamp
    if (sinceISO) {
      try {
        const logResult = await git.log({ '--since': sinceISO, '--name-only': null, '--format': '' });
        for (const commit of logResult.all) {
          const diff = (commit as Record<string, unknown>).diff;
          if (diff && typeof diff === 'object' && 'files' in (diff as object)) {
            for (const f of (diff as { files: Array<{ file: string }> }).files) {
              if (f.file) files.add(f.file);
            }
          }
        }
      } catch {
        // git log may fail on shallow clones or missing refs -- fall through to status
      }

      // Fallback: reflog-based diff when git log returned nothing
      if (files.size === 0) {
        try {
          const diffOutput = await git.diff(['--name-only', `--diff-filter=ACDMRT`, `HEAD@{${sinceISO}}..HEAD`]);
          for (const f of parseNameOnly(diffOutput)) files.add(f);
        } catch {
          // Reflog may not have the timestamp -- acceptable
        }
      }
    }

    // Uncommitted + staged files (status covers both)
    const status = await git.status();
    for (const f of status.not_added) files.add(f);
    for (const f of status.modified) files.add(f);
    for (const f of status.created) files.add(f);
    for (const f of status.renamed.map(r => r.to)) files.add(f);
    for (const f of status.staged) files.add(f);

    return Array.from(files).filter(Boolean);
  } catch {
    return [];
  }
}

export async function collectGitAuditSummary(
  projectRoot: string,
  baseCommit?: string,
): Promise<GitAuditSummary> {
  const git = simpleGit(projectRoot);
  const headCommit = await getHeadCommit(projectRoot);
  const status = await git.status();
  const dirtyWorkingTree = !status.isClean();

  let changedFilesSinceBase: string[] = [];
  if (baseCommit) {
    try {
      changedFilesSinceBase = parseNameOnly(
        await git.diff(['--name-only', `${baseCommit}..HEAD`]),
      );
    } catch {
      changedFilesSinceBase = [];
    }
  }

  const uncommittedFiles = parseNameOnly(await git.diff(['--name-only']));
  const stagedFiles = parseNameOnly(await git.diff(['--cached', '--name-only']));
  const allUncommittedFiles = Array.from(new Set([
    ...uncommittedFiles,
    ...stagedFiles,
    ...status.not_added,
  ]));

  return {
    baseCommit,
    headCommit,
    dirtyWorkingTree,
    changedFilesSinceBase,
    uncommittedFiles: allUncommittedFiles,
  };
}
