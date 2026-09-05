import { existsSync, readFileSync, realpathSync } from "node:fs";
import { isAbsolute, relative, resolve, sep } from "node:path";
import { CliError, requiredPosition, stringOption, stringOptions, type CliInvocation, type CliResult } from "../kernel/cli.ts";
import type { BuiltInPlugin, PluginContext } from "../kernel/loader.ts";
import {
  dedupItems,
  defaultLimits,
  evaluateCondition,
  extractJson,
  fillPlaceholders,
  getPath,
  graphDirectories,
  invalidGraph,
  listGraphs,
  parseGraph,
  resolveGraph,
  shippedGraphs,
  validateAgainstSchema,
  type GraphDefinition,
  type GraphEdge,
  type GraphLimits,
  type GraphNode,
  type GraphOrigin,
  type GraphSource,
  type JoinedItem,
} from "./graph-file.ts";
import { grantTrust, pluginTrustPredicate } from "./plugin-trust.ts";
import { profileDirectories, resolveProfile } from "./profiles.ts";
import { registerSessionCommand } from "./session-required.ts";
import { requireSlpActor } from "./slp-v2.ts";

// Hub d78/d79/d88: a passive graph runtime. maestro holds the definition, the
// run state and the journal, executes function, router, join and foreach
// nodes itself, and hands agent and human nodes back to whatever is driving.
// It never spawns a model (A1) and never writes SLP state (A7).

type NodeState = "pending" | "issued" | "done" | "failed" | "skipped";
type Executor = "subagent" | "team";

interface RunRow {
  created_at: string;
  executor: Executor;
  graph: string;
  input: string;
  limits: string;
  loops: number;
  origin: GraphOrigin;
  path: string | null;
  run_id: string;
  source: string;
  stopped: string | null;
  updated_at: string;
  verdict: string | null;
}

interface NodeRow {
  attempts: number;
  files: string | null;
  inputs: string;
  instance_key: string;
  kind: string;
  node_id: string;
  profile: string | null;
  prompt: string | null;
  result: string | null;
  round: number;
  run_id: string;
  schema: string | null;
  state: NodeState;
  work_id: string | null;
}

interface Run {
  definition: GraphDefinition;
  input: Record<string, unknown>;
  limits: GraphLimits;
  row: RunRow;
  scopes: Map<string, string | null>;
}

interface Stop {
  limit: keyof GraphLimits;
  used: number;
}

const instanceSeparator = "@";

function parseJson<T>(text: string | null, fallback: T): T {
  if (text === null) return fallback;
  try {
    return JSON.parse(text) as T;
  } catch {
    return fallback;
  }
}

function refOf(row: Pick<NodeRow, "instance_key" | "node_id">): string {
  return row.instance_key === "" ? row.node_id : `${row.node_id}${instanceSeparator}${row.instance_key}`;
}

function splitRef(ref: string): { instance: string; node: string } {
  const at = ref.indexOf(instanceSeparator);
  return at < 0 ? { instance: "", node: ref } : { instance: ref.slice(at + 1), node: ref.slice(0, at) };
}

function shellQuote(value: string): string {
  return `'${value.replaceAll("'", "'\\''")}'`;
}

function forwardEdges(definition: GraphDefinition): GraphEdge[] {
  return definition.edges.filter((edge) => edge.maxRounds === undefined);
}

// A foreach scope is every node reachable from the foreach along forward
// edges without crossing a join; those nodes exist once per item.
function computeScopes(definition: GraphDefinition): Map<string, string | null> {
  const scopes = new Map<string, string | null>(definition.nodes.map((node) => [node.id, null]));
  const byId = new Map(definition.nodes.map((node) => [node.id, node]));
  const edges = forwardEdges(definition);
  for (const node of definition.nodes) {
    if (node.kind !== "foreach") continue;
    const stack = edges.filter((edge) => edge.from === node.id).map((edge) => edge.to);
    const seen = new Set<string>();
    while (stack.length > 0) {
      const id = stack.pop() as string;
      if (seen.has(id)) continue;
      seen.add(id);
      const target = byId.get(id) as GraphNode;
      if (target.kind === "join") continue;
      if (scopes.get(id) === null) scopes.set(id, node.id);
      for (const edge of edges) if (edge.from === id) stack.push(edge.to);
    }
  }
  return scopes;
}

function loopBody(definition: GraphDefinition, from: string, to: string): Set<string> {
  const edges = forwardEdges(definition);
  const reach = (start: string, next: (edge: GraphEdge) => string | null): Set<string> => {
    const seen = new Set<string>([start]);
    const stack = [start];
    while (stack.length > 0) {
      const id = stack.pop() as string;
      for (const edge of edges) {
        const step = next(edge);
        if (step !== null && edge.from === id && !seen.has(step)) {
          seen.add(step);
          stack.push(step);
        }
      }
    }
    return seen;
  };
  const downstream = reach(to, (edge) => edge.to);
  const upstreamOfFrom = new Set<string>([from]);
  const stack = [from];
  while (stack.length > 0) {
    const id = stack.pop() as string;
    for (const edge of edges) {
      if (edge.to === id && !upstreamOfFrom.has(edge.from)) {
        upstreamOfFrom.add(edge.from);
        stack.push(edge.from);
      }
    }
  }
  return new Set([...downstream].filter((id) => upstreamOfFrom.has(id)));
}

