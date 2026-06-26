import type { MissionControlBackgroundMode } from "@/tui/shared/ui-config.js";

export interface MaestroConfig {
  readonly ui?: {
    readonly missionControl?: {
      readonly backgroundMode?: MissionControlBackgroundMode;
    };
  };
}
