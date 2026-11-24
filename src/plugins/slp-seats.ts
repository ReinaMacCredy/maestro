import { createHash } from "node:crypto";
import { existsSync, readFileSync, realpathSync } from "node:fs";
import { chmod, lstat, mkdir, readdir, readFile, readlink, rm, symlink, writeFile } from "node:fs/promises";
import { join, sep } from "node:path";
import { CliError } from "../kernel/cli.ts";
import { retiredSkillNames } from "./skills.ts";

export const seatNames = ["lead", "peer"] as const;
export type SeatName = (typeof seatNames)[number];
export type SeatHarness = "claude" | "codex";

export interface SeatConfig {
  autocompact?: number;
  description: string;
  disallowed_tools?: string[];
  effort?: string;
  harness: SeatHarness;
  model: string;
  permission?: string;
  settings?: Record<string, unknown>;
  skills?: string[];
}

export interface Seat {
  body: string;
  config: SeatConfig;
  // d862: a frontmatter-only owner file first, then the file its mandate came from.
  paths: string[];
  source: Uint8Array;
}

const shippedSeats = join(import.meta.dir, "resources", "slp");
const shippedPack = join(import.meta.dir, "resources", "SLP.md");
const efforts = ["low", "medium", "high", "xhigh"];
const knownKeys = new Set([
  "autocompact",
  "description",
  "disallowed_tools",
  "effort",
  "harness",
  "model",
  "permission",
  // Accepted for owner files written before d906; it only ever reached the retired sub-agent file.
  "sandbox",
  "settings",
  "skills",
]);
const skillNamePattern = /^[a-z0-9][a-z0-9._-]*$/;

export function seatConfigRoot(home: string): string {
  return join(home, ".maestro", "claude");
}

export function seatDirectory(home: string, seat: SeatName): string {
  return join(seatConfigRoot(home), seat);
}

export function seatTokenPath(home: string): string {
  return join(seatConfigRoot(home), "oauth-token");
}

export function seatLaunchPath(home: string, harness: SeatHarness, seat: SeatName): string {
  return harness === "claude"
    ? join(seatDirectory(home, seat), "agents", `maestro-${seat}.md`)
    : join(home, ".codex", `maestro-${seat}.config.toml`);
}

export function seatFileDirectories(repo: string, home: string): string[] {
  return [join(repo, ".maestro", "profiles"), join(home, "maestro", "profiles"), shippedSeats];
}

function invalid(path: string, detail: string): CliError {
  return new CliError("INVALID_SEAT_FILE", `invalid seat file ${path}: ${detail}`, { path });
}

function isPlainObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function isNameList(value: unknown): value is string[] {
  return Array.isArray(value) && value.every((item) => typeof item === "string" && item.trim() !== "");
}

type Layer = "repo" | "owner" | "shipped";