function graphOrigin(path: string, repo: string, home: string): GraphOrigin {
  const real = (candidate: string) => {
    try {
      return realpathSync(candidate);
    } catch {
      return resolve(candidate);
    }
  };
  const within = (root: string, file: string) => {
    const rel = relative(real(root), real(file));
    return rel === "" || (rel !== ".." && !rel.startsWith(`..${sep}`) && !isAbsolute(rel));
  };
  if (within(shippedGraphs, path)) return "shipped";
  if (within(resolve(home, "maestro", "graphs"), path)) return "home";
  if (within(repo, path)) return "repo";
  return "file";
}

function describeGraph(definition: GraphDefinition) {
  return {
    edges: definition.edges.map((edge) => ({
      from: edge.from,
      to: edge.to,
      ...(edge.when ? { when: edge.when } : {}),
      ...(edge.maxRounds !== undefined ? { max_rounds: edge.maxRounds } : {}),
    })),
    input: definition.input,
    limits: definition.limits,
    nodes: definition.nodes.map((node) => ({
      node: node.id,
      kind: node.kind,
      ...(node.profile ? { profile: node.profile } : {}),
      ...(node.prompt !== undefined ? { prompt: node.prompt } : {}),
      ...(node.command ? { command: node.command } : {}),
      ...(node.schema ? { schema: node.schema } : {}),
      ...(node.writes ? { writes: true } : {}),
      ...(node.over ? { over: node.over } : {}),
      ...(node.dedupKey ? { key: node.dedupKey } : {}),
      ...(node.window !== undefined ? { window: node.window } : {}),
      ...(node.collect ? { collect: node.collect } : {}),
    })),
    ...(definition.verdict ? { verdict: definition.verdict } : {}),
  };
}

class GraphEngine {
  constructor(private readonly context: PluginContext, private readonly repo: string, private readonly home: string) {}

  private get database() {
    return this.context.store.database;
  }

  loadRun(id: string): Run {
    const row = this.database.query<RunRow, [string]>("SELECT * FROM graph_runs WHERE run_id = ?").get(id);
    if (!row) {
      throw new CliError("NOT_FOUND", `graph run not found: ${id}; run: maestro work list`, { command: "maestro work list", id });
    }
    const definition = parseGraph(row.graph, row.source);
    return {
      definition,
      input: parseJson<Record<string, unknown>>(row.input, {}),
      limits: parseJson<GraphLimits>(row.limits, { ...defaultLimits }),
      row,
      scopes: computeScopes(definition),
    };
  }

  rows(runId: string): NodeRow[] {
    return this.database
      .query<NodeRow, [string]>("SELECT * FROM graph_nodes WHERE run_id = ? ORDER BY rowid")
      .all(runId);
  }

  private journal(runId: string, type: string, payload: Record<string, unknown>): void {
    this.context.log.append({
      type,
      entityType: "work",
      entityId: runId,
      sessionId: this.context.sessions.current().id,
      payload,
    });
  }

  private setState(row: NodeRow, state: NodeState, patch: { prompt?: string | null; result?: unknown } = {}): void {
    const now = new Date().toISOString();
    const result = patch.result === undefined ? row.result : JSON.stringify(patch.result);
    const prompt = patch.prompt === undefined ? row.prompt : patch.prompt;
    this.database
      .query(
        `UPDATE graph_nodes SET state = ?, result = ?, prompt = ?, updated_at = ?
         WHERE run_id = ? AND node_id = ? AND instance_key = ?`,
      )
      .run(state, result, prompt, now, row.run_id, row.node_id, row.instance_key);
    row.state = state;
    row.result = result;
    row.prompt = prompt;
    this.journal(row.run_id, `graph.node.${state}`, {
      instance: row.instance_key || undefined,
      kind: row.kind,
      node: row.node_id,
      ref: refOf(row),
      round: row.round,
      ...(state === "failed" ? { error: patch.result } : {}),
    });
  }

  private nodeOf(run: Run, id: string): GraphNode {
    return run.definition.nodes.find((node) => node.id === id) as GraphNode;
  }

  private rowFor(rows: NodeRow[], node: string, instance: string): NodeRow | undefined {
    return rows.find((row) => row.node_id === node && row.instance_key === instance);
  }

  private producerRows(run: Run, rows: NodeRow[], producer: string, consumer: NodeRow): NodeRow[] {
    const producerScope = run.scopes.get(producer) ?? null;
    const consumerScope = run.scopes.get(consumer.node_id) ?? null;
    if (producerScope === null) return rows.filter((row) => row.node_id === producer);
    if (producerScope === consumerScope) {
      const row = this.rowFor(rows, producer, consumer.instance_key);
      return row ? [row] : [];
    }
    return rows.filter((row) => row.node_id === producer);
  }

  // The state a node sees: inputs, static results, same-instance results,
  // the item under a foreach, and its round.
  stateFor(run: Run, rows: NodeRow[], row: NodeRow | null): Record<string, unknown> {
    const state: Record<string, unknown> = { ...run.input, round: row?.round ?? 1, run: run.row.run_id };
    const scope = row ? run.scopes.get(row.node_id) ?? null : null;
    for (const node of run.definition.nodes) {
      const nodeScope = run.scopes.get(node.id) ?? null;
      if (nodeScope === null) {
        const found = this.rowFor(rows, node.id, "");
        if (found?.state === "done") state[node.id] = parseJson(found.result, null);
      } else if (row && nodeScope === scope) {
        const found = this.rowFor(rows, node.id, row.instance_key);
        if (found?.state === "done") state[node.id] = parseJson(found.result, null);
      } else {
        const instances: Record<string, unknown> = {};
        for (const found of rows) {
          if (found.node_id === node.id && found.state === "done") instances[found.instance_key] = parseJson(found.result, null);
        }
        if (Object.keys(instances).length > 0) state[node.id] = instances;
      }
    }
    if (row && row.instance_key !== "") {
      const inputs = parseJson<Record<string, unknown>>(row.inputs, {});
      state.item = inputs.item;
      state.index = inputs.index;
      state.instance = row.instance_key;
    }
    return state;
  }

