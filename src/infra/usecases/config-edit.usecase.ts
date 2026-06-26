import type { ConfigPort, ConfigScope } from "@/infra/ports/config.port.js";

export interface ConfigEditPreview {
  readonly scope: ConfigScope;
  readonly path: string;
  readonly content: string;
}

export async function previewConfigEdit(
  _config: ConfigPort,
  cwd: string,
  scope: ConfigScope,
  keyPath: string,
  value: string,
): Promise<ConfigEditPreview> {
  return {
    scope,
    path: `${cwd}/.maestro/config.yml`,
    content: `${keyPath}: ${value}\n`,
  };
}

export async function applyConfigEdit(
  _config: ConfigPort,
  _cwd: string,
  _scope: ConfigScope,
  _keyPath: string,
  _value: string,
): Promise<void> {
  throw new Error("Mission Control config editing is read-only in the TypeScript sidecar");
}
