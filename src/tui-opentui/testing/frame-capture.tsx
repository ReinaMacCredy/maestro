import type { CapturedFrame } from "@opentui/core";
import { createTestRenderer } from "@opentui/core/testing";
import { createRoot, flushSync } from "@opentui/react";

import type { AppState } from "../../tui/state/reducer.js";
import type { MissionControlSnapshot } from "../../tui/state/types.js";
import { MissionControlApp } from "../app/mission-control-app.js";

export interface OpenTuiFrameCaptureOptions {
  readonly snapshot: MissionControlSnapshot;
  readonly state?: AppState;
  readonly width: number;
  readonly height: number;
  readonly animationFrame?: number;
  readonly elapsedOffsetMs?: number;
}

export interface OpenTuiCapturedRender {
  readonly charFrame: string;
  readonly spans: CapturedFrame;
}

export async function captureMissionControlFrame(
  opts: OpenTuiFrameCaptureOptions,
): Promise<string> {
  const render = await captureMissionControlRender(opts);
  return render.charFrame;
}

export async function captureMissionControlRender(
  opts: OpenTuiFrameCaptureOptions,
): Promise<OpenTuiCapturedRender> {
  const reactActEnvironment = globalThis as typeof globalThis & {
    IS_REACT_ACT_ENVIRONMENT?: boolean;
  };
  const previousActEnvironment = reactActEnvironment.IS_REACT_ACT_ENVIRONMENT;
  reactActEnvironment.IS_REACT_ACT_ENVIRONMENT = false;
  const setup = await createTestRenderer({
    width: opts.width,
    height: opts.height,
  });
  const root = createRoot(setup.renderer);

  try {
    flushSync(() => {
      root.render(
        <MissionControlApp
          snapshot={opts.snapshot}
          state={opts.state}
          width={opts.width}
          height={opts.height}
          animationFrame={opts.animationFrame}
          elapsedOffsetMs={opts.elapsedOffsetMs}
        />,
      );
    });

    await setup.renderOnce();
    return {
      charFrame: setup.captureCharFrame(),
      spans: setup.captureSpans(),
    };
  } finally {
    flushSync(() => {
      root.unmount();
    });
    setup.renderer.destroy();
    if (previousActEnvironment === undefined) {
      delete reactActEnvironment.IS_REACT_ACT_ENVIRONMENT;
    } else {
      reactActEnvironment.IS_REACT_ACT_ENVIRONMENT = previousActEnvironment;
    }
  }
}
