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
import { delimiter, dirname, join, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { CliError, type CliResult } from "../kernel/cli.ts";
import type { BuiltInPlugin } from "../kernel/loader.ts";
import { readInstallStamp, writeInstallStamp } from "./install-stamp.ts";
import { writeSourceRecord } from "./source-record.ts";

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
];

const managedIgnoreBegin = "# maestro-ts:begin";
const managedIgnoreEnd = "# maestro-ts:end";
const managedIgnoreBlock = `${managedIgnoreBegin}\nmaestro.db\nmaestro.db-*\nconfig\n${managedIgnoreEnd}`;
const managedAdapters = [
  ".maestro/hooks/record.ts",
  ".claude/hooks/maestro-record.ts",
  ".codex/hooks/maestro-record.ts",
];

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

async function writeHookConfig(path: string, command: string): Promise<void> {
  const config = await readJson<HookConfig>(path, { hooks: {} });
  config.hooks ??= {};
  for (const event of ["SessionStart", "UserPromptSubmit"]) {
    const groups = config.hooks[event] ?? [];
    for (const group of groups) {
      group.hooks = group.hooks.filter(
        (handler) =>
          !managedAdapters.some((adapter) => handler.command.includes(adapter)),
      );
    }
    config.hooks[event] = [
      ...groups.filter((group) => group.hooks.length > 0),
      {
        hooks: [
          {
            type: "command",
            command,
            statusMessage: "Loading maestro state",
          },
        ],
      },
    ];
  }
  await mkdir(dirname(path), { recursive: true });
  await writeFile(path, `${JSON.stringify(config, null, 2)}\n`);
}

async function writeMirror(path: string): Promise<void> {
  const begin = "<!-- maestro:begin -->";
  const end = "<!-- maestro:end -->";
  const block = `${begin}\nLive maestro state is injected by hooks. Use \`maestro status\` for the current session view and \`maestro ready\` for available work.\nTrack work with \`maestro work add|start|done\`; method depth: \`maestro recipe show work\`.\nIf no harness hook fired, run \`maestro hook record --event SessionStart\` and read the brief from stdout.\nFailed commands print a JSON error envelope on stderr and exit nonzero; when the fix is mechanical, the message names the next command to run.\n${end}`;
  const existing = existsSync(path) ? await readFile(path, "utf8") : "";
  const cleaned = existing.replace(
    /\n?<!-- maestro:begin -->[\s\S]*?<!-- maestro:end -->\n?/g,
    "\n",
  );
  await writeFile(path, `${cleaned.trimEnd()}${cleaned.trim() ? "\n\n" : ""}${block}\n`);
}

async function writeManagedIgnore(path: string): Promise<void> {
  const existing = existsSync(path) ? await readFile(path, "utf8") : "";
  const cleaned = existing.replace(
    /\n?# maestro-ts:begin\n[\s\S]*?# maestro-ts:end\n?/g,
    "\n",
  );
  await mkdir(dirname(path), { recursive: true });
  await writeFile(path, `${cleaned.trimEnd()}${cleaned.trim() ? "\n\n" : ""}${managedIgnoreBlock}\n`);
}

