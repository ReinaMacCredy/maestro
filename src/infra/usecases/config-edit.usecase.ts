import type { MaestroConfig } from "@/infra/domain/config-types.js";
import { MaestroError } from "@/shared/errors.js";
import type { ConfigPort, ConfigScope } from "../ports/config.port.js";
import { stringifyYaml } from "@/shared/lib/yaml.js";

export interface ConfigEditPreview {
  readonly scope: ConfigScope;
  readonly path: string;
  readonly content: string;
}

export async function previewConfigEdit(
  configPort: ConfigPort,
  projectDir: string,
  scope: ConfigScope,
  keyPath: string,
  draftValue: string,
): Promise<ConfigEditPreview> {
  const layers = await configPort.loadLayers(projectDir);
  assertScopeHealthy(layers.errors, scope);
  const baseConfig = structuredClone((scope === "project" ? layers.project : layers.global) ?? {}) as MaestroConfig;
  setNestedValue(baseConfig as Record<string, unknown>, keyPath, parseDraftValue(draftValue));

  return {
    scope,
    path: layers.paths[scope],
    content: stringifyYaml(baseConfig).trimEnd(),
  };
}

export async function applyConfigEdit(
  configPort: ConfigPort,
  projectDir: string,
  scope: ConfigScope,
  keyPath: string,
  draftValue: string,
): Promise<void> {
  const layers = await configPort.loadLayers(projectDir);
  assertScopeHealthy(layers.errors, scope);
  const baseConfig = structuredClone((scope === "project" ? layers.project : layers.global) ?? {}) as MaestroConfig;
  setNestedValue(baseConfig as Record<string, unknown>, keyPath, parseDraftValue(draftValue));
  await configPort.write(scope, projectDir, baseConfig);
}

function assertScopeHealthy(
  errors: readonly { scope: ConfigScope; message: string }[],
  scope: ConfigScope,
): void {
  const error = errors.find((item) => item.scope === scope);
  if (!error) return;

  throw new MaestroError(`Cannot edit ${scope} config while it has YAML errors`, [
    error.message,
  ]);
}

function setNestedValue(
  target: Record<string, unknown>,
  keyPath: string,
  value: unknown,
): void {
  const segments = keyPath.split(".");
  let cursor: Record<string, unknown> = target;

  for (const segment of segments.slice(0, -1)) {
    const next = cursor[segment];
    if (typeof next === "object" && next !== null && !Array.isArray(next)) {
      cursor = next as Record<string, unknown>;
      continue;
    }

    cursor[segment] = {};
    cursor = cursor[segment] as Record<string, unknown>;
  }

  const leaf = segments.at(-1);
  if (!leaf) {
    throw new MaestroError("Config key path cannot be empty");
  }

  cursor[leaf] = value;
}

function parseDraftValue(draftValue: string): boolean | number | string {
  if (draftValue === "on") return true;
  if (draftValue === "off") return false;
  if (/^-?\d+$/.test(draftValue)) {
    return Number.parseInt(draftValue, 10);
  }
  return draftValue;
}
