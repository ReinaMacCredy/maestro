import { describe, expect, it } from "bun:test";
import {
  wrapBlock,
  hasBlock,
  extractBlock,
  injectBlock,
  replaceBlock,
  removeBlock,
  removeLegacyBlock,
  hasSetupBlock,
  injectSetupBlock,
  replaceSetupBlock,
  hasSetupReference,
  injectSetupReference,
} from "@/infra/lib/agent-block.js";

const SAMPLE = "## Cross-Agent Handoff (maestro)\n\nPick up a handoff.";
const WRAPPED = `<!-- maestro:start -->\n${SAMPLE}\n<!-- maestro:end -->`;
const SETUP_SAMPLE = "## Maestro\n\nProject wired into the harness.";
const SETUP_WRAPPED = `<!-- maestro-setup:start -->\n${SETUP_SAMPLE}\n<!-- maestro-setup:end -->`;

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

  // Regression: FIX-6 -- the non-global regex only replaced the first managed
  // block, leaving stale duplicates (e.g. from a merge-conflict resolution)
  // in place. A single-block fixture would still pass without the `g` flag;
  // two blocks is the minimum input that exposes the bug.
  it("rewrites ALL duplicate managed blocks (not just the first)", () => {
    const STALE = "## Stale\n\nOld payload one.";
    const STALE_WRAPPED = `<!-- maestro:start -->\n${STALE}\n<!-- maestro:end -->`;
    const content = `# Config\n\n${STALE_WRAPPED}\n\n## Middle\n\n${STALE_WRAPPED}\n\n## End`;
    const result = replaceBlock(content, "## Fresh\n\nNew payload.");
    expect(result).not.toContain("Old payload one.");
    // Both blocks rewritten -- the new payload should appear twice.
    const matches = result?.match(/New payload\./g);
    expect(matches?.length).toBe(2);
    expect(result).toContain("## Middle");
    expect(result).toContain("## End");
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

  // Regression: FIX-6 (category sibling of replaceBlock) -- removeBlock uses
  // the same _G regex. Two managed blocks must both be stripped.
  it("strips ALL duplicate managed blocks (not just the first)", () => {
    const content = `# Config\n\n${WRAPPED}\n\n## Middle\n\n${WRAPPED}\n\n## End`;
    const result = removeBlock(content);
    expect(result).not.toContain("maestro:start");
    expect(result).not.toContain("Pick up a handoff.");
    expect(result).toContain("# Config");
    expect(result).toContain("## Middle");
    expect(result).toContain("## End");
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

describe("hasSetupBlock", () => {
  it("returns true when setup markers present", () => {
    expect(hasSetupBlock(`Existing content\n\n${SETUP_WRAPPED}\n`)).toBe(true);
  });

  it("returns false when no markers", () => {
    expect(hasSetupBlock("Plain markdown\n")).toBe(false);
  });

  it("returns false when only legacy markers present", () => {
    expect(hasSetupBlock(`# Config\n\n${WRAPPED}\n`)).toBe(false);
  });
});

describe("injectSetupBlock", () => {
  it("appends to existing content with double newline", () => {
    const result = injectSetupBlock("# My Project\n\nCustom notes.", SETUP_SAMPLE);
    expect(result).toBe(`# My Project\n\nCustom notes.\n\n${SETUP_WRAPPED}\n`);
  });

  it("handles empty content", () => {
    expect(injectSetupBlock("", SETUP_SAMPLE)).toBe(`${SETUP_WRAPPED}\n`);
  });

  it("preserves the existing block when re-injecting (idempotency is callsite-checked)", () => {
    const seeded = `# Config\n\n${SETUP_WRAPPED}\n`;
    const result = injectSetupBlock(seeded, SETUP_SAMPLE);
    expect(result).toContain(SETUP_WRAPPED);
  });

  it("produces a block that hasBlock (legacy) does not see", () => {
    const result = injectSetupBlock("", SETUP_SAMPLE);
    expect(hasBlock(result)).toBe(false);
    expect(hasSetupBlock(result)).toBe(true);
  });
});

describe("replaceSetupBlock", () => {
  it("replaces an existing setup block in place", () => {
    const content = `# Config\n\n${SETUP_WRAPPED}\n\n## Other`;
    const newBody = "## Updated\n\nRicher content from the skill.";
    const result = replaceSetupBlock(content, newBody);
    expect(result).toContain("<!-- maestro-setup:start -->");
    expect(result).toContain("Richer content from the skill.");
    expect(result).not.toContain("Project wired into the harness.");
    expect(result).toContain("## Other");
  });

  it("returns null when no setup block exists", () => {
    expect(replaceSetupBlock("No block here", "new")).toBeNull();
  });

  it("returns null when only a legacy block exists", () => {
    expect(replaceSetupBlock(`# Config\n\n${WRAPPED}\n`, "new")).toBeNull();
  });

  // Regression: FIX-6 sibling -- the setup-block regex carries the same
  // `g`-flag fix; two duplicate setup blocks must both be rewritten.
  it("rewrites ALL duplicate setup blocks", () => {
    const content = `# Config\n\n${SETUP_WRAPPED}\n\n## Middle\n\n${SETUP_WRAPPED}\n\n## End`;
    const result = replaceSetupBlock(content, "## Fresh setup\n\nNew payload.");
    expect(result).not.toContain("Project wired into the harness.");
    const matches = result?.match(/New payload\./g);
    expect(matches?.length).toBe(2);
  });
});

describe("hasSetupReference", () => {
  it("returns true when @AGENTS.md present", () => {
    expect(hasSetupReference("@AGENTS.md\n")).toBe(true);
  });

  it("returns false when only @MAESTRO.md present", () => {
    expect(hasSetupReference("@MAESTRO.md\n")).toBe(false);
  });

  it("returns false when reference is missing", () => {
    expect(hasSetupReference("# CLAUDE.md\n\nNothing here.\n")).toBe(false);
  });
});

describe("injectSetupReference", () => {
  it("writes the reference into empty content", () => {
    expect(injectSetupReference("")).toBe("@AGENTS.md\n");
  });

  it("appends to existing content with double newline", () => {
    expect(injectSetupReference("# CLAUDE.md\n\nNotes.")).toBe("# CLAUDE.md\n\nNotes.\n\n@AGENTS.md\n");
  });

  it("is a no-op when reference already present", () => {
    const seeded = "# CLAUDE.md\n\n@AGENTS.md\n";
    expect(injectSetupReference(seeded)).toBe(seeded);
  });

  it("preserves an unrelated @other-doc.md line", () => {
    const result = injectSetupReference("@other-doc.md\n");
    expect(result).toContain("@other-doc.md");
    expect(result).toContain("@AGENTS.md");
  });
});
