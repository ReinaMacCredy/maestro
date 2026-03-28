import type { MaestroConfig } from "../domain/types.js";

export interface ConfigPort {
  load(projectDir: string): Promise<MaestroConfig>;
  write(scope: "global" | "project", projectDir: string, config: MaestroConfig): Promise<void>;
  exists(scope: "global" | "project", projectDir: string): Promise<boolean>;
}
