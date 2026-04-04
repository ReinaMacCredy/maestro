/**
 * Automated TUI render health check.
 * Renders all applicable preview screens and validates output integrity.
 */
import { Buffer } from "../terminal/buffer.js";
import type { MissionControlSnapshot } from "../state/types.js";
import { PREVIEW_SCREENS, type PreviewScreen } from "./preview-state.js";
import { renderPreviewFrame } from "./render.js";

const DEFAULT_CHECK_WIDTH = 120;
const DEFAULT_CHECK_HEIGHT = 40;

export interface RenderCheckScreenResult {
  screen: string;
  status: "pass" | "fail" | "skip";
  size: string;
  warnings: string[];
}

export interface RenderCheckResult {
  screens: RenderCheckScreenResult[];
  summary: { total: number; passed: number; failed: number; skipped: number };
}

interface RenderCheckOptions {
  width?: number;
  height?: number;
}

/**
 * Render every applicable preview screen at a fixed size and check for
 * common layout defects. Returns machine-parseable results.
 */
export function runRenderCheck(
  snapshot: MissionControlSnapshot,
  opts: RenderCheckOptions = {},
): RenderCheckResult {
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
      const frame = renderPreviewFrame({
        snapshot,
        screen,
        width,
        height,
        format: "plain",
      });

      const lines = frame.split("\n");

      // Check: "undefined" in output (common bug indicator)
      for (let i = 0; i < lines.length; i++) {
        if (lines[i]!.includes("undefined")) {
          warnings.push(`Contains 'undefined' at line ${i + 1}`);
        }
      }

      // Check: completely empty body (all spaces between borders)
      const bodyLines = lines.slice(4, -4);
      const emptyBodyLines = bodyLines.filter(
        (line) => line.replace(/[│┃|]/g, "").trim().length === 0,
      );
      if (bodyLines.length > 0 && emptyBodyLines.length === bodyLines.length) {
        warnings.push("Body area is entirely empty");
      }

      // Check: box-drawing integrity (top-left corner must exist)
      if (lines.length > 0 && !lines[0]!.startsWith("\u250C")) {
        warnings.push("Missing top-left box corner");
      }

      // Check: minimum content (frame should have meaningful text)
      const contentChars = frame.replace(/[─│┌┐└┘├┤┬┴┼╭╮╰╯╶╴╷╵ \n\r\t]/g, "");
      if (contentChars.length < 10) {
        warnings.push("Frame has very little content");
      }

      // Check: NaN in output (broken number formatting)
      for (let i = 0; i < lines.length; i++) {
        if (lines[i]!.includes("NaN")) {
          warnings.push(`Contains 'NaN' at line ${i + 1}`);
        }
      }

      results.push({
        screen,
        status: warnings.length > 0 ? "fail" : "pass",
        size: `${width}x${height}`,
        warnings,
      });
    } catch (err) {
      results.push({
        screen,
        status: "fail",
        size: `${width}x${height}`,
        warnings: [`Render threw: ${err instanceof Error ? err.message : String(err)}`],
      });
    }
  }

  const passed = results.filter((r) => r.status === "pass").length;
  const failed = results.filter((r) => r.status === "fail").length;
  const skipped = results.filter((r) => r.status === "skip").length;

  return {
    screens: results,
    summary: { total: results.length, passed, failed, skipped },
  };
}

function getCheckableScreens(snapshot: MissionControlSnapshot): PreviewScreen[] {
  if (snapshot.mode === "mission") {
    return [...PREVIEW_SCREENS];
  }
  return ["dashboard", "features", "config", "runtime", "workers"];
}
