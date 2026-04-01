import { describe, expect, it } from "bun:test";
import { Buffer } from "../../../../src/tui/terminal/buffer.js";
import {
  applyModalBackdrop,
  buildOverlayRenderSpec,
  layoutModal,
  renderModal,
} from "../../../../src/tui/widgets/modal.js";
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
      renderSpec: buildOverlayRenderSpec("feature-action"),
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
        renderSpec: buildOverlayRenderSpec("feature-action"),
      });

    const selectedRow = layout.itemRects[1]!;
    expect(buf.getCell(selectedRow.y, selectedRow.x + 2)?.bg).toBe(PALETTE.overlaySelectedBg);
    expect(buf.getCell(selectedRow.y, selectedRow.x + 2)?.fg).toBe(PALETTE.overlaySelectedFg);
  });

    it("renders the palette with an amber selection row and centered title", () => {
    const buf = new Buffer(90, 28);
      const layout = renderModal(buf, { x: 0, y: 0, width: 90, height: 28 }, {
        mode: "palette",
        title: "Command Palette",
        query: "han",
      items: [
        {
          label: "Handoffs",
          detail: "Review pending cross-agent handoffs",
          hint: "H",
          section: "Navigate",
        },
        {
          label: "Runtime",
          detail: "List live Maestro runtime work for this mission",
          hint: "P",
          section: "Navigate",
        },
        ],
        selectedIndex: 0,
        footer: "Enter open · Esc close",
        renderSpec: buildOverlayRenderSpec("command-palette"),
      });

      const titleRow = buf.toString().split("\n")[layout.y + 1] ?? "";
      expect(titleRow).toContain("Command Palette");
      expect(buf.getCell(layout.itemRects[0]!.y, layout.itemRects[0]!.x + 2)?.bg).toBe(PALETTE.yellow);
      expect(buf.getCell(layout.itemRects[0]!.y, layout.itemRects[0]!.x + 2)?.fg).toBe(PALETTE.headerBg);
      expect(buf.getCell(layout.itemRects[1]!.y, layout.itemRects[1]!.x + 2)?.dim).toBe(false);
    });

  it("renders status line when provided", () => {
    const buf = new Buffer(80, 24);
      renderModal(buf, { x: 0, y: 0, width: 80, height: 24 }, {
        mode: "info",
        title: "Test",
        items: [{ text: "Item", style: "block" }],
        footer: "Esc close",
        renderSpec: buildOverlayRenderSpec("config"),
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
        renderSpec: buildOverlayRenderSpec("feature-action"),
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
        renderSpec: buildOverlayRenderSpec("config"),
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
        renderSpec: buildOverlayRenderSpec("config"),
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
        renderSpec: buildOverlayRenderSpec("feature-action"),
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
          renderSpec: buildOverlayRenderSpec("feature-action"),
        });

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
          renderSpec: buildOverlayRenderSpec("config"),
        });

      expect(layout.itemRects).toEqual([]);
      expect(layout.itemRowIndexes).toEqual([]);
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
      const layout = renderModal(buf, { x: 0, y: 0, width: 90, height: 28 }, {
        mode: "palette",
        title: "Command Palette",
        query: "pro",
      items: [
        {
          label: "Runtime",
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
        renderSpec: buildOverlayRenderSpec("command-palette"),
      });

    const text = buf.toString();
    expect(text).toContain("Command Palette");
    expect(text).toContain("> pro");
    expect(text).toContain("navigate");
    expect(text).toContain("runtime");
    expect(text).toContain("Ctrl+T");
    expect(text).toContain("esc");
    expect(text).not.toContain("Enter open · Esc close");
    expect(buf.getCell(layout.y, layout.x)?.bg).toBe(PALETTE.overlaySurfaceBg);
  });

  it("sanitizes menu detail text before it reaches the terminal buffer", () => {
    const buf = new Buffer(90, 28);
      renderModal(buf, { x: 0, y: 0, width: 90, height: 28 }, {
        mode: "menu",
        title: "Runtime",
        items: [
        {
          label: "Inspect runtime",
          detail: "\u001b[2Jruntime failed\u0007",
          hint: "P",
          section: "Navigate",
        },
        ],
        selectedIndex: 0,
        footer: "Esc close",
        renderSpec: buildOverlayRenderSpec("feature-action"),
      });

    const text = buf.toString();
    expect(text).toContain("runtime failed");
    expect(text).not.toContain("\u001b");
    expect(text).not.toContain("\u0007");
    expect(text).not.toContain("[2J");
  });

  it("keeps at least one selectable row in short but valid palette heights", () => {
      const layout = layoutModal({ x: 1, y: 5, width: 78, height: 8 }, {
        mode: "palette",
        title: "Command Palette",
        query: "",
      items: [
        {
          label: "Tasks",
          detail: "Browse mission features and focus a specific item",
          hint: "F",
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
        renderSpec: buildOverlayRenderSpec("command-palette"),
      });

    expect(layout.contentRect.height).toBeGreaterThan(0);
    expect(layout.itemRects.length).toBeGreaterThan(0);
  });

    it("renders split overlays with selectable left rows and live detail content", () => {
      const buf = new Buffer(100, 30);
        const layout = renderModal(buf, { x: 0, y: 0, width: 100, height: 30 }, {
          mode: "split",
          title: "Dependencies",
          eyebrow: "Bug hunt auth flow",
        items: [
          { label: "1. f3 Implement API endpoints", section: "Upstream" },
          { label: "status: open", selectable: false, tone: "muted" },
          { label: "2. f5 Verify migrations", section: "Downstream" },
          { label: "status: review", selectable: false, tone: "muted" },
          { label: "blocked by: 2 open dependencies", section: "Summary", selectable: false, tone: "muted" },
          ],
          selectedIndex: 1,
          detailItems: [
            { text: "Graph", section: "Graph", tone: "accent" },
            { text: "f6 Bug hunt auth flow [BLOCKED]" },
            { text: "├─ blocked by f3 [OPEN]" },
            { text: "└─ blocked by f5 [REVIEW]" },
          ],
          footer: "Enter jump · Left back · Esc close",
          renderSpec: buildOverlayRenderSpec("dependencies"),
        });

      expect(layout.itemRects.length).toBe(2);
      expect(layout.itemRowIndexes).toEqual([0, 2]);
      expect(layout.detailRect).toBeDefined();
      expect(buf.toString()).toContain("Upstream");
      expect(buf.toString()).toContain("Graph");
      expect(buf.toString()).toContain("blocked by f5");
      expect(buf.getCell(layout.itemRects[1]!.y, layout.itemRects[1]!.x + 2)?.bg).toBe(PALETTE.yellow);
    });

      it("omits hit boxes for non-selectable split rows", () => {
        const layout = layoutModal({ x: 0, y: 0, width: 100, height: 30 }, {
          mode: "split",
          title: "Runtime",
          items: [
          { label: "Pending", selectable: false, section: "Summary" },
          { label: "f2 · Configure database" },
          { label: "status: live", selectable: false, tone: "muted" },
        ],
        selectedIndex: 0,
          detailItems: [
            { text: "Configure database", tone: "accent" },
            { text: "agent", detail: "codex" },
          ],
          footer: "Enter inspect · Esc close",
          renderSpec: buildOverlayRenderSpec("processes"),
        });

        expect(layout.itemRects.length).toBe(1);
        expect(layout.itemRowIndexes).toEqual([1]);
        expect(layout.detailRect?.width ?? 0).toBeGreaterThan(10);
      });

      it("uses the standard shell size for palette, dependencies, runtime, and config", () => {
        const parent = { x: 0, y: 0, width: 120, height: 40 };
        const palette = layoutModal(parent, {
          mode: "palette",
          title: "Command Palette",
          query: "",
          items: [{ label: "Tasks", hint: "F", section: "Navigate" }],
          selectedIndex: 0,
          renderSpec: buildOverlayRenderSpec("command-palette"),
        });
        const dependencies = layoutModal(parent, {
          mode: "split",
          title: "Dependencies",
          items: [{ label: "f2 Configure database", section: "Upstream" }],
          selectedIndex: 0,
          detailItems: [{ text: "Graph", section: "Graph" }],
          renderSpec: buildOverlayRenderSpec("dependencies"),
        });
        const runtime = layoutModal(parent, {
          mode: "split",
          title: "Runtime",
          items: [{ label: "f2 Configure database" }],
          selectedIndex: 0,
          detailItems: [{ text: "agent", detail: "codex" }],
          renderSpec: buildOverlayRenderSpec("processes"),
        });
        const config = layoutModal(parent, {
          mode: "info",
          title: "Config",
          items: [{ text: "Git available", section: "Environment" }],
          renderSpec: buildOverlayRenderSpec("config"),
        });

        expect(palette.width).toBe(76);
        expect(palette.height).toBe(20);
        expect(dependencies.width).toBe(76);
        expect(dependencies.height).toBe(20);
        expect(runtime.width).toBe(76);
        expect(runtime.height).toBe(20);
        expect(config.width).toBe(76);
        expect(config.height).toBe(20);
      });

      it("keeps handoffs wider and applies per-overlay split ratios", () => {
        const parent = { x: 0, y: 0, width: 120, height: 40 };
        const dependencies = layoutModal(parent, {
          mode: "split",
          title: "Dependencies",
          items: [{ label: "f2 Configure database", section: "Upstream" }],
          selectedIndex: 0,
          detailItems: [{ text: "Graph", section: "Graph" }],
          renderSpec: buildOverlayRenderSpec("dependencies"),
        });
        const runtime = layoutModal(parent, {
          mode: "split",
          title: "Runtime",
          items: [{ label: "f2 Configure database" }],
          selectedIndex: 0,
          detailItems: [{ text: "agent", detail: "codex" }],
          renderSpec: buildOverlayRenderSpec("processes"),
        });
        const handoffs = layoutModal(parent, {
          mode: "split",
          title: "Handoffs",
          items: [{ label: "h-12 · claude-code" }],
          selectedIndex: 0,
          detailItems: [{ text: "message", detail: "Need review" }],
          renderSpec: buildOverlayRenderSpec("handoffs"),
        });

        const dependenciesLeftWidth = dependencies.itemRects[0]?.width ?? 0;
        const runtimeLeftWidth = runtime.itemRects[0]?.width ?? 0;
        const handoffLeftWidth = handoffs.itemRects[0]?.width ?? 0;

        expect(handoffs.width).toBe(94);
        expect(handoffs.height).toBe(20);
        expect(runtimeLeftWidth).toBeLessThan(dependenciesLeftWidth);
        expect(handoffLeftWidth).toBeLessThan(handoffs.width / 2);
        expect((handoffs.detailRect?.width ?? 0)).toBeGreaterThan(dependencies.detailRect?.width ?? 0);
      });
    });
