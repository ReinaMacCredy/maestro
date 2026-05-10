import { execArgv } from "./shell.js";

/** Git's well-known empty tree object SHA. Diffing against it shows every
 *  committed change since repo creation — the right "base" for a brand-new
 *  repo with no parent branch to merge from. */
const EMPTY_TREE_SHA = "4b825dc642cb6eb9a060e54bf8d69288fbee4904";

// Per-process memoization keyed on cwd. A single CLI invocation always runs
// against one cwd, so callers in `verdict request` / `task verify` /
// `policy check` / `merge auto` that each independently call
// resolveHeadSha or resolveDefaultBase share one git subprocess instead of
// each spawning their own. Tests that `process.chdir()` between cases get
// fresh cache entries automatically because the cache is cwd-keyed.
const headShaCache = new Map<string, Promise<string>>();
const defaultBaseCache = new Map<string, Promise<string>>();

/**
 * Resolve the default base ref for a diff. Walks a fallback chain so the
 * verifier still produces a meaningful diff in greenfield repos and on
 * platforms where `git init` defaults to `master` rather than `main`.
 *
 * 1. Upstream tracking branch's merge-base with HEAD (PR-style flow).
 * 2. merge-base with `main`, then `master`, then `trunk` (local branch flow).
 * 3. Empty-tree SHA — full repo content as the diff. Without this fallback,
 *    a greenfield repo on `master` returns the literal `"main"` ref, which
 *    git can't resolve, the diff comes back empty, and the trust verifier
 *    looks healthy backed by no evidence.
 *
 * Returns the SHA when available so subsequent git commands stay stable
 * across HEAD movement.
 */
export async function resolveDefaultBase(): Promise<string> {
  const cwd = process.cwd();
  const cached = defaultBaseCache.get(cwd);
  if (cached) return cached;
  const promise = computeDefaultBase();
  defaultBaseCache.set(cwd, promise);
  return promise;
}

async function computeDefaultBase(): Promise<string> {
  const upstream = await execArgv(["git", "rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"]);
  if (upstream.exitCode === 0 && upstream.stdout) {
    const upstreamRef = upstream.stdout;
    const mergeBase = await execArgv(["git", "merge-base", "HEAD", upstreamRef]);
    if (mergeBase.exitCode === 0 && mergeBase.stdout) {
      return mergeBase.stdout;
    }
    return upstreamRef;
  }

  // When HEAD is at the tip of (or behind) the parent-branch candidate, the
  // merge-base equals HEAD itself and the diff is empty. Skip those — the
  // user is likely on the parent branch in a greenfield repo, where the
  // sensible "base" is the empty tree (i.e., everything since creation).
  // Reuse the cached resolveHeadSha so verdict request doesn't pay a second
  // `git rev-parse HEAD` here.
  const headSha = await resolveHeadSha();
  for (const candidate of ["main", "master", "trunk"]) {
    const mergeBase = await execArgv(["git", "merge-base", "HEAD", candidate]);
    if (
      mergeBase.exitCode === 0 &&
      mergeBase.stdout &&
      mergeBase.stdout !== headSha
    ) {
      return mergeBase.stdout;
    }
  }

  return EMPTY_TREE_SHA;
}

/** Resolve current HEAD sha, falling back to the literal `"HEAD"` on failure. */
export async function resolveHeadSha(): Promise<string> {
  const cwd = process.cwd();
  const cached = headShaCache.get(cwd);
  if (cached) return cached;
  const promise = (async (): Promise<void> => {
    const head = await execArgv(["git", "rev-parse", "HEAD"]);
    return head.exitCode === 0 && head.stdout ? head.stdout : "HEAD";
  })();
  headShaCache.set(cwd, promise);
  return promise;
}
