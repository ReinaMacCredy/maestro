/**
 * Fake `gh` CLI shim for E2E tests that exercise the CI check-run path.
 *
 * Creates a tmp dir with a Node script named `gh` prepended to $PATH.
 * The script intercepts:
 *   gh api repos/<owner>/<repo>/check-runs          -X POST  --input -
 *   gh api repos/<owner>/<repo>/check-runs/<id>     -X PATCH --input -
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

export interface FakeGhShimState {
  readonly checkRuns: CheckRunRecord[];
}

export interface FakeGhShim {
  /** Directory prepended to PATH — contains the `gh` script */
  readonly binDir: string;
  /** Path to the JSON state file written by the shim */
  readonly stateFile: string;
  /** Read the current state (parsed from the JSON file) */
  readState(): FakeGhShimState;
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
const path = require("node:path");

const STATE_FILE = '${escapedPath}';

function readState() {
  try {
    return JSON.parse(fs.readFileSync(STATE_FILE, "utf8"));
  } catch {
    return { checkRuns: [], nextId: 1 };
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

// Expect: api <endpoint> [-X POST|PATCH] [--input -]
if (args[0] !== "api") {
  process.stderr.write("fake-gh: unrecognized subcommand: " + args[0] + "\\n");
  process.exit(1);
}

const endpoint = args[1];
if (!endpoint) {
  process.stderr.write("fake-gh: missing endpoint\\n");
  process.exit(1);
}

// Parse -X flag
let method = "GET";
for (let i = 2; i < args.length; i++) {
  if (args[i] === "-X" && args[i + 1]) {
    method = args[i + 1].toUpperCase();
    i++;
  }
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
  await writeFile(stateFile, JSON.stringify({ checkRuns: [], nextId: 1 }, null, 2));

  return {
    binDir,
    stateFile,

    readState(): FakeGhShimState {
      const raw = require("node:fs").readFileSync(stateFile, "utf8") as string;
      return JSON.parse(raw) as FakeGhShimState;
    },

    async cleanup(): Promise<void> {
      await rm(tmpBase, { recursive: true, force: true });
    },
  };
}
