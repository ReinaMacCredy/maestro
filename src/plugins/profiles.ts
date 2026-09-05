import { createHash } from "node:crypto";
import { existsSync, readFileSync } from "node:fs";
import { lstat, mkdir, readdir, readFile, realpath, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { CliError } from "../kernel/cli.ts";

// Hub d83/d91/d98: one profile shape for SLP seats, council seats and graph
// nodes; install renders each into both harnesses' native launch bundles.
export type ProfileHarness = "claude" | "codex";
export type ProfileEffort = "low" | "medium" | "high" | "xhigh";

export interface ProfileFrontmatter {
  autocompact?: number;
  description: string;
  disallowed_tools?: string[];
  effort?: ProfileEffort;
  harness: ProfileHarness;
  model: string;
  permission?: string;
  sandbox?: string;
}

export interface Profile {
  body: string;
  frontmatter: ProfileFrontmatter;
  name: string;
  path: string;
  source: Uint8Array;
}

export interface ProfileSync {
  hubPackVersion: string | null;
  removed: string[];
  rendered: string[];
  resolvedTargets: Array<{ real: string; target: string }>;
}

export const seatProfileNames = ["team-supervisor", "lead", "peer"] as const;
export type SeatProfileName = (typeof seatProfileNames)[number];

const shippedProfiles = join(import.meta.dir, "resources", "profiles");
const shippedPack = join(import.meta.dir, "resources", "SLP.md");
const efforts: readonly string[] = ["low", "medium", "high", "xhigh"];
// disallowed_tools is not Claude-only: install renders a Claude agent file
// for every profile whatever its harness, and the key governs that render
// (owner ruling 2026-09-05: no seat has a question tool).
const claudeOnlyKeys = ["permission", "autocompact"] as const;
const knownKeys = new Set([
  "harness",
  "model",
  "effort",
  "permission",
  "sandbox",
  "autocompact",
  "disallowed_tools",
  "description",
]);

export function profileDirectories(repo: string, home: string): string[] {
  return [join(repo, ".maestro", "profiles"), join(home, "maestro", "profiles"), shippedProfiles];
}

function invalid(path: string, detail: string): CliError {
  return new CliError("INVALID_PROFILE", `invalid profile ${path}: ${detail}`, { path });
}

export function parseProfile(path: string, text: string): { body: string; frontmatter: ProfileFrontmatter } {
  const match = /^---\r?\n([\s\S]*?)\r?\n---\r?\n([\s\S]*)$/.exec(text);
  if (!match) throw invalid(path, "expected YAML frontmatter between --- lines followed by the body");
  let parsed: unknown;
  try {
    parsed = Bun.YAML.parse(match[1] ?? "");
  } catch (error) {
    throw invalid(path, `frontmatter is not YAML: ${error instanceof Error ? error.message : String(error)}`);
  }
  if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
    throw invalid(path, "frontmatter must be a mapping");
  }
  const raw = parsed as Record<string, unknown>;
  for (const key of Object.keys(raw)) {
    if (!knownKeys.has(key)) throw invalid(path, `unknown key ${key}`);
  }
  const harness = raw.harness;
  if (harness !== "claude" && harness !== "codex") {
    throw invalid(path, `harness must be claude or codex, got ${JSON.stringify(harness ?? null)}`);
  }
  const model = raw.model === undefined ? "default" : raw.model;
  if (typeof model !== "string" || model.trim() === "") throw invalid(path, "model must be a non-empty string");
  const frontmatter: ProfileFrontmatter = {
    description: typeof raw.description === "string" ? raw.description : "",
    harness,
    model,
  };
  if (raw.description !== undefined && typeof raw.description !== "string") {
    throw invalid(path, "description must be a string");
  }
  if (raw.effort !== undefined) {
    if (typeof raw.effort !== "string" || !efforts.includes(raw.effort)) {
      throw invalid(path, `effort must be one of ${efforts.join(", ")}, got ${JSON.stringify(raw.effort)}`);
    }
    frontmatter.effort = raw.effort as ProfileEffort;
  }
  for (const key of claudeOnlyKeys) {
    if (raw[key] !== undefined && harness !== "claude") throw invalid(path, `${key} applies to harness claude only`);
  }
  if (raw.sandbox !== undefined && harness !== "codex") throw invalid(path, "sandbox applies to harness codex only");
  if (raw.permission !== undefined) {
    if (typeof raw.permission !== "string" || raw.permission.trim() === "") {
      throw invalid(path, "permission must be a Claude permissionMode value");
    }
    frontmatter.permission = raw.permission;
  }
  if (raw.sandbox !== undefined) {
    if (typeof raw.sandbox !== "string" || raw.sandbox.trim() === "") {
      throw invalid(path, "sandbox must be a Codex sandbox_mode value");
    }
    frontmatter.sandbox = raw.sandbox;
  }
  if (raw.autocompact !== undefined) {
    if (typeof raw.autocompact !== "number" || !Number.isInteger(raw.autocompact) || raw.autocompact <= 0) {
      throw invalid(path, "autocompact must be a positive integer");
    }
    frontmatter.autocompact = raw.autocompact;
  }
  if (raw.disallowed_tools !== undefined) {
    if (
      !Array.isArray(raw.disallowed_tools) ||
      raw.disallowed_tools.some((tool) => typeof tool !== "string" || tool.trim() === "")
    ) {
      throw invalid(path, "disallowed_tools must be a list of tool names");
    }
    frontmatter.disallowed_tools = raw.disallowed_tools as string[];
  }
  const body = (match[2] ?? "").trim();
  if (body === "") throw invalid(path, "missing body: the mandate below the frontmatter is empty");
  return { body, frontmatter };
}

