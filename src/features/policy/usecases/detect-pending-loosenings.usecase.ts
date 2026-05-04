import { execFileSync } from "node:child_process";
import { join } from "node:path";
import { readJson, writeJson } from "@/shared/lib/fs.js";
import { MaestroError } from "@/shared/errors.js";
import type { PolicyKind } from "../domain/policy-types.js";
import { classifyPolicyEdit } from "./classify-policy-edit.usecase.js";
import type { PolicyEdit } from "./classify-policy-edit.usecase.js";

export type { PolicyEdit };

export interface PendingLoosening {
  readonly commitSha: string;
  readonly commitTime: string;
  readonly effectiveAt: string;
  readonly kind: PolicyKind;
  readonly file: string;
  readonly edit: PolicyEdit;
}

/** 30-day soak window for loosenings before they take effect */
export const LOOSENING_SOAK_DAYS = 30;

/**
 * 60-day lookback window: more than the 30-day soak so we never miss a
 * still-pending loosening within the soak window.
 */
const LOOKBACK_DAYS = 60;

const CACHE_REL_PATH = ".maestro/policies/.pending-loosenings.json";

interface CacheFile {
  readonly headSha: string;
  readonly generatedAt: string;
  readonly items: readonly PendingLoosening[];
}

const POLICY_FILE_PATTERN = /^\.maestro\/policies\/[^/]+\.yaml$/;

function kindFromFile(file: string): PolicyKind | undefined {
  const name = file.split("/").pop() ?? "";
  switch (name) {
    case "risk.yaml": return "risk";
    case "autopilot.yaml": return "autopilot";
    case "release.yaml": return "release";
    case "sensitive-paths.yaml": return "sensitive-paths";
    case "owners.yaml": return "owners";
    default: return undefined;
  }
}

function git(args: string[], cwd: string): string {
  try {
    return execFileSync("git", args, {
      cwd,
      encoding: "utf8",
      stdio: ["pipe", "pipe", "pipe"],
    });
  } catch (err: unknown) {
    const e = err as { stderr?: string; stdout?: string };
    const msg = e.stderr ?? e.stdout ?? String(err);
    throw new MaestroError(`git ${args[0]} failed: ${msg.trim()}`, []);
  }
}

function gitSafe(args: string[], cwd: string): string {
  try {
    return git(args, cwd);
  } catch {
    return "";
  }
}

function headSha(projectRoot: string): string {
  return git(["rev-parse", "HEAD"], projectRoot).trim();
}

function toIso(epochSeconds: number): string {
  return new Date(epochSeconds * 1000).toISOString();
}

function addDays(iso: string, days: number): string {
  const d = new Date(iso);
  d.setUTCDate(d.getUTCDate() + days);
  return d.toISOString();
}

/**
 * Parse the output of `git log --pretty=format:"%H %ct" --name-only --diff-filter=AM`.
 * Returns an array of { sha, commitEpoch, files[] }.
 */
function parseGitLog(output: string): Array<{ sha: string; commitEpoch: number; files: string[] }> {
  const commits: Array<{ sha: string; commitEpoch: number; files: string[] }> = [];
  if (!output.trim()) return commits;

  const lines = output.split("\n");
  let current: { sha: string; commitEpoch: number; files: string[] } | undefined;

  for (const line of lines) {
    if (!line.trim()) {
      // blank line separates commits
      continue;
    }
    // Header line: "<sha> <epoch>"
    const headerMatch = /^([0-9a-f]{40}) (\d+)$/.exec(line.trim());
    if (headerMatch) {
      if (current) commits.push(current);
      current = {
        sha: headerMatch[1],
        commitEpoch: parseInt(headerMatch[2], 10),
        files: [],
      };
      continue;
    }
    // File name line
    if (current && POLICY_FILE_PATTERN.test(line.trim())) {
      current.files.push(line.trim());
    }
  }
  if (current) commits.push(current);
  return commits;
}

function getFileAtCommit(sha: string, file: string, projectRoot: string): string {
  // git show <sha>:<file>; returns empty string if ENOENT
  return gitSafe(["show", `${sha}:${file}`], projectRoot);
}

function getFileAtParent(sha: string, file: string, projectRoot: string): string {
  // Parent of the first commit is empty; git show SHA^:file fails gracefully
  return gitSafe(["show", `${sha}^:${file}`], projectRoot);
}

async function readCache(projectRoot: string): Promise<CacheFile | undefined> {
  return readJson<CacheFile>(join(projectRoot, CACHE_REL_PATH));
}

async function writeCache(projectRoot: string, cache: CacheFile): Promise<void> {
  await writeJson(join(projectRoot, CACHE_REL_PATH), cache);
}

async function recomputeLoosenings(projectRoot: string): Promise<readonly PendingLoosening[]> {
  const lookbackEpoch = Math.floor(Date.now() / 1000) - LOOKBACK_DAYS * 86400;
  const logOutput = gitSafe(
    [
      "log",
      `--after=${lookbackEpoch}`,
      "--pretty=format:%H %ct",
      "--name-only",
      "--diff-filter=AM",
      "--",
      ".maestro/policies/*.yaml",
    ],
    projectRoot,
  );

  if (!logOutput.trim()) return [];

  const commits = parseGitLog(logOutput);
  const loosenings: PendingLoosening[] = [];

  for (const { sha, commitEpoch, files } of commits) {
    for (const file of files) {
      const kind = kindFromFile(file);
      if (!kind || kind === "owners") continue;

      const newYaml = getFileAtCommit(sha, file, projectRoot);
      const oldYaml = getFileAtParent(sha, file, projectRoot);

      let classification;
      try {
        classification = classifyPolicyEdit({ oldYaml, newYaml, kind });
      } catch {
        // malformed YAML in history — skip
        continue;
      }

      const commitTimeIso = toIso(commitEpoch);
      const effectiveAtIso = addDays(commitTimeIso, LOOSENING_SOAK_DAYS);

      for (const edit of classification.loosenings) {
        loosenings.push({
          commitSha: sha,
          commitTime: commitTimeIso,
          effectiveAt: effectiveAtIso,
          kind,
          file,
          edit,
        });
      }
    }
  }

  return loosenings;
}

export interface DetectPendingLooseningsOptions {
  readonly projectRoot: string;
}

export async function detectPendingLoosenings(
  opts: DetectPendingLooseningsOptions,
): Promise<readonly PendingLoosening[]> {
  const { projectRoot } = opts;

  // Try cache
  let sha: string;
  try {
    sha = headSha(projectRoot);
  } catch {
    // Not a git repo or no commits — return empty
    return [];
  }

  const cache = await readCache(projectRoot);
  if (cache?.headSha === sha) {
    const now = Date.now();
    return cache.items.filter((i) => Date.parse(i.effectiveAt) > now);
  }

  // Cache miss or stale — recompute
  let items: readonly PendingLoosening[];
  try {
    items = await recomputeLoosenings(projectRoot);
  } catch {
    // Shallow clone or no history — return empty
    return [];
  }

  await writeCache(projectRoot, {
    headSha: sha,
    generatedAt: new Date().toISOString(),
    items: items as PendingLoosening[],
  });

  const now = Date.now();
  return items.filter((i) => Date.parse(i.effectiveAt) > now);
}

/** Factory that returns a bound detector for use in services */
export function buildDetectPendingLoosenings(
  projectRoot: string,
): () => Promise<readonly PendingLoosening[]> {
  return () => detectPendingLoosenings({ projectRoot });
}
