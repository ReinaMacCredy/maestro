import { existsSync } from "node:fs";
import { mkdir, readFile, rm, unlink, writeFile } from "node:fs/promises";
import { basename, join } from "node:path";
import { CliError, type CliInvocation, type CliResult } from "../kernel/cli.ts";
import {
  importPluginEntrypoint,
  resolvePluginEntrypoint,
  type BuiltInPlugin,
  type PluginRecord,
} from "../kernel/loader.ts";

interface ConfigEntry {
  config?: unknown;
  disabled?: boolean;
  name: string;
}

interface ConfigFile {
  plugins: ConfigEntry[];
}

function requireName(invocation: CliInvocation, label = "plugin name"): string {
  const value = invocation.positionals[0];
  if (!value) throw new CliError("MISSING_ARGUMENT", `missing ${label}`);
  return value;
}

function validateName(name: string): void {
  if (!/^[a-z0-9][a-z0-9-]*$/.test(name)) {
    throw new CliError("INVALID_NAME", `invalid plugin name: ${name}`);
  }
}

async function readRepoConfig(path: string): Promise<ConfigFile> {
  if (!existsSync(path)) return { plugins: [] };
  return JSON.parse(await readFile(path, "utf8")) as ConfigFile;
}

async function updateEntry(path: string, name: string, disabled: boolean): Promise<void> {
  const config = await readRepoConfig(path);
  const entry = config.plugins.find((candidate) => candidate.name === name);
  if (entry) entry.disabled = disabled;
  else config.plugins.push({ name, disabled });
  await mkdir(join(path, ".."), { recursive: true });
  await writeFile(path, `${JSON.stringify(config)}\n`);
}

async function removeEntry(path: string, name: string): Promise<void> {
  const config = await readRepoConfig(path);
  config.plugins = config.plugins.filter((candidate) => candidate.name !== name);
  await mkdir(join(path, ".."), { recursive: true });
  await writeFile(path, `${JSON.stringify(config)}\n`);
}

function formatRecord(record: PluginRecord): string {
  const diagnostic = record.diagnostic ? ` (${record.diagnostic})` : "";
  return `${record.name}\t${record.source}\t${record.status}${diagnostic}`;
}