export async function resolveProfile(name: string, directories: readonly string[]): Promise<Profile | null> {
  if (!/^[a-z0-9](?:[a-z0-9-]*[a-z0-9])?$/.test(name)) return null;
  for (const directory of directories) {
    const path = join(directory, `${name}.md`);
    if (!existsSync(path)) continue;
    const source = new Uint8Array(await readFile(path));
    const parsed = parseProfile(path, new TextDecoder().decode(source));
    return { ...parsed, name, path, source };
  }
  return null;
}

export async function listProfileNames(directories: readonly string[]): Promise<string[]> {
  const names = new Set<string>();
  for (const directory of directories) {
    if (!existsSync(directory)) continue;
    for (const entry of await readdir(directory)) {
      if (entry.endsWith(".md")) names.add(entry.slice(0, -".md".length));
    }
  }
  return [...names].sort();
}

export function profileDigest(profile: Profile): string {
  return createHash("sha256").update(profile.source).digest("hex");
}

// A Peer variant is named peer-<x> and composes as itself; a node or council
// profile <x> composes as peer-<x> (SPEC item 7).
export function composedPeerName(name: string): string {
  return name.startsWith("peer-") ? name : `peer-${name}`;
}

export function renderedProfilePath(home: string, harness: ProfileHarness, renderedName: string): string {
  return harness === "claude"
    ? join(home, ".claude", "agents", `maestro-${renderedName}.md`)
    : join(home, ".codex", `maestro-${renderedName}.config.toml`);
}

function tomlString(value: string): string {
  return `"${value.replaceAll("\\", "\\\\").replaceAll('"', '\\"')}"`;
}

function tomlMultiline(value: string): string {
  return `"""\n${value.replaceAll("\\", "\\\\").replaceAll('"', '\\"')}\n"""`;
}

export interface RenderedProfile {
  claude: string;
  codexAgent: string;
  codexSession: string;
}