  private readiness(run: Run, rows: NodeRow[], row: NodeRow): "ready" | "skip" | "wait" {
    const node = this.nodeOf(run, row.node_id);
    const incoming = forwardEdges(run.definition).filter((edge) => edge.to === row.node_id);
    if (incoming.length === 0) return "ready";
    let active = false;
    for (const edge of incoming) {
      const source = this.nodeOf(run, edge.from);
      const scope = run.scopes.get(row.node_id) ?? null;
      if (source.kind === "foreach" && scope === source.id) {
        active = true;
        continue;
      }
      const sourceScope = run.scopes.get(source.id) ?? null;
      if (node.kind === "join" && sourceScope !== null) {
        const foreachRow = this.rowFor(rows, sourceScope, row.instance_key) ?? this.rowFor(rows, sourceScope, "");
        if (!foreachRow || foreachRow.state === "pending" || foreachRow.state === "issued") return "wait";
        if (foreachRow.state === "skipped") continue;
      }
      const producers = this.producerRows(run, rows, edge.from, row);
      if (producers.length === 0) {
        if (node.kind === "join" && sourceScope !== null) {
          active = true;
          continue;
        }
        return "wait";
      }
      if (producers.some((producer) => producer.state === "pending" || producer.state === "issued")) return "wait";
      if (producers.some((producer) => producer.state === "failed")) return "wait";
      const doneRows = producers.filter((producer) => producer.state === "done");
      if (source.kind === "router") {
        const selected = doneRows.some((producer) =>
          (parseJson<{ selected?: string[] }>(producer.result, {}).selected ?? []).includes(row.node_id),
        );
        if (selected) active = true;
        continue;
      }
      if (doneRows.length > 0 || (node.kind === "join" && sourceScope !== null)) active = true;
    }
    return active ? "ready" : "skip";
  }

  private executeFunction(run: Run, rows: NodeRow[], row: NodeRow): void {
    const node = this.nodeOf(run, row.node_id);
    const state = this.stateFor(run, rows, row);
    const command = fillPlaceholders(node.command as string, state, shellQuote);
    const child = Bun.spawnSync(["sh", "-c", command], {
      cwd: this.repo,
      env: { ...process.env, MAESTRO_GRAPH_RUN: run.row.run_id, MAESTRO_GRAPH_NODE: refOf(row) },
      stderr: "pipe",
      stdout: "pipe",
    });
    const stdout = child.stdout?.toString() ?? "";
    const stderr = child.stderr?.toString() ?? "";
    if (child.exitCode !== 0) {
      this.setState(row, "failed", {
        result: { command, error: `exit ${child.exitCode}: ${stderr.trim() || stdout.trim()}` },
      });
      return;
    }
    const trimmed = stdout.trimEnd();
    let result: unknown = trimmed;
    try {
      result = JSON.parse(trimmed);
    } catch {}
    this.setState(row, "done", { result });
  }

  private trustCache = new Map<string, boolean | Promise<boolean>>();

  async trusted(run: Run): Promise<boolean> {
    const path = run.row.path;
    if (!path || run.row.origin !== "repo") return true;
    const cached = this.trustCache.get(path);
    if (typeof cached === "boolean") return cached;
    const granted = await pluginTrustPredicate(this.home)({ root: path, source: "repo" });
    this.trustCache.set(path, granted);
    return granted;
  }

  private executeRouter(run: Run, rows: NodeRow[], row: NodeRow): void {
    const state = this.stateFor(run, rows, row);
    const edges = forwardEdges(run.definition).filter((edge) => edge.from === row.node_id);
    const matched = edges.filter((edge) => edge.when && evaluateCondition(edge.when, state)).map((edge) => edge.to);
    const selected = matched.length > 0 ? matched : edges.filter((edge) => !edge.when).map((edge) => edge.to);
    this.setState(row, "done", { result: { selected } });
  }

  private executeForeach(run: Run, rows: NodeRow[], row: NodeRow): Stop | null {
    const node = this.nodeOf(run, row.node_id);
    const state = this.stateFor(run, rows, row);
    const list = getPath(state, node.over as string);
    if (!Array.isArray(list)) {
      this.setState(row, "failed", { result: { error: `over ${node.over} is not a list on the run state` } });
      return null;
    }
    const scoped = run.definition.nodes.filter((candidate) => run.scopes.get(candidate.id) === node.id);
    const total = rows.length + list.length * scoped.length;
    if (total > run.limits.nodes) return { limit: "nodes", used: total };
    const now = new Date().toISOString();
    const insert = this.database.query(
      `INSERT INTO graph_nodes
        (run_id, node_id, instance_key, kind, state, profile, prompt, schema, inputs, result, round, files, attempts, work_id, created_at, updated_at)
       VALUES (?, ?, ?, ?, 'pending', ?, NULL, ?, ?, NULL, 1, NULL, 0, NULL, ?, ?)`,
    );
    const keys: string[] = [];
    for (const [index, item] of list.entries()) {
      const key = node.key ? String(getPath(item, node.key) ?? index) : String(index);
      keys.push(key);
      for (const scopedNode of scoped) {
        insert.run(
          run.row.run_id,
          scopedNode.id,
          key,
          scopedNode.kind,
          scopedNode.profile ?? null,
          scopedNode.schema ? JSON.stringify(scopedNode.schema) : null,
          JSON.stringify({ index, item }),
          now,
          now,
        );
      }
    }
    this.journal(run.row.run_id, "graph.node.instances", { node: node.id, count: list.length, keys, nodes: scoped.map((scopedNode) => scopedNode.id) });
    this.setState(row, "done", { result: { count: list.length, keys } });
    return null;
  }