export const pluginManagerPlugin: BuiltInPlugin = {
  name: "plugin-host",
  apply(context) {
    const repo = process.cwd();
    const home = process.env.HOME ?? repo;
    const configPath = join(repo, ".maestro", "config");

    context.effect(() =>
      context.cli.register("plugin list", (): CliResult => {
        const records = [...context.loader.records].sort((left, right) =>
          left.name.localeCompare(right.name),
        );
        return { data: { plugins: records }, text: records.map(formatRecord).join("\n") };
      }),
    );

    context.effect(() =>
      context.cli.register("plugin enable", async (invocation): Promise<CliResult> => {
        const name = requireName(invocation);
        const record = context.loader.records.find((candidate) => candidate.name === name);
        if (!record || record.diagnostic === "plugin source not found") {
          throw new CliError(
            "PLUGIN_SOURCE_NOT_FOUND",
            `plugin source not found: ${name}`,
            { plugin: name },
          );
        }
        if (record.status === "error") {
          throw new CliError(
            "INVALID_PLUGIN",
            `plugin source is not loadable: ${name}: ${record.diagnostic ?? "unknown error"}`,
            { plugin: name },
          );
        }
        await updateEntry(configPath, name, false);
        context.log.append({
          type: "plugin.enable",
          entityType: "plugin",
          entityId: name,
          sessionId: context.sessions.current().id,
        });
        return { data: { name, disabled: false }, text: `${name} enabled` };
      }, {}, 1),
    );

    context.effect(() =>
      context.cli.register("plugin disable", async (invocation): Promise<CliResult> => {
        const name = requireName(invocation);
        await updateEntry(configPath, name, true);
        context.log.append({
          type: "plugin.disable",
          entityType: "plugin",
          entityId: name,
          sessionId: context.sessions.current().id,
        });
        await context.loader.unload(name);
        return { data: { name, disabled: true }, text: `${name} disabled` };
      }, {}, 1),
    );

    context.effect(() =>
      context.cli.register("plugin new", async (invocation): Promise<CliResult> => {
        const name = requireName(invocation);
        validateName(name);
        const directory = join(repo, ".maestro", "plugins");
        const path = join(directory, `${name}.ts`);
        if (existsSync(path)) throw new CliError("ALREADY_EXISTS", `plugin already exists: ${name}`);
        await mkdir(directory, { recursive: true });
        await writeFile(
          path,
          `export default {\n  name: ${JSON.stringify(name)},\n  apply(ctx) {\n    ctx.effect(() => ctx.cli.register(${JSON.stringify(name)}, async () => ${JSON.stringify(name)}));\n  },\n};\n`,
        );
        context.log.append({
          type: "plugin.new",
          entityType: "plugin",
          entityId: name,
          sessionId: context.sessions.current().id,
          payload: { path },
        });
        return { data: { name, path, source: "repo" }, text: `${name} created at ${path}` };
      }, {}, 1),
    );

    context.effect(() =>
      context.cli.register("plugin add", async (invocation): Promise<CliResult> => {
        const url = requireName(invocation, "git URL");
        const name = basename(url.replace(/\/$/, "")).replace(/\.git$/, "");
        validateName(name);
        const directory = join(home, ".maestro", "plugins");
        const destination = join(directory, name);
        if (existsSync(destination)) {
          throw new CliError("ALREADY_EXISTS", `plugin already exists: ${name}`);
        }
        await mkdir(directory, { recursive: true });
        const child = Bun.spawn(["git", "clone", "--quiet", "--", url, destination], {
          stdout: "pipe",
          stderr: "pipe",
        });
        const [stderr, exitCode] = await Promise.all([
          new Response(child.stderr).text(),
          child.exited,
        ]);
        if (exitCode !== 0) {
          await rm(destination, { recursive: true, force: true });
          throw new CliError("CLONE_FAILED", stderr.trim() || `git clone failed: ${url}`);
        }
        let pluginName: string;
        try {
          const entrypoint = resolvePluginEntrypoint(destination);
          if (!entrypoint) {
            throw new CliError(
              "INVALID_PLUGIN",
              `plugin entrypoint not found: expected index.ts or one root .ts file in ${name}`,
              { plugin: name },
            );
          }
          const plugin = await importPluginEntrypoint(entrypoint);
          validateName(plugin.name);
          pluginName = plugin.name;
          await updateEntry(configPath, pluginName, false);
        } catch (error) {
          await rm(destination, { recursive: true, force: true });
          if (error instanceof CliError) throw error;
          throw new CliError(
            "INVALID_PLUGIN",
            `plugin entrypoint is not loadable: ${name}: ${error instanceof Error ? error.message : String(error)}`,
            { plugin: name },
          );
        }
        context.log.append({
          type: "plugin.add",
          entityType: "plugin",
          entityId: pluginName,
          sessionId: context.sessions.current().id,
          payload: { url, destination },
        });
        return {
          data: { name: pluginName, path: destination, source: "global" },
          text: `${pluginName} added globally`,
        };
      }, {}, 1),
    );

    context.effect(() =>
      context.cli.register("plugin remove", async (invocation): Promise<CliResult> => {
        const name = requireName(invocation);
        const record = context.loader.records.find((candidate) => candidate.name === name);
        if (!record) throw new CliError("NOT_FOUND", `plugin not found: ${name}`);
        if (record.source === "built-in") {
          throw new CliError("BUILT_IN", `built-in plugin cannot be removed: ${name}`);
        }
        await context.loader.unload(name);
        if (record.path) {
          if (record.artifact === "file") {
            await unlink(record.path);
          } else if (record.artifact === "directory") {
            await rm(join(record.path, ".."), { recursive: true, force: true });
          }
        }
        await removeEntry(configPath, name);
        context.log.append({
          type: "plugin.remove",
          entityType: "plugin",
          entityId: name,
          sessionId: context.sessions.current().id,
        });
        return { data: { name }, text: `${name} removed` };
      }, {}, 1),
    );
  },
};
