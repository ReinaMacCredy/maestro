import { Database } from "bun:sqlite";
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { basename, dirname, join, resolve, sep } from "node:path";
import { CliError, stringOptions, type CliInvocation, type CliResult } from "../kernel/cli.ts";
import type { BuiltInPlugin, PluginContext } from "../kernel/loader.ts";
import { resolveStoreLocation, tableExists } from "../kernel/store.ts";
import { resolveHomeDirectory, resolveHubRoom, samePath } from "./home.ts";
import type { BriefService } from "./coordination.ts";
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
// Reads take an id or a slug, so a slug shaped like a minted fact id
// (nextPrefixedId: "m" + integer) would shadow that row on show and retract.
const generatedFactId = /^m\d+$/;
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

// Memory verbs write the Hub store only (d775). From any other cwd a writing
// verb runs through the Hub's own CLI (lesson.ts precedent): the project store
// never holds global facts, and the child keeps a Hub store this binary cannot
// read honest. Path options travel as absolute paths because the child's cwd
// is the Hub room.
function hubStore(): { room: string; storePath: string } {
  const hub = resolveHubRoom();
  if (!existsSync(hub.storePath)) {
    throw new CliError(
      "HUB_UNAVAILABLE",
      `the Hub store ${hub.storePath} does not exist; run maestro install once`,
      { room: hub.room },
    );
  }
  return hub;
}

function writesHubDirectly(context: PluginContext): boolean {
  return samePath(context.store.path, resolveHubRoom().storePath);
}

// The child must land on the Hub store or it would forward again without end
// (a git-managed $HOME makes ~/maestro resolve its store to $HOME/.maestro).
// The parent checks the resolution first; the env mark refuses a second hop.
const forwardedMark = "MAESTRO_MEMORY_FORWARDED";

async function forwardToHub<T>(invocation: CliInvocation, verb: string): Promise<T> {
  const hub = hubStore();
  if (process.env[forwardedMark] === "1") {
    throw new CliError(
      "HUB_UNAVAILABLE",
      `maestro memory ${verb} was forwarded once already and ${process.cwd()} still is not the Hub store ${hub.storePath}`,
      { room: hub.room },
    );
  }
  const resolved = resolveStoreLocation(hub.room).path;
  if (!samePath(resolved, hub.storePath)) {
    throw new CliError(
      "HUB_UNAVAILABLE",
      `the Hub room ${hub.room} resolves its store to ${resolved}, not ${hub.storePath}; is ${hub.room} inside another git repository?`,
      { room: hub.room, resolved },
    );
  }
  const args = ["memory", verb, ...invocation.positionals];
  for (const [key, value] of Object.entries(invocation.options)) {
    for (const item of Array.isArray(value) ? value : [value]) {
      if (item === false) continue;
      args.push(`--${key}`);
      if (typeof item === "string") args.push(key === "from" || key === "out" ? resolve(item) : item);
    }
  }
  const cli = resolve(process.argv[1] ?? join(import.meta.dir, "..", "..", "bin", "maestro.ts"));
  const child = Bun.spawn([process.execPath, cli, ...args, "--json"], {
    cwd: hub.room,
    env: { ...process.env, [forwardedMark]: "1" },
    stdout: "pipe",
    stderr: "pipe",
  });
  const [stdout, stderr, exitCode] = await Promise.all([
    new Response(child.stdout).text(),
    new Response(child.stderr).text(),
    child.exited,
  ]);
  if (exitCode !== 0) {
    // Plugin-load warnings precede the envelope on stderr; the envelope is the
    // last JSON line, and failureEnvelope spreads the details flat into error.
    const envelopeLine = stderr.trim().split("\n").reverse().find((line) => line.startsWith("{"));
    let failure: { code?: string; message?: string; [detail: string]: unknown } = {};
    try {
      failure = (JSON.parse(envelopeLine ?? "") as { error?: typeof failure }).error ?? {};
    } catch {
      // stderr carried no envelope; report the exit code
    }
    const { code, message, ...details } = failure;
    throw new CliError(
      code ?? "HUB_UNAVAILABLE",
      message ?? `maestro memory ${verb} in ${hub.room} exited ${exitCode}`,
      { room: hub.room, ...details },
    );
  }
  return (JSON.parse(stdout) as { data: T }).data;
}