  private executeJoin(run: Run, rows: NodeRow[], row: NodeRow): void {
    const node = this.nodeOf(run, row.node_id);
    const items: JoinedItem[] = [];
    for (const edge of forwardEdges(run.definition).filter((candidate) => candidate.to === row.node_id)) {
      for (const producer of this.producerRows(run, rows, edge.from, row)) {
        if (producer.state !== "done") continue;
        const result = parseJson<unknown>(producer.result, null);
        const collected = node.collect ? getPath(result, node.collect) : result;
        const entries = Array.isArray(collected) ? collected : collected === undefined ? [] : [collected];
        for (const entry of entries) {
          const base = entry && typeof entry === "object" && !Array.isArray(entry)
            ? (entry as Record<string, unknown>)
            : { value: entry };
          items.push({
            ...base,
            producer: producer.node_id,
            ...(producer.instance_key !== "" ? { instance: producer.instance_key } : {}),
          });
        }
      }
    }
    const kept = node.dedupKey ? dedupItems(items, node.dedupKey, node.window ?? 0) : items;
    this.setState(row, "done", { result: { items: kept, total: items.length } });
  }

  private issue(run: Run, rows: NodeRow[], row: NodeRow): Stop | null {
    const inflight = rows.filter((candidate) => candidate.state === "issued").length;
    if (inflight + 1 > run.limits.fanout) return { limit: "fanout", used: inflight + 1 };
    const node = this.nodeOf(run, row.node_id);
    const prompt = fillPlaceholders(node.prompt as string, this.stateFor(run, rows, row));
    this.setState(row, "issued", { prompt });
    return null;
  }

  // Loop-back edges leaving a node that just finished: the target and every
  // node between it and the source re-run with the next round (d78).
  private fireLoops(run: Run, rows: NodeRow[], row: NodeRow): Stop | null {
    const loops = run.definition.edges.filter((edge) => edge.maxRounds !== undefined && edge.from === row.node_id);
    if (loops.length === 0) return null;
    const state = this.stateFor(run, rows, row);
    for (const edge of loops) {
      if (edge.when && !evaluateCondition(edge.when, state)) continue;
      const target = this.rowFor(rows, edge.to, run.scopes.get(edge.to) === run.scopes.get(row.node_id) ? row.instance_key : "");
      if (!target || target.round >= (edge.maxRounds as number)) continue;
      const used = run.row.loops + 1;
      if (used > run.limits.loops) return { limit: "loops", used };
      const body = loopBody(run.definition, edge.from, edge.to);
      const now = new Date().toISOString();
      const round = target.round + 1;
      for (const member of rows) {
        if (!body.has(member.node_id)) continue;
        if (run.scopes.get(member.node_id) === run.scopes.get(row.node_id) && member.instance_key !== row.instance_key) continue;
        this.database
          .query(
            `UPDATE graph_nodes SET state = 'pending', result = NULL, prompt = NULL, round = ?, attempts = 0, updated_at = ?
             WHERE run_id = ? AND node_id = ? AND instance_key = ?`,
          )
          .run(round, now, member.run_id, member.node_id, member.instance_key);
        member.state = "pending";
        member.result = null;
        member.round = round;
      }
      run.row.loops = used;
      this.database.query("UPDATE graph_runs SET loops = ?, updated_at = ? WHERE run_id = ?").run(used, now, run.row.run_id);
      this.journal(run.row.run_id, "graph.loop", { from: edge.from, to: edge.to, round, max_rounds: edge.maxRounds, nodes: [...body] });
      return null;
    }
    return null;
  }

  async advance(run: Run): Promise<Record<string, unknown>> {
    if (run.row.stopped || run.row.verdict !== null) return this.envelope(run, this.rows(run.row.run_id));
    let stop: Stop | null = null;
    let untrusted: CliError | null = null;
    let changed = true;
    while (changed && !stop && !untrusted) {
      changed = false;
      const rows = this.rows(run.row.run_id);
      for (const row of rows) {
        if (row.state !== "pending") continue;
        const status = this.readiness(run, rows, row);
        if (status === "wait") continue;
        if (status === "skip") {
          this.setState(row, "skipped");
          changed = true;
          continue;
        }
        switch (row.kind) {
          case "function": {
            if (!(await this.trusted(run))) {
              const installed = run.row.path?.startsWith(`${resolve(this.repo, ".maestro", "graphs")}${sep}`);
              const trustCommand = `maestro graph trust ${installed ? run.definition.name : `--file ${run.row.path}`}`;
              untrusted = new CliError(
                "GRAPH_UNTRUSTED",
                `function node ${row.node_id} of repo graph ${run.definition.name} runs a shell command; review ${run.row.path}, then: ${trustCommand} and maestro graph next ${run.row.run_id}`,
                { command: trustCommand, graph: run.definition.name, node: row.node_id, path: run.row.path, run: run.row.run_id },
              );
              break;
            }
            this.executeFunction(run, rows, row);
            if ((row.state as NodeState) === "done") stop = this.fireLoops(run, rows, row);
            break;
          }
          case "router":
            this.executeRouter(run, rows, row);
            stop = this.fireLoops(run, rows, row);
            break;
          case "foreach":
            stop = this.executeForeach(run, rows, row);
            break;
          case "join":
            this.executeJoin(run, rows, row);
            stop = this.fireLoops(run, rows, row);
            break;
          default:
            stop = this.issue(run, rows, row);
        }
        changed = true;
        break;
      }
    }
    if (untrusted) throw untrusted;
    const rows = this.rows(run.row.run_id);
    if (stop) {
      this.finish(run, rows, { stopped: stop });
    } else if (rows.some((row) => row.state === "failed")) {
      this.finish(run, rows, { failed: rows.find((row) => row.state === "failed") as NodeRow });
    } else if (!rows.some((row) => row.state === "pending" || row.state === "issued")) {
      this.finish(run, rows, {});
    }
    return this.envelope(run, rows);
  }

