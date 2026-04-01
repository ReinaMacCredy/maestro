import { describe, expect, it } from "bun:test";
import { Buffer } from "../../../../src/tui/terminal/buffer.js";
import { reset } from "../../../../src/tui/terminal/ansi.js";

describe("Buffer", () => {
  describe("construction", () => {
    it("creates buffer with correct dimensions", () => {
      const buf = new Buffer(10, 5);
      expect(buf.width).toBe(10);
      expect(buf.height).toBe(5);
    });

    it("handles zero dimensions", () => {
      const buf = new Buffer(0, 0);
      expect(buf.width).toBe(0);
      expect(buf.height).toBe(0);
      expect(buf.toString()).toBe("");
    });

    it("clamps negative dimensions to zero", () => {
      const buf = new Buffer(-5, -3);
      expect(buf.width).toBe(0);
      expect(buf.height).toBe(0);
    });
  });

  describe("set", () => {
    it("sets a cell at position", () => {
      const buf = new Buffer(5, 3);
      buf.set(1, 2, "X", { fg: 196 });
      const cell = buf.getCell(1, 2);
      expect(cell?.char).toBe("X");
      expect(cell?.fg).toBe(196);
    });

    it("ignores out-of-bounds writes", () => {
      const buf = new Buffer(5, 3);
      buf.set(10, 10, "X");
      buf.set(-1, 0, "Y");
      // No crash, buffer unchanged
      expect(buf.toString()).toBe("");
    });

    it("takes only first character of multi-char string", () => {
      const buf = new Buffer(5, 1);
      buf.set(0, 0, "AB");
      expect(buf.getCell(0, 0)?.char).toBe("A");
    });
  });

    describe("writeText", () => {
    it("writes text starting at position", () => {
      const buf = new Buffer(10, 1);
      const written = buf.writeText(0, 2, "hello");
      expect(written).toBe(5);
      expect(buf.toString()).toBe("  hello");
    });

    it("truncates text at buffer edge", () => {
      const buf = new Buffer(5, 1);
      const written = buf.writeText(0, 3, "hello");
      expect(written).toBe(2);
      expect(buf.toString()).toBe("   he");
    });

      it("applies style to all characters", () => {
        const buf = new Buffer(10, 1);
        buf.writeText(0, 0, "abc", { fg: 196, bold: true });
        for (let i = 0; i < 3; i++) {
          expect(buf.getCell(0, i)?.fg).toBe(196);
          expect(buf.getCell(0, i)?.bold).toBe(true);
        }
      });

      it("strips ANSI escapes and control characters before writing", () => {
        const buf = new Buffer(20, 1);
        const written = buf.writeText(0, 0, "\u001b[2Jhi\u0007");
        expect(written).toBe(2);
        expect(buf.toString()).toBe("hi");
      });
    });

  describe("fillRow", () => {
    it("fills entire row with character", () => {
      const buf = new Buffer(5, 2);
      buf.fillRow(0, "-");
      expect(buf.toString()).toBe("-----");
    });
  });

  describe("fillRect", () => {
    it("fills rectangular region", () => {
      const buf = new Buffer(6, 4);
      buf.fillRect({ x: 1, y: 1, width: 3, height: 2 }, "#");
      expect(buf.toString()).toBe("\n ###\n ###");
    });
  });

  describe("drawBorder", () => {
    it("draws border around rect", () => {
      const buf = new Buffer(5, 3);
      buf.drawBorder({ x: 0, y: 0, width: 5, height: 3 });
      const text = buf.toString();
      expect(text).toContain("\u250c");   // topLeft
      expect(text).toContain("\u2510");   // topRight
      expect(text).toContain("\u2514");   // bottomLeft
      expect(text).toContain("\u2518");   // bottomRight
      expect(text).toContain("\u2500");   // horizontal
      expect(text).toContain("\u2502");   // vertical
    });

    it("does nothing for rects smaller than 2x2", () => {
      const buf = new Buffer(3, 1);
      buf.drawBorder({ x: 0, y: 0, width: 3, height: 1 });
      expect(buf.toString()).toBe("");
    });
  });

  describe("diff", () => {
    it("returns empty when buffers are identical", () => {
      const a = new Buffer(5, 3);
      const b = new Buffer(5, 3);
      expect(a.diff(b)).toEqual([]);
    });

    it("detects changed cells", () => {
      const prev = new Buffer(5, 3);
      const curr = new Buffer(5, 3);
      curr.set(1, 2, "X");
      const changes = curr.diff(prev);
      expect(changes.length).toBe(1);
      expect(changes[0]!.row).toBe(1);
      expect(changes[0]!.col).toBe(2);
      expect(changes[0]!.cell.char).toBe("X");
    });

    it("detects style changes on same character", () => {
      const prev = new Buffer(5, 1);
      prev.set(0, 0, "A", { fg: 1 });
      const curr = new Buffer(5, 1);
      curr.set(0, 0, "A", { fg: 2 });
      const changes = curr.diff(prev);
      expect(changes.length).toBe(1);
      expect(changes[0]!.cell.fg).toBe(2);
    });

    it("returns multiple changes", () => {
      const prev = new Buffer(10, 5);
      const curr = new Buffer(10, 5);
      curr.writeText(0, 0, "hello");
      curr.writeText(2, 0, "world");
      const changes = curr.diff(prev);
      expect(changes.length).toBe(10);
    });
  });

  describe("clone", () => {
    it("creates independent copy", () => {
      const original = new Buffer(5, 3);
      original.set(1, 2, "X");
      const copy = original.clone();
      copy.set(1, 2, "Y");
      expect(original.getCell(1, 2)?.char).toBe("X");
      expect(copy.getCell(1, 2)?.char).toBe("Y");
    });
  });

  describe("toString", () => {
    it("renders buffer as plain text", () => {
      const buf = new Buffer(10, 3);
      buf.writeText(0, 0, "Line 1");
      buf.writeText(1, 0, "Line 2");
      buf.writeText(2, 0, "Line 3");
      expect(buf.toString()).toBe("Line 1\nLine 2\nLine 3");
    });

    it("trims trailing whitespace per line", () => {
      const buf = new Buffer(10, 1);
      buf.writeText(0, 0, "hi");
      expect(buf.toString()).toBe("hi");
    });

    it("trims trailing empty lines", () => {
      const buf = new Buffer(10, 5);
      buf.writeText(0, 0, "only first");
      expect(buf.toString()).toBe("only first");
    });
  });

  describe("toAnsiString", () => {
    it("renders ANSI style sequences for styled cells", () => {
      const buf = new Buffer(10, 1);
      buf.writeText(0, 0, "+ src/tui/index.ts", { fg: 46, bold: true });

      const text = buf.toAnsiString();
      expect(text).toContain("\u001b[");
      expect(text).toContain("+ src/tui/");
      expect(text.endsWith(reset)).toBe(true);
    });

    it("trims trailing empty lines like plain output", () => {
      const buf = new Buffer(10, 3);
      buf.writeText(0, 0, "hi", { fg: 220 });

      const text = buf.toAnsiString();
      expect(text.includes("\n\n")).toBe(false);
    });
  });
});