export function parseSeatFile(path: string, text: string, layer: Layer): { body: string; config: SeatConfig } {
  const match = /^---\r?\n([\s\S]*?)\r?\n---\r?\n([\s\S]*)$/.exec(text);
  if (!match) throw invalid(path, "expected YAML frontmatter between --- lines followed by the body");
  let parsed: unknown;
  try {
    parsed = Bun.YAML.parse(match[1] ?? "");
  } catch (error) {
    throw invalid(path, `frontmatter is not YAML: ${error instanceof Error ? error.message : String(error)}`);
  }
  if (!isPlainObject(parsed)) throw invalid(path, "frontmatter must be a mapping");
  for (const key of Object.keys(parsed)) {
    if (!knownKeys.has(key)) throw invalid(path, `unknown key ${key}`);
  }
  // d855: settings reaches the token-bearing env and skills the filesystem, so a repository may set neither.
  for (const key of ["settings", "skills"]) {
    if (layer === "repo" && parsed[key] !== undefined) {
      throw invalid(path, `${key} is owner-only: a repo .maestro/profiles file cannot set it; move it to ~/maestro/profiles or rely on the shipped seat file`);
    }
  }
  const { autocompact, description = "", disallowed_tools, effort, harness, model = "default", permission, settings, skills } = parsed;
  if (harness !== "claude" && harness !== "codex") {
    throw invalid(path, `harness must be claude or codex, got ${JSON.stringify(harness ?? null)}`);
  }
  if (typeof model !== "string" || model.trim() === "") throw invalid(path, "model must be a non-empty string");
  if (typeof description !== "string") throw invalid(path, "description must be a string");
  if (effort !== undefined && (typeof effort !== "string" || !efforts.includes(effort))) {
    throw invalid(path, `effort must be one of ${efforts.join(", ")}, got ${JSON.stringify(effort)}`);
  }
  if (permission !== undefined && (typeof permission !== "string" || permission.trim() === "")) {
    throw invalid(path, "permission must be a Claude permissionMode value");
  }
  if (autocompact !== undefined && (typeof autocompact !== "number" || !Number.isInteger(autocompact) || autocompact <= 0)) {
    throw invalid(path, "autocompact must be a positive integer");
  }
  if (disallowed_tools !== undefined && !isNameList(disallowed_tools)) {
    throw invalid(path, "disallowed_tools must be a list of tool names");
  }
  if (skills !== undefined && !(isNameList(skills) && skills.every((skill) => skillNamePattern.test(skill)))) {
    throw invalid(path, `skills must be a list of plain skill directory names (${skillNamePattern.source})`);
  }
  if (settings !== undefined && !isPlainObject(settings)) throw invalid(path, "settings must be a mapping of Claude settings keys");
  const body = (match[2] ?? "").trim();
  if (body === "" && layer === "shipped") throw invalid(path, "missing body: the mandate below the frontmatter is empty");
  return { body, config: { ...parsed, description, harness, model } as SeatConfig };
}

// d862: a frontmatter-only seat file inherits the mandate of the layer below it.
export async function resolveSeat(seat: SeatName, repo: string, home: string): Promise<Seat> {
  const layers: Layer[] = ["repo", "owner", "shipped"];
  const directories = seatFileDirectories(repo, home);
  let shadow: { config: SeatConfig; path: string; source: Uint8Array } | null = null;
  for (const [index, directory] of directories.entries()) {
    const path = join(directory, `${seat}.md`);
    if (!existsSync(path)) continue;
    const source = new Uint8Array(await readFile(path));
    const parsed = parseSeatFile(path, new TextDecoder().decode(source), layers[index] as Layer);
    if (parsed.body === "") {
      shadow ??= { config: parsed.config, path, source };
      continue;
    }
    if (!shadow) return { ...parsed, paths: [path], source };
    // The generation pin hashes both files, since both decide the seat.
    return { body: parsed.body, config: shadow.config, paths: [shadow.path, path], source: new Uint8Array([...shadow.source, ...source]) };
  }
  throw new CliError("SEAT_FILE_MISSING", `${join(shippedSeats, `${seat}.md`)} is missing; reinstall maestro`);
}

export function seatDigest(seat: Seat): string {
  return createHash("sha256").update(seat.source).digest("hex");
}

function packSection(pack: string, marker: string): string | null {
  return new RegExp(`<!-- slp:${marker}:begin -->([\\s\\S]*?)<!-- slp:${marker}:end -->`).exec(pack)?.[1]?.trim() || null;
}

// d892: both blocks come from one pack, the one a generation pins, never one from each.
async function sharedContracts(home: string): Promise<{ lead: string; peer: string; peerSection: "missing" | "present" }> {
  const hubPack = join(home, "maestro", "SLP.md");
  const fromHub = existsSync(hubPack) ? await readFile(hubPack, "utf8") : null;
  const source = fromHub !== null && packSection(fromHub, "shared") !== null ? fromHub : await readFile(shippedPack, "utf8");
  const shared = packSection(source, "shared");
  if (!shared) throw new CliError("INVALID_SLP_PACK", `${shippedPack} is missing section: shared`);
  const peer = packSection(source, "shared:peer");
  return { lead: shared, peer: peer ?? shared, peerSection: peer === null ? "missing" : "present" };
}

