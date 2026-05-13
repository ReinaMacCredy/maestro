import { describe, expect, it } from "bun:test";
import {
  checkTocBudget,
  assertTocBudget,
  DEFAULT_TOC_BUDGET,
} from "@/features/setup";

describe("checkTocBudget", () => {
  it("reports ok at small sizes", () => {
    const r = checkTocBudget("a\nb\nc\n");
    expect(r.status).toBe("ok");
    expect(r.lines).toBe(3);
  });

  it("reports warn between warn and hard limit", () => {
    const content = Array.from({ length: 150 }, (_, i) => `line ${i}`).join("\n");
    const r = checkTocBudget(content);
    expect(r.status).toBe("warn");
  });

  it("reports exceeded above the hard limit", () => {
    const content = Array.from({ length: 170 }, (_, i) => `line ${i}`).join("\n");
    const r = checkTocBudget(content);
    expect(r.status).toBe("exceeded");
  });

  it("respects custom budget", () => {
    const r = checkTocBudget("a\nb\nc\n", { hardLimit: 2, warnLimit: 1 });
    expect(r.status).toBe("exceeded");
  });
});

describe("assertTocBudget", () => {
  it("throws when content exceeds hard limit", () => {
    const content = Array.from({ length: 180 }, (_, i) => `line ${i}`).join("\n");
    expect(() => assertTocBudget("AGENTS.md", content)).toThrow(/exceeds TOC size budget/);
  });

  it("does not throw at warn level", () => {
    const content = Array.from({ length: 150 }, (_, i) => `line ${i}`).join("\n");
    expect(() => assertTocBudget("AGENTS.md", content)).not.toThrow();
  });

  it("exposes default budget", () => {
    expect(DEFAULT_TOC_BUDGET.hardLimit).toBe(160);
    expect(DEFAULT_TOC_BUDGET.warnLimit).toBe(140);
  });
});
