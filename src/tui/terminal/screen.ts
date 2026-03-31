/**
 * Screen -- alternate screen, raw mode, row-by-row rendering.
 * Writes each row separately to avoid large string construction.
 */
import { Buffer } from "./buffer.js";
import {
  moveTo,
  style,
  reset,
  hideCursor,
  showCursor,
  enterAltScreen,
  exitAltScreen,
  enableMouse,
  disableMouse,
} from "./ansi.js";

export class Screen {
  private _width: number;
  private _height: number;
  private active = false;

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
    process.stdout.write(enterAltScreen + hideCursor + enableMouse);
    if (process.stdin.isTTY && typeof process.stdin.setRawMode === "function") {
      process.stdin.setRawMode(true);
      process.stdin.resume();
    }
  }

  /** Exit alternate screen, show cursor, restore cooked mode. */
  exit(): void {
    if (!this.active) return;
    this.active = false;
    process.stdout.write(reset + showCursor + disableMouse + exitAltScreen);
    if (process.stdin.isTTY && typeof process.stdin.setRawMode === "function") {
      process.stdin.setRawMode(false);
    }
    if (typeof process.stdin.pause === "function") {
      try {
        process.stdin.pause();
      } catch {
        // Ignore cleanup errors during shutdown.
      }
    }
  }

  /** Create a buffer for drawing. Caller fills it, then calls render(). */
  createBuffer(): Buffer {
    return new Buffer(this._width, this._height);
  }

  /** Render a filled buffer to the terminal, row by row. */
  render(buf: Buffer): void {
    const out = process.stdout;
    let lastFg = -2;
    let lastBg = -2;
    let lastBold = false;
    let lastDim = false;

    for (let r = 0; r < buf.height && r < this._height; r++) {
      // Build one row at a time to keep memory flat
      let row = moveTo(r, 0);
      for (let c = 0; c < buf.width && c < this._width; c++) {
        const cell = buf.getCell(r, c);
        if (!cell) { row += " "; continue; }
        if (cell.fg !== lastFg || cell.bg !== lastBg || cell.bold !== lastBold || cell.dim !== lastDim) {
          row += style(cell.fg, cell.bg, cell.bold, cell.dim);
          lastFg = cell.fg;
          lastBg = cell.bg;
          lastBold = cell.bold;
          lastDim = cell.dim;
        }
        row += cell.char;
      }
      out.write(row);
    }
    out.write(reset);
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
