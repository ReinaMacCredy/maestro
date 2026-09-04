import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { basename, dirname, join, resolve, sep } from "node:path";
import { CliError, type CliInvocation, type CliResult } from "../kernel/cli.ts";
import type { BuiltInPlugin, PluginContext } from "../kernel/loader.ts";
import { resolveHomeDirectory, resolveHubRoom, samePath } from "./home.ts";
import { registerSessionCommand } from "./session-required.ts";

export interface MemoryFact {
  id: string;
  slug: string;
  kind: string;
  description: string;
  body: string;
  source: string;
  sourcePath: string;
  contentHash: string;
  recordedAt: string | null;
  state: "active" | "superseded";
  supersedesId: string | null;
  supersededById: string | null;
  retiredReason: string | null;
  createdAt: string;
  updatedAt: string;
}

interface FactRow {
  id: string;
  slug: string;
  kind: string;
  description: string;
  body: string;
  source: string;
  source_path: string;
  content_hash: string;
  recorded_at: string | null;
  state: "active" | "superseded";
  supersedes_id: string | null;
  superseded_by_id: string | null;
  retired_reason: string | null;
  created_at: string;
  updated_at: string;
}

interface BufferFact {
  body: string;
  contentHash: string;
  description: string;
  kind: string;
  problems: string[];
  recordedAt: string | null;
  slug: string;
  source: string;
  sourcePath: string;
  supersedes: string | null;
}

export interface IngestAction {
  action: "promoted" | "updated" | "skipped" | "refused";
  id: string | null;
  path: string;
  reason: string | null;
  slug: string;
  source: string;
}

const kindOrder = ["user", "feedback", "project", "reference"];
const renderHeader = /^<!-- rendered by maestro memory render; content-hash: ([0-9a-f]{64}); .* -->\n/;

function fromRow(row: FactRow): MemoryFact {
  return {
    id: row.id,
    slug: row.slug,
    kind: row.kind,
    description: row.description,
    body: row.body,
    source: row.source,
    sourcePath: row.source_path,
    contentHash: row.content_hash,
    recordedAt: row.recorded_at,
    state: row.state,
    supersedesId: row.supersedes_id,
    supersededById: row.superseded_by_id,
    retiredReason: row.retired_reason,
    createdAt: row.created_at,
    updatedAt: row.updated_at,
  };
}

function sha256(text: string): string {
  return createHash("sha256").update(text).digest("hex");
}

function required(invocation: CliInvocation, index: number, label: string): string {
  const value = invocation.positionals[index]?.trim();
  if (!value) throw new CliError("MISSING_ARGUMENT", `missing ${label}`);
  return value;
}

function stringOption(invocation: CliInvocation, name: string): string | null {
  const value = invocation.options[name];
  return typeof value === "string" && value.trim() ? value.trim() : null;
}

function listOption(invocation: CliInvocation, name: string): string[] {
  const value = invocation.options[name];
  if (typeof value === "string") return [value];
  return Array.isArray(value) ? value : [];
}

// Memory verbs write the Hub store only (d775). A project store never holds
// global facts, so the verb refuses there instead of writing through a child.
function requireHub(context: PluginContext): { room: string; storePath: string } {
  const hub = resolveHubRoom();
  if (!samePath(context.store.path, hub.storePath)) {
    throw new CliError(
      "NOT_HUB_STORE",
      `maestro memory reads and writes the Hub store; run it from ${hub.room}`,
      { room: hub.room },
    );
  }
  return hub;
}

function getFact(context: PluginContext, key: string): MemoryFact | null {
  const row = context.store.database
    .query<FactRow, [string, string]>("SELECT * FROM memory_facts WHERE id = ? OR slug = ?")
    .get(key, key);
  return row ? fromRow(row) : null;
}

function listFacts(context: PluginContext, all: boolean): MemoryFact[] {
  return context.store.database
    .query<FactRow, []>(
      all
        ? "SELECT * FROM memory_facts ORDER BY slug"
        : "SELECT * FROM memory_facts WHERE state = 'active' ORDER BY slug",
    )
    .all()
    .map(fromRow);
}

function hasSearchIndex(context: PluginContext): boolean {
  return context.store.database
    .query<{ present: number }, [string]>(
      "SELECT 1 AS present FROM sqlite_master WHERE type = 'table' AND name = ?",
    )
    .get("search_index") !== null;
}

