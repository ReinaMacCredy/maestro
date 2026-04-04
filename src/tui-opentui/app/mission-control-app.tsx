import { TextAttributes } from "@opentui/core";

import type { MissionControlSnapshot } from "../../tui/state/types.js";

export interface MissionControlAppProps {
  readonly snapshot: MissionControlSnapshot;
}

export function MissionControlApp({ snapshot }: MissionControlAppProps) {
  return (
    <box flexDirection="column" flexGrow={1} padding={1}>
      <text>Mission Control</text>
      <text attributes={TextAttributes.DIM}>
        {snapshot.mode === "mission" ? snapshot.missionTitle : snapshot.home?.headline ?? "Home"}
      </text>
    </box>
  );
}
