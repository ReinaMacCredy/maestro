import { describe, expect, it } from "bun:test";
import { Buffer } from "../../../../src/tui/terminal/buffer.js";
import { renderModal } from "../../../../src/tui/widgets/modal.js";

describe("renderModal", () => {
  it("renders centered within parent rect", () => {
    const buf = new Buffer(80, 24);
    const modalRect = renderModal(buf, { x: 0, y: 0, width: 80, height: 24 }, {
      title: "Test Modal",
      items: ["Option 1", "Option 2", "Option 3"],
      selectedIndex: 0,
    });

    // Modal should be centered
    expect(modalRect.x).toBeGreaterThan(0);
    expect(modalRect.y).toBeGreaterThan(0);

    const text = buf.toString();
    expect(text).toContain("Test Modal");
    expect(text).toContain("Option 1");
    expect(text).toContain("Option 2");
    expect(text).toContain("Option 3");
  });

  it("shows selection indicator", () => {
    const buf = new Buffer(80, 24);
    renderModal(buf, { x: 0, y: 0, width: 80, height: 24 }, {
      title: "Select",
      items: ["A", "B", "C"],
      selectedIndex: 1,
    });

    const text = buf.toString();
    expect(text).toContain("> B");
  });

  it("renders status line when provided", () => {
    const buf = new Buffer(80, 24);
    renderModal(buf, { x: 0, y: 0, width: 80, height: 24 }, {
      title: "Test",
      items: ["Item"],
      selectedIndex: 0,
      statusLine: "Press Escape to close",
    });

    const text = buf.toString();
    expect(text).toContain("Press Escape");
  });

  it("handles empty items gracefully", () => {
    const buf = new Buffer(80, 24);
    const modalRect = renderModal(buf, { x: 0, y: 0, width: 80, height: 24 }, {
      title: "Empty",
      items: [],
      selectedIndex: 0,
    });

    expect(modalRect.width).toBeGreaterThan(0);
    expect(modalRect.height).toBeGreaterThan(0);
  });
});
