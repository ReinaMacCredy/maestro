import type { MaestroConfig } from "@/infra/domain/config-types.js";

export type ConfigScope = "global" | "project";

export interface ConfigLoadError {
  readonly scope: ConfigScope;
  readonly path: string;
  readonly message: string;
}

export interface ConfigLayers {
  readonly defaults: MaestroConfig;
  readonly effective: MaestroConfig;
  readonly global?: MaestroConfig;
  readonly project?: MaestroConfig;
  readonly errors: readonly ConfigLoadError[];
  readonly paths: Readonly<Record<ConfigScope, string>>;
}

export interface ConfigPort {
  load(projectDir: string): Promise<MaestroConfig>;
  loadLayers(projectDir: string): Promise<ConfigLayers>;
  write(scope: ConfigScope, projectDir: string, config: MaestroConfig): Promise<void>;
  exists(scope: ConfigScope, projectDir: string): Promise<boolean>;
}
