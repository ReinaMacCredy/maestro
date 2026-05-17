import { appendFile, mkdir, readdir, readFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import type {
  EvidenceFilter,
  EvidenceRow,
  EvidenceStorePort,
} from "./evidence-store.port.js";

const DEFAULT_EVIDENCE_DIR = ".maestro/evidence";

function dateStamp(date: Date): string {
  return date.toISOString().slice(0, 10);
}

function matchesFilter(row: EvidenceRow, filter: EvidenceFilter | undefined): boolean {
  if (!filter) return true;
  if (filter.kind !== undefined && row.kind !== filter.kind) return false;
  if (filter.task_id !== undefined) {
    const rowTaskId = "task_id" in row ? row.task_id : undefined;
    if (rowTaskId !== filter.task_id) return false;
  }
  if (filter.mission_id !== undefined) {
    const rowPlanId = "mission_id" in row ? row.mission_id : undefined;
    if (rowPlanId !== filter.mission_id) return false;
  }
  return true;
}

export interface JsonlEvidenceStoreOptions {
  readonly repoRoot: string;
  readonly subdir?: string;
  readonly clock?: () => Date;
}

export class JsonlEvidenceStore implements EvidenceStorePort {
  readonly #dir: string;
  readonly #clock: () => Date;

  constructor(options: JsonlEvidenceStoreOptions) {
    this.#dir = join(options.repoRoot, options.subdir ?? DEFAULT_EVIDENCE_DIR);
    this.#clock = options.clock ?? (() => new Date());
  }

  async append(row: EvidenceRow): Promise<void> {
    const file = join(this.#dir, `${dateStamp(this.#clock())}.jsonl`);
    await mkdir(dirname(file), { recursive: true });
    const line = `${JSON.stringify(row)}\n`;
    await appendFile(file, line, "utf8");
  }

  async list(filter?: EvidenceFilter): Promise<readonly EvidenceRow[]> {
    let entries: string[];
    try {
      entries = await readdir(this.#dir);
    } catch (err) {
      if ((err as NodeJS.ErrnoException).code === "ENOENT") return [];
      throw err;
    }
    const files = entries.filter((name) => name.endsWith(".jsonl")).sort();
    const rows: EvidenceRow[] = [];
    for (const name of files) {
      const text = await readFile(join(this.#dir, name), "utf8");
      for (const line of text.split("\n")) {
        if (line.length === 0) continue;
        const row = JSON.parse(line) as EvidenceRow;
        if (matchesFilter(row, filter)) rows.push(row);
      }
    }
    return rows;
  }

  // Walk newest file first and short-circuit on match. Evidence ids are
  // unique, so the first hit is the answer. Worst case is still O(rows),
  // but the average case is far smaller than read-all-then-find.
  async read(id: string): Promise<EvidenceRow | undefined> {
    let entries: string[];
    try {
      entries = await readdir(this.#dir);
    } catch (err) {
      if ((err as NodeJS.ErrnoException).code === "ENOENT") return undefined;
      throw err;
    }
    const files = entries
      .filter((name) => name.endsWith(".jsonl"))
      .sort()
      .reverse();
    for (const name of files) {
      const text = await readFile(join(this.#dir, name), "utf8");
      for (const line of text.split("\n")) {
        if (line.length === 0) continue;
        const row = JSON.parse(line) as EvidenceRow;
        if (row.id === id) return row;
      }
    }
    return undefined;
  }
}
