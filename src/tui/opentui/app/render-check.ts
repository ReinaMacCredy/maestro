import type { MissionControlSnapshot } from "../../state/types.js";
import { PREVIEW_SCREENS, type PreviewScreen } from "../../app/preview-state.js";
import type { RenderCheckResult, RenderCheckScreenResult } from "../../app/render-check-contract.js";
import { renderOpenTuiPreviewFrame } from "./preview.js";

const DEFAULT_CHECK_WIDTH = 120;
const DEFAULT_CHECK_HEIGHT = 40;

export interface OpenTuiRenderCheckOptions {
  readonly width?: number;
  readonly height?: number;
}

export async function runOpenTuiRenderCheck(
  snapshot: MissionControlSnapshot,
  opts: OpenTuiRenderCheckOptions = {},
): Promise<RenderCheckResult> {
  const width = opts.width ?? DEFAULT_CHECK_WIDTH;
  const height = opts.height ?? DEFAULT_CHECK_HEIGHT;
  const applicableScreens = getCheckableScreens(snapshot);
  const results: RenderCheckScreenResult[] = [];

  for (const screen of PREVIEW_SCREENS) {
    if (!applicableScreens.includes(screen)) {
      results.push({
        screen,
        status: "skip",
        size: `${width}x${height}`,
        warnings: [`Requires mission (current mode: ${snapshot.mode})`],
      });
      continue;
    }

    const warnings: string[] = [];

    try {
      const frame = await renderOpenTuiPreviewFrame({
        snapshot,
        screen,
        width,
        height,
        format: "plain",
      });

      const lines = frame.split("\n");
      for (let i = 0; i < lines.length; i++) {
        if (lines[i]!.includes("undefined")) {
          warnings.push(`Contains 'undefined' at line ${i + 1}`);
        }
        if (lines[i]!.includes("NaN")) {
          warnings.push(`Contains 'NaN' at line ${i + 1}`);
        }
      }

      const bodyLines = lines.slice(2, -2);
      const emptyBodyLines = bodyLines.filter((line) => line.trim().length === 0);
      if (bodyLines.length > 0 && emptyBodyLines.length === bodyLines.length) {
        warnings.push("Body area is entirely empty");
      }

      if (lines.length > 0 && !lines[0]!.startsWith("\u250C")) {
        warnings.push("Missing top-left box corner");
      }

      const contentChars = frame.replace(/[─│┌┐└┘├┤┬┴┼╭╮╰╯╶╴╷╵ \n\r\t]/g, "");
      if (contentChars.length < 10) {
        warnings.push("Frame has very little content");
      }

      results.push({
        screen,
        status: warnings.length > 0 ? "fail" : "pass",
        size: `${width}x${height}`,
        warnings,
      });
    } catch (error) {
      results.push({
        screen,
        status: "fail",
        size: `${width}x${height}`,
        warnings: [`Render threw: ${error instanceof Error ? error.message : String(error)}`],
      });
    }
  }

  const passed = results.filter((result) => result.status === "pass").length;
  const failed = results.filter((result) => result.status === "fail").length;
  const skipped = results.filter((result) => result.status === "skip").length;

  return {
    screens: results,
    summary: {
      total: results.length,
      passed,
      failed,
      skipped,
    },
  };
}

function getCheckableScreens(snapshot: MissionControlSnapshot): PreviewScreen[] {
  if (snapshot.mode === "mission") {
    return [...PREVIEW_SCREENS];
  }
  return ["dashboard", "features", "config", "runtime", "workers"];
}
