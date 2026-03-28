import { homedir } from "node:os";
import { join } from "node:path";
import type { MaestroConfig } from "../domain/types.js";
import type { ConfigPort } from "../ports/config.port.js";
import { ensureDir } from "../lib/fs.js";
import { parseYaml, stringifyYaml, deepMerge } from "../lib/yaml.js";

const GLOBAL_DIR = join(homedir(), ".maestro");
const CONFIG_FILE = "config.yaml";

const DEFAULT_CONFIG: MaestroConfig = {
  sessionDetection: {
    enabled: true,
    agents: ["claude-code"],
  },
};

export class YamlConfigAdapter implements ConfigPort {
  async load(projectDir: string): Promise<MaestroConfig> {
    let config = { ...DEFAULT_CONFIG };

    const globalConfig = await readConfigFile(join(GLOBAL_DIR, CONFIG_FILE));
    if (globalConfig) {
      config = deepMerge(config, globalConfig);
    }

    const projectConfig = await readConfigFile(
      join(projectDir, ".maestro", CONFIG_FILE),
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
    const dir =
      scope === "global" ? GLOBAL_DIR : join(projectDir, ".maestro");
    await ensureDir(dir);
    const path = join(dir, CONFIG_FILE);
    await Bun.write(path, stringifyYaml(config));
  }

  async exists(
    scope: "global" | "project",
    projectDir: string,
  ): Promise<boolean> {
    const dir =
      scope === "global" ? GLOBAL_DIR : join(projectDir, ".maestro");
    const path = join(dir, CONFIG_FILE);
    return Bun.file(path).exists();
  }
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