async function removeManagedHooks(path: string): Promise<boolean> {
  if (!existsSync(path)) return false;
  const config = await readJson<HookConfig>(path, { hooks: {} });
  let changed = false;
  if (config.hooks) {
    for (const [event, groups] of Object.entries(config.hooks)) {
      const retained = groups
        .map((group) => {
          const hooks = group.hooks.filter(
            (handler) => !managedAdapters.some((adapter) => handler.command.includes(adapter)),
          );
          if (hooks.length !== group.hooks.length) changed = true;
          return { ...group, hooks };
        })
        .filter((group) => group.hooks.length > 0);
      if (retained.length > 0) {
        config.hooks[event] = retained;
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

async function removeManagedMirror(path: string): Promise<boolean> {
  if (!existsSync(path)) return false;
  const existing = await readFile(path, "utf8");
  const cleaned = existing.replace(
    /\n?<!-- maestro:begin -->[\s\S]*?<!-- maestro:end -->\n?/g,
    "\n",
  );
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

async function removeManagedIgnore(path: string): Promise<boolean> {
  if (!existsSync(path)) return false;
  const existing = await readFile(path, "utf8");
  const cleaned = existing.replace(
    /\n?# maestro-ts:begin\n[\s\S]*?# maestro-ts:end\n?/g,
    "\n",
  );
  if (cleaned === existing) return false;
  const normalized = cleaned.replace(/\n{3,}/g, "\n\n").trimEnd();
  if (!normalized) {
    await rm(path, { force: true });
  } else {
    await writeFile(path, `${normalized}\n`);
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
    if (await removeManagedMirror(path)) removed.push(`${path} mirror block`);
  }
  const config = join(repo, ".maestro", "config");
  if (await removeManagedPolicyConfig(config)) removed.push(`${config} managed plugins`);
  const ignore = join(repo, ".maestro", ".gitignore");
  if (await removeManagedIgnore(ignore)) removed.push(`${ignore} managed block`);
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
    context.effect(() =>
      context.cli.register("install", async (): Promise<CliResult> => {
        const repo = process.cwd();
        const home = process.env.HOME ?? repo;
        const localBin = join(home, ".local", "bin");
        const shim = join(localBin, "maestro");
        const legacy = join(localBin, "maestro-legacy");
        const runtimeRoot = join(home, ".maestro", "runtime");
        const sourceRoot = await resolveSourceRoot(repo);
        const installedRuntime = await samePath(sourceRoot, runtimeRoot);
        if (!installedRuntime) {
          const existingLegacy = await executable("maestro-legacy");
          const existingMaestro = await executable("maestro");
          if (!existingLegacy && !existingMaestro) {
            throw new CliError(
              "ROLLBACK_MISSING",
              "maestro-legacy must exist on PATH, or an existing maestro must be available to preserve",
            );
          }
          await mkdir(localBin, { recursive: true });
          if (!existingLegacy && existingMaestro) {
            await copyFile(existingMaestro, legacy);
            await chmod(legacy, 0o755);
          }
          if (!(await executable("maestro-legacy"))) {
            throw new CliError(
              "ROLLBACK_NOT_ON_PATH",
              `${legacy} was created but maestro-legacy is not available on PATH`,
            );
          }

          const stampBefore = await readInstallStamp(runtimeRoot);
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

        const codexConfigPath = join(repo, ".codex", "hooks.json");
        const codexHooksBefore = existsSync(codexConfigPath)
          ? await readFile(codexConfigPath, "utf8")
          : null;
        const adapters = [
          {
            configPath: join(repo, ".claude", "settings.json"),
            harness: "claude" as const,
            hookCommand: "bun .claude/hooks/maestro-record.ts",
            hookPath: join(repo, ".claude", "hooks", "maestro-record.ts"),
          },
          {
            configPath: join(repo, ".codex", "hooks.json"),
            harness: "codex" as const,
            hookCommand: "bun .codex/hooks/maestro-record.ts",
            hookPath: join(repo, ".codex", "hooks", "maestro-record.ts"),
          },
        ];
        for (const adapter of adapters) {
          await mkdir(dirname(adapter.hookPath), { recursive: true });
          await writeFile(adapter.hookPath, hookSource(adapter.harness));
          await chmod(adapter.hookPath, 0o755);
          await writeHookConfig(adapter.configPath, adapter.hookCommand);
        }
        const codexHooksChanged = codexHooksBefore !== await readFile(codexConfigPath, "utf8");
        await writeMirror(join(repo, "AGENTS.md"));
        await writeMirror(join(repo, "CLAUDE.md"));

        if (!installedRuntime) {
          await writeFile(
            shim,
            `#!/usr/bin/env bun\nawait import(${JSON.stringify(pathToFileURL(join(runtimeRoot, "bin", "maestro.ts")).href)});\n`,
          );
          await chmod(shim, 0o755);
        }
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
            (codexHooksChanged ? "\nreview Codex hook trust with /hooks" : ""),
        };
      }, { description: "Install Maestro runtime and repository hook wiring." }),
    );
  },
};
