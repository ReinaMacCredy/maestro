import { describe, expect, it } from "bun:test";
import { splitH, splitV, inset, type Rect } from "../../../../src/tui/terminal/layout.js";

const ROOT: Rect = { x: 0, y: 0, width: 120, height: 32 };

describe("splitH", () => {
  it("splits by equal ratios", () => {
    const [left, right] = splitH(ROOT, [1, 1]);
    expect(left!.x).toBe(0);
    expect(left!.width).toBe(60);
    expect(right!.x).toBe(60);
    expect(right!.width).toBe(60);
    // Heights unchanged
    expect(left!.height).toBe(32);
    expect(right!.height).toBe(32);
  });

  it("splits by unequal ratios (40/60)", () => {
    const [left, right] = splitH(ROOT, [2, 3]);
    expect(left!.width).toBe(48);
    expect(right!.width).toBe(72);
    expect(left!.width + right!.width).toBe(120);
  });

  it("assigns remainder to last segment", () => {
    // 3-way split of 10 cols: 3.33 + 3.33 + 3.33 -> 3 + 3 + 4
    const rects = splitH({ x: 0, y: 0, width: 10, height: 1 }, [1, 1, 1]);
    const totalWidth = rects.reduce((s, r) => s + r.width, 0);
    expect(totalWidth).toBe(10);
  });

  it("returns empty for empty ratios", () => {
    expect(splitH(ROOT, [])).toEqual([]);
  });

  it("handles zero ratios", () => {
    const rects = splitH(ROOT, [0, 0]);
    expect(rects.length).toBe(2);
    rects.forEach((r) => expect(r.width).toBe(0));
  });
});

describe("splitV", () => {
  it("splits by fixed sizes", () => {
    const [top, mid, bot] = splitV(ROOT, [1, 1, 30]);
    expect(top!.y).toBe(0);
    expect(top!.height).toBe(1);
    expect(mid!.y).toBe(1);
    expect(mid!.height).toBe(1);
    expect(bot!.y).toBe(2);
    expect(bot!.height).toBe(30);
  });

  it("uses flex (negative) for remaining space", () => {
    // Header=1, flex, footer=1 in 32 rows
    const [header, body, footer] = splitV(ROOT, [1, -1, 1]);
    expect(header!.height).toBe(1);
    expect(body!.height).toBe(30); // 32 - 1 - 1
    expect(footer!.height).toBe(1);
  });

  it("flex is zero when fixed sizes exceed parent", () => {
    const [a, flex, b] = splitV({ x: 0, y: 0, width: 10, height: 5 }, [3, -1, 3]);
    expect(flex!.height).toBe(0); // 5 - 3 - 3 = -1 -> clamped to 0
  });

  it("returns empty for empty sizes", () => {
    expect(splitV(ROOT, [])).toEqual([]);
  });

  it("preserves x and width", () => {
    const parent: Rect = { x: 10, y: 5, width: 50, height: 20 };
    const rects = splitV(parent, [5, -1, 5]);
    rects.forEach((r) => {
      expect(r.x).toBe(10);
      expect(r.width).toBe(50);
    });
  });
});

describe("inset", () => {
  it("shrinks rect by padding", () => {
    const result = inset(ROOT, 1);
    expect(result).toEqual({ x: 1, y: 1, width: 118, height: 30 });
  });

  it("clamps to zero for large padding", () => {
    const result = inset({ x: 0, y: 0, width: 4, height: 4 }, 3);
    expect(result.width).toBe(0);
    expect(result.height).toBe(0);
  });

  it("handles zero padding", () => {
    const result = inset(ROOT, 0);
    expect(result).toEqual(ROOT);
  });
});
