/**
 * Fake `gh` CLI shim for E2E tests that exercise the CI check-run path and
 * auto-merge trigger path.
 *
 * Creates a tmp dir with a Node script named `gh` prepended to $PATH.
 * The script intercepts:
 *   gh api repos/<owner>/<repo>/check-runs          -X POST  --input -
 *   gh api repos/<owner>/<repo>/check-runs/<id>     -X PATCH --input -
 *   gh pr merge <number> --auto --repo <owner>/<repo> [--merge|--squash|--rebase]
 *
 * State is persisted to a JSON file so tests can assert on it after running
 * the compiled CLI.
 */
import { mkdir, chmod, writeFile, readFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { rm } from "node:fs/promises";

export interface CheckRunRecord {
  readonly id: number;
  readonly operation: "POST" | "PATCH";
  readonly endpoint: string;
  readonly head_sha?: string;
  readonly name?: string;
  readonly conclusion?: string;
  readonly output?: { title?: string; summary?: string };
  /** Raw parsed request body */
  readonly body: Record<string, unknown>;
}

/**
 * Record of a `gh pr merge --auto` invocation captured by the shim.
 */
export interface PrMergeRecord {
  readonly pr: number;
  readonly repo?: string;
  readonly mergeMethod: "merge" | "squash" | "rebase";
  /** Raw argv slice after `gh pr merge` */
  readonly args: string[];
}

/**
 * Record of a `gh api repos/<owner>/<repo>/pulls/<n>` GET invocation.
 */
export interface PrLookupRecord {
  readonly repository: string;
  readonly pr: number;
}

export interface FakeGhShimState {
  readonly checkRuns: CheckRunRecord[];
  readonly prMergeCalls: PrMergeRecord[];
  readonly prLookupCalls: PrLookupRecord[];
  /** Open PR numbers returned by the list-open-PRs endpoint. */
  readonly openPrs: number[];
  /** Map from PR number to file list returned by the PR-files endpoint. */
  readonly prFiles: Map<number, string[]>;
}

export interface FakeGhShim {
  /** Directory prepended to PATH — contains the `gh` script */
  readonly binDir: string;
  /** Path to the JSON state file written by the shim */
  readonly stateFile: string;
  /** Read the current state (parsed from the JSON file) */
  readState(): FakeGhShimState;
  /**
   * Configure what `user.login` the shim returns for `gh api repos/<repo>/pulls/<n>`.
   * Call before the CLI invocation that triggers a PR author lookup.
   */
  setPrAuthor(author: string): void;
  /**
   * Seed the list of open PR numbers returned by
   * `gh api repos/<repo>/pulls?state=open`.
   */
  setOpenPrs(prs: readonly number[]): void;
  /**
   * Seed the file list returned by
   * `gh api repos/<repo>/pulls/<pr>/files`.
   */
  setPrFiles(pr: number, files: readonly string[]): void;
  /** Remove the tmp dir */
  cleanup(): Promise<void>;
}

// ─── shim script source ───────────────────────────────────────────────────────

/**
 * Build the Node shim script content. The script is written to disk and
 * executed as `./bin/gh` when the compiled CLI calls `gh api ...`.
 *
 * The shim reads stdin (--input -), parses argv, and appends a record to the
 * state JSON file.
 *
 * Invocation patterns emitted by GhCliAdapter:
 *   POST:  gh api repos/<repo>/check-runs  -X POST  --input -
 *   PATCH: gh api repos/<repo>/check-runs/<id> -X PATCH --input -
 */
function buildShimSource(stateFile: string): string {
  // Escape the path for embedding in a JS string literal.
  const escapedPath = stateFile.replace(/\\/g, "\\\\").replace(/'/g, "\\'");

  return `#!/usr/bin/env node
"use strict";

const fs = require("node:fs");

const STATE_FILE = '${escapedPath}';

function readState() {
  try {
    const raw = JSON.parse(fs.readFileSync(STATE_FILE, "utf8"));
    // prFiles is stored as an array of [pr, files] pairs for JSON compatibility
    if (raw.prFilesArray) {
      raw.prFiles = new Map(raw.prFilesArray);
    } else {
      raw.prFiles = new Map();
    }
    return raw;
  } catch {
    return { checkRuns: [], prMergeCalls: [], prLookupCalls: [], nextId: 1, prAuthor: "test-user", openPrs: [], prFiles: new Map() };
  }
}

function writeState(state) {
  fs.writeFileSync(STATE_FILE, JSON.stringify(state, null, 2));
}

function readStdin() {
  try {
    return fs.readFileSync("/dev/stdin", "utf8");
  } catch {
    return "";
  }
}

const args = process.argv.slice(2);
const subcommand = args[0];

// ─── gh pr merge <number> --auto --repo <repo> [--merge|--squash|--rebase] ────
if (subcommand === "pr" && args[1] === "merge") {
  const prArgRaw = args[2];
  const pr = parseInt(prArgRaw, 10);
  if (!pr || isNaN(pr)) {
    process.stderr.write("fake-gh: pr merge: missing or invalid PR number\\n");
    process.exit(1);
  }

  const mergeArgs = args.slice(2);
  let repo = undefined;
  let mergeMethod = "merge";

  for (let i = 0; i < mergeArgs.length; i++) {
    if (mergeArgs[i] === "--repo" && mergeArgs[i + 1]) {
      repo = mergeArgs[i + 1];
      i++;
    } else if (mergeArgs[i] === "--squash") {
      mergeMethod = "squash";
    } else if (mergeArgs[i] === "--rebase") {
      mergeMethod = "rebase";
    } else if (mergeArgs[i] === "--merge") {
      mergeMethod = "merge";
    }
  }

  const state = readState();
  if (!state.prMergeCalls) state.prMergeCalls = [];
  state.prMergeCalls.push({ pr, repo, mergeMethod, args: mergeArgs });
  writeState(state);
  process.stdout.write("Auto-merge enabled for PR #" + pr + "\\n");
  process.exit(0);
}

// ─── gh api <endpoint> ─────────────────────────────────────────────────────────
if (subcommand !== "api") {
  process.stderr.write("fake-gh: unrecognized subcommand: " + subcommand + "\\n");
  process.exit(1);
}

// Parse -X / --jq / --paginate flags. Endpoint is the first non-flag arg after "api".
let method = "GET";
let jqExpr = undefined;
let endpoint = undefined;
for (let i = 1; i < args.length; i++) {
  if (args[i] === "-X" && args[i + 1]) {
    method = args[i + 1].toUpperCase();
    i++;
  } else if (args[i] === "--jq" && args[i + 1]) {
    jqExpr = args[i + 1];
    i++;
  } else if (args[i] === "--paginate") {
    // shim returns all rows in one response, ignore
  } else if (args[i].startsWith("--input")) {
    // --input - reads stdin; consume the next arg if it's a value
    if (args[i] === "--input" && args[i + 1]) i++;
  } else if (endpoint === undefined) {
    endpoint = args[i];
  }
}
if (!endpoint) {
  process.stderr.write("fake-gh: missing endpoint\\n");
  process.exit(1);
}

// ─── GET repos/<owner>/<repo>/pulls/<n> ─────────────────────────────────────
// Used by GhCliAdapter.getPullRequestAuthor with --jq .user.login
const pullsMatch = endpoint.match(/^repos\\/([^/]+)\\/([^/]+)\\/pulls\\/(\\d+)$/);
if (pullsMatch && method === "GET") {
  const repository = pullsMatch[1] + "/" + pullsMatch[2];
  const pr = parseInt(pullsMatch[3], 10);
  const state = readState();
  if (!state.prLookupCalls) state.prLookupCalls = [];
  state.prLookupCalls.push({ repository, pr });
  writeState(state);

  const author = state.prAuthor ?? "test-user";
  const responseBody = { user: { login: author } };

  // If --jq .user.login was passed, return just the value
  if (jqExpr === ".user.login") {
    process.stdout.write(author + "\\n");
  } else {
    process.stdout.write(JSON.stringify(responseBody) + "\\n");
  }
  process.exit(0);
}

// ─── GET repos/<owner>/<repo>/pulls?state=open ──────────────────────────────
// Used by GhCliAdapter.listOpenPullRequests with --jq '.[].number'
const openPullsMatch = endpoint.match(/^repos\\/([^/]+)\\/([^/]+)\\/pulls(\\?.*)?$/);
if (openPullsMatch && method === "GET" && !endpoint.match(/\\/pulls\\/\\d/)) {
  const state = readState();
  const openPrs = state.openPrs ?? [];
  // jqExpr is '.[].number'; output one number per line (what real jq produces)
  if (openPrs.length > 0) {
    process.stdout.write(openPrs.join("\\n") + "\\n");
  }
  process.exit(0);
}

// ─── GET repos/<owner>/<repo>/pulls/<n>/files ─────────────────────────────────
// Used by GhCliAdapter.getPullRequestFiles with --jq '.[].filename'
const prFilesMatch = endpoint.match(/^repos\\/([^/]+)\\/([^/]+)\\/pulls\\/(\\d+)\\/files(\\?.*)?$/);
if (prFilesMatch && method === "GET") {
  const pr = parseInt(prFilesMatch[3], 10);
  const state = readState();
  const files = state.prFiles.get(pr) ?? [];
  // jqExpr is '.[].filename'; output one path per line (what real jq produces)
  if (files.length > 0) {
    process.stdout.write(files.join("\\n") + "\\n");
  }
  process.exit(0);
}

// Determine if this is a check-runs call
// POST: repos/<owner>/<repo>/check-runs
// PATCH: repos/<owner>/<repo>/check-runs/<id>
const postMatch = endpoint.match(/^repos\\/[^/]+\\/[^/]+\\/check-runs$/);
const patchMatch = endpoint.match(/^repos\\/[^/]+\\/[^/]+\\/check-runs\\/(\\d+)$/);

if (!postMatch && !patchMatch) {
  process.stderr.write("fake-gh: unrecognized endpoint: " + endpoint + "\\n");
  process.exit(1);
}

const rawStdin = readStdin().trim();
let body = {};
if (rawStdin.length > 0) {
  try {
    body = JSON.parse(rawStdin);
  } catch {
    process.stderr.write("fake-gh: failed to parse stdin JSON\\n");
    process.exit(1);
  }
}

const state = readState();

if (method === "POST" && postMatch) {
  const id = state.nextId ?? 1;
  const record = {
    id,
    operation: "POST",
    endpoint,
    head_sha: body.head_sha,
    name: body.name,
    conclusion: body.conclusion,
    output: body.output,
    body,
  };
  state.checkRuns.push(record);
  state.nextId = id + 1;
  writeState(state);
  // gh api returns the created check-run object; must include numeric id
  process.stdout.write(JSON.stringify({ id }) + "\\n");
  process.exit(0);
}

if ((method === "PATCH") && patchMatch) {
  const checkRunId = parseInt(patchMatch[1], 10);
  const existing = state.checkRuns.find((r) => r.id === checkRunId);
  if (!existing) {
    process.stderr.write("fake-gh: check-run id " + checkRunId + " not found\\n");
    process.exit(1);
  }
  // Merge patch fields into the existing record
  const updated = {
    ...existing,
    operation: "PATCH",
    conclusion: body.conclusion ?? existing.conclusion,
    output: body.output ?? existing.output,
    body,
  };
  const idx = state.checkRuns.indexOf(existing);
  state.checkRuns[idx] = updated;
  writeState(state);
  process.stdout.write(JSON.stringify({ id: checkRunId }) + "\\n");
  process.exit(0);
}

process.stderr.write("fake-gh: method " + method + " not handled for endpoint " + endpoint + "\\n");
process.exit(1);
`;
}

// ─── public API ───────────────────────────────────────────────────────────────

export async function createFakeGhShim(): Promise<FakeGhShim> {
  const tmpBase = join(tmpdir(), `maestro-fake-gh-${Date.now()}-${Math.random().toString(36).slice(2)}`);
  const binDir = join(tmpBase, "bin");
  const stateFile = join(tmpBase, "gh-state.json");

  await mkdir(binDir, { recursive: true });

  const shimScript = join(binDir, "gh");
  await writeFile(shimScript, buildShimSource(stateFile));
  await chmod(shimScript, 0o755);

  // Initialise empty state so readState() never throws before first call.
  await writeFile(stateFile, JSON.stringify(
    { checkRuns: [], prMergeCalls: [], prLookupCalls: [], nextId: 1, prAuthor: "test-user", openPrs: [], prFilesArray: [] },
    null,
    2,
  ));

  return {
    binDir,
    stateFile,

    readState(): FakeGhShimState {
      const raw = require("node:fs").readFileSync(stateFile, "utf8") as string;
      const parsed = JSON.parse(raw) as Record<string, unknown>;
      const prFilesArray = (parsed["prFilesArray"] as Array<[number, string[]]>) ?? [];
      return {
        checkRuns: (parsed["checkRuns"] as CheckRunRecord[]) ?? [],
        prMergeCalls: (parsed["prMergeCalls"] as PrMergeRecord[]) ?? [],
        prLookupCalls: (parsed["prLookupCalls"] as PrLookupRecord[]) ?? [],
        openPrs: (parsed["openPrs"] as number[]) ?? [],
        prFiles: new Map(prFilesArray),
      };
    },

    setPrAuthor(author: string): void {
      const raw = require("node:fs").readFileSync(stateFile, "utf8") as string;
      const state = JSON.parse(raw) as Record<string, unknown>;
      state["prAuthor"] = author;
      require("node:fs").writeFileSync(stateFile, JSON.stringify(state, null, 2));
    },

    setOpenPrs(prs: readonly number[]): void {
      const raw = require("node:fs").readFileSync(stateFile, "utf8") as string;
      const state = JSON.parse(raw) as Record<string, unknown>;
      state["openPrs"] = [...prs];
      require("node:fs").writeFileSync(stateFile, JSON.stringify(state, null, 2));
    },

    setPrFiles(pr: number, files: readonly string[]): void {
      const raw = require("node:fs").readFileSync(stateFile, "utf8") as string;
      const state = JSON.parse(raw) as Record<string, unknown>;
      const arr = (state["prFilesArray"] as Array<[number, string[]]>) ?? [];
      const idx = arr.findIndex((e) => e[0] === pr);
      if (idx >= 0) {
        arr[idx] = [pr, [...files]];
      } else {
        arr.push([pr, [...files]]);
      }
      state["prFilesArray"] = arr;
      require("node:fs").writeFileSync(stateFile, JSON.stringify(state, null, 2));
    },

    async cleanup(): Promise<void> {
      await rm(tmpBase, { recursive: true, force: true });
    },
  };
}
