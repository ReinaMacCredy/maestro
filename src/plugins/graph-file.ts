import { existsSync, readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { CliError } from "../kernel/cli.ts";

// Hub d80/d81/d82/d84/d100: a graph is one markdown file whose YAML
// frontmatter declares the structure and whose "## <node>" sections hold the
// agent and human prompts. Everything here is data; nothing is evaluated.

export const nodeKinds = ["agent", "function", "router", "join", "foreach", "human"] as const;
export type NodeKind = (typeof nodeKinds)[number];

export type Condition =
  | { all: Condition[] }
  | { any: Condition[] }
  | { not: Condition }
  | { path: string; eq?: unknown; ne?: unknown; gt?: number; gte?: number; lt?: number; lte?: number };

export interface GraphNode {
  collect?: string;
  command?: string;
  id: string;
  key?: string;
  kind: NodeKind;
  dedupKey?: string[];
  over?: string;
  profile?: string;
  prompt?: string;
  schema?: unknown;
  window?: number;
  writes?: boolean;
}

export interface GraphEdge {
  from: string;
  maxRounds?: number;
  to: string;
  when?: Condition;
}

export interface GraphInput {
  default?: unknown;
  description?: string;
  required: boolean;
}

export interface GraphLimits {
  fanout: number;
  loops: number;
  nodes: number;
}

export interface GraphDefinition {
  description: string;
  edges: GraphEdge[];
  input: Record<string, GraphInput>;
  limits: GraphLimits;
  name: string;
  nodes: GraphNode[];
  text: string;
  verdict?: string;
}

export type GraphOrigin = "repo" | "home" | "shipped" | "file";

export interface GraphSource {
  name: string;
  origin: GraphOrigin;
  path: string;
  text: string;
}

export const defaultLimits: GraphLimits = { fanout: 12, loops: 3, nodes: 40 };
export const shippedGraphs = join(import.meta.dir, "graphs");

const pathPattern = /^[A-Za-z_][A-Za-z0-9_-]*(?:\.[A-Za-z0-9_-]+)*$/;
const idPattern = /^[a-z0-9][a-z0-9-]*$/;
const placeholderPattern = /\{([A-Za-z_][A-Za-z0-9_-]*(?:\.[A-Za-z0-9_-]+)*)\}/g;
export const reservedRoots = new Set(["item", "instance", "index", "round", "run"]);
const nodeKeys = new Set(["kind", "profile", "schema", "writes", "command", "over", "key", "collect", "window"]);
const edgeKeys = new Set(["from", "to", "when", "max_rounds"]);
const limitKeys = new Set(["nodes", "loops", "fanout"]);

export function invalidGraph(name: string, detail: string, details: Record<string, unknown> = {}): CliError {
  return new CliError("GRAPH_INVALID", `invalid graph ${name}: ${detail}`, { graph: name, ...details });
}

function record(value: unknown): Record<string, unknown> | null {
  return value && typeof value === "object" && !Array.isArray(value) ? (value as Record<string, unknown>) : null;
}

export function isPath(value: unknown): value is string {
  return typeof value === "string" && pathPattern.test(value);
}

// A condition is data over the run state: a path (truthiness), a path with
// one comparator, or all/any/not over conditions. Anything else, including
// a string that is not a plain dotted path, is refused as data (A2).
export function parseCondition(value: unknown, where: string, name: string): Condition {
  if (typeof value === "string") {
    if (!isPath(value)) {
      throw invalidGraph(name, `${where}: condition ${JSON.stringify(value)} is not a dotted path; conditions are data (a path, {path, eq|ne|gt|gte|lt|lte}, all, any, not), never code`);
    }
    return { path: value };
  }
  const object = record(value);
  if (!object) throw invalidGraph(name, `${where}: condition must be a path string or a mapping`);
  const keys = Object.keys(object);
  if (keys.length === 1 && (keys[0] === "all" || keys[0] === "any")) {
    const list = object[keys[0]];
    if (!Array.isArray(list) || list.length === 0) throw invalidGraph(name, `${where}: ${keys[0]} needs a non-empty list`);
    const conditions = list.map((entry, index) => parseCondition(entry, `${where} ${keys[0]}[${index}]`, name));
    return keys[0] === "all" ? { all: conditions } : { any: conditions };
  }
  if (keys.length === 1 && keys[0] === "not") return { not: parseCondition(object.not, `${where} not`, name) };
  if (!isPath(object.path)) throw invalidGraph(name, `${where}: condition mapping needs path (a dotted path)`);
  const comparators = keys.filter((key) => key !== "path");
  if (comparators.length > 1) throw invalidGraph(name, `${where}: one comparator per condition, got ${comparators.join(", ")}`);
  const condition: Condition = { path: object.path };
  const comparator = comparators[0];
  if (comparator === undefined) return condition;
  if (comparator === "eq" || comparator === "ne") {
    condition[comparator] = object[comparator];
    return condition;
  }
  if (comparator === "gt" || comparator === "gte" || comparator === "lt" || comparator === "lte") {
    if (typeof object[comparator] !== "number") throw invalidGraph(name, `${where}: ${comparator} needs a number`);
    condition[comparator] = object[comparator] as number;
    return condition;
  }
  throw invalidGraph(name, `${where}: unknown condition key ${comparator}`);
}

export function getPath(state: unknown, path: string): unknown {
  let current: unknown = state;
  for (const segment of path.split(".")) {
    if (current === null || current === undefined) return undefined;
    if (typeof current !== "object") return undefined;
    current = (current as Record<string, unknown>)[segment];
  }
  return current;
}

function truthy(value: unknown): boolean {
  if (Array.isArray(value)) return value.length > 0;
  return Boolean(value);
}

export function evaluateCondition(condition: Condition, state: unknown): boolean {
  if ("all" in condition) return condition.all.every((entry) => evaluateCondition(entry, state));
  if ("any" in condition) return condition.any.some((entry) => evaluateCondition(entry, state));
  if ("not" in condition) return !evaluateCondition(condition.not, state);
  const value = getPath(state, condition.path);
  if ("eq" in condition) return JSON.stringify(value) === JSON.stringify(condition.eq);
  if ("ne" in condition) return JSON.stringify(value) !== JSON.stringify(condition.ne);
  if (typeof value !== "number") {
    if ("gt" in condition || "gte" in condition || "lt" in condition || "lte" in condition) return false;
    return truthy(value);
  }
  if (condition.gt !== undefined) return value > condition.gt;
  if (condition.gte !== undefined) return value >= condition.gte;
  if (condition.lt !== undefined) return value < condition.lt;
  if (condition.lte !== undefined) return value <= condition.lte;
  return truthy(value);
}

export function placeholdersOf(text: string): string[] {
  return [...text.matchAll(placeholderPattern)].map((match) => match[1] as string);
}

export function fillPlaceholders(text: string, state: unknown, quote?: (value: string) => string): string {
  return text.replace(placeholderPattern, (_match, path: string) => {
    const value = getPath(state, path);
    const rendered = value === undefined || value === null
      ? ""
      : typeof value === "string"
        ? value
        : JSON.stringify(value, null, 2);
    return quote ? quote(rendered) : rendered;
  });
}

function splitFrontmatter(name: string, text: string): { body: string; frontmatter: unknown } {
  const match = /^---\r?\n([\s\S]*?)\r?\n---\r?\n?([\s\S]*)$/.exec(text);
  if (!match) throw invalidGraph(name, "expected YAML frontmatter between --- lines followed by the body");
  let parsed: unknown;
  try {
    parsed = Bun.YAML.parse(match[1] ?? "");
  } catch (error) {
    throw invalidGraph(name, `frontmatter is not YAML: ${error instanceof Error ? error.message : String(error)}`);
  }
  return { body: match[2] ?? "", frontmatter: parsed };
}

function sectionsOf(body: string): Map<string, string> {
  const sections = new Map<string, string>();
  let current: string | null = null;
  let lines: string[] = [];
  const flush = () => {
    if (current !== null) sections.set(current, lines.join("\n").trim());
  };
  for (const line of body.split("\n")) {
    const heading = /^## (\S+)\s*$/.exec(line);
    if (heading) {
      flush();
      current = heading[1] as string;
      lines = [];
      continue;
    }
    if (current !== null) lines.push(line);
  }
  flush();
  return sections;
}

function parseNode(name: string, id: string, raw: unknown, sections: Map<string, string>): GraphNode {
  if (!idPattern.test(id)) throw invalidGraph(name, `node ${id}: ids are lowercase letters, digits and dashes`);
  const object = record(raw);
  if (!object) throw invalidGraph(name, `node ${id}: must be a mapping with kind`);
  for (const key of Object.keys(object)) {
    if (!nodeKeys.has(key)) throw invalidGraph(name, `node ${id}: unknown key ${key}`);
  }
  const kind = object.kind;
  if (typeof kind !== "string" || !(nodeKinds as readonly string[]).includes(kind)) {
    throw invalidGraph(name, `node ${id}: unknown kind ${JSON.stringify(kind ?? null)}; kinds are ${nodeKinds.join(", ")}`, { node: id });
  }
  const node: GraphNode = { id, kind: kind as NodeKind };
  if (kind === "agent" || kind === "human") {
    const prompt = sections.get(id);
    if (prompt === undefined || prompt === "") {
      throw invalidGraph(name, `${kind} node ${id} has no "## ${id}" section holding its prompt`, { node: id });
    }
    node.prompt = prompt;
  }
  if (kind === "agent") {
    if (typeof object.profile !== "string" || object.profile.trim() === "") {
      throw invalidGraph(name, `agent node ${id} needs profile: <name>`, { node: id });
    }
    node.profile = object.profile;
    if (object.schema !== undefined) {
      if (!record(object.schema)) throw invalidGraph(name, `agent node ${id}: schema must be a mapping`, { node: id });
      node.schema = object.schema;
    }
    if (object.writes !== undefined) {
      if (typeof object.writes !== "boolean") throw invalidGraph(name, `agent node ${id}: writes must be true or false`, { node: id });
      node.writes = object.writes;
    }
  } else if (object.profile !== undefined || object.schema !== undefined || object.writes !== undefined) {
    throw invalidGraph(name, `node ${id}: profile, schema and writes apply to agent nodes only`, { node: id });
  }
  if (kind === "function") {
    if (typeof object.command !== "string" || object.command.trim() === "") {
      throw invalidGraph(name, `function node ${id} needs command: <shell command>`, { node: id });
    }
    node.command = object.command;
  } else if (object.command !== undefined) {
    throw invalidGraph(name, `node ${id}: command applies to function nodes only`, { node: id });
  }
  if (kind === "foreach") {
    if (!isPath(object.over)) throw invalidGraph(name, `foreach node ${id} needs over: <dotted path to a list>`, { node: id });
    node.over = object.over;
    if (object.key !== undefined) {
      if (!isPath(object.key)) throw invalidGraph(name, `foreach node ${id}: key must be a dotted path into each item`, { node: id });
      node.key = object.key;
    }
  }
  if (kind === "join") {
    if (object.collect !== undefined) {
      if (!isPath(object.collect)) throw invalidGraph(name, `join node ${id}: collect must be a dotted path`, { node: id });
      node.collect = object.collect;
    }
    if (object.key !== undefined) {
      const key = Array.isArray(object.key) ? object.key : [object.key];
      if (key.length === 0 || key.some((field) => !isPath(field))) {
        throw invalidGraph(name, `join node ${id}: key must be a list of dotted paths`, { node: id });
      }
      node.dedupKey = key as string[];
    }
    if (object.window !== undefined) {
      if (typeof object.window !== "number" || object.window < 0) {
        throw invalidGraph(name, `join node ${id}: window must be a non-negative number`, { node: id });
      }
      node.window = object.window;
    }
  } else if (object.collect !== undefined || object.window !== undefined || (kind !== "foreach" && object.key !== undefined)) {
    throw invalidGraph(name, `node ${id}: collect, key and window apply to join nodes (key also to foreach)`, { node: id });
  }
  return node;
}

function parseLimits(name: string, raw: unknown): GraphLimits {
  const limits = { ...defaultLimits };
  if (raw === undefined) return limits;
  const object = record(raw);
  if (!object) throw invalidGraph(name, "limits must be a mapping of nodes, loops, fanout");
  for (const [key, value] of Object.entries(object)) {
    if (!limitKeys.has(key)) throw invalidGraph(name, `limits: unknown key ${key}`);
    if (typeof value !== "number" || !Number.isInteger(value) || value < 0) {
      throw invalidGraph(name, `limits.${key} must be a non-negative integer`);
    }
    limits[key as keyof GraphLimits] = value;
  }
  return limits;
}

function parseInputs(name: string, raw: unknown): Record<string, GraphInput> {
  const inputs: Record<string, GraphInput> = {};
  if (raw === undefined) return inputs;
  const object = record(raw);
  if (!object) throw invalidGraph(name, "input must be a mapping of name to {required, default, description}");
  for (const [key, value] of Object.entries(object)) {
    if (!/^[A-Za-z_][A-Za-z0-9_-]*$/.test(key)) throw invalidGraph(name, `input ${key}: names are letters, digits, dashes and underscores`);
    if (reservedRoots.has(key)) throw invalidGraph(name, `input ${key}: reserved name`);
    const spec = record(value) ?? {};
    if (value !== null && value !== undefined && !record(value)) {
      throw invalidGraph(name, `input ${key}: must be a mapping (required, default, description)`);
    }
    const input: GraphInput = { required: spec.required === true };
    if (spec.default !== undefined) input.default = spec.default;
    if (typeof spec.description === "string") input.description = spec.description;
    if (input.required && input.default !== undefined) throw invalidGraph(name, `input ${key}: required inputs take no default`);
    inputs[key] = input;
  }
  return inputs;
}

export function parseGraph(label: string, text: string): GraphDefinition {
  const { body, frontmatter } = splitFrontmatter(label, text);
  const object = record(frontmatter);
  if (!object) throw invalidGraph(label, "frontmatter must be a mapping");
  const name = object.name;
  if (typeof name !== "string" || !idPattern.test(name)) {
    throw invalidGraph(label, "name must be a lowercase identifier (letters, digits, dashes)");
  }
  for (const key of Object.keys(object)) {
    if (!["name", "description", "input", "nodes", "edges", "limits", "verdict"].includes(key)) {
      throw invalidGraph(name, `unknown frontmatter key ${key}`);
    }
  }
  const description = typeof object.description === "string" ? object.description : "";
  const input = parseInputs(name, object.input);
  const limits = parseLimits(name, object.limits);
  const rawNodes = record(object.nodes);
  if (!rawNodes || Object.keys(rawNodes).length === 0) throw invalidGraph(name, "nodes must be a non-empty mapping of node id to definition");
  const sections = sectionsOf(body);
  const nodes = Object.entries(rawNodes).map(([id, raw]) => parseNode(name, id, raw, sections));
  const ids = new Set(nodes.map((node) => node.id));
  for (const key of Object.keys(input)) {
    if (ids.has(key)) throw invalidGraph(name, `input ${key} collides with node ${key}`);
  }
  for (const node of nodes) {
    if (reservedRoots.has(node.id)) throw invalidGraph(name, `node ${node.id}: reserved id`, { node: node.id });
  }
  const rawEdges = object.edges ?? [];
  if (!Array.isArray(rawEdges)) throw invalidGraph(name, "edges must be a list of {from, to, when?, max_rounds?}");
  const edges: GraphEdge[] = rawEdges.map((raw, index) => {
    const edge = record(raw);
    if (!edge) throw invalidGraph(name, `edge ${index}: must be a mapping with from and to`);
    for (const key of Object.keys(edge)) {
      if (!edgeKeys.has(key)) throw invalidGraph(name, `edge ${index}: unknown key ${key}`);
    }
    const from = edge.from;
    const to = edge.to;
    if (typeof from !== "string" || typeof to !== "string") throw invalidGraph(name, `edge ${index}: from and to must be node ids`);
    for (const end of [from, to]) {
      if (!ids.has(end)) {
        throw invalidGraph(name, `edge from ${from} to ${to} names a missing node ${end}`, { edge: { from, to }, node: end });
      }
    }
    const parsed: GraphEdge = { from, to };
    if (edge.max_rounds !== undefined) {
      if (typeof edge.max_rounds !== "number" || !Number.isInteger(edge.max_rounds) || edge.max_rounds < 1) {
        throw invalidGraph(name, `edge from ${from} to ${to}: max_rounds must be a positive integer`);
      }
      parsed.maxRounds = edge.max_rounds;
    }
    if (edge.when !== undefined) parsed.when = parseCondition(edge.when, `edge from ${from} to ${to}`, name);
    return parsed;
  });
  const byId = new Map(nodes.map((node) => [node.id, node]));
  for (const edge of edges) {
    const source = byId.get(edge.from) as GraphNode;
    if (edge.when && source.kind !== "router" && edge.maxRounds === undefined) {
      throw invalidGraph(name, `edge from ${edge.from} to ${edge.to}: when applies to edges leaving a router or to loop-back edges (max_rounds)`, { node: edge.from });
    }
    if (source.kind === "foreach" && edge.maxRounds !== undefined) {
      throw invalidGraph(name, `edge from ${edge.from} to ${edge.to}: a foreach node cannot loop back`, { node: edge.from });
    }
  }
  for (const node of nodes) {
    if (node.kind === "router") {
      const routed = edges.filter((edge) => edge.from === node.id && edge.maxRounds === undefined);
      if (!routed.some((edge) => edge.when)) {
        throw invalidGraph(name, `router ${node.id} has no outgoing edge with when:`, { node: node.id });
      }
    }
    if (node.kind === "foreach" && !edges.some((edge) => edge.from === node.id && edge.maxRounds === undefined)) {
      throw invalidGraph(name, `foreach ${node.id} has no outgoing edge to instantiate`, { node: node.id });
    }
    if (node.kind === "join" && !edges.some((edge) => edge.to === node.id && edge.maxRounds === undefined)) {
      throw invalidGraph(name, `join ${node.id} has no incoming edge to wait for`, { node: node.id });
    }
    const roots = new Set([...Object.keys(input), ...ids, ...reservedRoots]);
    for (const path of placeholdersOf(node.prompt ?? node.command ?? "")) {
      const root = path.split(".")[0] as string;
      if (!roots.has(root)) {
        throw invalidGraph(name, `node ${node.id}: placeholder {${path}} names no input, node or item`, { node: node.id });
      }
    }
    for (const path of [node.over, node.collect].filter((value): value is string => typeof value === "string")) {
      const root = path.split(".")[0] as string;
      if (node.kind === "foreach" && !roots.has(root)) {
        throw invalidGraph(name, `node ${node.id}: over ${path} names no input or node`, { node: node.id });
      }
    }
  }
  let verdict: string | undefined;
  if (object.verdict !== undefined) {
    if (!isPath(object.verdict)) throw invalidGraph(name, "verdict must be a dotted path over the run state");
    verdict = object.verdict;
  }
  return { description, edges, input, limits, name, nodes, text, verdict };
}

export function graphDirectories(repo: string, home: string): Array<{ origin: GraphOrigin; path: string }> {
  return [
    { origin: "repo", path: join(repo, ".maestro", "graphs") },
    { origin: "home", path: join(home, "maestro", "graphs") },
    { origin: "shipped", path: shippedGraphs },
  ];
}

export function resolveGraph(name: string, directories: ReadonlyArray<{ origin: GraphOrigin; path: string }>): GraphSource | null {
  if (!idPattern.test(name)) return null;
  for (const { origin, path: directory } of directories) {
    const path = join(directory, `${name}.md`);
    if (!existsSync(path)) continue;
    return { name, origin, path, text: readFileSync(path, "utf8") };
  }
  return null;
}

export interface GraphListing {
  name: string;
  origin: GraphOrigin;
  path: string;
  shadows: Array<{ origin: GraphOrigin; path: string }>;
}

export function listGraphs(directories: ReadonlyArray<{ origin: GraphOrigin; path: string }>): GraphListing[] {
  const listings = new Map<string, GraphListing>();
  for (const { origin, path: directory } of directories) {
    if (!existsSync(directory)) continue;
    for (const entry of readdirSync(directory).sort()) {
      if (!entry.endsWith(".md")) continue;
      const name = entry.slice(0, -".md".length);
      const path = join(directory, entry);
      const existing = listings.get(name);
      if (existing) {
        existing.shadows.push({ origin, path });
      } else {
        listings.set(name, { name, origin, path, shadows: [] });
      }
    }
  }
  return [...listings.values()].sort((left, right) => left.name.localeCompare(right.name));
}

// Live row 17 (g18): a schema block alone did not hold a sonnet node to the
// shape; the brief leads with the required keys in one plain sentence, top
// level and inside array items, derived from the same schema the run checks.
export function schemaKeySentence(schema: unknown): string | null {
  const spec = record(schema);
  if (!spec) return null;
  const required = Array.isArray(spec.required) ? spec.required.filter((key): key is string => typeof key === "string") : [];
  const properties = record(spec.properties) ?? {};
  // "exactly these keys" is right only when every property is required; a
  // literal reader given the council report shape would otherwise drop the
  // optional fields the node prompt asks for.
  const optional = Object.keys(properties).filter((key) => !required.includes(key));
  const parts = [
    required.length === 0
      ? "Return one JSON object"
      : optional.length === 0
        ? `Return one JSON object with exactly these keys: ${required.join(", ")}`
        : `Return one JSON object with the required keys ${required.join(", ")} and any of the optional keys ${optional.join(", ")}`,
  ];
  for (const [key, property] of Object.entries(properties)) {
    const items = record(record(property)?.items);
    const nested = Array.isArray(items?.required) ? items.required.filter((entry): entry is string => typeof entry === "string") : [];
    if (nested.length > 0) parts.push(`nested ${key} objects need ${nested.join(", ")}`);
  }
  parts.push("no prose before or after.");
  return parts.join("; ");
}

// A small JSON-schema subset: type, properties, required, items, enum. Enough
// to hold a sub-agent to the shape a node declares (d82); never a dependency.
export function validateAgainstSchema(value: unknown, schema: unknown, at = "$"): string | null {
  const spec = record(schema);
  if (!spec) return null;
  if (spec.enum !== undefined && Array.isArray(spec.enum)) {
    if (!spec.enum.some((candidate) => JSON.stringify(candidate) === JSON.stringify(value))) {
      return `${at}: expected one of ${JSON.stringify(spec.enum)}`;
    }
  }
  const types = typeof spec.type === "string" ? [spec.type] : Array.isArray(spec.type) ? spec.type : [];
  if (types.length > 0) {
    const actual = value === null ? "null" : Array.isArray(value) ? "array" : typeof value;
    const matches = types.some((type) =>
      type === actual || (type === "integer" && typeof value === "number" && Number.isInteger(value)),
    );
    if (!matches) return `${at}: expected ${types.join("|")}, got ${actual}`;
  }
  if (record(value) && (spec.properties !== undefined || spec.required !== undefined)) {
    const object = value as Record<string, unknown>;
    for (const key of Array.isArray(spec.required) ? spec.required : []) {
      if (typeof key === "string" && !(key in object)) return `${at}: missing required ${key}`;
    }
    const properties = record(spec.properties) ?? {};
    for (const [key, property] of Object.entries(properties)) {
      if (!(key in object)) continue;
      const problem = validateAgainstSchema(object[key], property, `${at}.${key}`);
      if (problem) return problem;
    }
  }
  if (Array.isArray(value) && spec.items !== undefined) {
    for (const [index, entry] of value.entries()) {
      const problem = validateAgainstSchema(entry, spec.items, `${at}[${index}]`);
      if (problem) return problem;
    }
  }
  return null;
}

function balancedJson(text: string, open: string, close: string): unknown | undefined {
  const start = text.indexOf(open);
  if (start < 0) return undefined;
  let depth = 0;
  let inString = false;
  let escaped = false;
  for (let index = start; index < text.length; index += 1) {
    const char = text[index];
    if (inString) {
      if (escaped) escaped = false;
      else if (char === "\\") escaped = true;
      else if (char === '"') inString = false;
      continue;
    }
    if (char === '"') inString = true;
    else if (char === "{" || char === "[") depth += 1;
    else if (char === "}" || char === "]") {
      depth -= 1;
      if (depth === 0) {
        try {
          return JSON.parse(text.slice(start, index + 1));
        } catch {
          return undefined;
        }
      }
    }
  }
  return undefined;
}

// The first JSON block in free text: the whole text, a fenced block, or the
// first balanced object or array (d82).
export function extractJson(text: string): unknown | undefined {
  const trimmed = text.trim();
  if (trimmed === "") return undefined;
  try {
    return JSON.parse(trimmed);
  } catch {}
  for (const match of trimmed.matchAll(/```(?:json)?\s*\n([\s\S]*?)```/g)) {
    try {
      return JSON.parse((match[1] ?? "").trim());
    } catch {}
  }
  const objectStart = trimmed.indexOf("{");
  const arrayStart = trimmed.indexOf("[");
  const first = objectStart < 0 ? "[" : arrayStart < 0 ? "{" : objectStart < arrayStart ? "{" : "[";
  const primary = first === "{" ? balancedJson(trimmed, "{", "}") : balancedJson(trimmed, "[", "]");
  if (primary !== undefined) return primary;
  return first === "{" ? balancedJson(trimmed, "[", "]") : balancedJson(trimmed, "{", "}");
}

export interface JoinedItem {
  [field: string]: unknown;
  producer: string;
  provenance?: string[];
}

function keyMatches(left: unknown, right: unknown, window: number): boolean {
  if (typeof left === "number" && typeof right === "number") return Math.abs(left - right) <= window;
  return JSON.stringify(left) === JSON.stringify(right);
}

// Deterministic dedup (d82): same key within the line window keeps the first
// item and lists the other producers as provenance.
export function dedupItems(items: JoinedItem[], key: readonly string[], window: number): JoinedItem[] {
  const kept: JoinedItem[] = [];
  for (const item of items) {
    const match = kept.find((candidate) =>
      key.every((field) => keyMatches(getPath(candidate, field), getPath(item, field), window)),
    );
    if (!match) {
      kept.push({ ...item });
      continue;
    }
    match.provenance = [...(match.provenance ?? []), item.producer];
  }
  return kept;
}
