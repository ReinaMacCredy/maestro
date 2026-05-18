// Shared helpers for scenario rubric runners.
// All rubric.ts files import from this module.

import { readdir, readFile, stat } from "node:fs/promises";
import { join } from "node:path";
import type {
  EvidenceRow,
  TransitionEvidenceRow,
  LintViolationEvidenceRow,
} from "../../../src/repo/evidence-store.port.js";

export type { EvidenceRow, TransitionEvidenceRow, LintViolationEvidenceRow };

export interface CheckResult {
  readonly id: string;
  readonly description: string;
  readonly pass: boolean;
  readonly evidence?: string;
  readonly note?: string;
}

export interface RubricResult {
  readonly scenario: string;
  readonly projectDir: string;
  readonly pass: boolean;
  readonly checks: readonly CheckResult[];
}

// ---------------------------------------------------------------------------
// Reusable row predicates
// ---------------------------------------------------------------------------

export function isTaskTransitionTo(row: EvidenceRow, state: string): boolean {
  return (
    row.kind === "transition" &&
    "task_id" in row &&
    typeof row.task_id === "string" &&
    row.task_id.length > 0 &&
    row.to_state === state
  );
}

export function isMissionTransitionTo(row: EvidenceRow, state: string): boolean {
  return (
    row.kind === "transition" &&
    "mission_id" in row &&
    typeof row.mission_id === "string" &&
    row.mission_id.length > 0 &&
    !("task_id" in row && typeof row.task_id === "string" && row.task_id.length > 0) &&
    row.to_state === state
  );
}

export function isChildDraftRow(row: EvidenceRow): boolean {
  return (
    row.kind === "transition" &&
    "task_id" in row &&
    typeof row.task_id === "string" &&
    row.task_id.length > 0 &&
    "mission_id" in row &&
    typeof row.mission_id === "string" &&
    row.mission_id.length > 0 &&
    row.to_state === "draft"
  );
}

export function isLintViolation(row: EvidenceRow): boolean {
  return row.kind === "lint-violation";
}

// ---------------------------------------------------------------------------
// Evidence loading
// ---------------------------------------------------------------------------

export async function loadEvidence(projectDir: string): Promise<EvidenceRow[]> {
  const dir = join(projectDir, ".maestro/evidence");
  let entries: string[];
  try {
    entries = (await readdir(dir)).filter((f) => f.endsWith(".jsonl"));
  } catch (err) {
    if ((err as NodeJS.ErrnoException).code === "ENOENT") return [];
    throw err;
  }
  const rows: EvidenceRow[] = [];
  for (const name of entries.sort()) {
    const content = await readFile(join(dir, name), "utf8");
    for (const line of content.split("\n")) {
      if (!line.trim()) continue;
      try {
        rows.push(JSON.parse(line) as EvidenceRow);
      } catch {
        // skip malformed lines
      }
    }
  }
  return rows;
}

// ---------------------------------------------------------------------------
// Predicate checks
// ---------------------------------------------------------------------------

export function mustHave(
  rows: readonly EvidenceRow[],
  predicate: (r: EvidenceRow) => boolean,
  id: string,
  description: string,
): CheckResult {
  const hit = rows.find(predicate);
  if (hit) {
    return { id, description, pass: true, evidence: JSON.stringify(hit) };
  }
  return {
    id,
    description,
    pass: false,
    note: `no row matched predicate among ${rows.length} evidence row(s)`,
  };
}

export function mustNotHave(
  rows: readonly EvidenceRow[],
  predicate: (r: EvidenceRow) => boolean,
  id: string,
  description: string,
): CheckResult {
  const hit = rows.find(predicate);
  if (!hit) {
    return { id, description, pass: true };
  }
  return {
    id,
    description,
    pass: false,
    evidence: JSON.stringify(hit),
    note: "unexpected row found",
  };
}

// ---------------------------------------------------------------------------
// File / directory checks
// ---------------------------------------------------------------------------

export async function mustExistFile(
  path: string,
  id: string,
  description: string,
): Promise<CheckResult> {
  try {
    const s = await stat(path);
    if (s.isFile()) return { id, description, pass: true };
    return { id, description, pass: false, note: `path exists but is not a file: ${path}` };
  } catch {
    return { id, description, pass: false, note: `file not found: ${path}` };
  }
}

export async function mustExistDir(
  path: string,
  id: string,
  description: string,
): Promise<CheckResult> {
  try {
    const s = await stat(path);
    if (s.isDirectory()) return { id, description, pass: true };
    return { id, description, pass: false, note: `path exists but is not a directory: ${path}` };
  } catch {
    return { id, description, pass: false, note: `directory not found: ${path}` };
  }
}

// ---------------------------------------------------------------------------
// Sub-agent exit sentinel
// ---------------------------------------------------------------------------

export async function readSubAgentExit(projectDir: string): Promise<unknown | null> {
  const path = join(projectDir, ".maestro/scenarios/sub-agent-exit.json");
  try {
    const content = await readFile(path, "utf8");
    return JSON.parse(content);
  } catch {
    return null;
  }
}

// ---------------------------------------------------------------------------
// Standalone runner shared by every scenario's rubric.ts
// ---------------------------------------------------------------------------

export function printRubricResult(scenarioName: string, result: RubricResult): void {
  for (const c of result.checks) {
    const marker = c.pass ? "[PASS]" : "[FAIL]";
    console.log(`${marker} ${c.id}: ${c.description}`);
    if (!c.pass && c.note) console.log(`       note: ${c.note}`);
    if (c.evidence) console.log(`       evidence: ${c.evidence}`);
  }
  console.log(result.pass ? `\nSCENARIO ${scenarioName}: PASS` : `\nSCENARIO ${scenarioName}: FAIL`);
}

export async function runRubricMain(
  scenarioName: string,
  runRubric: (projectDir: string) => Promise<RubricResult>,
): Promise<never> {
  const projectDir = process.argv[2];
  if (!projectDir) {
    console.error(`Usage: bun tests/scenarios/${scenarioName}/rubric.ts <project-dir>`);
    process.exit(1);
  }
  const result = await runRubric(projectDir);
  printRubricResult(scenarioName, result);
  process.exit(result.pass ? 0 : 1);
}
