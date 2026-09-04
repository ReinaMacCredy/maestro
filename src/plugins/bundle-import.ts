import { cpSync, existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import { basename, join, relative } from "node:path";
import type { PluginContext } from "../kernel/loader.ts";
import { tableExists } from "../kernel/store.ts";
import { getTermByName, upsertTerm } from "./term.ts";

// One line of the import report per source item, so a .waymark/ tree can be
// reconciled item by item before it is removed (A2 of bundle maestro-v3).
export interface ImportEntry {
  action: "imported" | "copied" | "exists" | "updated" | "skipped" | "unknown";
  detail: string | null;
  id: string | null;
  kind: "active" | "paused" | "archive" | "memory" | "adr" | "term" | "extra" | "file" | "dir";
  name: string;
}

export interface ImportReport {
  counts: Record<ImportEntry["kind"], Record<ImportEntry["action"], number>>;
  dryRun: boolean;
  entries: ImportEntry[];
  source: string;
  target: string;
}

const trioFiles = ["SPEC.md", "NOTES.md", "VERIFY.md"] as const;
const bundleStates = ["active", "paused", "archive"] as const;
const knownTopLevel = new Set([...bundleStates, "MEMORY.md", "CONTEXT.md", "adr"]);

interface MemoryLine {
  label: string | null;
  line: number;
  state: "locked" | "draft";
  text: string;
  unit: string | null;
}

interface MemoryScan {
  decisions: MemoryLine[];
  other: Array<{ line: number; text: string }>;
}

// Two shapes exist in the wild: astra-family link bullets
// `- [label](path): text` (also `—` separated), and synara/Weave memory-unit
// blocks whose bullets under a "Decisions" heading read `- \`LOCKED D-001\`: text`.
export function scanMemoryFile(text: string): MemoryScan {
  const decisions: MemoryLine[] = [];
  const other: MemoryScan["other"] = [];
  let unit: string | null = null;
  let inDecisions = false;
  text.split("\n").forEach((raw, index) => {
    const line = index + 1;
    const unitMarker = /^<!--\s*memory-unit:([^\s]+)\s*-->/.exec(raw);
    if (unitMarker) {
      unit = unitMarker[1] ?? null;
      inDecisions = false;
      return;
    }
    if (/^#{1,6}\s/.test(raw)) {
      inDecisions = unit !== null && /decision/i.test(raw);
      return;
    }
    if (!raw.startsWith("- ")) return;
    const body = raw.slice(2).trim();
    const linked = /^\[([^\]]+)\]\([^)]*\)\s*(?::|—|-)\s*(.+)$/.exec(body);
    if (linked) {
      decisions.push({ label: linked[1] ?? null, line, state: "locked", text: (linked[2] ?? "").trim(), unit: null });
      return;
    }
    if (inDecisions) {
      const coded = /^`?([A-Z]+)\s+([A-Z]+-\d+)`?\s*:\s*(.+)$/.exec(body);
      if (coded) {
        decisions.push({
          label: coded[2] ?? null,
          line,
          state: coded[1] === "LOCKED" ? "locked" : "draft",
          text: (coded[3] ?? "").trim(),
          unit,
        });
      } else {
        decisions.push({ label: null, line, state: "locked", text: body, unit });
      }
      return;
    }
    // `Key: value` bullets are bundle metadata (work id, proofs); any other
    // plain bullet is repository memory and lands as a draft to review.
    if (/^[A-Z][A-Za-z ]{1,30}:\s/.test(body) || unit !== null) {
      other.push({ line, text: body });
      return;
    }
    decisions.push({ label: null, line, state: "draft", text: body, unit: null });
  });
  return { decisions, other };
}

interface AdrScan {
  body: string;
  state: "locked" | "superseded" | "draft";
  title: string;
}

