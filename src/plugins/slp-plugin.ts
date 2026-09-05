import { existsSync } from "node:fs";
import { mkdir, readFile, realpath, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { HerdrClient, SlpRuntimeError } from "./herdr-client.ts";

// Hub d96: maestro ships a Herdr plugin manifest; install renders it with the
// installed binary and links it over the socket, uninstall unlinks it. Every
// step touches only the `maestro` plugin (A4). The plugin root is the Hub
// room (d833): Herdr runs hooks with the plugin directory as cwd, and the
// kernel refuses a cwd under .maestro, so the room is where the hooks run.

export const herdrPluginId = "maestro";
const manifestSource = join(import.meta.dir, "resources", "herdr-plugin", "herdr-plugin.toml");

export function herdrPluginDirectory(home: string): string {
  return join(home, "maestro");
}

export function herdrPluginManifestPath(home: string): string {
  return join(herdrPluginDirectory(home), "herdr-plugin.toml");
}

export async function renderHerdrPluginManifest(binary: string, version: string): Promise<string> {
  const template = await readFile(manifestSource, "utf8");
  return template
    .replace(/^version = ".*"$/m, `version = ${JSON.stringify(version)}`)
    .replaceAll('command = ["maestro",', `command = [${JSON.stringify(binary)},`);
}

export interface HerdrPluginLink {
  directory: string;
  status: "linked" | "present" | "unreachable";
  warning: string | null;
}

async function samePlace(left: string | undefined, right: string): Promise<boolean> {
  if (!left) return false;
  try {
    return (await realpath(left)) === (await realpath(right));
  } catch {
    return left === right;
  }
}

export async function linkHerdrPlugin(
  home: string,
  binary: string,
  version: string,
  environment: Record<string, string | undefined> = process.env,
): Promise<HerdrPluginLink> {
  const directory = herdrPluginDirectory(home);
  await mkdir(directory, { recursive: true });
  await writeFile(herdrPluginManifestPath(home), await renderHerdrPluginManifest(binary, version));
  const client = new HerdrClient(environment);
  try {
    const present = (await client.pluginList()).find((plugin) => plugin.plugin_id === herdrPluginId);
    if (present && await samePlace(present.plugin_root, directory)) {
      return { directory, status: "present", warning: null };
    }
    // A link left by another home (an earlier install, a test fixture) points
    // at a manifest that may be gone; the id is ours, so it moves here.
    if (present) await client.pluginUnlink(herdrPluginId);
    await client.pluginLink(directory);
    return { directory, status: "linked", warning: null };
  } catch (error) {
    if (!(error instanceof SlpRuntimeError)) throw error;
    return {
      directory,
      status: "unreachable",
      warning: `herdr plugin: not linked (${error.message}); rerun maestro install with Herdr running`,
    };
  }
}

export async function unlinkHerdrPlugin(
  home: string,
  environment: Record<string, string | undefined> = process.env,
): Promise<string[]> {
  const removed: string[] = [];
  const manifest = herdrPluginManifestPath(home);
  try {
    if (await new HerdrClient(environment).pluginUnlink(herdrPluginId)) removed.push(`herdr plugin ${herdrPluginId}`);
  } catch (error) {
    if (!(error instanceof SlpRuntimeError)) throw error;
    process.stderr.write(`warning: herdr plugin ${herdrPluginId} was not unlinked (${error.message})\n`);
  }
  if (existsSync(manifest)) {
    await rm(manifest, { force: true });
    removed.push(manifest);
  }
  return removed;
}