function tomlString(value: string): string {
  return `"${value.replaceAll("\\", "\\\\").replaceAll('"', '\\"')}"`;
}

// d854: a Bash(...) pattern on the agent line drops the whole Bash tool, so patterns go to the settings deny.
function renderSeat(seat: SeatName, config: SeatConfig, mandate: string): { claude: string; codex: string } {
  const bare = (config.disallowed_tools ?? []).filter((tool) => !tool.includes("("));
  const claudeLines = [`name: maestro-${seat}`, `description: ${JSON.stringify(config.description)}`];
  if (config.model !== "default") claudeLines.push(`model: ${config.model}`);
  if (config.effort) claudeLines.push(`effort: ${config.effort}`);
  if (config.permission) claudeLines.push(`permissionMode: ${config.permission}`);
  if (bare.length > 0) claudeLines.push(`disallowedTools: ${bare.join(", ")}`);
  // claude --agent glues the body to its identity sentence, so the body opens with a blank line.
  const claude = `---\n${claudeLines.join("\n")}\n---\n\n${mandate}\n`;
  const codexLines: string[] = [];
  if (config.model !== "default") codexLines.push(`model = ${tomlString(config.model)}`);
  if (config.effort) codexLines.push(`model_reasoning_effort = ${tomlString(config.effort)}`);
  codexLines.push(`developer_instructions = """\n${mandate.replaceAll("\\", "\\\\").replaceAll('"', '\\"')}\n"""`);
  return { claude, codex: `${codexLines.join("\n")}\n` };
}

const herdrHookScript = "herdr-agent-state.sh";

// A6: the seat does not read the owner's settings, so it carries the Herdr SessionStart hook itself.
function herdrSessionHook(home: string): Record<string, unknown> {
  const userSettings = join(home, ".claude", "settings.json");
  if (existsSync(userSettings)) {
    try {
      const parsed = JSON.parse(readFileSync(userSettings, "utf8")) as {
        hooks?: { SessionStart?: Array<{ hooks?: Array<{ command?: string }> }> };
      };
      const group = parsed.hooks?.SessionStart?.find((candidate) =>
        candidate.hooks?.some((hook) => typeof hook.command === "string" && hook.command.includes(herdrHookScript))
      );
      if (group) return group as Record<string, unknown>;
    } catch {}
  }
  return {
    matcher: "*",
    hooks: [{ type: "command", command: `bash '${join(home, ".claude", "hooks", herdrHookScript)}' session`, timeout: 10 }],
  };
}

export function readSeatToken(home: string): string | null {
  const path = seatTokenPath(home);
  if (!existsSync(path)) return null;
  const token = readFileSync(path, "utf8").trim();
  return token === "" ? null : token;
}

// d852: <home> is an ancestor of every project, so the owner's CLAUDE.md is excluded by absolute path.
function seatSettingsBase(home: string, token: string | null): Record<string, unknown> {
  return {
    permissions: { defaultMode: "bypassPermissions" },
    claudeMdExcludes: [join(home, ".claude", "CLAUDE.md"), join(home, ".claude", "rules", "**")],
    skipDangerousModePermissionPrompt: true,
    hooks: { SessionStart: [herdrSessionHook(home)] },
    env: {
      ...(token ? { CLAUDE_CODE_OAUTH_TOKEN: token } : {}),
      CLAUDE_CODE_DISABLE_WORKFLOWS: "1",
      CLAUDE_CODE_DISABLE_CRON: "1",
      CLAUDE_CODE_DISABLE_FAST_MODE: "1",
      CLAUDE_CODE_DISABLE_BUNDLED_SKILLS: "1",
      CLAUDE_CODE_DISABLE_GIT_INSTRUCTIONS: "1",
    },
    autoMemoryEnabled: false,
    disableWorkflows: true,
    workflowKeywordTriggerEnabled: false,
    attribution: { commit: "", pr: "", sessionUrl: false },
    enabledPlugins: {},
  };
}