  private verdictOf(run: Run, rows: NodeRow[]): unknown {
    const state = this.stateFor(run, rows, null);
    if (run.definition.verdict) return getPath(state, run.definition.verdict) ?? null;
    const edges = forwardEdges(run.definition);
    const sinks = run.definition.nodes.filter((node) => !edges.some((edge) => edge.from === node.id));
    if (sinks.length === 1) return state[(sinks[0] as GraphNode).id] ?? null;
    return Object.fromEntries(sinks.map((sink) => [sink.id, state[sink.id] ?? null]));
  }

  private finish(run: Run, rows: NodeRow[], outcome: { failed?: NodeRow; stopped?: Stop }): void {
    if (run.row.stopped !== null || run.row.verdict !== null) return;
    const now = new Date().toISOString();
    const verdict = outcome.failed || outcome.stopped ? null : this.verdictOf(run, rows);
    const stopped = outcome.stopped
      ? `LIMIT:${outcome.stopped.limit}:${outcome.stopped.used}`
      : outcome.failed
        ? `FAILED:${refOf(outcome.failed)}`
        : null;
    this.database
      .query("UPDATE graph_runs SET verdict = ?, stopped = ?, updated_at = ? WHERE run_id = ?")
      .run(JSON.stringify(verdict), stopped, now, run.row.run_id);
    run.row.verdict = JSON.stringify(verdict);
    run.row.stopped = stopped;
    const evidence = stopped ? `graph ${run.definition.name} ${stopped}` : `graph ${run.definition.name} verdict: ${JSON.stringify(verdict)}`;
    this.database
      .query("UPDATE work SET state = 'done', evidence = ?, held_by = NULL, updated_at = ? WHERE id = ?")
      .run(evidence, now, run.row.run_id);
    this.journal(run.row.run_id, outcome.stopped ? "graph.stopped" : outcome.failed ? "graph.failed" : "graph.done", {
      ...(outcome.stopped ? { limit: outcome.stopped.limit, used: outcome.stopped.used } : {}),
      ...(outcome.failed ? { node: refOf(outcome.failed), error: parseJson(outcome.failed.result, null) } : {}),
      ...(stopped ? {} : { verdict }),
    });
  }

  envelope(run: Run, rows: NodeRow[]): Record<string, unknown> {
    const state = this.stateFor(run, rows, null);
    const base = {
      run: run.row.run_id,
      graph: run.definition.name,
      executor: run.row.executor,
      round: Math.max(1, ...rows.map((row) => row.round)),
      state,
    };
    if (run.row.stopped?.startsWith("LIMIT:")) {
      const [, limit, used] = run.row.stopped.split(":");
      return { ...base, done: true, stopped: "LIMIT", limit, used: Number(used), partial: state, nodes: [] };
    }
    if (run.row.stopped?.startsWith("FAILED:")) {
      const failed = rows.find((row) => row.state === "failed");
      return {
        ...base,
        done: true,
        failed: { node: run.row.stopped.slice("FAILED:".length), error: failed ? parseJson(failed.result, null) : null },
        nodes: [],
      };
    }
    if (run.row.verdict !== null) return { ...base, done: true, verdict: parseJson(run.row.verdict, null), nodes: [] };
    const nodes = rows
      .filter((row) => row.state === "issued")
      .map((row) => ({
        ref: refOf(row),
        node: row.node_id,
        ...(row.instance_key !== "" ? { instance: row.instance_key } : {}),
        kind: row.kind,
        ...(row.profile ? { profile: row.profile } : {}),
        prompt: row.prompt ?? "",
        ...(row.schema ? { schema: parseJson(row.schema, null) } : {}),
        inputs: parseJson(row.inputs, {}),
        round: row.round,
      }));
    return { ...base, done: false, nodes };
  }

