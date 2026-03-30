/**
 * Screen -- alternate screen, raw mode, double-buffer flush.
 * Wraps Buffer with lifecycle management for interactive TUI.
 */
import { Buffer, type CellChange } from "./buffer.js";
import { moveTo, style, reset, hideCursor, showCursor, enterAltScreen, exitAltScreen, clearScreen } from "./ansi.js";

export class Screen {
  private front: Buffer;
  private back: Buffer;
  private _width: number;
  private _height: number;
  private resizeCallbacks: Array<() => void> = [];
  private active = false;

  constructor() {
    this._width = process.stdout.columns || 80;
    this._height = process.stdout.rows || 24;
    this.front = new Buffer(this._width, this._height);
    this.back = new Buffer(this._width, this._height);
  }

  get width(): number { return this._width; }
  get height(): number { return this._height; }

  /** Enter alternate screen, hide cursor, enable raw mode. */
  enter(): void {
    if (this.active) return;
    this.active = true;
    process.stdout.write(enterAltScreen + hideCursor + clearScreen);
    if (process.stdin.isTTY && typeof process.stdin.setRawMode === "function") {
      process.stdin.setRawMode(true);
      process.stdin.resume();
    }
    process.stdout.on("resize", this.handleResize);
  }

  /** Exit alternate screen, show cursor, restore cooked mode. */
  exit(): void {
    if (!this.active) return;
    this.active = false;
    process.stdout.off("resize", this.handleResize);
    process.stdout.write(reset + showCursor + exitAltScreen);
    if (process.stdin.isTTY && typeof process.stdin.setRawMode === "function") {
      process.stdin.setRawMode(false);
      process.stdin.pause();
    }
  }

  /** Get the back buffer for drawing. */
  buffer(): Buffer {
    return this.back;
  }

  /** Diff back vs front, emit ANSI for changed cells, swap buffers. */
  flush(): void {
    const changes = this.back.diff(this.front);
    if (changes.length === 0) return;

    let out = "";
    let lastFg = -2;
    let lastBg = -2;
    let lastBold = false;
    let lastDim = false;

    for (const ch of changes) {
      out += moveTo(ch.row, ch.col);
      // Only emit style if it changed
      if (ch.cell.fg !== lastFg || ch.cell.bg !== lastBg || ch.cell.bold !== lastBold || ch.cell.dim !== lastDim) {
        out += style(ch.cell.fg, ch.cell.bg, ch.cell.bold, ch.cell.dim);
        lastFg = ch.cell.fg;
        lastBg = ch.cell.bg;
        lastBold = ch.cell.bold;
        lastDim = ch.cell.dim;
      }
      out += ch.cell.char;
    }
    out += reset;
    process.stdout.write(out);

    // Swap: front becomes back's state
    this.front = this.back.clone();
    this.back = new Buffer(this._width, this._height);
  }

  /** Register a resize callback. */
  onResize(cb: () => void): void {
    this.resizeCallbacks.push(cb);
  }

  /** Handle terminal resize. */
  private handleResize = (): void => {
    this._width = process.stdout.columns || 80;
    this._height = process.stdout.rows || 24;
    this.front = new Buffer(this._width, this._height);
    this.back = new Buffer(this._width, this._height);
    for (const cb of this.resizeCallbacks) {
      cb();
    }
  };

  /** Force a full redraw on next flush. */
  invalidate(): void {
    this.front = new Buffer(this._width, this._height);
  }
}
