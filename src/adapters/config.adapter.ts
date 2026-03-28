import { homedir } from "node:os";
import { join } from "node:path";
import type { MaestroConfig } from "../domain/types.js";
import type { ConfigPort } from "../ports/config.port.js";
import { DEFAULT_CONFIG, MAESTRO_DIR } from "../domain/defaults.js";
import { ensureDir, writeText } from "../lib/fs.js";
import { parseYaml, stringifyYaml, deepMerge } from "../lib/yaml.js";

const GLOBAL_DIR = join(homedir(), MAESTRO_DIR);
const CONFIG_FILE = "config.yaml";

export class YamlConfigAdapter implements ConfigPort {
  async load(projectDir: string): Promise<MaestroConfig> {
    let config = { ...DEFAULT_CONFIG };

    const globalConfig = await readConfigFile(join(GLOBAL_DIR, CONFIG_FILE));
    if (globalConfig) {
      config = deepMerge(config, globalConfig);
    }

    const projectConfig = await readConfigFile(
      join(projectDir, MAESTRO_DIR, CONFIG_FILE),
    );
    if (projectConfig) {
      config = deepMerge(config, projectConfig);
    }

    return config;
  }

  async write(
    scope: "global" | "project",
    projectDir: string,
    config: MaestroConfig,
  ): Promise<void> {
    const dir = scopeDir(scope, projectDir);
    await ensureDir(dir);
    await writeText(join(dir, CONFIG_FILE), stringifyYaml(config));
  }

  async exists(
    scope: "global" | "project",
    projectDir: string,
  ): Promise<boolean> {
    return Bun.file(join(scopeDir(scope, projectDir), CONFIG_FILE)).exists();
  }
}

function scopeDir(scope: "global" | "project", projectDir: string): string {
  return scope === "global" ? GLOBAL_DIR : join(projectDir, MAESTRO_DIR);
}

async function readConfigFile(
  path: string,
): Promise<MaestroConfig | undefined> {
  const file = Bun.file(path);
  if (!(await file.exists())) return undefined;
  const content = await file.text();
  if (!content.trim()) return undefined;
  return parseYaml<MaestroConfig>(content);
}
