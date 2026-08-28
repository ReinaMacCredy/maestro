import { constants, existsSync } from "node:fs";
import {
  access,
  chmod,
  copyFile,
  cp,
  mkdir,
  readFile,
  realpath,
  rm,
  writeFile,
} from "node:fs/promises";
import { basename, delimiter, dirname, join, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { CliError, requiredPosition, type CliResult } from "../kernel/cli.ts";
import type { BuiltInPlugin } from "../kernel/loader.ts";
import { warnBeforeRuntimeActivation } from "./activation-scan.ts";
import { readInstallStamp, writeInstallStamp } from "./install-stamp.ts";
import { resolveHomeDirectory } from "./home.ts";
import { installInRoomMessage, isRoom, scaffoldRoom } from "./room.ts";
import { grandfatherHomePlugins } from "./plugin-trust.ts";
import { formatSkillSync, materializeSkills } from "./skills.ts";
import { registerSessionCommand } from "./session-required.ts";
import { sourceRecordPath, writeSourceRecord } from "./source-record.ts";

interface PluginEntry {
  disabled?: boolean;
  name: string;
}

interface PluginConfig {
  plugins: PluginEntry[];
}

interface HookHandler {
  command: string;
  statusMessage?: string;
  type: "command";
}

interface HookGroup {
  hooks: HookHandler[];
  matcher?: string;
}

interface HookConfig {
  description?: string;
  hooks?: Record<string, HookGroup[]>;
  [key: string]: unknown;
}

interface RoomClaudeSettings extends HookConfig {
  permissions?: {
    deny?: unknown[];
    [key: string]: unknown;
  };
}

interface PackageJson {
  name?: string;
  version: string;
}

const policyDefaults: PluginEntry[] = [
  { name: "policy-proof", disabled: false },
  { name: "policy-breakdown", disabled: false },
  { name: "policy-tdd", disabled: true },
  { name: "policy-qa", disabled: true },
  { name: "policy-research", disabled: true },
  { name: "policy-witness", disabled: true },
  { name: "policy-lifecycle", disabled: true },
];

const managedIgnoreBegin = "# maestro-ts:begin";
const managedIgnoreEnd = "# maestro-ts:end";
const managedIgnoreBlock = `${managedIgnoreBegin}\nmaestro.db\nmaestro.db-*\nconfig\n${managedIgnoreEnd}`;
const managedAdapters = [
  ".maestro/hooks/record.ts",
  ".claude/hooks/maestro-record.ts",
  ".codex/hooks/maestro-record.ts",
];
const shellSourceLine =
  '[[ -f "$HOME/maestro/shellrc" ]] && source "$HOME/maestro/shellrc" # maestro';

function emptyObject(value: Record<string, unknown>): boolean {
  return Object.keys(value).length === 0;
}

async function executable(name: string): Promise<string | null> {
  for (const directory of (process.env.PATH ?? "").split(delimiter)) {
    if (!directory) continue;
    const candidate = join(directory, name);
    try {
      await access(candidate, constants.X_OK);
      return candidate;
    } catch {
      continue;
    }
  }
  return null;
}

async function readJson<T>(path: string, fallback: T): Promise<T> {
  if (!existsSync(path)) return fallback;
  try {
    return JSON.parse(await readFile(path, "utf8")) as T;
  } catch (error) {
    throw new CliError(
      "INVALID_CONFIG",
      `cannot update invalid JSON ${path}: ${error instanceof Error ? error.message : String(error)}`,
    );
  }
}

async function writePolicyConfig(path: string): Promise<void> {
  const config = await readJson<PluginConfig>(path, { plugins: [] });
  const defaults = new Map(policyDefaults.map((entry) => [entry.name, entry]));
  const retained = config.plugins.filter((entry) => !defaults.has(entry.name));
  config.plugins = [...retained, ...policyDefaults];
  await mkdir(dirname(path), { recursive: true });
  await writeFile(path, `${JSON.stringify(config)}\n`);
}

async function writeHookConfig(
  path: string,
  command: string,
  harness: "claude" | "codex",
): Promise<void> {
  const config = await readJson<HookConfig>(path, { hooks: {} });
  config.hooks ??= {};
  for (const event of ["SessionStart", "UserPromptSubmit"]) {
    const retained = retainForeignHookGroups(config.hooks[event] ?? []);
    const handler: HookHandler = {
      type: "command",
      command,
      statusMessage: "Loading maestro state",
    };
    config.hooks[event] = [
      ...retained.groups,
      {
        hooks: [handler],
      },
    ];
  }
  if (harness === "claude") {
    const retained = retainForeignHookGroups(config.hooks.PreToolUse ?? []);
    config.hooks.PreToolUse = [
      ...retained.groups,
      {
        matcher: "Agent|Task",
        hooks: [{ type: "command", command }],
      },
    ];
  }
  const retiredGroups = retainForeignHookGroups(config.hooks.PostToolUse ?? []).groups;
  if (retiredGroups.length > 0) {
    config.hooks.PostToolUse = retiredGroups;
  } else {
    delete config.hooks.PostToolUse;
  }
  await mkdir(dirname(path), { recursive: true });
  await writeFile(path, `${JSON.stringify(config, null, 2)}\n`);
}

async function writeRoomDenySettings(room: string): Promise<void> {
  const path = join(room, ".claude", "settings.json");
  const settings = await readJson<RoomClaudeSettings>(path, {});
  const existingPermissions = settings.permissions;
  if (
    existingPermissions !== undefined &&
    (typeof existingPermissions !== "object" ||
      existingPermissions === null ||
      Array.isArray(existingPermissions))
  ) {
    throw new CliError("INVALID_CONFIG", `cannot update invalid permissions in ${path}`);
  }
  const permissions = existingPermissions ?? {};
  if (permissions.deny !== undefined && !Array.isArray(permissions.deny)) {
    throw new CliError("INVALID_CONFIG", `cannot update non-array permissions.deny in ${path}`);
  }
  const deny = [...(permissions.deny ?? [])];
  for (const tool of ["Agent", "Task"]) {
    if (!deny.includes(tool)) deny.push(tool);
  }
  settings.permissions = { ...permissions, deny };
  await writeFile(path, `${JSON.stringify(settings, null, 2)}\n`);
  await chmod(path, 0o600);
}

function retainForeignHookGroups(groups: HookGroup[]): { changed: boolean; groups: HookGroup[] } {
  let changed = false;
  const retained = groups
    .map((group) => {
      const hooks = group.hooks.filter(
        (handler) => !managedAdapters.some((adapter) => handler.command.includes(adapter)),
      );
      if (hooks.length !== group.hooks.length) changed = true;
      return { ...group, hooks };
    })
    .filter((group) => group.hooks.length > 0);
  return { changed, groups: retained };
}

function upsertManagedBlock(existing: string, pattern: RegExp, block: string): string {
  const cleaned = existing.replace(pattern, "\n");
  return `${cleaned.trimEnd()}${cleaned.trim() ? "\n\n" : ""}${block}\n`;
}

async function writeMirror(path: string): Promise<void> {
  const begin = "<!-- maestro:begin -->";
  const end = "<!-- maestro:end -->";
  const block = `${begin}\nThe Lead of this repository is the agent the room started as \`lead-<repo basename>\`; a pane it opens with a dispatch is a Peer named \`peer-<dispatch id>\`; a session with any other name holds only what its accepted dispatch says; the room at ~/maestro is the Supervisor. Roles: \`maestro recipe show slp\`.\nThe repository's own \`AGENTS.md\` and \`CLAUDE.md\` text outside this block is its Workspace Protocol and may declare protected areas, hotspots, restart rules, and local verification; read it before taking work or opening a dispatch.\nLive maestro state is injected by hooks. Use \`maestro status\` for the current session view and \`maestro ready\` for available work.\nTrack work with \`maestro work add|start|done\`; method depth: \`maestro recipe show work\`.\nIf no harness hook fired, run \`maestro hook record --event SessionStart\` and read the brief from stdout.\nFailed commands print a JSON error envelope on stderr and exit nonzero; when the fix is mechanical, the message names the next command to run.\n${end}`;
  const existing = existsSync(path) ? await readFile(path, "utf8") : "";
  await writeFile(
    path,
    upsertManagedBlock(
      existing,
      /\n?<!-- maestro:begin -->[\s\S]*?<!-- maestro:end -->\n?/g,
      block,
    ),
  );
}

async function writeManagedIgnore(path: string): Promise<void> {
  const existing = existsSync(path) ? await readFile(path, "utf8") : "";
  await mkdir(dirname(path), { recursive: true });
  await writeFile(
    path,
    upsertManagedBlock(
      existing,
      /\n?# maestro-ts:begin\n[\s\S]*?# maestro-ts:end\n?/g,
      managedIgnoreBlock,
    ),
  );
}

async function registerRepository(home: string, repo: string): Promise<void> {
  const registry = join(home, "maestro", "registry");
  await mkdir(dirname(registry), { recursive: true });
  const existing = existsSync(registry) ? await readFile(registry, "utf8") : "";
  const entries = existing.split(/\r?\n/).filter(Boolean);
  const next = [...new Set([...entries, resolve(repo)])];
  const content = `${next.join("\n")}\n`;
  if (content !== existing) await writeFile(registry, content);
  await chmod(registry, 0o600);
}

export async function forgetRepository(home: string, repo: string): Promise<boolean> {
  const registry = join(home, "maestro", "registry");
  if (!existsSync(registry)) return false;
  const existing = await readFile(registry, "utf8");
  const retained: string[] = [];
  for (const entry of existing.split(/\r?\n/).filter(Boolean)) {
    if (!(await samePath(entry, repo))) retained.push(entry);
  }
  const content = retained.length > 0 ? `${retained.join("\n")}\n` : "";
  if (content === existing) return false;
  await writeFile(registry, content);
  await chmod(registry, 0o600);
  return true;
}

async function writeShellSource(home: string): Promise<boolean> {
  const shell = basename(process.env.SHELL ?? "");
  const rcName = shell === "zsh" ? ".zshrc" : shell === "bash" ? ".bashrc" : null;
  if (!rcName) return false;
  const shellRc = join(home, rcName);
  const backup = `${shellRc}.maestro.bak`;
  const existing = existsSync(shellRc) ? await readFile(shellRc, "utf8") : "";
  const managedCount = existing.split("\n").filter((line) => line === shellSourceLine).length;
  if (managedCount === 1) return true;
  if (!existsSync(backup)) {
    if (existsSync(shellRc)) {
      await copyFile(shellRc, backup);
    } else {
      await writeFile(backup, "");
    }
  }
  const retained = existing.split("\n").filter((line) => line !== shellSourceLine).join("\n");
  const prefix = retained === "" || retained.endsWith("\n") ? retained : `${retained}\n`;
  await writeFile(shellRc, `${prefix}${shellSourceLine}\n`);
  return true;
}

async function initializeRoomStore(home: string, room: string, runtimeRoot: string): Promise<void> {
  for (const [args, environment] of [
    [["version"], {}],
    [["room", "mark"], { MAESTRO_ROOM_SCAFFOLD: "1" }],
  ] as const) {
    const child = Bun.spawn(
      [process.execPath, join(runtimeRoot, "bin", "maestro.ts"), ...args],
      {
        cwd: room,
        env: {
          ...process.env,
          ...environment,
          HOME: home,
          MAESTRO_READ_ONLY: "0",
          MAESTRO_SESSION_NONE: "1",
        },
        stdout: "pipe",
        stderr: "pipe",
      },
    );
    const [, stderr, exitCode] = await Promise.all([
      new Response(child.stdout).text(),
      new Response(child.stderr).text(),
      child.exited,
    ]);
    if (exitCode !== 0) {
      throw new CliError(
        "ROOM_STORE_INIT",
        `cannot initialize ${join(room, ".maestro", "maestro.db")}: ${stderr.trim()}`,
      );
    }
  }
  await chmod(join(room, ".maestro"), 0o700);
  for (const suffix of ["", "-wal", "-shm"]) {
    const path = join(room, ".maestro", `maestro.db${suffix}`);
    if (existsSync(path)) await chmod(path, 0o600);
  }
}

async function removeManagedHooks(path: string): Promise<boolean> {
  if (!existsSync(path)) return false;
  const config = await readJson<HookConfig>(path, { hooks: {} });
  let changed = false;
  if (config.hooks) {
    for (const [event, groups] of Object.entries(config.hooks)) {
      const retained = retainForeignHookGroups(groups);
      if (retained.changed) changed = true;
      if (retained.groups.length > 0) {
        config.hooks[event] = retained.groups;
      } else {
        delete config.hooks[event];
      }
    }
    if (emptyObject(config.hooks)) delete config.hooks;
  }
  if (!changed) return false;
  if (emptyObject(config)) {
    await rm(path, { force: true });
  } else {
    await writeFile(path, `${JSON.stringify(config, null, 2)}\n`);
  }
  return true;
}

async function removeManagedBlock(path: string, pattern: RegExp): Promise<boolean> {
  if (!existsSync(path)) return false;
  const existing = await readFile(path, "utf8");
  const cleaned = existing.replace(pattern, "\n");
  if (cleaned === existing) return false;
  const normalized = cleaned.replace(/\n{3,}/g, "\n\n").trimEnd();
  if (!normalized) {
    await rm(path, { force: true });
  } else {
    await writeFile(path, `${normalized}\n`);
  }
  return true;
}

async function removeManagedPolicyConfig(path: string): Promise<boolean> {
  if (!existsSync(path)) return false;
  const config = await readJson<Record<string, unknown> & { plugins?: PluginEntry[] }>(path, {});
  const plugins = Array.isArray(config.plugins) ? config.plugins : [];
  const managed = new Set(policyDefaults.map((entry) => entry.name));
  const retained = plugins.filter((entry) => !managed.has(entry.name));
  if (retained.length === plugins.length) return false;
  if (retained.length > 0) {
    config.plugins = retained;
  } else {
    delete config.plugins;
  }
  if (emptyObject(config)) {
    await rm(path, { force: true });
  } else {
    await writeFile(path, `${JSON.stringify(config)}\n`);
  }
  return true;
}

export async function uninstallRepo(repo: string): Promise<string[]> {
  const removed: string[] = [];
  for (const path of [
    join(repo, ".claude", "hooks", "maestro-record.ts"),
    join(repo, ".codex", "hooks", "maestro-record.ts"),
  ]) {
    if (!existsSync(path)) continue;
    await rm(path, { force: true });
    removed.push(path);
  }
  for (const path of [
    join(repo, ".claude", "settings.json"),
    join(repo, ".codex", "hooks.json"),
  ]) {
    if (await removeManagedHooks(path)) removed.push(`${path} managed hooks`);
  }
  for (const path of [join(repo, "AGENTS.md"), join(repo, "CLAUDE.md")]) {
    if (
      await removeManagedBlock(
        path,
        /\n?<!-- maestro:begin -->[\s\S]*?<!-- maestro:end -->\n?/g,
      )
    ) removed.push(`${path} mirror block`);
  }
  const config = join(repo, ".maestro", "config");
  if (await removeManagedPolicyConfig(config)) removed.push(`${config} managed plugins`);
  const ignore = join(repo, ".maestro", ".gitignore");
  if (
    await removeManagedBlock(
      ignore,
      /\n?# maestro-ts:begin\n[\s\S]*?# maestro-ts:end\n?/g,
    )
  ) removed.push(`${ignore} managed block`);
  return removed;
}

function hookSource(harness: "claude" | "codex"): string {
  return `#!/usr/bin/env bun
const raw = await Bun.stdin.text();
const input = raw.trim() ? JSON.parse(raw) : {};
const event = typeof input.hook_event_name === "string" ? input.hook_event_name : "SessionStart";
const sessionId = typeof input.session_id === "string" ? input.session_id : undefined;
const child = Bun.spawn(["maestro", "hook", "record", "--event", event, "--harness", "${harness}"], {
  cwd: typeof input.cwd === "string" ? input.cwd : process.cwd(),
  env: { ...process.env, ...(sessionId ? { MAESTRO_SESSION_ID: sessionId } : {}) },
  stdin: new TextEncoder().encode(raw),
  stdout: "pipe",
  stderr: "pipe",
});
const [stdout, stderr, exitCode] = await Promise.all([
  new Response(child.stdout).text(),
  new Response(child.stderr).text(),
  child.exited,
]);
if (stdout) process.stdout.write(stdout);
if (stderr) process.stderr.write(stderr);
process.exitCode = exitCode;
`;
}

async function writeHarnessWiring(root: string): Promise<boolean> {
  const codexConfigPath = join(root, ".codex", "hooks.json");
  const codexHooksBefore = existsSync(codexConfigPath)
    ? await readFile(codexConfigPath, "utf8")
    : null;
  const adapters = [
    {
      configPath: join(root, ".claude", "settings.json"),
      harness: "claude" as const,
      hookCommand: "bun .claude/hooks/maestro-record.ts",
      hookPath: join(root, ".claude", "hooks", "maestro-record.ts"),
    },
    {
      configPath: codexConfigPath,
      harness: "codex" as const,
      hookCommand: "bun .codex/hooks/maestro-record.ts",
      hookPath: join(root, ".codex", "hooks", "maestro-record.ts"),
    },
  ];
  for (const adapter of adapters) {
    await mkdir(dirname(adapter.hookPath), { recursive: true });
    await writeFile(adapter.hookPath, hookSource(adapter.harness));
    await chmod(adapter.hookPath, 0o755);
    await writeHookConfig(adapter.configPath, adapter.hookCommand, adapter.harness);
  }
  return codexHooksBefore !== await readFile(codexConfigPath, "utf8");
}

// Codex records hook trust in ~/.codex/config.toml as
// [hooks.state."<hooks.json>:<event>:<group>:<index>"] trusted_hash = "sha256:..."
// (Codex CLI 0.149, read 2026-08-27). The hashed bytes are undocumented, so
// this only proves Codex recorded trust for both events at this path; it never
// verifies the hash.
export async function codexHookTrustRecorded(root: string, home: string): Promise<boolean> {
  const config = join(home, ".codex", "config.toml");
  if (!existsSync(config)) return false;
  const text = await readFile(config, "utf8");
  const hooks = join(root, ".codex", "hooks.json");
  const missing = new Set(["session_start", "user_prompt_submit"]);
  for (const [, path, event, body] of text.matchAll(
    /^\[hooks\.state\."(.+?):([a-z_]+):0:0"\]\n((?:(?!\n\[)[\s\S])*)/gm,
  )) {
    if (!event || !missing.has(event) || !path || !body) continue;
    if (!/^\s*trusted_hash\s*=\s*"sha256:[0-9a-f]+"/m.test(body)) continue;
    // Codex records the path as it saw it; the fixture and macOS /tmp can
    // differ by a symlink, so compare resolved paths.
    if (await samePath(path, hooks)) missing.delete(event);
  }
  return missing.size === 0;
}

async function samePath(left: string, right: string): Promise<boolean> {
  if (resolve(left) === resolve(right)) return true;
  try {
    return (await realpath(left)) === (await realpath(right));
  } catch {
    return false;
  }
}

export async function syncRuntime(sourceRoot: string, runtimeRoot: string): Promise<void> {
  if (await samePath(sourceRoot, runtimeRoot)) return;
  await rm(runtimeRoot, { recursive: true, force: true });
  await mkdir(runtimeRoot, { recursive: true });
  for (const entry of ["package.json", "tsconfig.json", "bin", "src"]) {
    await cp(join(sourceRoot, entry), join(runtimeRoot, entry), { recursive: true });
  }
}

// Codex resolves project hook config to the git MAIN worktree, so wiring written
// only into a linked worktree is never read. Measured with Codex CLI v0.149.1
// running in a linked worktree: its /hooks panel names the main checkout's file
// and loads exactly one project config (d39).
export async function gitMainWorktree(repo: string): Promise<string | null> {
  const git = Bun.spawn(["git", "-C", repo, "rev-parse", "--git-common-dir"], {
    stderr: "ignore",
    stdout: "pipe",
  });
  const out = (await new Response(git.stdout).text()).trim();
  if ((await git.exited) !== 0 || !out) return null;
  const commonDir = resolve(repo, out);
  const root = basename(commonDir) === ".git" ? dirname(commonDir) : commonDir;
  return (await samePath(root, repo)) ? null : root;
}

export async function resolveSourceRoot(repo: string): Promise<string> {
  const loadedRoot = resolve(import.meta.dir, "..", "..");
  const packagePath = join(repo, "package.json");
  if (!existsSync(packagePath) || !existsSync(join(repo, "bin", "maestro.ts"))) {
    return loadedRoot;
  }
  try {
    const packageJson = JSON.parse(await readFile(packagePath, "utf8")) as PackageJson;
    return packageJson.name === "maestro" ? repo : loadedRoot;
  } catch {
    return loadedRoot;
  }
}

export async function readGitHeadCommit(sourceRoot: string): Promise<string | null> {
  const git = Bun.spawn(["git", "-C", sourceRoot, "rev-parse", "HEAD"], {
    stderr: "ignore",
    stdout: "pipe",
  });
  const [stdout, exitCode] = await Promise.all([
    new Response(git.stdout).text(),
    git.exited,
  ]);
  const commit = stdout.trim();
  return exitCode === 0 && /^[0-9a-f]{40}$/.test(commit) ? commit : null;
}

async function readStampedCommit(sourceRoot: string): Promise<string | null> {
  const result = await readInstallStamp(sourceRoot);
  return result.status === "valid" ? result.stamp.commit : null;
}

export async function stampRuntime(sourceRoot: string, runtimeRoot: string): Promise<void> {
  const packageJson = JSON.parse(
    await readFile(join(sourceRoot, "package.json"), "utf8"),
  ) as PackageJson;
  let commit: string | null;
  if (await samePath(sourceRoot, runtimeRoot)) {
    commit = await readStampedCommit(sourceRoot);
  } else {
    commit = await readGitHeadCommit(sourceRoot);
    commit ??= await readStampedCommit(sourceRoot);
  }
  if (!commit) {
    throw new CliError(
      "SOURCE_COMMIT_UNKNOWN",
      "cannot determine the Maestro source commit; run install from the Maestro source checkout",
    );
  }
  await writeInstallStamp(runtimeRoot, {
    version: packageJson.version,
    commit,
    installedAt: new Date().toISOString(),
  });
}

export const installPlugin: BuiltInPlugin = {
  name: "install",
  apply(context) {
    context.store.migrate(`
      CREATE TABLE IF NOT EXISTS meta (
        key TEXT PRIMARY KEY,
        value TEXT NOT NULL
      );
    `);
    context.effect(() =>
      context.cli.register("room mark", (): CliResult => {
        if (process.env.MAESTRO_ROOM_SCAFFOLD !== "1") {
          throw new CliError(
            "ROOM_MARK_INTERNAL",
            "room mark is reserved for the room-scaffolding code path",
          );
        }
        context.store.database
          .query(
            `INSERT INTO meta (key, value) VALUES ('kind', 'room')
             ON CONFLICT(key) DO UPDATE SET value = excluded.value`,
          )
          .run();
        return { data: { kind: "room" }, text: "room marked" };
      }),
    );
    context.effect(() =>
      registerSessionCommand(
        context,
        "room forget",
        async (invocation): Promise<CliResult> => {
          const path = resolve(requiredPosition(invocation, 0, "repository path"));
          const forgotten = await forgetRepository(resolveHomeDirectory(), path);
          return {
            data: { forgotten, path },
            text: forgotten ? `forgot: ${path}` : `not registered: ${path}`,
          };
        },
        {
          description: "Remove one repository from the room registry without uninstalling it.",
          positionals: [{ name: "path", required: true }],
          rootDescription: "Manage the room repository registry.",
        },
      ),
    );

    context.effect(() =>
      registerSessionCommand(context, "install", async (): Promise<CliResult> => {
        if (isRoom(context.store.database)) {
          throw new CliError("INSTALL_IN_ROOM", installInRoomMessage);
        }
        const repo = process.cwd();
        const home = resolveHomeDirectory();
        await grandfatherHomePlugins(home);
        const existingSourceRecord = sourceRecordPath(home);
        if (existsSync(existingSourceRecord)) {
          await chmod(dirname(existingSourceRecord), 0o700);
          await chmod(existingSourceRecord, 0o600);
        }
        const localBin = join(home, ".local", "bin");
        const shim = join(localBin, "maestro");
        const legacy = join(localBin, "maestro-legacy");
        const runtimeRoot = join(home, ".maestro", "runtime");
        const sourceRoot = await resolveSourceRoot(repo);
        const installedRuntime = await samePath(sourceRoot, runtimeRoot);
        if (!installedRuntime) {
          const existingLegacy = await executable("maestro-legacy");
          const existingMaestro = await executable("maestro");
          await mkdir(localBin, { recursive: true });
          let createdLegacy = false;
          if (!existingLegacy && existingMaestro) {
            await copyFile(existingMaestro, legacy);
            await chmod(legacy, 0o755);
            createdLegacy = true;
          }
          if (createdLegacy && !(await executable("maestro-legacy"))) {
            throw new CliError(
              "ROLLBACK_NOT_ON_PATH",
              `${legacy} was created but maestro-legacy is not available on PATH`,
            );
          }

          const stampBefore = await readInstallStamp(runtimeRoot);
          await warnBeforeRuntimeActivation(home, "install");
          await syncRuntime(sourceRoot, runtimeRoot);
          await stampRuntime(sourceRoot, runtimeRoot);
          // Wiring content (mirror blocks, hook sources) lives as constants in
          // the RUNNING process, which may still be the pre-sync runtime; when
          // the synced code changed, re-exec install in the new runtime so the
          // wiring it writes matches the code it installs.
          const stampAfter = await readInstallStamp(runtimeRoot);
          // import.meta.dir is symlink-resolved (e.g. /var -> /private/var);
          // compare realpaths or the runtime never recognizes itself.
          const runningRoot = resolve(import.meta.dir, "..", "..");
          if (
            !process.env.MAESTRO_INSTALL_REEXEC &&
            runningRoot === (await realpath(runtimeRoot)) &&
            stampBefore.status === "valid" &&
            stampAfter.status === "valid" &&
            stampBefore.stamp.commit !== stampAfter.stamp.commit
          ) {
            const child = Bun.spawnSync(
              [process.execPath, join(runtimeRoot, "bin", "maestro.ts"), ...process.argv.slice(2)],
              {
                cwd: repo,
                env: { ...process.env, MAESTRO_INSTALL_REEXEC: "1" },
                stdout: "inherit",
                stderr: "inherit",
              },
            );
            process.exit(child.exitCode ?? 1);
          }
          if ((await readGitHeadCommit(sourceRoot)) !== null) {
            await writeSourceRecord(home, sourceRoot);
          }
        }
        await writePolicyConfig(join(repo, ".maestro", "config"));
        await writeManagedIgnore(join(repo, ".maestro", ".gitignore"));

        const codexHooksChanged = await writeHarnessWiring(repo);
        const mainWorktree = await gitMainWorktree(repo);
        if (mainWorktree) {
          const mirrorHook = join(mainWorktree, ".codex", "hooks", "maestro-record.ts");
          await mkdir(dirname(mirrorHook), { recursive: true });
          await writeFile(mirrorHook, hookSource("codex"));
          await chmod(mirrorHook, 0o755);
          await writeHookConfig(
            join(mainWorktree, ".codex", "hooks.json"),
            "bun .codex/hooks/maestro-record.ts",
            "codex",
          );
        }
        await writeMirror(join(repo, "AGENTS.md"));
        await writeMirror(join(repo, "CLAUDE.md"));

        const skillCommit = (await readGitHeadCommit(sourceRoot)) ??
          (await readStampedCommit(runtimeRoot));
        const skillSync = skillCommit ? await materializeSkills(home, skillCommit) : null;

        if (!installedRuntime) {
          await writeFile(
            shim,
            `#!/usr/bin/env bun\nawait import(${JSON.stringify(pathToFileURL(join(runtimeRoot, "bin", "maestro.ts")).href)});\n`,
          );
          await chmod(shim, 0o755);
        }
        const room = await scaffoldRoom(home);
        await writeHarnessWiring(room);
        await writeRoomDenySettings(room);
        const roomCodexHookTrustRecorded = await codexHookTrustRecorded(room, home);
        await initializeRoomStore(home, room, runtimeRoot);
        await registerRepository(home, repo);
        const shellSourceWritten = await writeShellSource(home);
        context.log.append({
          type: "install",
          entityType: "repo",
          entityId: repo,
          sessionId: context.sessions.current().id,
          payload: { runtimeRoot, shim },
        });
        return {
          data: { repo, runtimeRoot, shim, legacy },
          text:
            `maestro installed for ${repo}` +
            "\nwrote: .maestro/, .claude/hooks/, .codex/hooks/, AGENTS.md, CLAUDE.md" +
            " (AGENTS.md and CLAUDE.md carry the same maestro block for Claude and Codex)" +
            (mainWorktree
              ? `\nalso wrote ${join(mainWorktree, ".codex")} (Codex reads project hooks from the git main worktree)`
              : "") +
            (skillSync ? `\n${formatSkillSync(skillSync)}` : "") +
            `\nroom: ${room}` +
            `\nregistered: ${resolve(repo)} in ${join(home, "maestro", "registry")}` +
            (shellSourceWritten ? "" : `\nadd this line to your shell startup file: ${shellSourceLine}`) +
            `\nroom Codex setup: trust ${room} when Codex asks, then open /hooks and trust both room-local Maestro hooks; start a new Codex session afterward` +
            (roomCodexHookTrustRecorded ? " (Codex has recorded trust for both hooks; the hash is not verifiable here)" : "") +
            (codexHooksChanged ? "\nreview Codex hook trust with /hooks" : ""),
        };
      }, { description: "Install Maestro runtime and repository hook wiring." }),
    );
  },
};
