import { describe, expect, it } from "bun:test";
import { ScrollState } from "../../../../src/tui/widgets/scrollable.js";

describe("ScrollState", () => {
  it("initializes with selection at 0", () => {
    const s = new ScrollState(10, 5);
    expect(s.selected).toBe(0);
    expect(s.offset).toBe(0);
  });

  it("moves selection down", () => {
    const s = new ScrollState(10, 5);
    expect(s.moveDown()).toBe(true);
    expect(s.selected).toBe(1);
  });

  it("moves selection up", () => {
    const s = new ScrollState(10, 5);
    s.moveDown();
    s.moveDown();
    expect(s.moveUp()).toBe(true);
    expect(s.selected).toBe(1);
  });

  it("cannot move below total - 1", () => {
    const s = new ScrollState(3, 5);
    s.moveDown();
    s.moveDown();
    expect(s.moveDown()).toBe(false);
    expect(s.selected).toBe(2);
  });

  it("cannot move above 0", () => {
    const s = new ScrollState(5, 3);
    expect(s.moveUp()).toBe(false);
    expect(s.selected).toBe(0);
  });

  it("scrolls viewport when selection exceeds bottom", () => {
    const s = new ScrollState(10, 3);
    s.moveDown(); // 1
    s.moveDown(); // 2
    s.moveDown(); // 3 -- should scroll
    expect(s.selected).toBe(3);
    expect(s.offset).toBe(1);
  });

  it("scrolls viewport up when selection moves above offset", () => {
    const s = new ScrollState(10, 3);
    s.setSelected(5);
    expect(s.offset).toBe(3);
    s.moveUp();
    s.moveUp();
    s.moveUp(); // selected=2, offset should adjust
    expect(s.selected).toBe(2);
    expect(s.offset).toBe(2);
  });

  it("handles empty list", () => {
    const s = new ScrollState(0, 5);
    expect(s.selected).toBe(0);
    expect(s.moveDown()).toBe(false);
    expect(s.moveUp()).toBe(false);
  });

  it("handles list shorter than viewport", () => {
    const s = new ScrollState(3, 10);
    s.moveDown();
    s.moveDown();
    expect(s.offset).toBe(0);
    expect(s.selected).toBe(2);
  });

  it("setTotal clamps selection", () => {
    const s = new ScrollState(10, 5);
    s.setSelected(8);
    s.setTotal(5);
    expect(s.selected).toBe(4);
  });

  it("visibleRange returns correct bounds", () => {
    const s = new ScrollState(10, 3);
    s.setSelected(5);
    const range = s.visibleRange;
    expect(range.end - range.start).toBe(3);
    expect(range.start).toBe(s.offset);
  });
});
