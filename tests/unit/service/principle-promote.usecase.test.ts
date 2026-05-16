import { describe, expect, it } from "bun:test";
import {
  CorrectionNotFoundError,
  CorrectionNotLintViolationError,
  principlePromote,
  renderPrincipleScaffold,
  ruleIdToSlug,
} from "@/service/principle-promote.usecase.js";
import type {
  EvidenceRow,
  EvidenceStorePort,
  LintViolationEvidenceRow,
} from "@/repo/evidence-store.port.js";
import type { PrinciplesStorePort } from "@/repo/principles-store.port.js";
import type { Principle } from "@/types/principle.js";

function memEvidence(rows: EvidenceRow[]): EvidenceStorePort {
  return {
    append: async () => {},
    list: async () => rows,
  };
}

function memPrinciples(existing: string[] = []): {
  store: PrinciplesStorePort;
  written: Array<{ slug: string; content: string }>;
} {
  const written: Array<{ slug: string; content: string }> = [];
  const set = new Set(existing);
  const store: PrinciplesStorePort = {
    list: async () => [],
    get: async (slug) => {
      if (set.has(slug)) {
        return {
          slug,
          rule: "",
          rationale: "",
          scan_command: "",
          fix_recipe: "",
        } as Principle;
      }
      return undefined;
    },
    exists: async (slug) => set.has(slug),
    write: async (slug, content) => {
      written.push({ slug, content });
      set.add(slug);
    },
  };
  return { store, written };
}

const ROW: LintViolationEvidenceRow = {
  id: "evd-abc-001",
  kind: "lint-violation",
  timestamp: "2026-05-15T10:00:00Z",
  rule_id: "prefer_shared_utils",
  severity: "error",
  file: "src/features/x/foo.ts",
  line: 42,
  message: "duplicate helper detected",
  remediation: "Move into src/shared/lib/<helper>.ts.",
};

describe("ruleIdToSlug", () => {
  it("replaces underscores with hyphens", () => {
    expect(ruleIdToSlug("prefer_shared_utils")).toBe("prefer-shared-utils");
  });
  it("lower-cases input", () => {
    expect(ruleIdToSlug("Layer_Order")).toBe("layer-order");
  });
  it("trims whitespace", () => {
    expect(ruleIdToSlug("  rule  ")).toBe("rule");
  });
});

describe("renderPrincipleScaffold", () => {
  it("emits all 4 required sections", () => {
    const md = renderPrincipleScaffold("prefer-shared-utils", ROW);
    expect(md).toContain("# prefer-shared-utils");
    expect(md).toContain("## Rule");
    expect(md).toContain("duplicate helper detected");
    expect(md).toContain("## Rationale");
    expect(md).toContain("evd-abc-001");
    expect(md).toContain("src/features/x/foo.ts:42");
    expect(md).toContain("## Scan Command");
    expect(md).toContain("## Fix Recipe");
    expect(md).toContain("Move into src/shared/lib");
  });

  it("falls back when remediation is absent", () => {
    const stripped: LintViolationEvidenceRow = { ...ROW, remediation: undefined };
    const md = renderPrincipleScaffold("x", stripped);
    expect(md).toContain("Investigate the original correction");
  });
});

describe("principlePromote", () => {
  it("writes the principle markdown and returns the path", async () => {
    const { store, written } = memPrinciples();
    const result = await principlePromote(
      { evidenceStore: memEvidence([ROW]), principlesStore: store },
      { correction_id: "evd-abc-001" },
    );
    expect(result.slug).toBe("prefer-shared-utils");
    expect(result.path).toBe("docs/principles/prefer-shared-utils.md");
    expect(result.rule_id).toBe("prefer_shared_utils");
    expect(written).toHaveLength(1);
    expect(written[0]?.slug).toBe("prefer-shared-utils");
    expect(written[0]?.content).toContain("# prefer-shared-utils");
  });

  it("collision-suffix -2 when base slug already exists", async () => {
    const { store, written } = memPrinciples(["prefer-shared-utils"]);
    const result = await principlePromote(
      { evidenceStore: memEvidence([ROW]), principlesStore: store },
      { correction_id: "evd-abc-001" },
    );
    expect(result.slug).toBe("prefer-shared-utils-2");
    expect(written[0]?.slug).toBe("prefer-shared-utils-2");
  });

  it("collision-suffix walks until -3 when -2 also exists", async () => {
    const { store } = memPrinciples([
      "prefer-shared-utils",
      "prefer-shared-utils-2",
    ]);
    const result = await principlePromote(
      { evidenceStore: memEvidence([ROW]), principlesStore: store },
      { correction_id: "evd-abc-001" },
    );
    expect(result.slug).toBe("prefer-shared-utils-3");
  });

  it("throws CorrectionNotFoundError when id missing", async () => {
    const { store } = memPrinciples();
    await expect(
      principlePromote(
        { evidenceStore: memEvidence([ROW]), principlesStore: store },
        { correction_id: "evd-nope" },
      ),
    ).rejects.toThrow(CorrectionNotFoundError);
  });

  it("throws CorrectionNotLintViolationError when kind is transition", async () => {
    const transition: EvidenceRow = {
      id: "evd-xyz",
      kind: "transition",
      timestamp: "2026-05-15T10:00:00Z",
      from_state: null,
      to_state: "draft",
      trigger_verb: "task:from-spec",
    };
    const { store } = memPrinciples();
    await expect(
      principlePromote(
        { evidenceStore: memEvidence([transition]), principlesStore: store },
        { correction_id: "evd-xyz" },
      ),
    ).rejects.toThrow(CorrectionNotLintViolationError);
  });

  it("supports rule_id without underscores (already kebab-able)", async () => {
    const row = { ...ROW, id: "evd-xyz", rule_id: "layer-order" };
    const { store, written } = memPrinciples();
    const result = await principlePromote(
      { evidenceStore: memEvidence([row]), principlesStore: store },
      { correction_id: "evd-xyz" },
    );
    expect(result.slug).toBe("layer-order");
    expect(written[0]?.slug).toBe("layer-order");
  });
});
