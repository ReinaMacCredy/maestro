import { describe, expect, it } from "bun:test";
import { checkScope } from "@/features/verify/usecases/checks/check-scope.js";
import type { Contract } from "@/features/task/domain/contract/contract-types.js";
import { CONTRACT_SCHEMA_VERSION } from "@/features/task/domain/contract/contract-types.js";

function makeContract(overrides: Partial<Contract> = {}): Contract {
  return {
    schemaVersion: CONTRACT_SCHEMA_VERSION,
    id: "c-001",
    taskId: "tsk-001",
    repoRoot: "/repo",
    status: "locked",
    createdAt: "2026-01-01T00:00:00.000Z",
    intent: "test",
    scope: {
      filesExpected: ["src/**"],
      filesForbidden: [],
    },
    doneWhen: [],
    amendments: [],
    createdBy: "session:test:1",
    configSnapshot: {
      strict: false,
      overlapPolicy: "fail",
      rebaseFallback: "best-effort",
      staleReclaimContractPolicy: "inherit",
    },
    ...overrides,
  };
}

describe("checkScope", () => {
  it("clean diff — all paths within scope — returns empty findings", () => {
    const contract = makeContract({
      scope: { filesExpected: ["src/**"], filesForbidden: [] },
    });
    const findings = checkScope(["src/foo.ts", "src/bar.ts"], contract);
    expect(findings).toEqual([]);
  });

  it("out-of-scope path — emits error finding", () => {
    const contract = makeContract({
      scope: { filesExpected: ["src/**"], filesForbidden: [] },
    });
    const findings = checkScope(["docs/README.md"], contract);
    expect(findings).toHaveLength(1);
    expect(findings[0].check).toBe("scope");
    expect(findings[0].severity).toBe("error");
    expect(findings[0].paths).toContain("docs/README.md");
  });

  it("forbidden path — emits error finding with forbidden details", () => {
    const contract = makeContract({
      scope: { filesExpected: ["src/**"], filesForbidden: [".env", "secrets/**"] },
    });
    const findings = checkScope([".env", "src/foo.ts"], contract);
    expect(findings).toHaveLength(1);
    expect(findings[0].check).toBe("scope");
    expect(findings[0].severity).toBe("error");
    expect(findings[0].paths).toContain(".env");
    expect(findings[0].details).toMatch(/filesForbidden/);
  });

  it("filesExpected=[**] allows all paths", () => {
    const contract = makeContract({
      scope: { filesExpected: ["**"], filesForbidden: [] },
    });
    const findings = checkScope(["anywhere/file.ts", "docs/x.md"], contract);
    expect(findings).toEqual([]);
  });

  it("filesExpected=[] allows all paths", () => {
    const contract = makeContract({
      scope: { filesExpected: [], filesForbidden: [] },
    });
    const findings = checkScope(["arbitrary.ts"], contract);
    expect(findings).toEqual([]);
  });

  it("paths under .maestro/ are exempt — substrate metadata is not user code", () => {
    // The diff between lock-commit and HEAD always contains the substrate's
    // own bookkeeping (contract files, tasks.jsonl, NOW.md). Gating those
    // against the user's `src/**` scope is a false positive that breaks
    // brownfield workflows.
    const contract = makeContract({
      scope: { filesExpected: ["src/**"], filesForbidden: [".maestro/policies/**"] },
    });
    const findings = checkScope(
      [
        "src/foo.ts",
        ".maestro/contracts/tsk-001/v1.json",
        ".maestro/tasks/NOW.md",
        ".maestro/tasks/tasks.jsonl",
        ".maestro/policies/risk.yaml",
      ],
      contract,
    );
    // Even an explicitly-forbidden .maestro/** path is exempt — substrate
    // changes are produced by the maestro CLI itself, not the user's task.
    expect(findings).toEqual([]);
  });

  it("path matching both filesExpected and filesForbidden — forbidden wins", () => {
    const contract = makeContract({
      scope: { filesExpected: ["src/**"], filesForbidden: ["src/secret.ts"] },
    });
    const findings = checkScope(["src/secret.ts"], contract);
    // forbidden match takes priority; no out-of-scope finding
    const forbiddenFindings = findings.filter((f) => f.paths.includes("src/secret.ts"));
    expect(forbiddenFindings).toHaveLength(1);
    expect(forbiddenFindings[0].details).toMatch(/filesForbidden/);
  });
});
