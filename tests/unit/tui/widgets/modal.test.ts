import { describe, expect, it } from "bun:test";
import { Buffer } from "../../../../src/tui/terminal/buffer.js";
import { applyModalBackdrop, renderModal } from "../../../../src/tui/widgets/modal.js";
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

  it("renders a full-width cyan selection row for menu items", () => {
    const buf = new Buffer(80, 24);
    const layout = renderModal(buf, { x: 0, y: 0, width: 80, height: 24 }, {
      mode: "menu",
      title: "Select",
      items: ["A", "B", "C"],
      selectedIndex: 1,
    });

    const selectedRow = layout.itemRects[1]!;
    expect(buf.getCell(selectedRow.y, selectedRow.x + 2)?.bg).toBe(PALETTE.overlaySelectedBg);
    expect(buf.getCell(selectedRow.y, selectedRow.x + 2)?.fg).toBe(PALETTE.overlaySelectedFg);
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

  it("uses overlay surface and selection theme tokens", () => {
    const buf = new Buffer(80, 24);
    const modalRect = renderModal(buf, { x: 0, y: 0, width: 80, height: 24 }, {
      mode: "menu",
      title: "Configure database",
      eyebrow: "f2 · Database config",
      items: ["Set status to assigned", "Set status to in-progress"],
      selectedIndex: 0,
    });

    expect(buf.getCell(modalRect.y, modalRect.x)?.bg).toBe(PALETTE.overlaySurfaceBg);

    const selectedRowCell = buf.getCell(modalRect.y + 4, modalRect.x + 2);
    expect(selectedRowCell?.bg).toBe(PALETTE.overlaySelectedBg);
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

  it("returns stable row hit boxes for selectable menu items", () => {
    const buf = new Buffer(80, 24);
    const layout = renderModal(buf, { x: 0, y: 0, width: 80, height: 24 }, {
      mode: "menu",
      title: "Select",
      items: ["A", "B", "C"],
      selectedIndex: 1,
      footer: "Esc close",
    }) as unknown as {
      itemRects: Array<{ x: number; y: number; width: number; height: number }>;
    };

    expect(layout.itemRects.length).toBe(3);
    expect(layout.itemRects[1]?.height).toBe(1);
    expect(layout.itemRects[1]?.width).toBeGreaterThan(8);
  });

  it("returns no selectable hit boxes for info cards", () => {
    const buf = new Buffer(80, 24);
    const layout = renderModal(buf, { x: 0, y: 0, width: 80, height: 24 }, {
      mode: "info",
      title: "Mission Directory",
      items: [{ text: ".maestro/missions/2026-03-31-001", style: "block", tone: "accent" }],
      footer: "Esc close",
    }) as unknown as {
      itemRects: Array<{ x: number; y: number; width: number; height: number }>;
    };

    expect(layout.itemRects).toEqual([]);
  });

  it("applies a dimmed backdrop behind overlays", () => {
    const buf = new Buffer(20, 6);
    buf.writeText(1, 1, "Mission", {
      fg: PALETTE.brightWhite,
      bg: PALETTE.panelBg,
      bold: true,
    });

    applyModalBackdrop(buf);

    const cell = buf.getCell(1, 1);
    expect(cell?.bg).toBe(PALETTE.overlayBackdropBg);
    expect(cell?.fg).toBe(PALETTE.gray);
    expect(cell?.bold).toBe(false);
    expect(cell?.dim).toBe(true);
  });

  it("renders palette rows with a query line, sections, and shortcut hints", () => {
    const buf = new Buffer(90, 28);
    renderModal(buf, { x: 0, y: 0, width: 90, height: 28 }, {
      mode: "palette",
      title: "Commands",
      query: "pro",
      items: [
        {
          label: "Processes",
          detail: "List live Maestro runtime work for this mission",
          hint: "P",
          section: "Navigate",
        },
        {
          label: "Exit",
          detail: "Close Mission Control cleanly",
          hint: "Ctrl+T",
          section: "Session",
        },
      ],
      selectedIndex: 0,
      footer: "Enter open · Esc close",
    });

    const text = buf.toString();
    expect(text).toContain("Commands");
    expect(text).toContain("pro");
    expect(text).toContain("Navigate");
    expect(text).toContain("Processes");
    expect(text).toContain("Ctrl+T");
    expect(text).toContain("esc");
  });
});
