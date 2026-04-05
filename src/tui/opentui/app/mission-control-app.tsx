import type { MouseEvent } from "@opentui/core";
import { useTerminalDimensions } from "@opentui/react";

import { createInitialState, type AppState } from "../../state/reducer.js";
import type { MissionControlSnapshot } from "../../state/types.js";
import { MissionControlScreen } from "../components/mission-control-screen.js";

export interface MissionControlAppProps {
  readonly snapshot: MissionControlSnapshot;
  readonly state?: AppState;
  readonly width?: number;
  readonly height?: number;
  readonly animationFrame?: number;
  readonly elapsedOffsetMs?: number;
  readonly onMouseDown?: (event: MouseEvent) => void;
}

export function MissionControlApp({
  snapshot,
  state,
  width,
  height,
  animationFrame,
  elapsedOffsetMs,
  onMouseDown,
}: MissionControlAppProps) {
  const dimensions = useTerminalDimensions();
  const resolvedState = state ?? createInitialState(snapshot);

  return (
    <MissionControlScreen
      state={resolvedState}
      width={width ?? dimensions.width}
      height={height ?? dimensions.height}
      animationFrame={animationFrame}
      elapsedOffsetMs={elapsedOffsetMs}
      onMouseDown={onMouseDown}
    />
  );
}