export function renderProfile(renderedName: string, frontmatter: ProfileFrontmatter, mandate: string): RenderedProfile {
  const claudeLines = [`name: maestro-${renderedName}`, `description: ${JSON.stringify(frontmatter.description)}`];
  if (frontmatter.model !== "default") claudeLines.push(`model: ${frontmatter.model}`);
  if (frontmatter.effort) claudeLines.push(`effort: ${frontmatter.effort}`);
  if (frontmatter.permission) claudeLines.push(`permissionMode: ${frontmatter.permission}`);
  if (frontmatter.disallowed_tools) {
    claudeLines.push(`disallowedTools: ${frontmatter.disallowed_tools.join(", ")}`);
  }
  // Verified 2026-09-05: claude --agent glues the body to its identity
  // sentence without a newline, so the body opens with a blank line.
  const claude = `---\n${claudeLines.join("\n")}\n---\n\n${mandate}\n`;
  const codexLines: string[] = [];
  if (frontmatter.model !== "default") codexLines.push(`model = ${tomlString(frontmatter.model)}`);
  if (frontmatter.effort) codexLines.push(`model_reasoning_effort = ${tomlString(frontmatter.effort)}`);
  const instructions = `developer_instructions = ${tomlMultiline(mandate)}`;
  // d93: the session profile launches a team Peer that must write the store,
  // so sandbox_mode renders only into the sub-agent role file.
  const codexSession = `${[...codexLines, instructions].join("\n")}\n`;
  const agentLines = [
    `name = ${tomlString(`maestro-${renderedName}`)}`,
    `description = ${tomlString(frontmatter.description)}`,
    ...codexLines,
  ];
  if (frontmatter.sandbox) agentLines.push(`sandbox_mode = ${tomlString(frontmatter.sandbox)}`);
  const codexAgent = `${[...agentLines, instructions].join("\n")}\n`;
  return { claude, codexAgent, codexSession };
}

function sharedContract(pack: string): string | null {
  const match = /<!-- slp:shared:begin -->([\s\S]*?)<!-- slp:shared:end -->/.exec(pack);
  return match?.[1]?.trim() || null;
}

// The seat mandate opens with the Hub's shared contract; a Hub pack without
// that section (owner-edited, or not yet migrated) falls back to the shipped
// one so install and update still complete, and team start is where the
// broken pack is refused.
async function sharedContractFor(home: string): Promise<string> {
  const hubPack = join(home, "maestro", "SLP.md");
  const fromHub = existsSync(hubPack) ? sharedContract(await readFile(hubPack, "utf8")) : null;
  const shared = fromHub ?? sharedContract(await readFile(shippedPack, "utf8"));
  if (!shared) throw new CliError("INVALID_SLP_PACK", `${shippedPack} is missing section: shared`);
  return shared;
}

interface RenderTarget {
  content: string;
  path: string;
}

// Every resolvable profile renders into ~/.claude/agents, ~/.codex/agents and
// ~/.codex/maestro-<name>.config.toml; only maestro-* files are written or
// removed there (anti-goal A1).
export async function planProfileRenders(home: string, repo: string): Promise<RenderTarget[]> {
  const directories = profileDirectories(repo, home);
  const shared = await sharedContractFor(home);
  const peer = await resolveProfile("peer", directories);
  if (!peer) throw new CliError("PROFILE_NOT_FOUND", "the peer profile is missing from every profile directory");
  const targets: RenderTarget[] = [];
  const push = (renderedName: string, frontmatter: ProfileFrontmatter, mandate: string) => {
    const rendered = renderProfile(renderedName, frontmatter, mandate);
    targets.push(
      { content: rendered.claude, path: join(home, ".claude", "agents", `maestro-${renderedName}.md`) },
      { content: rendered.codexSession, path: join(home, ".codex", `maestro-${renderedName}.config.toml`) },
      { content: rendered.codexAgent, path: join(home, ".codex", "agents", `maestro-${renderedName}.toml`) },
    );
  };
  for (const name of await listProfileNames(directories)) {
    const profile = await resolveProfile(name, directories);
    if (!profile) continue;
    if ((seatProfileNames as readonly string[]).includes(name)) {
      push(name, profile.frontmatter, `${shared}\n\n${profile.body}`);
      continue;
    }
    const composed = composedPeerName(name);
    if (composed !== name) push(name, profile.frontmatter, profile.body);
    push(composed, profile.frontmatter, `${shared}\n\n${peer.body}\n\n${profile.body}`);
  }
  return targets;
}

