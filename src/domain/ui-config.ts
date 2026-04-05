import type { MaestroConfig, MissionControlBackgroundMode } from "./types.js";

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

export function listIgnoredProjectConfigKeys(projectConfig: MaestroConfig | undefined): readonly string[] {
  const ignoredKeys: string[] = [];

  if (projectConfig?.ui?.missionControl?.backgroundMode !== undefined) {
    ignoredKeys.push("ui.missionControl.backgroundMode");
  }

  return ignoredKeys;
}

export function getMissionControlBackgroundMode(config: {
  ui?: { missionControl?: { backgroundMode?: MissionControlBackgroundMode } };
}): MissionControlBackgroundMode {
  return config.ui?.missionControl?.backgroundMode ?? "solid";
}