export function scanAdr(text: string): AdrScan {
  const lines = text.split("\n");
  const heading = lines.find((line) => /^#\s+/.test(line)) ?? "";
  const title = heading.replace(/^#\s+/, "").replace(/^ADR[- ]?\d+\s*[:.-]\s*/i, "").trim();
  const status = (lines.find((line) => /^\*{0,2}Status\*{0,2}:/i.test(line)) ?? "")
    .replace(/^\*{0,2}Status\*{0,2}:\s*/i, "")
    .trim()
    .toLowerCase();
  const state = status.startsWith("accept") ? "locked" : status.startsWith("supersed") ? "superseded" : "draft";
  const body = lines.filter((line) => line !== heading).join("\n").trim();
  return { body, state, title };
}

export function scanContextTerms(text: string): Array<{ definition: string; name: string }> {
  const terms: Array<{ definition: string; name: string }> = [];
  const lines = text.split("\n");
  for (let index = 0; index < lines.length; index += 1) {
    const match = /^\*\*(.+?)\*\*:\s*$/.exec(lines[index] ?? "");
    if (!match) continue;
    const parts: string[] = [];
    for (let cursor = index + 1; cursor < lines.length; cursor += 1) {
      const line = (lines[cursor] ?? "").trim();
      if (!line) break;
      parts.push(line.replace(/^_Avoid_:\s*/, "Avoid: "));
    }
    terms.push({ definition: parts.join(" "), name: (match[1] ?? "").trim().replaceAll(/\s+/g, "-") });
  }
  return terms;
}

function listDirectory(directory: string): string[] {
  return existsSync(directory) ? readdirSync(directory).sort() : [];
}

function countFiles(directory: string): number {
  let count = 0;
  for (const name of listDirectory(directory)) {
    const path = join(directory, name);
    count += statSync(path).isDirectory() ? countFiles(path) : 1;
  }
  return count;
}

function indexArchived(context: PluginContext, id: string, text: string): void {
  if (!tableExists(context.store.database, "search_index")) return;
  context.store.database
    .query("DELETE FROM search_index WHERE surface = 'bundle' AND entity_id = ?")
    .run(id);
  context.store.database
    .query("INSERT INTO search_index(surface, entity_id, text) VALUES ('bundle', ?, ?)")
    .run(id, text);
}

function readTrioSync(directory: string): { spec: string | null; notes: string | null; verify: string | null } {
  const read = (name: string) => {
    const path = join(directory, name);
    return existsSync(path) ? readFileSync(path, "utf8") : null;
  };
  return { spec: read("SPEC.md"), notes: read("NOTES.md"), verify: read("VERIFY.md") };
}

function decisionIdByText(context: PluginContext, text: string): string | null {
  return context.store.database
    .query<{ id: string }, [string]>("SELECT id FROM decisions WHERE text = ? ORDER BY id LIMIT 1")
    .get(text)?.id ?? null;
}

function emptyCounts(): ImportReport["counts"] {
  const kinds: ImportEntry["kind"][] = ["active", "paused", "archive", "memory", "adr", "term", "extra", "file", "dir"];
  const actions: ImportEntry["action"][] = ["imported", "copied", "exists", "updated", "skipped", "unknown"];
  const counts = {} as ImportReport["counts"];
  for (const kind of kinds) {
    counts[kind] = {} as Record<ImportEntry["action"], number>;
    for (const action of actions) counts[kind][action] = 0;
  }
  return counts;
}

export function importWaymarkTree(
  context: PluginContext,
  source: string,
  target: string,
  dryRun: boolean,
): ImportReport {
  const entries: ImportEntry[] = [];
  const add = (entry: ImportEntry) => entries.push(entry);
  const now = new Date().toISOString();
  const database = context.store.database;
  const bundleExists = (id: string) =>
    database.query<{ id: string }, [string]>("SELECT id FROM bundles WHERE id = ?").get(id) !== null;

  const run = () => {
    for (const state of bundleStates) {
      const stateDirectory = join(source, state);
      for (const name of listDirectory(stateDirectory)) {
        const path = join(stateDirectory, name);
        if (!statSync(path).isDirectory()) {
          add({ action: "unknown", detail: `file under ${state}/`, id: null, kind: "file", name: `${state}/${name}` });
          continue;
        }
        const trio = readTrioSync(path);
        const extras = listDirectory(path).filter((entry) => !(trioFiles as readonly string[]).includes(entry));
        if (trio.spec === null && trio.notes === null && trio.verify === null) {
          add({ action: "unknown", detail: "no SPEC.md/NOTES.md/VERIFY.md", id: null, kind: "dir", name: `${state}/${name}` });
          continue;
        }
        if (bundleExists(name)) {
          add({ action: "exists", detail: null, id: name, kind: state, name: `${state}/${name}` });
          continue;
        }
        const directory = join(target, ".maestro", "bundle", name);
        if (existsSync(directory)) {
          add({ action: "skipped", detail: `${directory} already exists`, id: null, kind: state, name: `${state}/${name}` });
          continue;
        }
        if (state === "archive") {
          // The store snapshot is the record; the directory copy keeps the
          // files beside the trio (live-test logs, prompts) from being lost.
          if (!dryRun) {
            cpSync(path, directory, { recursive: true });
            database
              .query(
                `INSERT INTO bundles (id, state, directory, spec, notes, verify, created_at, updated_at)
                 VALUES (?, 'archived', ?, ?, ?, ?, ?, ?)`,
              )
              .run(name, directory, trio.spec, trio.notes, trio.verify, now, now);
            indexArchived(context, name, [name, trio.spec, trio.notes, trio.verify].filter((part) => part).join("\n"));
          }
          add({ action: "imported", detail: `snapshot in store, copied to ${relative(target, directory)}`, id: name, kind: "archive", name: `${state}/${name}` });
          for (const extra of extras) {
            add({ action: "copied", detail: null, id: name, kind: "extra", name: `${state}/${name}/${extra}` });
          }
          continue;
        }
        if (!dryRun) {
          cpSync(path, directory, { recursive: true });
          database
            .query(
              `INSERT INTO bundles (id, state, directory, paused_at, created_at, updated_at)
               VALUES (?, 'active', ?, ?, ?, ?)`,
            )
            .run(name, directory, state === "paused" ? now : null, now, now);
        }
        add({ action: "imported", detail: `copied to ${relative(target, directory)}`, id: name, kind: state, name: `${state}/${name}` });
        for (const extra of extras) {
          add({ action: "copied", detail: null, id: name, kind: "extra", name: `${state}/${name}/${extra}` });
        }
      }
    }

    const memoryPath = join(source, "MEMORY.md");
    if (existsSync(memoryPath)) {
      const scan = scanMemoryFile(readFileSync(memoryPath, "utf8"));
      for (const line of scan.decisions) {
        const existing = decisionIdByText(context, line.text);
        const name = `MEMORY.md:${line.line}`;
        if (existing) {
          add({ action: "exists", detail: null, id: existing, kind: "memory", name });
          continue;
        }
        const id = dryRun ? "d?" : context.store.nextPrefixedId("decisions", "d");
        if (!dryRun) {
          const provenance = [
            `imported from ${relative(target, memoryPath) || memoryPath}:${line.line}`,
            line.label ? `label ${line.label}` : null,
            line.unit ? `memory unit ${line.unit}` : null,
          ].filter((part) => part).join("; ");
          database
            .query(
              `INSERT INTO decisions (id, text, rationale, state, parent_id, work_id, supersedes_id, superseded_by_id, created_at, updated_at)
               VALUES (?, ?, ?, ?, NULL, NULL, NULL, NULL, ?, ?)`,
            )
            .run(id, line.text, provenance, line.state, now, now);
        }
        add({ action: "imported", detail: line.label ?? line.unit ?? `${line.state}, unlinked bullet`, id, kind: "memory", name });
      }
      for (const line of scan.other) {
        add({ action: "skipped", detail: line.text.slice(0, 80), id: null, kind: "memory", name: `MEMORY.md:${line.line}` });
      }
    }

    for (const name of listDirectory(join(source, "adr"))) {
      const path = join(source, "adr", name);
      if (!name.endsWith(".md") || statSync(path).isDirectory()) {
        add({ action: "unknown", detail: null, id: null, kind: "file", name: `adr/${name}` });
        continue;
      }
      const adr = scanAdr(readFileSync(path, "utf8"));
      if (!adr.title) {
        add({ action: "unknown", detail: "no title", id: null, kind: "adr", name: `adr/${name}` });
        continue;
      }
      const existing = decisionIdByText(context, adr.title);
      if (existing) {
        add({ action: "exists", detail: null, id: existing, kind: "adr", name: `adr/${name}` });
        continue;
      }
      const id = dryRun ? "d?" : context.store.nextPrefixedId("decisions", "d");
      if (!dryRun) {
        database
          .query(
            `INSERT INTO decisions (id, text, rationale, state, parent_id, work_id, supersedes_id, superseded_by_id, created_at, updated_at)
             VALUES (?, ?, ?, ?, NULL, NULL, NULL, NULL, ?, ?)`,
          )
          .run(id, adr.title, `imported from ${relative(target, path) || path}\n\n${adr.body}`, adr.state, now, now);
      }
      add({ action: "imported", detail: adr.state, id, kind: "adr", name: `adr/${name}` });
    }

    const contextPath = join(source, "CONTEXT.md");
    if (existsSync(contextPath)) {
      for (const term of scanContextTerms(readFileSync(contextPath, "utf8"))) {
        const existing = getTermByName(context, term.name);
        if (existing && existing.definition === term.definition) {
          add({ action: "exists", detail: null, id: existing.id, kind: "term", name: `CONTEXT.md ${term.name}` });
          continue;
        }
        let id = existing?.id ?? "t?";
        if (!dryRun) id = upsertTerm(context, term.name, term.definition).term.id;
        add({ action: existing ? "updated" : "imported", detail: null, id, kind: "term", name: `CONTEXT.md ${term.name}` });
      }
    }

    for (const name of listDirectory(source)) {
      if (knownTopLevel.has(name)) continue;
      const path = join(source, name);
      if (statSync(path).isDirectory()) {
        add({ action: "unknown", detail: `${countFiles(path)} files, not imported`, id: null, kind: "dir", name });
      } else {
        add({ action: "skipped", detail: "not imported", id: null, kind: "file", name });
      }
    }
  };

  if (dryRun) run();
  else database.transaction(run)();

  const counts = emptyCounts();
  for (const entry of entries) counts[entry.kind][entry.action] += 1;
  return { counts, dryRun, entries, source, target };
}

export function formatImportReport(report: ImportReport): string {
  const lines = report.entries.map((entry) =>
    `${entry.kind} ${entry.name} -> ${entry.action}${entry.id ? ` ${entry.id}` : ""}${entry.detail ? ` (${entry.detail})` : ""}`
  );
  const summary = (Object.keys(report.counts) as ImportEntry["kind"][])
    .map((kind) => {
      const row = report.counts[kind];
      const total = Object.values(row).reduce((sum, value) => sum + value, 0);
      if (total === 0) return null;
      const parts = (Object.keys(row) as ImportEntry["action"][])
        .filter((action) => row[action] > 0)
        .map((action) => `${row[action]} ${action}`);
      return `${kind}: ${total} (${parts.join(", ")})`;
    })
    .filter((line): line is string => line !== null);
  lines.push(`--- ${basename(report.source)} from ${report.source}`, ...summary);
  lines.push(report.dryRun ? "dry-run: nothing written" : `imported into ${report.target}`);
  return lines.join("\n");
}
