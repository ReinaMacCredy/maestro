import { testRender } from "@opentui/react/test-utils";

import type { MissionControlSnapshot } from "../../tui/state/types.js";
import { MissionControlApp } from "../app/mission-control-app.js";

export interface OpenTuiFrameCaptureOptions {
  readonly snapshot: MissionControlSnapshot;
  readonly width: number;
  readonly height: number;
}

export async function captureMissionControlFrame(
  opts: OpenTuiFrameCaptureOptions,
): Promise<string> {
  const setup = await testRender(
    <MissionControlApp snapshot={opts.snapshot} />,
    {
      width: opts.width,
      height: opts.height,
    },
  );

  await setup.renderOnce();
  return setup.captureCharFrame();
}
