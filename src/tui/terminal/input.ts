/**
 * Raw stdin key parser.
 * Converts multi-byte escape sequences into structured Key objects.
 */

export type Key =
  | { type: "char"; char: string }
  | { type: "arrow"; direction: "up" | "down" | "left" | "right" }
  | { type: "enter" }
  | { type: "escape" }
  | { type: "backspace" }
  | { type: "delete" }
  | { type: "ctrl"; char: string }
  | { type: "function"; n: number }
  | { type: "mouse"; event: "down" | "up"; button: "left"; x: number; y: number };

const ESCAPE_FLUSH_DELAY_MS = 25;

interface ParseResult {
  readonly keys: Key[];
  readonly remainder: Uint8Array;
}

interface CsiParseResult {
  readonly key?: Key;
  readonly nextIndex: number;
  readonly incomplete?: boolean;
}

export interface BufferedKeyParser {
  push(data: Uint8Array): Key[];
  flushPending(): Key[];
  hasPending(): boolean;
}

/**
 * Parse raw bytes from stdin into Key events.
 * Handles: printable ASCII, arrow keys (CSI A/B/C/D), ctrl combos,
 * function keys (CSI 11~..CSI 24~), enter, escape.
 */
export function parseKeypress(data: Uint8Array): Key[] {
  return parseKeypressInternal(data, true).keys;
}

function parseKeypressInternal(data: Uint8Array, flushIncomplete: boolean): ParseResult {
  const keys: Key[] = [];
  let i = 0;

  while (i < data.length) {
    const byte = data[i]!;

    // ESC sequence
    if (byte === 0x1b) {
      // Bare escape (no more bytes or next byte isn't '[')
      if (i + 1 >= data.length) {
        if (flushIncomplete) {
          keys.push({ type: "escape" });
          i++;
          continue;
        }
        return { keys, remainder: data.slice(i) };
      }

      if (data[i + 1] === 0x5b) {
        // CSI sequence: ESC [
        i += 2;
        const result = parseCSI(data, i);
        if (result) {
          if (result.incomplete) {
            return { keys, remainder: data.slice(i - 2) };
          }
          if (result.key) {
            keys.push(result.key);
          }
          i = result.nextIndex;
        } else if (flushIncomplete) {
          keys.push({ type: "escape" });
          continue;
        } else {
          return { keys, remainder: data.slice(i - 2) };
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

    if (byte === 0x08 || byte === 0x7f) {
      keys.push({ type: "backspace" });
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

  return { keys, remainder: new Uint8Array() };
}

/** Parse CSI parameters starting at index i (after ESC [). */
function parseCSI(
  data: Uint8Array,
  i: number,
) : CsiParseResult | undefined {
  if (data[i] === 0x3c) {
    return parseSgrMouse(data, i + 1);
  }

  // Collect numeric parameter bytes (0x30-0x39 and 0x3b)
  let param = "";
  while (i < data.length && ((data[i]! >= 0x30 && data[i]! <= 0x39) || data[i] === 0x3b)) {
    param += String.fromCharCode(data[i]!);
    i++;
  }

  if (i >= data.length) {
    return { nextIndex: i, incomplete: true };
  }

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
    if (n === 3) {
      return { key: { type: "delete" }, nextIndex: i };
    }
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
): CsiParseResult | undefined {
  let param = "";
  while (i < data.length && ((data[i]! >= 0x30 && data[i]! <= 0x39) || data[i] === 0x3b)) {
    param += String.fromCharCode(data[i]!);
    i++;
  }

  if (i >= data.length) {
    return { nextIndex: i, incomplete: true };
  }

  const final = data[i]!;
  i++;
  if (final !== 0x4d && final !== 0x6d) return undefined;

  const [buttonRaw, xRaw, yRaw] = param.split(";").map((value) => Number.parseInt(value, 10));
  if (buttonRaw == null || xRaw == null || yRaw == null) {
    return undefined;
  }
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

export function createBufferedKeyParser(): BufferedKeyParser {
  let pending: Uint8Array = new Uint8Array();

  return {
    push(data: Uint8Array): Key[] {
      const merged = pending.length > 0 ? concatBytes(pending, data) : data;
      const result = parseKeypressInternal(merged, false);
      pending = result.remainder;
      return result.keys;
    },
    flushPending(): Key[] {
      if (pending.length === 0) return [];
      const result = parseKeypressInternal(pending, true);
      pending = result.remainder;
      return result.keys;
    },
    hasPending(): boolean {
      return pending.length > 0;
    },
  };
}

/**
 * Start listening for keypress events on stdin.
 * Returns a cleanup function to stop listening.
 * Requires raw mode to be enabled on stdin.
 */
export function startKeyListener(handler: (key: Key) => void): () => void {
  const parser = createBufferedKeyParser();
  let escapeTimer: ReturnType<typeof setTimeout> | undefined;

  const emit = (keys: readonly Key[]): void => {
    for (const key of keys) {
      handler(key);
    }
  };

  const clearEscapeTimer = (): void => {
    if (!escapeTimer) return;
    clearTimeout(escapeTimer);
    escapeTimer = undefined;
  };

  const schedulePendingFlush = (): void => {
    clearEscapeTimer();
    if (!parser.hasPending()) return;
    escapeTimer = setTimeout(() => {
      escapeTimer = undefined;
      emit(parser.flushPending());
    }, ESCAPE_FLUSH_DELAY_MS);
  };

  const onData = (chunk: Buffer) => {
    clearEscapeTimer();
    emit(parser.push(new Uint8Array(chunk)));
    schedulePendingFlush();
  };

  process.stdin.on("data", onData);
  return () => {
    clearEscapeTimer();
    process.stdin.off("data", onData);
  };
}

function concatBytes(left: Uint8Array, right: Uint8Array): Uint8Array {
  const merged = new Uint8Array(left.length + right.length);
  merged.set(left);
  merged.set(right, left.length);
  return merged;
}
