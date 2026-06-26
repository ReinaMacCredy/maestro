export type MissionControlBackgroundMode = "transparent" | "current";
export type MissionControlBackgroundModeInput =
  | MissionControlBackgroundMode
  | "solid"
  | "terminal";

export const MISSION_CONTROL_BACKGROUND_OPTIONS = [
  "transparent",
  "current",
] as const satisfies readonly MissionControlBackgroundMode[];

export interface MissionControlUiConfig {
  readonly backgroundMode?: MissionControlBackgroundModeInput;
}

export interface UiConfig {
  readonly missionControl?: MissionControlUiConfig;
}

const GLOBAL_ONLY_CONFIG_KEYS = [
  "ui.missionControl.backgroundMode",
] as const;

export function isGlobalOnlyConfigKey(keyPath: string): boolean {
  return GLOBAL_ONLY_CONFIG_KEYS.includes(keyPath as (typeof GLOBAL_ONLY_CONFIG_KEYS)[number]);
}

export function resolveConfigScopeForKey(
  keyPath: string,
  scope: "project" | "global",
): "project" | "global" {
  return isGlobalOnlyConfigKey(keyPath) ? "global" : scope;
}

/**
 * Reads any config-like object and returns the list of keys that are
 * only honoured at global scope (i.e. set in the project file but
 * ignored). Parameter type is narrow so this module does not depend on
 * the full `MaestroConfig` shape in `@/infra/domain/config-types.js`.
 */
export function listIgnoredProjectConfigKeys(
  projectConfig: { readonly ui?: UiConfig } | undefined,
): readonly string[] {
  const ignoredKeys: string[] = [];

  if (projectConfig?.ui?.missionControl?.backgroundMode !== undefined) {
    ignoredKeys.push("ui.missionControl.backgroundMode");
  }

  return ignoredKeys;
}

export function getMissionControlBackgroundMode(config: {
  ui?: { missionControl?: { backgroundMode?: MissionControlBackgroundModeInput } };
}): MissionControlBackgroundMode {
  return normalizeMissionControlBackgroundMode(config.ui?.missionControl?.backgroundMode);
}

export function normalizeMissionControlBackgroundMode(
  value: MissionControlBackgroundModeInput | string | undefined,
): MissionControlBackgroundMode {
  switch (value) {
    case "current":
    case "solid":
      return "current";
    case "transparent":
    case "terminal":
    case undefined:
      return "transparent";
    default:
      return "transparent";
  }
}

export function formatMissionControlBackgroundMode(
  value: MissionControlBackgroundModeInput | string | undefined,
): string {
  const mode = normalizeMissionControlBackgroundMode(value);
  return mode === "transparent" ? "Transparency (default)" : "Current theme";
}
