import { constants, existsSync } from "node:fs";
import {
  access,
  chmod,
  copyFile,
  cp,
  mkdir,
  readFile,
  rm,
  writeFile,
} from "node:fs/promises";
import { delimiter, dirname, join, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { CliError, type CliResult } from "../kernel/cli.ts";
import type { BuiltInPlugin } from "../kernel/loader.ts";

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

const policyDefaults: PluginEntry[] = [
  { name: "policy-proof", disabled: false },
  { name: "policy-breakdown", disabled: false },
  { name: "policy-tdd", disabled: true },
  { name: "policy-qa", disabled: true },
  { name: "policy-research", disabled: true },
];

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
      group.hooks = group.hooks.filter((handler) => !handler.command.includes(".maestro/hooks/record.ts"));
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
  const block = `${begin}\nLive maestro state is injected by hooks. Use \`maestro status\` for the current session view and \`maestro ready\` for available work.\n${end}`;
  const existing = existsSync(path) ? await readFile(path, "utf8") : "";
  const cleaned = existing.replace(
    /\n?<!-- maestro:begin -->[\s\S]*?<!-- maestro:end -->\n?/g,
    "\n",
  );
  await writeFile(path, `${cleaned.trimEnd()}${cleaned.trim() ? "\n\n" : ""}${block}\n`);
}

function hookSource(): string {
  return `#!/usr/bin/env bun
const raw = await Bun.stdin.text();
const input = raw.trim() ? JSON.parse(raw) : {};
const event = typeof input.hook_event_name === "string" ? input.hook_event_name : "SessionStart";
const sessionId = typeof input.session_id === "string" ? input.session_id : undefined;
const child = Bun.spawn(["maestro", "hook", "record", "--event", event], {
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

async function syncRuntime(sourceRoot: string, runtimeRoot: string): Promise<void> {
  if (resolve(sourceRoot) === resolve(runtimeRoot)) return;
  await rm(runtimeRoot, { recursive: true, force: true });
  await mkdir(runtimeRoot, { recursive: true });
  for (const entry of ["package.json", "tsconfig.json", "bin", "src"]) {
    await cp(join(sourceRoot, entry), join(runtimeRoot, entry), { recursive: true });
  }
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

        const runtimeRoot = join(home, ".maestro", "runtime");
        const sourceRoot = join(import.meta.dir, "..", "..");
        await syncRuntime(sourceRoot, runtimeRoot);
        await writePolicyConfig(join(repo, ".maestro", "config"));

        const hookPath = join(repo, ".maestro", "hooks", "record.ts");
        await mkdir(dirname(hookPath), { recursive: true });
        await writeFile(hookPath, hookSource());
        await chmod(hookPath, 0o755);
        const hookCommand = "bun .maestro/hooks/record.ts";
        await writeHookConfig(join(repo, ".codex", "hooks.json"), hookCommand);
        await writeHookConfig(join(repo, ".claude", "settings.json"), hookCommand);
        await writeMirror(join(repo, "AGENTS.md"));
        await writeMirror(join(repo, "CLAUDE.md"));

        await writeFile(
          shim,
          `#!/usr/bin/env bun\nawait import(${JSON.stringify(pathToFileURL(join(runtimeRoot, "bin", "maestro.ts")).href)});\n`,
        );
        await chmod(shim, 0o755);
        context.log.append({
          type: "install",
          entityType: "repo",
          entityId: repo,
          sessionId: context.sessions.current().id,
          payload: { runtimeRoot, shim },
        });
        return {
          data: { repo, runtimeRoot, shim, legacy },
          text: `maestro installed for ${repo}\nreview Codex hook trust with /hooks`,
        };
      }),
    );
  },
};
