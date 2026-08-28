import { existsSync } from "node:fs";
import { mkdir, readFile, rm, unlink, writeFile } from "node:fs/promises";
import { basename, join } from "node:path";
import { CliError, type CliInvocation, type CliResult } from "../kernel/cli.ts";
import {
  resolvePluginEntrypoint,
  type BuiltInPlugin,
  type PluginRecord,
} from "../kernel/loader.ts";
import { grantTrust, revokeTrust } from "./plugin-trust.ts";
import { registerSessionCommand } from "./session-required.ts";

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
  const requires = record.requires ? `\t${record.requires}` : "";
  return `${record.name}\t${record.source}\t${record.status}${diagnostic}${requires}`;
}

export const pluginManagerPlugin: BuiltInPlugin = {
  name: "plugin-host",
  apply(context) {
    const repo = process.cwd();
    const home = process.env.HOME ?? repo;
    const configPath = join(repo, ".maestro", "config");

    context.effect(() =>
      context.cli.register(
        "plugin list",
        (): CliResult => {
          const records = [...context.loader.records].sort((left, right) =>
            left.name.localeCompare(right.name),
          );
          return { data: { plugins: records }, text: records.map(formatRecord).join("\n") };
        },
        {
          description: "List built-in, global, and repository plugins.",
          mutates: false,
          rootDescription: "Manage built-in, global, and repository plugins.",
        },
      ),
    );

    context.effect(() =>
      registerSessionCommand(context, "plugin enable", async (invocation): Promise<CliResult> => {
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
        // Enabling is a statement about a plugin the user already vouched for.
        // If it could confer trust, a clone shipping its own .maestro/config
        // would be back to loading itself.
        if (record.status === "untrusted") {
          throw new CliError(
            "PLUGIN_UNTRUSTED",
            `plugin source is not trusted: ${name}; review it, then: maestro plugin trust ${name}`,
            { command: `maestro plugin trust ${name}`, plugin: name },
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
      }, {
        description: "Enable an installed plugin.",
        positionals: [{ name: "name", required: true }],
      }),
    );

    context.effect(() =>
      registerSessionCommand(context, "plugin disable", async (invocation): Promise<CliResult> => {
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
      }, {
        description: "Disable a plugin and unwind its effects.",
        positionals: [{ name: "name", required: true }],
      }),
    );

    context.effect(() =>
      registerSessionCommand(context, "plugin new", async (invocation): Promise<CliResult> => {
        const name = requireName(invocation);
        validateName(name);
        const directory = join(repo, ".maestro", "plugins");
        const path = join(directory, `${name}.ts`);
        if (existsSync(path)) throw new CliError("ALREADY_EXISTS", `plugin already exists: ${name}`);
        await mkdir(directory, { recursive: true });
        await writeFile(
          path,
          `export default {\n  name: ${JSON.stringify(name)},\n  apply(ctx) {\n    ctx.effect(() => ctx.cli.register(${JSON.stringify(name)}, async () => ${JSON.stringify(name)}, { description: ${JSON.stringify(`Run the ${name} plugin command.`)} }));\n  },\n};\n`,
        );
        context.log.append({
          type: "plugin.new",
          entityType: "plugin",
          entityId: name,
          sessionId: context.sessions.current().id,
          payload: { path },
        });
        return {
          data: { name, path, source: "repo" },
          text: `${name} created at ${path}\nit stays untrusted until: maestro plugin trust ${name}`,
        };
      }, {
        description: "Scaffold a repository-local plugin.",
        positionals: [{ name: "name", required: true }],
      }),
    );

    context.effect(() =>
      registerSessionCommand(context, "plugin add", async (invocation): Promise<CliResult> => {
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
        let digest: string;
        try {
          // Confirming an entrypoint exists is a filesystem question. Importing
          // it to read its declared name would run the clone's module scope at
          // add time, which is the thing being prevented.
          const entrypoint = resolvePluginEntrypoint(destination);
          if (!entrypoint) {
            throw new CliError(
              "INVALID_PLUGIN",
              `plugin entrypoint not found: expected index.ts or one root .ts file in ${name}`,
              { plugin: name },
            );
          }
          // Naming the git URL is the trust act; the grant covers the bytes just
          // cloned, so a later pull inside the clone revokes it.
          digest = await grantTrust(home, { root: destination, source: "global" });
        } catch (error) {
          await rm(destination, { recursive: true, force: true });
          if (error instanceof CliError) throw error;
          throw new CliError(
            "INVALID_PLUGIN",
            `plugin source is not usable: ${name}: ${error instanceof Error ? error.message : String(error)}`,
            { plugin: name },
          );
        }
        const pluginName = name;
        context.log.append({
          type: "plugin.add",
          entityType: "plugin",
          entityId: pluginName,
          sessionId: context.sessions.current().id,
          payload: { digest, url, destination },
        });
        return {
          data: { name: pluginName, path: destination, source: "global" },
          text: `${pluginName} added globally`,
        };
      }, {
        description: "Clone and enable a plugin from a Git URL.",
        positionals: [{ name: "url", required: true }],
      }),
    );

    context.effect(() =>
      registerSessionCommand(context, "plugin remove", async (invocation): Promise<CliResult> => {
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
        // Leave no grant pointing at a path this command just emptied: a later
        // plugin written to the same path would otherwise inherit the vouching
        // if its bytes happened to match.
        if (record.root) await revokeTrust(home, record.root);
        context.log.append({
          type: "plugin.remove",
          entityType: "plugin",
          entityId: name,
          sessionId: context.sessions.current().id,
        });
        return { data: { name }, text: `${name} removed` };
      }, {
        description: "Remove a managed plugin and its files.",
        positionals: [{ name: "name", required: true }],
      }),
    );
  },
};