// Reads resolve the Hub from any cwd the way search does: the project store's
// own memory_facts table is always empty and must never answer a read.
function readHub<T>(context: PluginContext, read: (database: Database) => T): T {
  if (writesHubDirectly(context)) return read(context.store.database);
  const database = new Database(hubStore().storePath, { readonly: true });
  try {
    return read(database);
  } finally {
    database.close();
  }
}

function getFact(database: Database, key: string): MemoryFact | null {
  if (!tableExists(database, "memory_facts")) return null;
  const row = database
    .query<FactRow, [string, string]>("SELECT * FROM memory_facts WHERE id = ? OR slug = ?")
    .get(key, key);
  return row ? fromRow(row) : null;
}

function listFacts(database: Database, all: boolean): MemoryFact[] {
  if (!tableExists(database, "memory_facts")) return [];
  return database
    .query<FactRow, []>(
      all
        ? "SELECT * FROM memory_facts ORDER BY slug"
        : "SELECT * FROM memory_facts WHERE state = 'active' ORDER BY slug",
    )
    .all()
    .map(fromRow);
}

// The buffers grow silently between ingests; this counts what the next
// ingest would promote or update, mirroring its gates (problems and
// superseded slugs are refused, so they never count).
function pendingBufferFacts(hubFacts: MemoryFact[], buffer: BufferFact[]): number {
  const bySlug = new Map(hubFacts.map((fact) => [fact.slug, fact]));
  return buffer.filter((fact) => {
    if (fact.problems.length > 0 || generatedFactId.test(fact.slug)) return false;
    const known = bySlug.get(fact.slug);
    if (!known) return true;
    return known.state === "active" &&
      known.contentHash !== fact.contentHash &&
      known.sourcePath === fact.sourcePath;
  }).length;
}

