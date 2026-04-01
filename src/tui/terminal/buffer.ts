/**
 * Cell grid with double-buffer diff algorithm.
 * Only changed cells are emitted as ANSI -- eliminates flicker.
 */
import type { Rect } from "./layout.js";
import { BOX } from "./ansi.js";
import { sanitizeTerminalText } from "../../lib/sanitize.js";

export interface Cell {
  char: string;
  fg: number;   // 256-color index, -1 = default
  bg: number;   // 256-color index, -1 = default
  bold: boolean;
  dim: boolean;
}

export interface CellChange {
  row: number;
  col: number;
  cell: Cell;
}

const DEFAULT_CELL: Cell = { char: " ", fg: -1, bg: -1, bold: false, dim: false };

function cellsEqual(a: Cell, b: Cell): boolean {
  return a.char === b.char && a.fg === b.fg && a.bg === b.bg && a.bold === b.bold && a.dim === b.dim;
}

export class Buffer {
  readonly width: number;
  readonly height: number;
  private cells: Cell[][];

  constructor(width: number, height: number) {
    this.width = Math.max(0, width);
    this.height = Math.max(0, height);
    this.cells = [];
    for (let r = 0; r < this.height; r++) {
      const row: Cell[] = [];
      for (let c = 0; c < this.width; c++) {
        row.push({ ...DEFAULT_CELL });
      }
      this.cells.push(row);
    }
  }

  /** Get cell at position. Returns undefined if out of bounds. */
  getCell(row: number, col: number): Cell | undefined {
    return this.cells[row]?.[col];
  }

  /** Set a single cell. Out-of-bounds writes are silently ignored. */
  set(row: number, col: number, char: string, s?: Partial<Cell>): void {
    if (row < 0 || row >= this.height || col < 0 || col >= this.width) return;
    const cell = this.cells[row]![col]!;
    cell.char = char.length > 0 ? char[0]! : " ";
    if (s) {
      if (s.fg !== undefined) cell.fg = s.fg;
      if (s.bg !== undefined) cell.bg = s.bg;
      if (s.bold !== undefined) cell.bold = s.bold;
      if (s.dim !== undefined) cell.dim = s.dim;
    }
  }

  /** Write a text string starting at (row, col). Returns number of columns written. */
  writeText(row: number, col: number, text: string, s?: Partial<Cell>): number {
    const sanitized = sanitizeTerminalText(text);
    let written = 0;
    for (let i = 0; i < sanitized.length; i++) {
      const c = col + i;
      if (c >= this.width) break;
      this.set(row, c, sanitized[i]!, s);
      written++;
    }
    return written;
  }

  /** Fill an entire row with a character and style. */
  fillRow(row: number, char: string, s?: Partial<Cell>): void {
    for (let c = 0; c < this.width; c++) {
      this.set(row, c, char, s);
    }
  }

  /** Fill a rect region with a character and style. */
  fillRect(rect: Rect, char: string, s?: Partial<Cell>): void {
    for (let r = rect.y; r < rect.y + rect.height; r++) {
      for (let c = rect.x; c < rect.x + rect.width; c++) {
        this.set(r, c, char, s);
      }
    }
  }

  /** Draw a single-line border around a rect. */
  drawBorder(rect: Rect, s?: Partial<Cell>): void {
    const { x, y, width, height } = rect;
    if (width < 2 || height < 2) return;

    // Corners
    this.set(y, x, BOX.topLeft, s);
    this.set(y, x + width - 1, BOX.topRight, s);
    this.set(y + height - 1, x, BOX.bottomLeft, s);
    this.set(y + height - 1, x + width - 1, BOX.bottomRight, s);

    // Horizontal edges
    for (let c = x + 1; c < x + width - 1; c++) {
      this.set(y, c, BOX.horizontal, s);
      this.set(y + height - 1, c, BOX.horizontal, s);
    }

    // Vertical edges
    for (let r = y + 1; r < y + height - 1; r++) {
      this.set(r, x, BOX.vertical, s);
      this.set(r, x + width - 1, BOX.vertical, s);
    }
  }

  /** Compute diff: only cells that differ from `previous`. */
  diff(previous: Buffer): CellChange[] {
    const changes: CellChange[] = [];
    const maxR = Math.min(this.height, previous.height);
    const maxC = Math.min(this.width, previous.width);

    for (let r = 0; r < maxR; r++) {
      for (let c = 0; c < maxC; c++) {
        const curr = this.cells[r]![c]!;
        const prev = previous.cells[r]![c]!;
        if (!cellsEqual(curr, prev)) {
          changes.push({ row: r, col: c, cell: { ...curr } });
        }
      }
    }

    // New cells in extended rows/cols (if this buffer is larger)
    for (let r = 0; r < this.height; r++) {
      const startC = r < maxR ? maxC : 0;
      for (let c = startC; c < this.width; c++) {
        const curr = this.cells[r]![c]!;
        if (!cellsEqual(curr, DEFAULT_CELL)) {
          changes.push({ row: r, col: c, cell: { ...curr } });
        }
      }
    }

    return changes;
  }

  /** Create a deep clone of this buffer. */
  clone(): Buffer {
    const copy = new Buffer(this.width, this.height);
    for (let r = 0; r < this.height; r++) {
      for (let c = 0; c < this.width; c++) {
        const src = this.cells[r]![c]!;
        const dst = copy.cells[r]![c]!;
        dst.char = src.char;
        dst.fg = src.fg;
        dst.bg = src.bg;
        dst.bold = src.bold;
        dst.dim = src.dim;
      }
    }
    return copy;
  }

  /** Render buffer as plain text (no ANSI). For testing / --once output. */
  toString(): string {
    const lines: string[] = [];
    for (let r = 0; r < this.height; r++) {
      let line = "";
      for (let c = 0; c < this.width; c++) {
        line += this.cells[r]![c]!.char;
      }
      lines.push(line.trimEnd());
    }
    // Trim trailing empty lines
    while (lines.length > 0 && lines[lines.length - 1] === "") {
      lines.pop();
    }
    return lines.join("\n");
  }
}