export function mergeSettings(base: Record<string, unknown>, overlay: Record<string, unknown>): Record<string, unknown> {
  const merged: Record<string, unknown> = { ...base };
  for (const [key, value] of Object.entries(overlay)) {
    if (value === null) {
      delete merged[key];
    } else if (isPlainObject(value) && isPlainObject(merged[key])) {
      merged[key] = mergeSettings(merged[key] as Record<string, unknown>, value);
    } else {
      merged[key] = value;
    }
  }
  return merged;
}

// d855: a fresh secret is created 0600 so no umask window exposes it.
export async function writeSecretFile(path: string, content: string): Promise<void> {
  if (!existsSync(path)) {
    await writeFile(path, content, { mode: 0o600 });
    return;
  }
  await chmod(path, 0o600);
  if (await readFile(path, "utf8") !== content) await writeFile(path, content);
}

// d846: read from stdin so the token never rides argv; it lives only in the 0600 token file and each seat's env.
export async function writeSeatToken(home: string, raw: string): Promise<void> {
  const token = raw.trim();
  if (token === "") {
    throw new CliError(
      "SEAT_TOKEN_EMPTY",
      "maestro install --seat-token reads the token from stdin and it was empty; run: claude setup-token | maestro install --seat-token",
    );
  }
  await mkdir(seatConfigRoot(home), { mode: 0o700, recursive: true });
  await chmod(seatConfigRoot(home), 0o700);
  await writeSecretFile(seatTokenPath(home), token);
}

export async function removeSeatConfig(home: string): Promise<string[]> {
  const removed: string[] = [];
  for (const path of [...seatNames.map((seat) => seatLaunchPath(home, "codex", seat)), seatConfigRoot(home)]) {
    if (!existsSync(path)) continue;
    await rm(path, { recursive: true, force: true });
    removed.push(path);
  }
  return removed;
}

async function ensureLink(link: string, target: string): Promise<void> {
  try {
    const entry = await lstat(link);
    if (!entry.isSymbolicLink()) return;
    if (await readlink(link) === target) return;
    await rm(link, { force: true });
  } catch (error) {
    if (!(error instanceof Error) || !("code" in error) || error.code !== "ENOENT") throw error;
  }
  await symlink(target, link);
}

// d855: a skill's realpath must stay under the root it was found in.
function resolveSkill(home: string, path: string, name: string): { name: string; target: string } {
  const roots = [join(home, "maestro", "skills"), join(home, ".claude", "skills")];
  for (const root of roots) {
    const target = join(root, name);
    if (!existsSync(join(target, "SKILL.md"))) continue;
    const real = realpathSync(target);
    if (real === realpathSync(root) || !real.startsWith(`${realpathSync(root)}${sep}`)) {
      throw invalid(path, `skill ${name} resolves to ${real}, outside ${root}`);
    }
    return { name, target };
  }
  throw invalid(path, `unknown skill ${name}: not in ${roots.map((root) => join(root, name)).join(" or ")}`);
}

async function materializeSeatDirectory(home: string, seat: SeatName, resolved: Seat, deny: string[], token: string | null): Promise<string> {
  const directory = seatDirectory(home, seat);
  const skills = (resolved.config.skills ?? [])
    .filter((name) => !(retiredSkillNames as readonly string[]).includes(name))
    .map((name) => resolveSkill(home, resolved.paths[0] as string, name));
  // d855: 0700 at creation; the chmods keep a pre-existing dir and file private.
  await mkdir(join(directory, "skills"), { mode: 0o700, recursive: true });
  await chmod(seatConfigRoot(home), 0o700);
  await chmod(directory, 0o700);
  const settings = mergeSettings(seatSettingsBase(home, token), resolved.config.settings ?? {});
  // d854: an overlay permissions.deny is unioned with the split, never replaced.
  if (deny.length > 0) {
    const permissions = isPlainObject(settings.permissions) ? settings.permissions : {};
    const overlayDeny = Array.isArray(permissions.deny) ? (permissions.deny as string[]) : [];
    settings.permissions = { ...permissions, deny: [...deny, ...overlayDeny.filter((tool) => !deny.includes(tool))] };
  }
  await writeSecretFile(join(directory, "settings.json"), `${JSON.stringify(settings, null, 2)}\n`);
  const skillsDirectory = join(directory, "skills");
  const wanted = new Set(skills.map((link) => link.name));
  for (const entry of await readdir(skillsDirectory)) {
    const path = join(skillsDirectory, entry);
    if (!wanted.has(entry) && (await lstat(path)).isSymbolicLink()) await rm(path, { force: true });
  }
  for (const link of skills) await ensureLink(join(skillsDirectory, link.name), link.target);
  for (const shared of ["projects", "plugins"]) await ensureLink(join(directory, shared), join(home, ".claude", shared));
  const claudeJson = join(directory, ".claude.json");
  if (existsSync(claudeJson)) {
    await chmod(claudeJson, 0o600);
  } else {
    await writeSecretFile(claudeJson, `${JSON.stringify({ hasCompletedOnboarding: true, mcpServers: {} }, null, 2)}\n`);
  }
  return directory;
}