function indexFact(context: PluginContext, fact: MemoryFact): void {
  if (!hasSearchIndex(context)) return;
  context.store.database
    .query("DELETE FROM search_index WHERE surface = 'memory' AND entity_id = ?")
    .run(fact.id);
  context.store.database
    .query("INSERT INTO search_index(surface, entity_id, text) VALUES ('memory', ?, ?)")
    .run(fact.id, `${fact.slug} ${fact.description} ${fact.body}`);
}

// --- buffer parsing ---------------------------------------------------------

function parseFrontmatter(text: string): { fields: Record<string, string>; body: string } | null {
  if (!text.startsWith("---\n")) return null;
  const end = text.indexOf("\n---", 4);
  if (end === -1) return null;
  const fields: Record<string, string> = {};
  let parent: string | null = null;
  for (const line of text.slice(4, end).split("\n")) {
    const match = /^(\s*)([A-Za-z_][\w-]*):\s*(.*)$/.exec(line);
    if (!match) continue;
    const indent = match[1] ?? "";
    const key = match[2] ?? "";
    const value = (match[3] ?? "").trim().replace(/^["']|["']$/g, "");
    if (indent.length === 0) {
      parent = value === "" ? key : null;
      if (value !== "") fields[key] = value;
    } else if (parent) {
      fields[`${parent}.${key}`] = value;
    }
  }
  const afterMarker = text.indexOf("\n", end + 1);
  return { fields, body: afterMarker === -1 ? "" : text.slice(afterMarker + 1) };
}

const timestampPrefix = /^\d{4}-?\d{2}-?\d{2}T[\d:-]+(?:[+-][\d:-]+)?-/;

function readBufferFact(path: string, source: string): BufferFact {
  const text = readFileSync(path, "utf8");
  const parsed = parseFrontmatter(text);
  const stem = basename(path).replace(/\.md$/, "");
  const problems: string[] = [];
  let slug: string;
  let description: string;
  let kind: string;
  let body: string;
  let recordedAt: string | null = null;
  let supersedes: string | null = null;
  if (parsed) {
    slug = parsed.fields.name ?? stem;
    description = parsed.fields.description ?? "";
    kind = parsed.fields["metadata.type"] ?? parsed.fields.type ?? "project";
    body = parsed.body.trim();
    recordedAt = parsed.fields["metadata.modified"] ?? parsed.fields.modified ?? parsed.fields.date ??
      null;
    supersedes = parsed.fields.supersedes ?? null;
  } else {
    slug = stem.replace(timestampPrefix, "");
    const lines = text.split("\n");
    const heading = lines.find((line) => /^#\s+/.test(line));
    description = (heading ? heading.replace(/^#+\s+/, "") : lines.find((line) => line.trim()) ?? "")
      .trim();
    kind = "project";
    body = (heading ? lines.filter((line) => line !== heading).join("\n") : text).trim();
  }
  if (!recordedAt) {
    try {
      recordedAt = statSync(path).mtime.toISOString();
    } catch {
      recordedAt = null;
    }
  }
  slug = slug.trim().toLowerCase().replace(/[^a-z0-9._-]+/g, "-").replace(/^-+|-+$/g, "");
  if (!slug) problems.push("no slug");
  if (!description) problems.push("no description");
  if (!body) problems.push("empty body");
  return {
    body,
    contentHash: sha256(`${description}\n\n${body}`),
    description,
    kind,
    problems,
    recordedAt,
    slug,
    source,
    sourcePath: path,
    supersedes,
  };
}

function markdownFiles(directory: string): string[] {
  if (!existsSync(directory)) return [];
  return readdirSync(directory)
    .filter((name) => name.endsWith(".md") && name !== "MEMORY.md")
    .sort()
    .map((name) => join(directory, name));
}

// Claude keys project memory dirs by the encoded cwd; fixtures under tmp and
// var/folders leave dirs behind that never hold owner facts.
const temporaryProject = /^-(private-)?(tmp|var-folders)-/;

export function sourceFor(directory: string): string {
  if (directory.includes(`${sep}.claude${sep}projects${sep}`)) return "claude-auto";
  if (directory.includes(`${sep}.codex${sep}memories${sep}`)) return "codex-adhoc";
  return "buffer";
}

export function defaultBufferDirectories(home: string): Array<{ directory: string; source: string }> {
  const projects = join(home, ".claude", "projects");
  const claude = existsSync(projects)
    ? readdirSync(projects)
      .filter((name) => !temporaryProject.test(name))
      .sort()
      .map((name) => join(projects, name, "memory"))
      .filter((directory) => existsSync(directory))
      .map((directory) => ({ directory, source: "claude-auto" }))
    : [];
  return [
    ...claude,
    { directory: join(home, ".codex", "memories", "extensions", "ad_hoc", "notes"), source: "codex-adhoc" },
  ];
}

function collectBufferFacts(directories: Array<{ directory: string; source: string }>): BufferFact[] {
  const facts = directories.flatMap(({ directory, source }) =>
    markdownFiles(directory).map((path) => readBufferFact(path, source))
  );
  // A retraction written after its target must land after it inside one batch.
  return facts.sort((left, right) =>
    (left.recordedAt ?? "~").localeCompare(right.recordedAt ?? "~") || left.slug.localeCompare(right.slug)
  );
}

// --- ingest -----------------------------------------------------------------

interface ViewEntry {
  hash: string;
  id: string;
  retiredReason: string | null;
  slug: string;
  sourcePath: string;
  state: "active" | "superseded";
  supersededById: string | null;
}

function ingest(
  context: PluginContext,
  facts: BufferFact[],
  dryRun: boolean,
): IngestAction[] {
  const view = new Map<string, ViewEntry>();
  const byId = new Map<string, ViewEntry>();
  for (const fact of listFacts(context, true)) {
    const entry: ViewEntry = {
      hash: fact.contentHash,
      id: fact.id,
      retiredReason: fact.retiredReason,
      slug: fact.slug,
      sourcePath: fact.sourcePath,
      state: fact.state,
      supersededById: fact.supersededById,
    };
    view.set(fact.slug, entry);
    byId.set(fact.id, entry);
  }
  const now = new Date().toISOString();
  const database = context.store.database;
  const actions: IngestAction[] = [];
  const record = (action: IngestAction["action"], fact: BufferFact, id: string | null, reason: string | null) => {
    actions.push({ action, id, path: fact.sourcePath, reason, slug: fact.slug, source: fact.source });
  };
  const insert = (fact: BufferFact, supersedesId: string | null): string => {
    const id = dryRun ? `m?` : context.store.nextPrefixedId("memory_facts", "m");
    if (!dryRun) {
      database
        .query(
          `INSERT INTO memory_facts(id, slug, kind, description, body, source, source_path, content_hash,
             recorded_at, state, supersedes_id, superseded_by_id, retired_reason, created_at, updated_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 'active', ?, NULL, NULL, ?, ?)`,
        )
        .run(
          id,
          fact.slug,
          fact.kind,
          fact.description,
          fact.body,
          fact.source,
          fact.sourcePath,
          fact.contentHash,
          fact.recordedAt,
          supersedesId,
          now,
          now,
        );
      indexFact(context, getFact(context, id) as MemoryFact);
    }
    const entry: ViewEntry = {
      hash: fact.contentHash,
      id,
      retiredReason: null,
      slug: fact.slug,
      sourcePath: fact.sourcePath,
      state: "active",
      supersededById: null,
    };
    view.set(fact.slug, entry);
    byId.set(id, entry);
    return id;
  };
  const update = (fact: BufferFact, entry: ViewEntry): void => {
    if (!dryRun) {
      database
        .query(
          `UPDATE memory_facts
           SET kind = ?, description = ?, body = ?, source = ?, source_path = ?, content_hash = ?,
               recorded_at = ?, updated_at = ?
           WHERE id = ?`,
        )
        .run(
          fact.kind,
          fact.description,
          fact.body,
          fact.source,
          fact.sourcePath,
          fact.contentHash,
          fact.recordedAt,
          now,
          entry.id,
        );
      indexFact(context, getFact(context, entry.id) as MemoryFact);
    }
    entry.hash = fact.contentHash;
    entry.sourcePath = fact.sourcePath;
  };
  // Two files sharing a slug would otherwise overwrite each other on every
  // pass; a same-slug fact from another file is a conflict to rename, not an update.
  const conflicts = (fact: BufferFact, entry: ViewEntry): boolean =>
    entry.hash !== fact.contentHash && entry.sourcePath !== fact.sourcePath;
  const supersede = (target: ViewEntry, byId_: string): void => {
    if (!dryRun) {
      database
        .query("UPDATE memory_facts SET state = 'superseded', superseded_by_id = ?, updated_at = ? WHERE id = ?")
        .run(byId_, now, target.id);
    }
    target.state = "superseded";
    target.supersededById = byId_;
  };

  const run = () => {
    for (const fact of facts) {
      if (fact.problems.length > 0) {
        record("refused", fact, null, `no evidence: ${fact.problems.join(", ")}`);
        continue;
      }
      const existing = view.get(fact.slug) ?? null;
      if (existing?.state === "superseded") {
        record(
          "refused",
          fact,
          existing.id,
          existing.supersededById
            ? `superseded: ${existing.id} was superseded by ${existing.supersededById}`
            : `retracted: ${existing.id}${existing.retiredReason ? ` (${existing.retiredReason})` : ""}`,
        );
        continue;
      }
      if (fact.supersedes) {
        const target = view.get(fact.supersedes) ?? byId.get(fact.supersedes) ?? null;
        if (!target) {
          record("refused", fact, null, `supersedes unknown fact: ${fact.supersedes}`);
          continue;
        }
        if (target.slug === fact.slug) {
          record("refused", fact, target.id, "a fact cannot supersede its own slug; a changed body updates in place");
          continue;
        }
        // The retraction's own re-ingest finds its target already superseded
        // by itself; that is a duplicate, not a second supersession.
        if (target.state === "superseded" && !(existing && target.supersededById === existing.id)) {
          record(
            "refused",
            fact,
            target.id,
            `supersedes ${target.id}, already superseded by ${target.supersededById ?? "a retraction"}`,
          );
          continue;
        }
        if (existing) {
          if (conflicts(fact, existing)) {
            record("refused", fact, existing.id, `slug conflict: ${existing.id} comes from ${existing.sourcePath}; rename one`);
            continue;
          }
          if (existing.hash === fact.contentHash) {
            record("skipped", fact, existing.id, "duplicate");
          } else {
            update(fact, existing);
            record("updated", fact, existing.id, `supersedes ${target.id}`);
          }
          supersede(target, existing.id);
          continue;
        }
        const id = insert(fact, target.id);
        supersede(target, id);
        record("promoted", fact, id, `supersedes ${target.id}`);
        continue;
      }
      if (existing) {
        if (conflicts(fact, existing)) {
          record("refused", fact, existing.id, `slug conflict: ${existing.id} comes from ${existing.sourcePath}; rename one`);
        } else if (existing.hash === fact.contentHash) {
          record("skipped", fact, existing.id, "duplicate");
        } else {
          update(fact, existing);
          record("updated", fact, existing.id, "body changed");
        }
        continue;
      }
      record("promoted", fact, insert(fact, null), null);
    }
  };
  if (dryRun) run();
  else database.transaction(run)();
  return actions;
}

function countActions(actions: IngestAction[]): Record<IngestAction["action"], number> {
  const counts = { promoted: 0, updated: 0, skipped: 0, refused: 0 };
  for (const action of actions) counts[action.action] += 1;
  return counts;
}

function formatAction(action: IngestAction): string {
  const id = action.id ? ` ${action.id}` : "";
  const reason = action.reason ? ` — ${action.reason}` : "";
  return `${action.action}${id} ${action.slug} (${action.source})${reason}`;
}

// --- render -----------------------------------------------------------------

export function renderIndex(facts: MemoryFact[]): { content: string; hash: string } {
  const kinds = [...new Set(facts.map((fact) => fact.kind))].sort((left, right) => {
    const l = kindOrder.indexOf(left);
    const r = kindOrder.indexOf(right);
    return (l === -1 ? kindOrder.length : l) - (r === -1 ? kindOrder.length : r) || left.localeCompare(right);
  });
  const lines = ["# Global memory", "", "Rendered from the Hub store. Recall the body with `maestro memory show <slug>`;", "change a fact through `maestro memory ingest` or `maestro memory retract`.", ""];
  for (const kind of kinds) {
    lines.push(`## ${kind.charAt(0).toUpperCase()}${kind.slice(1)}`, "");
    for (const fact of facts.filter((candidate) => candidate.kind === kind)) {
      const date = fact.recordedAt ? ` ${fact.recordedAt.slice(0, 10)}` : "";
      lines.push(`- ${fact.slug}: ${fact.description} (${fact.source}${date})`);
    }
    lines.push("");
  }
  const content = lines.join("\n");
  return { content, hash: sha256(content) };
}

function readRendered(path: string): { body: string; hash: string | null } | null {
  if (!existsSync(path)) return null;
  const text = readFileSync(path, "utf8");
  const match = renderHeader.exec(text);
  return match ? { body: text.slice(match[0].length), hash: match[1] ?? null } : { body: text, hash: null };
}

export function formatFact(fact: MemoryFact): string {
  return [
    `${fact.id} [${fact.state}] ${fact.slug} (${fact.kind}, ${fact.source})`,
    `description: ${fact.description}`,
    fact.recordedAt ? `recorded: ${fact.recordedAt}` : null,
    `source: ${fact.sourcePath}`,
    fact.supersedesId ? `supersedes: ${fact.supersedesId}` : null,
    fact.supersededById ? `superseded by: ${fact.supersededById}` : null,
    fact.retiredReason ? `retired: ${fact.retiredReason}` : null,
    "",
    fact.body,
  ]
    .filter((line): line is string => line !== null)
    .join("\n");
}

export const memoryPlugin: BuiltInPlugin = {
  name: "memory",
  requires:
    "memory ingest/list/show/retract/render: the Hub store holds global memory; per-tool buffers promote through supersession, dedup and evidence gates; the injected index is a render (d775, d776)",
  apply(context) {
    context.store.migrate(`
      CREATE TABLE IF NOT EXISTS memory_facts (
        id TEXT PRIMARY KEY,
        slug TEXT NOT NULL UNIQUE,
        kind TEXT NOT NULL,
        description TEXT NOT NULL,
        body TEXT NOT NULL,
        source TEXT NOT NULL,
        source_path TEXT NOT NULL,
        content_hash TEXT NOT NULL,
        recorded_at TEXT,
        state TEXT NOT NULL CHECK(state IN ('active', 'superseded')),
        supersedes_id TEXT REFERENCES memory_facts(id),
        superseded_by_id TEXT REFERENCES memory_facts(id),
        retired_reason TEXT,
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL
      );
    `);
    if (!context.store.readOnly) {
      for (const fact of listFacts(context, true)) indexFact(context, fact);
    }

    context.effect(() =>
      registerSessionCommand(
        context,
        "memory ingest",
        (invocation): CliResult => {
          requireHub(context);
          const dryRun = invocation.options["dry-run"] === true;
          const from = listOption(invocation, "from");
          const directories = from.length > 0
            ? from.map((directory) => ({ directory: resolve(directory), source: sourceFor(resolve(directory)) }))
            : defaultBufferDirectories(resolveHomeDirectory());
          const facts = collectBufferFacts(directories);
          const actions = ingest(context, facts, dryRun);
          const counts = countActions(actions);
          if (!dryRun) {
            context.log.append({
              type: "memory.ingest",
              entityType: "memory",
              entityId: "hub",
              sessionId: context.sessions.current().id,
              payload: { counts, directories: directories.map((entry) => entry.directory) },
            });
          }
          const summary = `${dryRun ? "dry-run: " : ""}promoted ${counts.promoted}, updated ${counts.updated}, skipped ${counts.skipped}, refused ${counts.refused} (${facts.length} facts from ${directories.length} directories)`;
          return {
            data: { actions, counts, directories: directories.map((entry) => entry.directory), dryRun },
            text: [...actions.map(formatAction), summary].join("\n"),
          };
        },
        {
          description: "Promote buffer facts into the Hub through supersession, dedup and evidence gates.",
          flags: {
            "--dry-run": { description: "Report the gates' verdicts without writing." },
            "--from": { description: "Read this directory of markdown facts instead of the default buffers.", multiple: true, value: true },
          },
          json: true,
        },
      ),
    );

    context.effect(() =>
      registerSessionCommand(
        context,
        "memory retract",
        (invocation): CliResult => {
          requireHub(context);
          const key = required(invocation, 0, "fact id or slug");
          const reason = stringOption(invocation, "reason");
          if (!reason) throw new CliError("MISSING_ARGUMENT", "missing --reason");
          const fact = getFact(context, key);
          if (!fact) throw new CliError("NOT_FOUND", `memory fact not found: ${key}`, { key });
          if (fact.state === "superseded") {
            throw new CliError("INVALID_STATE", `${fact.id} is already superseded`, { id: fact.id });
          }
          const now = new Date().toISOString();
          context.store.database
            .query("UPDATE memory_facts SET state = 'superseded', retired_reason = ?, updated_at = ? WHERE id = ?")
            .run(reason, now, fact.id);
          const updated = getFact(context, fact.id) as MemoryFact;
          indexFact(context, updated);
          context.log.append({
            type: "memory.retract",
            entityType: "memory",
            entityId: fact.id,
            sessionId: context.sessions.current().id,
            payload: { reason, slug: fact.slug },
          });
          return { data: { fact: updated }, text: `${fact.id} ${fact.slug} retracted: ${reason}` };
        },
        {
          description: "Retire one fact so the buffers can never promote it again.",
          flags: { "--reason": { description: "Why the fact no longer holds.", value: true } },
          json: true,
          positionals: [{ name: "fact", required: true }],
        },
      ),
    );

    context.effect(() =>
      context.cli.register(
        "memory list",
        (invocation): CliResult => {
          requireHub(context);
          const facts = listFacts(context, invocation.options.all === true);
          return {
            data: { facts },
            text: facts.length > 0
              ? facts.map((fact) => `${fact.id} [${fact.state}] ${fact.slug} (${fact.kind}): ${fact.description}`).join("\n")
              : "no memory facts; run: maestro memory ingest --dry-run",
          };
        },
        {
          description: "List Hub memory facts.",
          flags: { "--all": { description: "Include superseded and retracted facts." } },
          mutates: false,
        },
      ),
    );

    context.effect(() =>
      context.cli.register(
        "memory show",
        (invocation): CliResult => {
          requireHub(context);
          const key = required(invocation, 0, "fact id or slug");
          const fact = getFact(context, key);
          if (!fact) throw new CliError("NOT_FOUND", `memory fact not found: ${key}; run: maestro memory list`, { key });
          return { data: { fact }, text: formatFact(fact) };
        },
        {
          description: "Show one Hub memory fact with its body and links.",
          json: true,
          mutates: false,
          positionals: [{ name: "fact", required: true }],
        },
      ),
    );

    context.effect(() =>
      context.cli.register(
        "memory render",
        (invocation): CliResult => {
          const hub = requireHub(context);
          const check = invocation.options.check === true;
          const force = invocation.options.force === true;
          const path = resolve(stringOption(invocation, "out") ?? join(hub.room, "MEMORY.md"));
          const facts = listFacts(context, false);
          const rendered = renderIndex(facts);
          const existing = readRendered(path);
          const drifted = existing !== null && (existing.hash === null || sha256(existing.body) !== existing.hash);
          if (drifted && !force) {
            throw new CliError(
              "MEMORY_INDEX_DRIFT",
              `${path} was edited by hand since its last render; move the change into the Hub store (maestro memory ingest or retract), or rerun with --force to overwrite it`,
              { path },
            );
          }
          const status = existing === null ? "missing" : sha256(existing.body) === rendered.hash ? "current" : "stale";
          if (check) {
            if (status !== "current") {
              throw new CliError(
                status === "missing" ? "MEMORY_INDEX_MISSING" : "MEMORY_INDEX_STALE",
                `${path} is ${status}; run: maestro memory render`,
                { path, status },
              );
            }
            return { data: { facts: facts.length, hash: rendered.hash, path, status }, text: `${path} current (${facts.length} facts, ${rendered.hash.slice(0, 12)})` };
          }
          if (context.store.readOnly) {
            throw new CliError("READ_ONLY", "MAESTRO_READ_ONLY=1 cannot write the rendered index", { path });
          }
          mkdirSync(dirname(path), { recursive: true });
          writeFileSync(
            path,
            `<!-- rendered by maestro memory render; content-hash: ${rendered.hash}; do not hand-edit -->\n${rendered.content}`,
          );
          context.log.append({
            type: "memory.render",
            entityType: "memory",
            entityId: "hub",
            sessionId: context.sessions.current().id,
            payload: { facts: facts.length, hash: rendered.hash, path, previous: status },
          });
          return {
            data: { facts: facts.length, hash: rendered.hash, path, previous: status },
            text: `rendered ${path} (${facts.length} facts, ${rendered.hash.slice(0, 12)}; was ${status})`,
          };
        },
        {
          description: "Render the injected global index from the Hub store; refuse to overwrite a hand edit.",
          flags: {
            "--check": { description: "Report whether the rendered file is current without writing." },
            "--force": { description: "Overwrite a hand-edited render." },
            "--out": { description: "Write the render here instead of $HOME/maestro/MEMORY.md.", value: true },
          },
          json: true,
        },
      ),
    );
  },
};
