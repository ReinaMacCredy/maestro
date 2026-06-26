import type { MaestroConfig } from "@/infra/domain/config-types.js";

export type ConfigScope = "project" | "global";

export interface ConfigLayers {
  readonly effective: MaestroConfig;
  readonly defaults: MaestroConfig;
  readonly project?: MaestroConfig;
  readonly global?: MaestroConfig;
  readonly paths: {
    readonly project: string;
    readonly global: string;
  };
  readonly errors: readonly {
    readonly scope: ConfigScope;
    readonly message: string;
  }[];
}

export interface ConfigPort {
  readonly readLayers?: (cwd: string) => Promise<ConfigLayers>;
}
