import { homedir } from "node:os";
import { join } from "node:path";
import type { MaestroConfig } from "../domain/types.js";
import { MaestroError } from "@/shared/errors.js";
import type { ConfigLayers, ConfigLoadError, ConfigPort, ConfigScope } from "../ports/config.port.js";
import { DEFAULT_CONFIG, MAESTRO_DIR } from "../domain/defaults.js";
import { ensureDir, readText, writeText } from "../lib/fs.js";
import { parseYaml, stringifyYaml, deepMerge } from "../lib/yaml.js";

const GLOBAL_DIR = join(homedir(), MAESTRO_DIR);
const CONFIG_FILE = "config.yaml";

export class YamlConfigAdapter implements ConfigPort {
  async load(projectDir: string): Promise<MaestroConfig> {
    const layers = await this.loadLayers(projectDir);
    if (layers.errors.length > 0) {
      throw new MaestroError("Cannot load Maestro config due to YAML errors", layers.errors.map((error) =>
        `${error.scope}: ${error.message}`
      ));
    }
    return layers.effective;
  }

  async loadLayers(projectDir: string): Promise<ConfigLayers> {
    const paths = {
      global: join(GLOBAL_DIR, CONFIG_FILE),
      project: join(projectDir, MAESTRO_DIR, CONFIG_FILE),
    } satisfies Record<ConfigScope, string>;
    const errors: ConfigLoadError[] = [];

    const globalConfig = await readConfigFile(paths.global, "global", errors);
    const projectConfig = await readConfigFile(paths.project, "project", errors);

    let effective = { ...DEFAULT_CONFIG };
    if (globalConfig) {
      effective = deepMerge(effective, globalConfig);
    }
    if (projectConfig) {
      effective = deepMerge(effective, projectConfig);
    }

    return {
      defaults: DEFAULT_CONFIG,
      effective,
      global: globalConfig,
      project: projectConfig,
      errors,
      paths,
    };
  }

  async write(
    scope: ConfigScope,
    projectDir: string,
    config: MaestroConfig,
  ): Promise<void> {
    const dir = scopeDir(scope, projectDir);
    await ensureDir(dir);
    await writeText(join(dir, CONFIG_FILE), stringifyYaml(config));
  }

  async exists(
    scope: ConfigScope,
    projectDir: string,
  ): Promise<boolean> {
    return Bun.file(join(scopeDir(scope, projectDir), CONFIG_FILE)).exists();
  }
}

function scopeDir(scope: ConfigScope, projectDir: string): string {
  return scope === "global" ? GLOBAL_DIR : join(projectDir, MAESTRO_DIR);
}

async function readConfigFile(
  path: string,
  scope: ConfigScope,
  errors: ConfigLoadError[],
): Promise<MaestroConfig | undefined> {
  const content = await readText(path);
  if (!content?.trim()) return undefined;
  try {
    return parseYaml<MaestroConfig>(content);
  } catch (error) {
    errors.push({
      scope,
      path,
      message: error instanceof Error ? error.message : "Failed to parse YAML",
    });
    return undefined;
  }
}
