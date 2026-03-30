/**
 * Screen -- alternate screen, raw mode, direct ANSI write.
 * No double-buffer. Clears and redraws the full frame each time.
 * Simple and memory-efficient for compiled binaries.
 */
import { Buffer } from "./buffer.js";
import { moveTo, style, reset, hideCursor, showCursor, enterAltScreen, exitAltScreen, clearScreen } from "./ansi.js";

export class Screen {
  private _width: number;
  private _height: number;
  private active = false;
  private signalHandlers: Array<() => void> = [];

  constructor() {
    this._width = process.stdout.columns || 80;
    this._height = process.stdout.rows || 24;
  }

  get width(): number { return this._width; }
  get height(): number { return this._height; }

  /** Enter alternate screen, hide cursor, enable raw mode. */
  enter(): void {
    if (this.active) return;
    this.active = true;
    process.stdout.write(enterAltScreen + hideCursor);
    if (process.stdin.isTTY && typeof process.stdin.setRawMode === "function") {
      process.stdin.setRawMode(true);
      process.stdin.resume();
    }

    // Register signal handlers to restore terminal on unexpected exit
    const onSigint = () => { this.exit(); process.exit(130); };
    const onSigterm = () => { this.exit(); process.exit(143); };
    process.on("SIGINT", onSigint);
    process.on("SIGTERM", onSigterm);
    this.signalHandlers.push(
      () => process.off("SIGINT", onSigint),
      () => process.off("SIGTERM", onSigterm),
    );
  }

  /** Exit alternate screen, show cursor, restore cooked mode. */
  exit(): void {
    if (!this.active) return;
    this.active = false;
    for (const remove of this.signalHandlers) remove();
    this.signalHandlers.length = 0;
    process.stdout.write(reset + showCursor + exitAltScreen);
    if (process.stdin.isTTY && typeof process.stdin.setRawMode === "function") {
      process.stdin.setRawMode(false);
      process.stdin.pause();
    }
  }

  /** Create a buffer for drawing. Caller fills it, then calls render(). */
  createBuffer(): Buffer {
    return new Buffer(this._width, this._height);
  }

  /** Render a filled buffer to the terminal in one write. */
  render(buf: Buffer): void {
    // Build output: move to home, write each cell row by row
    const parts: string[] = [moveTo(0, 0)];
    let lastFg = -2;
    let lastBg = -2;
    let lastBold = false;
    let lastDim = false;

    for (let r = 0; r < buf.height && r < this._height; r++) {
      if (r > 0) parts.push(moveTo(r, 0));
      for (let c = 0; c < buf.width && c < this._width; c++) {
        const cell = buf.getCell(r, c);
        if (!cell) { parts.push(" "); continue; }
        if (cell.fg !== lastFg || cell.bg !== lastBg || cell.bold !== lastBold || cell.dim !== lastDim) {
          parts.push(style(cell.fg, cell.bg, cell.bold, cell.dim));
          lastFg = cell.fg;
          lastBg = cell.bg;
          lastBold = cell.bold;
          lastDim = cell.dim;
        }
        parts.push(cell.char);
      }
    }
    parts.push(reset);
    process.stdout.write(parts.join(""));
  }

  /** Update cached terminal dimensions. Returns true if size changed. */
  refreshSize(): boolean {
    const newW = process.stdout.columns || 80;
    const newH = process.stdout.rows || 24;
    if (newW === this._width && newH === this._height) return false;
    this._width = newW;
    this._height = newH;
    return true;
  }
}