  // Intake (d82): a declared schema validates JSON, otherwise the first JSON
  // block in the text; free text stays raw. One PARSE_FAILED retry, then the
  // node is failed.
  accept(run: Run, ref: string, text: string, files: string[]): Record<string, unknown> {
    // Live row 10 (2026-09-05): a late result on a finished run must not
    // fire loops or limits and rewrite the recorded outcome.
    if (run.row.stopped !== null || run.row.verdict !== null) {
      throw new CliError(
        "INVALID_STATE",
        `run ${run.row.run_id} is done (${run.row.stopped ?? "verdict recorded"}) and accepts no more results; run: maestro graph next ${run.row.run_id}`,
        { outcome: run.row.stopped ?? "verdict", ref, run: run.row.run_id },
      );
    }
    const { instance, node } = splitRef(ref);
    const rows = this.rows(run.row.run_id);
    const row = this.rowFor(rows, node, instance);
    if (!row) {
      throw new CliError("NOT_FOUND", `graph run ${run.row.run_id} has no node ${ref}; run: maestro graph next ${run.row.run_id}`, { ref, run: run.row.run_id });
    }
    if (row.state !== "issued") {
      throw new CliError("INVALID_STATE", `node ${ref} of run ${run.row.run_id} is ${row.state}, not issued; run: maestro graph next ${run.row.run_id}`, { ref, state: row.state });
    }
    const schema = parseJson<unknown>(row.schema, null);
    const extracted = extractJson(text);
    let result: unknown;
    if (schema) {
      const problem = extracted === undefined ? "no JSON found in the result" : validateAgainstSchema(extracted, schema);
      if (problem) {
        const attempts = row.attempts + 1;
        const now = new Date().toISOString();
        this.database
          .query("UPDATE graph_nodes SET attempts = ?, updated_at = ? WHERE run_id = ? AND node_id = ? AND instance_key = ?")
          .run(attempts, now, row.run_id, row.node_id, row.instance_key);
        if (attempts >= 2) {
          this.setState(row, "failed", { result: { error: `result did not match the schema twice: ${problem}` } });
          throw new CliError("PARSE_FAILED", `node ${ref}: ${problem}; the node is failed after two attempts`, { attempt: attempts, node: ref, retry: false, schema });
        }
        throw new CliError(
          "PARSE_FAILED",
          `node ${ref}: ${problem}; re-ask the sub-agent once for JSON matching the schema, then: maestro graph result ${run.row.run_id} ${ref} --file <path>`,
          { attempt: attempts, node: ref, retry: true, schema },
        );
      }
      result = extracted;
    } else {
      result = extracted === undefined ? text : extracted;
    }
    if (files.length > 0) {
      this.database
        .query("UPDATE graph_nodes SET files = ? WHERE run_id = ? AND node_id = ? AND instance_key = ?")
        .run(JSON.stringify(files), row.run_id, row.node_id, row.instance_key);
    }
    this.setState(row, "done", { result });
    const stop = this.fireLoops(run, rows, row);
    if (stop) this.finish(run, this.rows(run.row.run_id), { stopped: stop });
    return { run: run.row.run_id, ref, state: "done", files };
  }
}

async function readSource(invocation: CliInvocation, repo: string, home: string): Promise<{ source: GraphSource; args: string[] }> {
  const file = stringOption(invocation, "file");
  if (file !== undefined) {
    const text = file === "-" ? await Bun.stdin.text() : existsSync(file) ? readFileSync(file, "utf8") : null;
    if (text === null) throw new CliError("NOT_FOUND", `graph file not found: ${file}`, { file });
    const path = file === "-" ? "-" : resolve(file);
    const parsed = parseGraph(file === "-" ? "stdin" : path, text);
    return {
      args: invocation.positionals,
      source: { name: parsed.name, origin: file === "-" ? "file" : graphOrigin(path, repo, home), path, text },
    };
  }
  const name = requiredPosition(invocation, 0, "graph name (or --file <path>)");
  const source = resolveGraph(name, graphDirectories(repo, home));
  if (!source) {
    throw new CliError("GRAPH_NOT_FOUND", `graph not found: ${name}; run: maestro graph list`, { command: "maestro graph list", name });
  }
  return { args: invocation.positionals.slice(1), source };
}

function parseArgs(definition: GraphDefinition, args: string[]): Record<string, unknown> {
  const input: Record<string, unknown> = {};
  for (const [name, spec] of Object.entries(definition.input)) {
    if (spec.default !== undefined) input[name] = spec.default;
  }
  for (const arg of args) {
    const at = arg.indexOf("=");
    if (at <= 0) throw new CliError("GRAPH_INPUT", `expected key=value, got ${JSON.stringify(arg)}`, { argument: arg });
    const key = arg.slice(0, at);
    if (!(key in definition.input)) {
      throw new CliError("GRAPH_INPUT", `graph ${definition.name} declares no input ${key}; inputs: ${Object.keys(definition.input).join(", ") || "none"}`, { input: key });
    }
    input[key] = arg.slice(at + 1);
  }
  for (const [name, spec] of Object.entries(definition.input)) {
    if (spec.required && input[name] === undefined) {
      throw new CliError("GRAPH_INPUT", `graph ${definition.name} requires ${name}=<value>`, { input: name });
    }
  }
  return input;
}

function parseLimitOverrides(definition: GraphDefinition, overrides: string[]): GraphLimits {
  const limits = { ...definition.limits };
  for (const override of overrides) {
    const [key, value] = override.split("=");
    if (!key || value === undefined || !(key in limits)) {
      throw new CliError("INVALID_OPTION", `--limit takes nodes=N, loops=N or fanout=N, got ${JSON.stringify(override)}`);
    }
    const parsed = Number(value);
    if (!Number.isInteger(parsed) || parsed < 0) throw new CliError("INVALID_OPTION", `--limit ${key} must be a non-negative integer`);
    limits[key as keyof GraphLimits] = parsed;
  }
  return limits;
}

