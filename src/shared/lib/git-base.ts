import { execArgv } from "./shell.js";

/**
 * Resolve the default base ref for a diff: prefer the upstream tracking
 * branch's merge-base with HEAD, then fall back to merge-base with `main`,
 * then to the literal `"main"`. Returns the SHA when available so subsequent
 * git commands stay stable across HEAD movement.
 */
export async function resolveDefaultBase(): Promise<string> {
  const upstream = await execArgv(["git", "rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"]);
  if (upstream.exitCode === 0 && upstream.stdout) {
    const upstreamRef = upstream.stdout;
    const mergeBase = await execArgv(["git", "merge-base", "HEAD", upstreamRef]);
    if (mergeBase.exitCode === 0 && mergeBase.stdout) {
      return mergeBase.stdout;
    }
    return upstreamRef;
  }

  const mergeBaseMain = await execArgv(["git", "merge-base", "HEAD", "main"]);
  if (mergeBaseMain.exitCode === 0 && mergeBaseMain.stdout) {
    return mergeBaseMain.stdout;
  }

  return "main";
}

/** Resolve current HEAD sha, falling back to the literal `"HEAD"` on failure. */
export async function resolveHeadSha(): Promise<string> {
  const head = await execArgv(["git", "rev-parse", "HEAD"]);
  return head.exitCode === 0 && head.stdout ? head.stdout : "HEAD";
}
