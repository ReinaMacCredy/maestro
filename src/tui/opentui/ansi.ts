import { TextAttributes, getBaseAttributes } from "@opentui/core";
import { sanitizeTerminalText } from "../../lib/sanitize.js";

import type { CapturedFrame, CapturedSpan } from "@opentui/core";

export function capturedFrameToAnsi(frame: CapturedFrame): string {
  return frame.lines
    .map((line) => {
      if (line.spans.length === 0) {
        return "";
      }
      return `${line.spans.map((span) => `${spanToAnsi(span)}${sanitizeTerminalText(span.text)}`).join("")}\u001b[0m`;
    })
    .join("\n");
}

function spanToAnsi(span: CapturedSpan): string {
  const codes: string[] = ["0"];
  const [fgR, fgG, fgB, fgA] = span.fg.toInts();
  const [bgR, bgG, bgB, bgA] = span.bg.toInts();
  const attrs = getBaseAttributes(span.attributes);

  if (fgA > 0) {
    codes.push(`38;2;${fgR};${fgG};${fgB}`);
  }
  if (bgA > 0) {
    codes.push(`48;2;${bgR};${bgG};${bgB}`);
  }
  if (attrs & TextAttributes.BOLD) codes.push("1");
  if (attrs & TextAttributes.DIM) codes.push("2");
  if (attrs & TextAttributes.ITALIC) codes.push("3");
  if (attrs & TextAttributes.UNDERLINE) codes.push("4");
  if (attrs & TextAttributes.BLINK) codes.push("5");
  if (attrs & TextAttributes.INVERSE) codes.push("7");
  if (attrs & TextAttributes.HIDDEN) codes.push("8");
  if (attrs & TextAttributes.STRIKETHROUGH) codes.push("9");

  return `\u001b[${codes.join(";")}m`;
}
