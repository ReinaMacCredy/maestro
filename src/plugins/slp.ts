import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { CliError } from "../kernel/cli.ts";
import type { BuiltInPlugin, PluginContext } from "../kernel/loader.ts";

const enable = "maestro plugin enable slp";
const verbs = ["team", "slp", "decide", "work take", "work return", "work accept"];

function refuseWhileOff(args: readonly string[]): void {
  if (args[0] === "status") {
    throw new CliError(
      "PLUGIN_DISABLED",
      `status left core (d896): for tracked work and who holds it run: maestro work list; the team readout is part of the slp plugin, which is off; run: ${enable}`,
      { command: "maestro work list", plugin: "slp", verb: "status" },
    );
  }
  const verb = verbs.find((candidate) => candidate.split(" ").every((part, index) => args[index] === part));
  if (!verb) return;
  throw new CliError("PLUGIN_DISABLED", `${verb} is part of the slp plugin, which is off; run: ${enable}`, {
    command: enable,
    plugin: "slp",
    verb,
  });
}

// Seat files and the Herdr link are per machine while the switch is per repository, so a room or a global config that turns slp on keeps them.
function enabledForMachine(home: string): boolean {
  return [join(home, ".maestro", "config"), join(home, "maestro", ".maestro", "config")].some((path) => {
    if (!existsSync(path)) return false;
    try {
      const config = JSON.parse(readFileSync(path, "utf8")) as { plugins?: Array<{ disabled?: boolean; name?: string }> };
      return config.plugins?.some((entry) => entry.name === "slp" && entry.disabled === false) ?? false;
    } catch {
      return false;
    }
  });
}

async function removeStale(home: string, verb: string): Promise<string> {
  if (enabledForMachine(home)) return "";
  const { removeStaleMachineFiles } = await import("./slp-plugin.ts");
  const removed = await removeStaleMachineFiles(home);
  return removed.length > 0
    ? `the slp plugin is off, so ${verb} removed what an earlier install wrote for it: ${removed.join(", ")}; to turn it on: ${enable}`
    : "";
}

export const slpPlugin: BuiltInPlugin = {
  name: "slp",
  defaultDisabled: true,
  async apply(context: PluginContext) {
    const { applySlp } = await import("./slp-v2.ts");
    applySlp(context);
  },
  applyDisabled(context: PluginContext) {
    context.effect(() => context.cli.guard({ beforeUnknown: refuseWhileOff }));
    context.effect(() => context.events.on<{ home: string }, string>("install", ({ home }) => removeStale(home, "install")));
    context.effect(() => context.events.on<{ home: string }, string>("update", ({ home }) => removeStale(home, "update")));
    context.effect(() =>
      context.events.on<{ home: string }, string[]>("uninstall", async ({ home }) =>
        (await import("./slp-plugin.ts")).removeMachineFiles(home)
      )
    );
  },
};