function indexFact(context: PluginContext, fact: MemoryFact): void {
  if (!tableExists(context.store.database, "search_index")) return;
  context.store.database
    .query("DELETE FROM search_index WHERE surface = 'memory' AND entity_id = ?")
    .run(fact.id);
  if (fact.state !== "active") return;
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
  for (const fact of listFacts(context.store.database, true)) {
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
      indexFact(context, getFact(context.store.database, id) as MemoryFact);
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
      indexFact(context, getFact(context.store.database, entry.id) as MemoryFact);
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
      indexFact(context, getFact(context.store.database, target.id) as MemoryFact);
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
      if (generatedFactId.test(fact.slug)) {
        record("refused", fact, null, `slug takes the generated fact id shape m<number>: ${fact.slug}; rename the file`);
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

interface IngestData {
  actions: IngestAction[];
  counts: Record<IngestAction["action"], number>;
  directories: string[];
  dryRun: boolean;
  render?: RenderRefresh;
}

interface RetractData {
  fact: MemoryFact;
  render?: RenderRefresh;
}

function ingestText(data: IngestData): string {
  const { counts } = data;
  const summary = `${data.dryRun ? "dry-run: " : ""}promoted ${counts.promoted}, updated ${counts.updated}, skipped ${counts.skipped}, refused ${counts.refused} (${data.actions.length} facts from ${data.directories.length} directories)`;
  return [...data.actions.map(formatAction), summary, refreshText(data.render)]
    .filter((line): line is string => line !== null)
    .join("\n");
}

function retractText(data: RetractData): string {
  return [`${data.fact.id} ${data.fact.slug} retracted: ${data.fact.retiredReason}`, refreshText(data.render)]
    .filter((line): line is string => line !== null)
    .join("\n");
}

interface RenderData {
  facts: number;
  hash: string;
  path: string;
  previous?: string;
  status?: string;
}

// The rendered index is what every session loads, so a write that leaves it
// stale keeps injecting the old fact (UX F9). A hand edit is still never
// overwritten: the verb says what to run instead.
interface RenderRefresh {
  facts: number;
  path: string;
  status: "drift" | "missing" | "rendered";
}

function refreshRender(context: PluginContext): RenderRefresh {
  const path = join(resolveHubRoom().room, "MEMORY.md");
  const existing = readRendered(path);
  const facts = listFacts(context.store.database, false);
  if (existing === null) return { facts: facts.length, path, status: "missing" };
  if (existing.hash === null || sha256(existing.body) !== existing.hash) {
    return { facts: facts.length, path, status: "drift" };
  }
  writeRender(context, path, facts, existing);
  return { facts: facts.length, path, status: "rendered" };
}

function refreshText(refresh: RenderRefresh | undefined): string | null {
  if (!refresh) return null;
  if (refresh.status === "rendered") return `index re-rendered: ${refresh.path} (${refresh.facts} facts)`;
  if (refresh.status === "missing") return "index missing; run: maestro memory render";
  return "index not re-rendered: edited by hand; run: maestro memory render --force";
}

function writeRender(
  context: PluginContext,
  path: string,
  facts: MemoryFact[],
  existing: { body: string; hash: string | null } | null,
): RenderData {
  const rendered = renderIndex(facts);
  const status = existing === null ? "missing" : sha256(existing.body) === rendered.hash ? "current" : "stale";
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
  return { facts: facts.length, hash: rendered.hash, path, previous: status };
}

function renderText(data: RenderData): string {
  return data.previous === undefined
    ? `${data.path} current (${data.facts} facts, ${data.hash.slice(0, 12)})`
    : `rendered ${data.path} (${data.facts} facts, ${data.hash.slice(0, 12)}; was ${data.previous})`;
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
  inject: ["brief"],
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
      for (const fact of listFacts(context.store.database, true)) indexFact(context, fact);
    }

    context.effect(() =>
      (context.brief as BriefService).register(() => {
        const hub = resolveHubRoom();
        if (!existsSync(hub.storePath)) return "";
        try {
          const pending = pendingBufferFacts(
            readHub(context, (database) => listFacts(database, true)),
            collectBufferFacts(defaultBufferDirectories(resolveHomeDirectory())),
          );
          return pending > 0
            ? `memory: ${pending} buffer facts not yet in the Hub; run: maestro memory ingest --dry-run`
            : "";
        } catch {
          return "";
        }
      }, { events: ["SessionStart"] }),
    );

    context.effect(() =>
      registerSessionCommand(
        context,
        "memory ingest",
        async (invocation): Promise<CliResult> => {
          if (!writesHubDirectly(context)) {
            const data = await forwardToHub<IngestData>(invocation, "ingest");
            return { data, text: ingestText(data) };
          }
          const dryRun = invocation.options["dry-run"] === true;
          const from = stringOptions(invocation, "from");
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
          const data: IngestData = { actions, counts, directories: directories.map((entry) => entry.directory), dryRun };
          if (!dryRun && counts.promoted + counts.updated > 0) data.render = refreshRender(context);
          return { data, text: ingestText(data) };
        },
        {
          description: "Promote buffer facts into the Hub through supersession, dedup and evidence gates.",
          flags: {
            "--dry-run": { description: "Report the gates' verdicts without writing." },
            "--from": { description: "Read this directory of markdown facts instead of the default buffers.", multiple: true, value: true },
          },
        },
      ),
    );

    context.effect(() =>
      registerSessionCommand(
        context,
        "memory retract",
        async (invocation): Promise<CliResult> => {
          if (!writesHubDirectly(context)) {
            const data = await forwardToHub<RetractData>(invocation, "retract");
            return { data, text: retractText(data) };
          }
          const key = required(invocation, 0, "fact id or slug");
          const reason = stringOption(invocation, "reason");
          if (!reason) throw new CliError("MISSING_ARGUMENT", "missing --reason");
          const fact = getFact(context.store.database, key);
          if (!fact) throw new CliError("NOT_FOUND", `memory fact not found: ${key}`, { key });
          if (fact.state === "superseded") {
            throw new CliError("INVALID_STATE", `${fact.id} is already superseded`, { id: fact.id });
          }
          const now = new Date().toISOString();
          context.store.database
            .query("UPDATE memory_facts SET state = 'superseded', retired_reason = ?, updated_at = ? WHERE id = ?")
            .run(reason, now, fact.id);
          const updated = getFact(context.store.database, fact.id) as MemoryFact;
          indexFact(context, updated);
          context.log.append({
            type: "memory.retract",
            entityType: "memory",
            entityId: fact.id,
            sessionId: context.sessions.current().id,
            payload: { reason, slug: fact.slug },
          });
          const data: RetractData = { fact: updated, render: refreshRender(context) };
          return { data, text: retractText(data) };
        },
        {
          description: "Retire one fact so the buffers can never promote it again.",
          flags: { "--reason": { description: "Why the fact no longer holds.", value: true } },
          positionals: [{ name: "fact", required: true }],
        },
      ),
    );

    context.effect(() =>
      context.cli.register(
        "memory list",
        (invocation): CliResult => {
          const facts = readHub(context, (database) => listFacts(database, invocation.options.all === true));
          return {
            data: { facts },
            text: facts.length > 0
              ? facts.map((fact) => `${fact.id} [${fact.state}] ${fact.slug} (${fact.kind}): ${fact.description}`).join("\n")
              : "no memory facts; run: maestro memory ingest --dry-run",
          };
        },
        {
          description: "List Hub memory facts from any cwd.",
          flags: { "--all": { description: "Include superseded and retracted facts." } },
          mutates: false,
          rootDescription: "Global memory facts in the Hub store: read from any cwd, promote and retract from the Hub.",
        },
      ),
    );

    context.effect(() =>
      context.cli.register(
        "memory show",
        (invocation): CliResult => {
          const key = required(invocation, 0, "fact id or slug");
          const fact = readHub(context, (database) => getFact(database, key));
          if (!fact) throw new CliError("NOT_FOUND", `memory fact not found: ${key}; run: maestro memory list`, { key });
          return { data: { fact }, text: formatFact(fact) };
        },
        {
          description: "Show one Hub memory fact with its body and links, from any cwd.",
          mutates: false,
          positionals: [{ name: "fact", required: true }],
        },
      ),
    );

    context.effect(() =>
      context.cli.register(
        "memory render",
        async (invocation): Promise<CliResult> => {
          if (!writesHubDirectly(context)) {
            const data = await forwardToHub<RenderData>(invocation, "render");
            return { data, text: renderText(data) };
          }
          const hub = resolveHubRoom();
          const check = invocation.options.check === true;
          const force = invocation.options.force === true;
          const path = resolve(stringOption(invocation, "out") ?? join(hub.room, "MEMORY.md"));
          const facts = listFacts(context.store.database, false);
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
            const data: RenderData = { facts: facts.length, hash: rendered.hash, path, status };
            return { data, text: renderText(data) };
          }
          if (context.store.readOnly) {
            throw new CliError("READ_ONLY", "MAESTRO_READ_ONLY=1 cannot write the rendered index", { path });
          }
          const data = writeRender(context, path, facts, existing);
          return { data, text: renderText(data) };
        },
        {
          description: "Render the injected global index from the Hub store; refuse to overwrite a hand edit.",
          flags: {
            "--check": { description: "Report whether the rendered file is current without writing." },
            "--force": { description: "Overwrite a hand-edited render." },
            "--out": { description: "Write the render here instead of $HOME/maestro/MEMORY.md.", value: true },
          },
        },
      ),
    );
  },
};