// Hub d88: the executor is a property of the run. Default: team-executor
// when the driver is a proven role of a running SLP generation, which needs a
// pane identity; subagent everywhere else.
function defaultExecutor(context: PluginContext): Executor {
  try {
    requireSlpActor(context, ["team-supervisor", "lead", "peer"]);
    return "team";
  } catch {
    return "subagent";
  }
}

async function requireProfiles(definition: GraphDefinition, repo: string, home: string): Promise<void> {
  const directories = profileDirectories(repo, home);
  for (const node of definition.nodes) {
    if (node.kind !== "agent") continue;
    const profile = await resolveProfile(node.profile as string, directories);
    if (!profile) {
      throw invalidGraph(
        definition.name,
        `agent node ${node.id} names profile ${node.profile}, which resolves to no profile in ${directories.join(", ")}`,
        { node: node.id, profile: node.profile },
      );
    }
  }
}

export const graphPlugin: BuiltInPlugin = {
  name: "graph",
  inject: ["work"],
  apply(context) {
    const repo = process.cwd();
    const home = process.env.HOME ?? repo;
    context.store.migrate(`
      CREATE TABLE IF NOT EXISTS graph_runs (
        run_id TEXT PRIMARY KEY REFERENCES work(id),
        graph TEXT NOT NULL,
        origin TEXT NOT NULL,
        path TEXT,
        source TEXT NOT NULL,
        input TEXT NOT NULL,
        limits TEXT NOT NULL,
        executor TEXT NOT NULL,
        loops INTEGER NOT NULL DEFAULT 0,
        stopped TEXT,
        verdict TEXT,
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL
      );
      CREATE TABLE IF NOT EXISTS graph_nodes (
        run_id TEXT NOT NULL REFERENCES work(id),
        node_id TEXT NOT NULL,
        instance_key TEXT NOT NULL DEFAULT '',
        kind TEXT NOT NULL,
        state TEXT NOT NULL CHECK(state IN ('pending', 'issued', 'done', 'failed', 'skipped')),
        profile TEXT,
        prompt TEXT,
        schema TEXT,
        inputs TEXT NOT NULL DEFAULT '{}',
        result TEXT,
        round INTEGER NOT NULL DEFAULT 1,
        files TEXT,
        attempts INTEGER NOT NULL DEFAULT 0,
        work_id TEXT,
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL,
        PRIMARY KEY (run_id, node_id, instance_key)
      );
    `);
    const engine = new GraphEngine(context, repo, home);

    context.effect(() =>
      context.cli.register(
        "graph list",
        (): CliResult => {
          const graphs = listGraphs(graphDirectories(repo, home));
          return {
            data: { graphs },
            text: graphs.length === 0
              ? "no graphs; author one and run: maestro graph run --file <path>"
              : graphs
                  .map((graph) =>
                    `${graph.name}\t${graph.origin}\t${graph.path}` +
                    (graph.shadows.length > 0 ? `\tshadows: ${graph.shadows.map((shadow) => shadow.origin).join(", ")}` : ""),
                  )
                  .join("\n"),
          };
        },
        {
          description: "List graphs across repo, room and shipped locations with shadowing.",
          mutates: false,
          rootDescription: "Run pre-known multi-agent paths as passive graphs both harnesses drive identically.",
        },
      ),
    );

    context.effect(() =>
      context.cli.register(
        "graph show",
        (invocation): CliResult => {
          const name = requiredPosition(invocation, 0, "graph name");
          const source = resolveGraph(name, graphDirectories(repo, home));
          if (!source) {
            throw new CliError("GRAPH_NOT_FOUND", `graph not found: ${name}; run: maestro graph list`, { command: "maestro graph list", name });
          }
          const definition = parseGraph(source.path, source.text);
          return {
            data: { name, origin: source.origin, path: source.path, text: source.text, graph: describeGraph(definition) },
            text: source.text,
          };
        },
        {
          description: "Show one graph file from the nearest location.",
          mutates: false,
          positionals: [{ name: "name", required: true }],
        },
      ),
    );

    context.effect(() =>
      registerSessionCommand(
        context,
        "graph run",
        async (invocation): Promise<CliResult> => {
          const { args, source } = await readSource(invocation, repo, home);
          const definition = parseGraph(source.path, source.text);
          await requireProfiles(definition, repo, home);
          const input = parseArgs(definition, args);
          const limits = parseLimitOverrides(definition, stringOptions(invocation, "limit"));
          const requested = stringOption(invocation, "executor");
          if (requested !== undefined && requested !== "subagent" && requested !== "team") {
            throw new CliError("INVALID_OPTION", `--executor takes subagent or team, got ${JSON.stringify(requested)}`);
          }
          const executor: Executor = (requested as Executor | undefined) ?? defaultExecutor(context);
          const session = context.sessions.current();
          const now = new Date().toISOString();
          const title = `graph ${definition.name}${Object.entries(input).map(([key, value]) => ` ${key}=${typeof value === "string" ? value : JSON.stringify(value)}`).join("")}`;
          const id = context.store.database.transaction(() => {
            const runId = context.store.nextPrefixedId("work", "w");
            context.sessions.record("graph.run");
            context.store.database
              .query(
                `INSERT INTO work (id, title, kind, state, parent_id, acceptance, atomic_reason, held_by, created_at, updated_at)
                 VALUES (?, ?, 'graph', 'active', NULL, ?, NULL, ?, ?, ?)`,
              )
              .run(runId, title, `the run of graph ${definition.name} reaches a verdict`, session.id, now, now);
            context.store.database
              .query(
                `INSERT INTO graph_runs (run_id, graph, origin, path, source, input, limits, executor, loops, stopped, verdict, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, 0, NULL, NULL, ?, ?)`,
              )
              .run(runId, definition.name, source.origin, source.path === "-" ? null : source.path, source.text, JSON.stringify(input), JSON.stringify(limits), executor, now, now);
            const insert = context.store.database.query(
              `INSERT INTO graph_nodes
                (run_id, node_id, instance_key, kind, state, profile, prompt, schema, inputs, result, round, files, attempts, work_id, created_at, updated_at)
               VALUES (?, ?, '', ?, 'pending', ?, NULL, ?, '{}', NULL, 1, NULL, 0, NULL, ?, ?)`,
            );
            const scopes = computeScopes(definition);
            for (const node of definition.nodes) {
              if (scopes.get(node.id) !== null) continue;
              insert.run(runId, node.id, node.kind, node.profile ?? null, node.schema ? JSON.stringify(node.schema) : null, now, now);
            }
            context.log.append({
              type: "graph.run",
              entityType: "work",
              entityId: runId,
              sessionId: session.id,
              payload: { graph: definition.name, origin: source.origin, path: source.path, input, limits, executor, holder: session.id },
            });
            return runId;
          }).immediate();
          const envelope = await engine.advance(engine.loadRun(id));
          return { data: envelope, text: JSON.stringify(envelope) };
        },
        {
          description: "Start a graph run (by name or --file) and return its first pull envelope.",
          flags: {
            "--executor": { description: "subagent: the harness spawns each agent node; team: nodes bind to SLP work items; default by environment.", value: true },
            "--file": { description: "Run an uninstalled graph markdown file (- for stdin) instead of a named one.", value: true },
            "--limit": { description: "Override a structural limit for this run: nodes=N, loops=N or fanout=N.", multiple: true, value: true },
          },
          maxPositionals: 65,
          positionals: [{ name: "name", required: false }],
        },
      ),
    );

    context.effect(() =>
      registerSessionCommand(
        context,
        "graph next",
        async (invocation): Promise<CliResult> => {
          const id = requiredPosition(invocation, 0, "run id");
          const envelope = await engine.advance(engine.loadRun(id));
          return { data: envelope, text: JSON.stringify(envelope) };
        },
        {
          description: "Advance a run: execute ready function, router, join and foreach nodes and return the agent and human nodes to spawn.",
          positionals: [{ name: "run", required: true }],
        },
      ),
    );

    context.effect(() =>
      registerSessionCommand(
        context,
        "graph result",
        (invocation): CliResult => {
          const id = requiredPosition(invocation, 0, "run id");
          const ref = requiredPosition(invocation, 1, "node ref");
          const file = stringOption(invocation, "file");
          const inline = stringOption(invocation, "text");
          if ((file === undefined) === (inline === undefined)) {
            throw new CliError("INVALID_OPTION", "graph result takes exactly one of --file <path> or --text <result>");
          }
          let text: string;
          if (file !== undefined) {
            if (!existsSync(file)) throw new CliError("NOT_FOUND", `result file not found: ${file}`, { file });
            text = readFileSync(file, "utf8");
          } else {
            text = inline as string;
          }
          const files = stringOptions(invocation, "files").flatMap((entry) => entry.split(",")).map((entry) => entry.trim()).filter(Boolean);
          const run = engine.loadRun(id);
          const accepted = engine.accept(run, ref, text, files);
          return { data: accepted, text: `${id} ${ref} done; run: maestro graph next ${id}` };
        },
        {
          description: "Record an agent or human node's result (validated against its schema when one is declared).",
          flags: {
            "--file": { description: "Read the result from this file.", value: true },
            "--files": { description: "Files the node changed, comma separated.", multiple: true, value: true },
            "--text": { description: "The result as inline text.", value: true },
          },
          positionals: [
            { name: "run", required: true },
            { name: "node", required: true },
          ],
        },
      ),
    );

    context.effect(() =>
      registerSessionCommand(
        context,
        "graph trust",
        async (invocation): Promise<CliResult> => {
          const file = stringOption(invocation, "file");
          let path: string;
          let name: string;
          if (file !== undefined) {
            path = resolve(file);
            if (!existsSync(path)) throw new CliError("NOT_FOUND", `graph file not found: ${file}`, { file });
            name = parseGraph(path, readFileSync(path, "utf8")).name;
          } else {
            name = requiredPosition(invocation, 0, "graph name (or --file <path>)");
            const source = resolveGraph(name, graphDirectories(repo, home));
            if (!source) {
              throw new CliError("GRAPH_NOT_FOUND", `graph not found: ${name}; run: maestro graph list`, { command: "maestro graph list", name });
            }
            if (source.origin !== "repo") {
              throw new CliError("NOT_REPO_GRAPH", `${name} is a ${source.origin} graph and runs fully without a grant`, { name, origin: source.origin });
            }
            path = source.path;
          }
          const digest = await grantTrust(home, { root: path, source: "repo" });
          context.log.append({
            type: "graph.trust",
            entityType: "graph",
            entityId: name,
            sessionId: context.sessions.current().id,
            payload: { digest, path },
          });
          return { data: { digest, name, path }, text: `${name} trusted\n${path}\n${digest}` };
        },
        {
          description: "Trust a repo graph's current file so its function nodes may execute.",
          flags: { "--file": { description: "Trust an uninstalled graph file under the repo instead of a named one.", value: true } },
          positionals: [{ name: "name", required: false }],
        },
      ),
    );
  },
};
