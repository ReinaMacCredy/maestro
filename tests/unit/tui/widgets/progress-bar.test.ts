import { describe, expect, it } from "bun:test";
import { Buffer } from "../../../../src/tui/terminal/buffer.js";
import { renderProgressBar } from "../../../../src/tui/widgets/progress-bar.js";

describe("renderProgressBar", () => {
  it("renders 0% bar", () => {
    const buf = new Buffer(30, 1);
    renderProgressBar(buf, 0, 0, { ratio: 0, width: 12 });
    const text = buf.toString();
    expect(text).toContain("[");
    expect(text).toContain("]");
    expect(text).not.toContain("=");
  });

  it("renders 50% bar", () => {
    const buf = new Buffer(30, 1);
    renderProgressBar(buf, 0, 0, { ratio: 0.5, width: 12 });
    const text = buf.toString();
    expect(text).toContain("=");
    expect(text).toContain(" ");
  });

  it("renders 100% bar", () => {
    const buf = new Buffer(30, 1);
    renderProgressBar(buf, 0, 0, { ratio: 1, width: 12 });
    const text = buf.toString();
    const inner = text.slice(1, -1);
    expect(inner).not.toContain(" ");
  });

  it("returns 0 for width < 4", () => {
    const buf = new Buffer(30, 1);
    const written = renderProgressBar(buf, 0, 0, { ratio: 0.5, width: 3 });
    expect(written).toBe(0);
  });

  it("clamps ratio to 0-1", () => {
    const buf = new Buffer(30, 1);
    renderProgressBar(buf, 0, 0, { ratio: 1.5, width: 12 });
    const text = buf.toString();
    expect(text).toContain("[");
    expect(text).toContain("]");
  });
});
