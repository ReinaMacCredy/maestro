import { describe, expect, it } from "bun:test";
import { parseKeypress } from "../../../../src/tui/terminal/input.js";

function bytes(...values: number[]): Uint8Array {
  return new Uint8Array(values);
}

describe("parseKeypress", () => {
  describe("printable characters", () => {
    it("parses single printable char", () => {
      const keys = parseKeypress(bytes(0x61)); // 'a'
      expect(keys).toEqual([{ type: "char", char: "a" }]);
    });

    it("parses multiple chars in one buffer", () => {
      const keys = parseKeypress(bytes(0x68, 0x69)); // 'hi'
      expect(keys).toEqual([
        { type: "char", char: "h" },
        { type: "char", char: "i" },
      ]);
    });

    it("parses space", () => {
      const keys = parseKeypress(bytes(0x20));
      expect(keys).toEqual([{ type: "char", char: " " }]);
    });
  });

  describe("arrow keys", () => {
    it("parses up arrow (ESC [ A)", () => {
      const keys = parseKeypress(bytes(0x1b, 0x5b, 0x41));
      expect(keys).toEqual([{ type: "arrow", direction: "up" }]);
    });

    it("parses down arrow (ESC [ B)", () => {
      const keys = parseKeypress(bytes(0x1b, 0x5b, 0x42));
      expect(keys).toEqual([{ type: "arrow", direction: "down" }]);
    });

    it("parses right arrow (ESC [ C)", () => {
      const keys = parseKeypress(bytes(0x1b, 0x5b, 0x43));
      expect(keys).toEqual([{ type: "arrow", direction: "right" }]);
    });

    it("parses left arrow (ESC [ D)", () => {
      const keys = parseKeypress(bytes(0x1b, 0x5b, 0x44));
      expect(keys).toEqual([{ type: "arrow", direction: "left" }]);
    });
  });

  describe("ctrl combos", () => {
    it("parses ctrl+c (0x03)", () => {
      const keys = parseKeypress(bytes(0x03));
      expect(keys).toEqual([{ type: "ctrl", char: "c" }]);
    });

    it("parses ctrl+t (0x14)", () => {
      const keys = parseKeypress(bytes(0x14));
      expect(keys).toEqual([{ type: "ctrl", char: "t" }]);
    });

    it("parses ctrl+a (0x01)", () => {
      const keys = parseKeypress(bytes(0x01));
      expect(keys).toEqual([{ type: "ctrl", char: "a" }]);
    });
  });

  describe("special keys", () => {
    it("parses enter (0x0d)", () => {
      const keys = parseKeypress(bytes(0x0d));
      expect(keys).toEqual([{ type: "enter" }]);
    });

    it("parses newline as enter (0x0a)", () => {
      const keys = parseKeypress(bytes(0x0a));
      expect(keys).toEqual([{ type: "enter" }]);
    });

    it("parses bare escape", () => {
      const keys = parseKeypress(bytes(0x1b));
      expect(keys).toEqual([{ type: "escape" }]);
    });
  });

  describe("function keys", () => {
    it("parses F1 (ESC [ 11 ~)", () => {
      const keys = parseKeypress(bytes(0x1b, 0x5b, 0x31, 0x31, 0x7e));
      expect(keys).toEqual([{ type: "function", n: 1 }]);
    });

    it("parses F5 (ESC [ 15 ~)", () => {
      const keys = parseKeypress(bytes(0x1b, 0x5b, 0x31, 0x35, 0x7e));
      expect(keys).toEqual([{ type: "function", n: 5 }]);
    });

    it("parses F12 (ESC [ 24 ~)", () => {
      const keys = parseKeypress(bytes(0x1b, 0x5b, 0x32, 0x34, 0x7e));
      expect(keys).toEqual([{ type: "function", n: 12 }]);
    });
  });

  describe("mixed input", () => {
    it("parses arrow followed by char", () => {
      const keys = parseKeypress(bytes(0x1b, 0x5b, 0x41, 0x71)); // Up, 'q'
      expect(keys).toEqual([
        { type: "arrow", direction: "up" },
        { type: "char", char: "q" },
      ]);
    });

    it("handles empty input", () => {
      const keys = parseKeypress(bytes());
      expect(keys).toEqual([]);
    });
  });
});
