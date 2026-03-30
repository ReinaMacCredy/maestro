import { describe, expect, it } from "bun:test";
import { Buffer } from "../../../../src/tui/terminal/buffer.js";
import { renderModal } from "../../../../src/tui/widgets/modal.js";
import { PALETTE } from "../../../../src/tui/theme.js";

describe("renderModal", () => {
  it("renders centered within parent rect", () => {
    const buf = new Buffer(80, 24);
    const modalRect = renderModal(buf, { x: 0, y: 0, width: 80, height: 24 }, {
      mode: "menu",
      title: "Test Modal",
      eyebrow: "Pick an action",
      items: ["Option 1", "Option 2", "Option 3"],
      selectedIndex: 0,
      footer: "Esc close",
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
      mode: "menu",
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
      mode: "info",
      title: "Test",
      items: [{ text: "Item", style: "block" }],
      footer: "Esc close",
    });

    const text = buf.toString();
    expect(text).toContain("Esc close");
  });

  it("uses theme tokens for the border and selected row", () => {
    const buf = new Buffer(80, 24);
    const modalRect = renderModal(buf, { x: 0, y: 0, width: 80, height: 24 }, {
      mode: "menu",
      title: "Configure database",
      eyebrow: "f2 · Database config",
      items: ["Set status to assigned", "Set status to in-progress"],
      selectedIndex: 0,
    });

    expect(buf.getCell(modalRect.y, modalRect.x)?.fg).toBe(PALETTE.border);

    const selectedRowCell = buf.getCell(modalRect.y + 4, modalRect.x + 2);
    expect(selectedRowCell?.bg).toBe(PALETTE.selectedBg);
  });

  it("renders info cards without a selection chevron", () => {
    const buf = new Buffer(80, 24);
    renderModal(buf, { x: 0, y: 0, width: 80, height: 24 }, {
      mode: "info",
      title: "Mission Directory",
      eyebrow: "Project-local runtime path",
      items: [{ text: ".maestro/missions/2026-03-30-030", style: "block", tone: "accent" }],
      footer: "Esc close",
    });

    const text = buf.toString();
    expect(text).toContain("Mission Directory");
    expect(text).toContain(".maestro/missions/2026-03-30-030");
    expect(text).not.toContain("> ");
  });

  it("preserves the trailing mission id when truncating long paths", () => {
    const buf = new Buffer(50, 20);
    renderModal(buf, { x: 0, y: 0, width: 50, height: 20 }, {
      mode: "info",
      title: "Mission Directory",
      items: [{
        text: "/very/long/project/path/.maestro/missions/2026-03-30-030",
        style: "block",
        tone: "accent",
      }],
      footer: "Esc close",
    });

    const text = buf.toString();
    expect(text).toContain("2026-03-30-030");
    expect(text).toContain("...");
  });

  it("handles empty menu items gracefully", () => {
    const buf = new Buffer(80, 24);
    const modalRect = renderModal(buf, { x: 0, y: 0, width: 80, height: 24 }, {
      mode: "menu",
      title: "Empty",
      items: [],
      selectedIndex: 0,
    });

    expect(modalRect.width).toBeGreaterThan(0);
    expect(modalRect.height).toBeGreaterThan(0);
  });
});
