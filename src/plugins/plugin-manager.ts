import { existsSync } from "node:fs";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { CliError, type CliInvocation, type CliResult } from "../kernel/cli.ts";
import type { BuiltInPlugin, PluginRecord } from "../kernel/loader.ts";
import { registerSessionCommand } from "./session-required.ts";

interface ConfigEntry {
  config?: unknown;
  disabled?: boolean;
  name: string;
}

interface ConfigFile {
  plugins: ConfigEntry[];
}

function requireName(invocation: CliInvocation): string {
  const value = invocation.positionals[0];
  if (!value) throw new CliError("MISSING_ARGUMENT", "missing plugin name");
  return value;
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

function formatRecord(record: PluginRecord): string {
  const diagnostic = record.diagnostic ? ` (${record.diagnostic})` : "";
  const requires = record.requires ? `\t${record.requires}` : "";
  return `${record.name}\t${record.source}\t${record.status}${diagnostic}${requires}`;
}

export const pluginManagerPlugin: BuiltInPlugin = {
  name: "plugin-host",
  apply(context) {
    const configPath = join(process.cwd(), ".maestro", "config");

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
          description: "List built-in plugins.",
          mutates: false,
          rootDescription: "Manage built-in plugins.",
        },
      ),
    );

    context.effect(() =>
      registerSessionCommand(context, "plugin enable", async (invocation): Promise<CliResult> => {
        const name = requireName(invocation);
        const record = context.loader.records.find((candidate) => candidate.name === name);
        if (!record) {
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
  },
};