export interface SeatSync {
  hubPackVersion: string | null;
  hubPeerSection: "missing" | "present";
  launchFiles: string[];
  seatDirectories: string[];
  seatToken: "missing" | "present";
}

// Both carriers are written whatever the harness, so an owner file that switches harness needs no second step.
export async function materializeSeats(home: string, repo: string): Promise<SeatSync> {
  const shared = await sharedContracts(home);
  const token = readSeatToken(home);
  const launchFiles: string[] = [];
  const seatDirectories: string[] = [];
  for (const seat of seatNames) {
    const resolved = await resolveSeat(seat, repo, home);
    const shipped = parseSeatFile(join(shippedSeats, `${seat}.md`), await readFile(join(shippedSeats, `${seat}.md`), "utf8"), "shipped");
    // d855: the shipped seat's patterns are the floor of its deny.
    const deny = [...shipped.config.disallowed_tools ?? [], ...resolved.config.disallowed_tools ?? []]
      .filter((tool, index, all) => tool.includes("(") && all.indexOf(tool) === index);
    const rendered = renderSeat(seat, resolved.config, `${shared[seat]}\n\n${resolved.body}`);
    for (const [harness, content] of [["claude", rendered.claude], ["codex", rendered.codex]] as const) {
      const path = seatLaunchPath(home, harness, seat);
      await mkdir(join(path, ".."), { recursive: true });
      if (!existsSync(path) || await readFile(path, "utf8") !== content) await writeFile(path, content);
      launchFiles.push(path);
    }
    seatDirectories.push(await materializeSeatDirectory(home, seat, resolved, deny, token));
  }
  const hubPack = join(home, "maestro", "SLP.md");
  return {
    hubPackVersion: existsSync(hubPack) ? /<!-- slp:version=([^\s]+) -->/.exec(readFileSync(hubPack, "utf8"))?.[1] ?? "unknown" : null,
    hubPeerSection: shared.peerSection,
    launchFiles,
    seatDirectories,
    seatToken: token ? "present" : "missing",
  };
}

export function formatSeatSync(sync: SeatSync, home: string): string {
  const parts = [`seat dirs: ${sync.seatDirectories.join(", ")}`];
  // A stale pack renders a stale mandate, and team start refuses it anyway (D7).
  if (sync.hubPackVersion !== null && sync.hubPackVersion !== "3") {
    parts.push(
      `warning: the Hub SLP.md is pack version ${sync.hubPackVersion}, so the rendered seats carry its shared contract and team start will refuse it; migrate it to version 3 (no Observer section) and run maestro install again`,
    );
  }
  // d892: install never refreshes the Hub pack, so this names the hand-copy that finishes the change.
  if (sync.hubPeerSection === "missing") {
    parts.push(
      `warning: ${join(home, "maestro", "SLP.md")} has no <!-- slp:shared:peer:begin -->...<!-- slp:shared:peer:end --> block, so the Peer seat renders from that pack's shared contract instead; copy the block from ${shippedPack} between its slp:shared:end and slp:role:hub-supervisor:begin lines and run maestro install again`,
    );
  }
  return parts.join("\n");
}
