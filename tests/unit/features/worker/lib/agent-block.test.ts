import { describe, expect, it } from "bun:test";
import {
  wrapBlock,
  hasBlock,
  extractBlock,
  injectBlock,
  replaceBlock,
  removeBlock,
  removeLegacyBlock,
} from "@/features/worker";

const SAMPLE = "## Cross-Agent Handoff (maestro)\n\nPick up a handoff.";
const WRAPPED = `<!-- maestro:start -->\n${SAMPLE}\n<!-- maestro:end -->`;

describe("wrapBlock", () => {
  it("wraps content with markers", () => {
    expect(wrapBlock(SAMPLE)).toBe(WRAPPED);
  });
});

describe("hasBlock", () => {
  it("returns true when markers present", () => {
    expect(hasBlock(`Some content\n\n${WRAPPED}\n`)).toBe(true);
  });

  it("returns false when no markers", () => {
    expect(hasBlock("Just some markdown\n")).toBe(false);
  });
});

describe("extractBlock", () => {
  it("returns content between markers", () => {
    expect(extractBlock(`Prefix\n\n${WRAPPED}\n\nSuffix`)).toBe(SAMPLE);
  });

  it("returns null when no block", () => {
    expect(extractBlock("No markers here")).toBeNull();
  });
});

describe("injectBlock", () => {
  it("appends to existing content with double newline", () => {
    const result = injectBlock("# My Config\n\nSome stuff", SAMPLE);
    expect(result).toBe(`# My Config\n\nSome stuff\n\n${WRAPPED}\n`);
  });

  it("handles empty content", () => {
    const result = injectBlock("", SAMPLE);
    expect(result).toBe(`${WRAPPED}\n`);
  });

  it("handles whitespace-only content", () => {
    const result = injectBlock("  \n\n  ", SAMPLE);
    expect(result).toBe(`${WRAPPED}\n`);
  });

  it("trims trailing whitespace before appending", () => {
    const result = injectBlock("# Config\n\n\n\n", SAMPLE);
    expect(result).toBe(`# Config\n\n${WRAPPED}\n`);
  });
});

describe("replaceBlock", () => {
  it("replaces existing block with new content", () => {
    const content = `# Config\n\n${WRAPPED}\n\n## Other`;
    const newBlock = "## Updated\n\nNew content.";
    const result = replaceBlock(content, newBlock);
    expect(result).toContain("<!-- maestro:start -->");
    expect(result).toContain("New content.");
    expect(result).not.toContain("Pick up a handoff.");
    expect(result).toContain("## Other");
  });

  it("returns null when no block exists", () => {
    expect(replaceBlock("No block here", "new")).toBeNull();
  });
});

describe("removeBlock", () => {
  it("removes the block and cleans whitespace", () => {
    const content = `# Config\n\n${WRAPPED}\n\n## Other`;
    const result = removeBlock(content);
    expect(result).not.toContain("maestro:start");
    expect(result).toContain("# Config");
    expect(result).toContain("## Other");
  });

  it("returns null when no block", () => {
    expect(removeBlock("No block")).toBeNull();
  });

  it("handles block at end of file", () => {
    const content = `# Config\n\n${WRAPPED}`;
    const result = removeBlock(content);
    expect(result).toBe("# Config\n");
    expect(result).not.toContain("maestro");
  });
});

describe("removeLegacyBlock", () => {
  it("removes unmarked legacy section", () => {
    const content = `# Config\n\n## Cross-Agent Handoff (maestro)\n\nOld stuff here.\nMore old stuff.\n\n## Other Section`;
    const result = removeLegacyBlock(content);
    expect(result).not.toContain("Cross-Agent Handoff");
    expect(result).not.toContain("Old stuff");
    expect(result).toContain("# Config");
    expect(result).toContain("## Other Section");
  });

  it("removes legacy section at end of file", () => {
    const content = `# Config\n\n## Cross-Agent Handoff (maestro)\n\nOld stuff here.`;
    const result = removeLegacyBlock(content);
    expect(result).not.toContain("Cross-Agent Handoff");
    expect(result).toContain("# Config");
  });

  it("returns null when no legacy section", () => {
    expect(removeLegacyBlock("# No legacy here\n")).toBeNull();
  });

  it("does not remove marked blocks", () => {
    const content = `# Config\n\n${WRAPPED}\n`;
    expect(removeLegacyBlock(content)).toBeNull();
  });
});