const renderedPatterns: Array<{ directory: (home: string) => string; pattern: RegExp }> = [
  { directory: (home) => join(home, ".claude", "agents"), pattern: /^maestro-.+\.md$/ },
  { directory: (home) => join(home, ".codex"), pattern: /^maestro-.+\.config\.toml$/ },
  { directory: (home) => join(home, ".codex", "agents"), pattern: /^maestro-.+\.toml$/ },
];

async function renderedFiles(home: string): Promise<string[]> {
  const files: string[] = [];
  for (const { directory, pattern } of renderedPatterns) {
    const path = directory(home);
    if (!existsSync(path)) continue;
    for (const entry of await readdir(path)) {
      if (pattern.test(entry)) files.push(join(path, entry));
    }
  }
  return files;
}

async function resolvedTargets(home: string): Promise<Array<{ real: string; target: string }>> {
  const out: Array<{ real: string; target: string }> = [];
  for (const { directory } of renderedPatterns) {
    const target = directory(home);
    try {
      if (!(await lstat(target)).isSymbolicLink()) continue;
    } catch {
      continue;
    }
    const real = await realpath(target);
    if (real !== target) out.push({ real, target });
  }
  return out;
}

function hubPackVersion(home: string): string | null {
  const hubPack = join(home, "maestro", "SLP.md");
  if (!existsSync(hubPack)) return null;
  return /<!-- slp:version=([^\s]+) -->/.exec(readFileSync(hubPack, "utf8"))?.[1] ?? "unknown";
}

export async function materializeProfiles(home: string, repo: string): Promise<ProfileSync> {
  const targets = await planProfileRenders(home, repo);
  const keep = new Set(targets.map((target) => target.path));
  const rendered: string[] = [];
  for (const target of targets) {
    await mkdir(join(target.path, ".."), { recursive: true });
    const existing = existsSync(target.path) ? await readFile(target.path, "utf8") : null;
    if (existing !== target.content) await writeFile(target.path, target.content);
    rendered.push(target.path);
  }
  const removed: string[] = [];
  for (const file of await renderedFiles(home)) {
    if (keep.has(file)) continue;
    await rm(file, { force: true });
    removed.push(file);
  }
  return {
    hubPackVersion: hubPackVersion(home),
    removed,
    rendered,
    resolvedTargets: await resolvedTargets(home),
  };
}

export async function removeRenderedProfiles(home: string): Promise<string[]> {
  const removed: string[] = [];
  for (const file of await renderedFiles(home)) {
    await rm(file, { force: true });
    removed.push(file);
  }
  return removed;
}

export function formatProfileSync(sync: ProfileSync): string {
  const names = [...new Set(
    sync.rendered.map((path) => /maestro-(.+?)\.(?:md|toml|config\.toml)$/.exec(path)?.[1] ?? path),
  )];
  const parts = [`profiles rendered: ${names.join(", ")}`];
  if (sync.removed.length > 0) parts.push(`profiles removed: ${sync.removed.join(", ")}`);
  for (const { real, target } of sync.resolvedTargets) parts.push(`${target} resolves to ${real}`);
  // The seats were rendered from this Hub pack's shared contract; a stale
  // pack renders a stale mandate, and team start refuses it anyway (D7).
  if (sync.hubPackVersion !== null && sync.hubPackVersion !== "3") {
    parts.push(
      `warning: the Hub SLP.md is pack version ${sync.hubPackVersion}, so the rendered seats carry its shared contract and team start will refuse it; migrate it to version 3 (slp:profile markers, no Observer section) and run maestro install again`,
    );
  }
  return parts.join("\n");
}
