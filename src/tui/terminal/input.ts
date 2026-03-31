/**
 * Raw stdin key parser.
 * Converts multi-byte escape sequences into structured Key objects.
 */

export type Key =
  | { type: "char"; char: string }
  | { type: "arrow"; direction: "up" | "down" | "left" | "right" }
  | { type: "enter" }
  | { type: "escape" }
  | { type: "ctrl"; char: string }
  | { type: "function"; n: number }
  | { type: "mouse"; event: "down" | "up"; button: "left"; x: number; y: number };

/**
 * Parse raw bytes from stdin into Key events.
 * Handles: printable ASCII, arrow keys (CSI A/B/C/D), ctrl combos,
 * function keys (CSI 11~..CSI 24~), enter, escape.
 */
export function parseKeypress(data: Uint8Array): Key[] {
  const keys: Key[] = [];
  let i = 0;

  while (i < data.length) {
    const byte = data[i]!;

    // ESC sequence
    if (byte === 0x1b) {
      // Bare escape (no more bytes or next byte isn't '[')
      if (i + 1 >= data.length) {
        keys.push({ type: "escape" });
        i++;
        continue;
      }

      if (data[i + 1] === 0x5b) {
        // CSI sequence: ESC [
        i += 2;
        const result = parseCSI(data, i);
        if (result) {
          if (result.key) {
            keys.push(result.key);
          }
          i = result.nextIndex;
        }
        continue;
      }

      // ESC + char = bare escape followed by the char
      keys.push({ type: "escape" });
      i++;
      continue;
    }

    // Ctrl combos (0x01-0x1a except 0x0d=enter, 0x1b=escape)
    if (byte === 0x0d || byte === 0x0a) {
      keys.push({ type: "enter" });
      i++;
      continue;
    }

    if (byte >= 0x01 && byte <= 0x1a) {
      keys.push({ type: "ctrl", char: String.fromCharCode(byte + 0x60) });
      i++;
      continue;
    }

    // Printable ASCII
    if (byte >= 0x20 && byte <= 0x7e) {
      keys.push({ type: "char", char: String.fromCharCode(byte) });
      i++;
      continue;
    }

    // Skip unrecognized bytes
    i++;
  }

  return keys;
}

/** Parse CSI parameters starting at index i (after ESC [). */
function parseCSI(
  data: Uint8Array,
  i: number,
): { key?: Key; nextIndex: number } | undefined {
  if (data[i] === 0x3c) {
    return parseSgrMouse(data, i + 1);
  }

  // Collect numeric parameter bytes (0x30-0x39 and 0x3b)
  let param = "";
  while (i < data.length && ((data[i]! >= 0x30 && data[i]! <= 0x39) || data[i] === 0x3b)) {
    param += String.fromCharCode(data[i]!);
    i++;
  }

  if (i >= data.length) return undefined;

  const final = data[i]!;
  i++;

  // Arrow keys: ESC [ A/B/C/D
  if (final === 0x41) return { key: { type: "arrow", direction: "up" }, nextIndex: i };
  if (final === 0x42) return { key: { type: "arrow", direction: "down" }, nextIndex: i };
  if (final === 0x43) return { key: { type: "arrow", direction: "right" }, nextIndex: i };
  if (final === 0x44) return { key: { type: "arrow", direction: "left" }, nextIndex: i };

  // Function keys: ESC [ N ~ (where N is the function key code)
  if (final === 0x7e) {
    const n = parseInt(param, 10);
    const fnMap: Record<number, number> = {
      11: 1, 12: 2, 13: 3, 14: 4, 15: 5,
      17: 6, 18: 7, 19: 8, 20: 9, 21: 10,
      23: 11, 24: 12,
    };
    if (fnMap[n] !== undefined) {
      return { key: { type: "function", n: fnMap[n]! }, nextIndex: i };
    }
  }

  return undefined;
}

function parseSgrMouse(
  data: Uint8Array,
  i: number,
): { key?: Key; nextIndex: number } | undefined {
  let param = "";
  while (i < data.length && ((data[i]! >= 0x30 && data[i]! <= 0x39) || data[i] === 0x3b)) {
    param += String.fromCharCode(data[i]!);
    i++;
  }

  if (i >= data.length) return undefined;

  const final = data[i]!;
  i++;
  if (final !== 0x4d && final !== 0x6d) return undefined;

  const [buttonRaw, xRaw, yRaw] = param.split(";").map((value) => Number.parseInt(value, 10));
  if (!Number.isFinite(buttonRaw) || !Number.isFinite(xRaw) || !Number.isFinite(yRaw)) {
    return undefined;
  }

  const isLeftButton = (buttonRaw & 0b11) === 0 && buttonRaw < 64;
  if (!isLeftButton) return { nextIndex: i };

  return {
    key: {
      type: "mouse",
      event: final === 0x4d ? "down" : "up",
      button: "left",
      x: Math.max(0, xRaw - 1),
      y: Math.max(0, yRaw - 1),
    },
    nextIndex: i,
  };
}

/**
 * Start listening for keypress events on stdin.
 * Returns a cleanup function to stop listening.
 * Requires raw mode to be enabled on stdin.
 */
export function startKeyListener(handler: (key: Key) => void): () => void {
  const onData = (chunk: Buffer) => {
    const keys = parseKeypress(new Uint8Array(chunk));
    for (const key of keys) {
      handler(key);
    }
  };

  process.stdin.on("data", onData);
  return () => {
    process.stdin.off("data", onData);
  };
}
