import { describe, expect, it, beforeEach, afterEach } from "bun:test";
import { mkdtemp, mkdir, writeFile, rm } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { runTrustVerifier } from "@/features/verify/usecases/trust-verifier.js";
import type { TrustVerifierInput, TrustVerifierDeps } from "@/features/verify/usecases/trust-verifier.js";
import type { Contract } from "@/features/task/domain/contract/contract-types.js";
import { CONTRACT_SCHEMA_VERSION } from "@/features/task/domain/contract/contract-types.js";

let tmpDir: string;

beforeEach(async () => {
  tmpDir = await mkdtemp(join(tmpdir(), "trust-verifier-"));
  await mkdir(join(tmpDir, ".maestro", "policies"), { recursive: true });
});

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true });
});

function makeContract(overrides: Partial<Contract> = {}): Contract {
  return {
    schemaVersion: CONTRACT_SCHEMA_VERSION,
    id: "c-001",
    taskId: "tsk-001",
    repoRoot: tmpDir,
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

const stubProbe: TrustVerifierDeps["gitSignatureProbe"] = {
  showSignatureLog: async () => "",
};

describe("runTrustVerifier", () => {
  it("clean diff — zero findings", async () => {
    await writeFile(join(tmpDir, "package.json"), JSON.stringify({ scripts: {} }));
    await writeFile(join(tmpDir, "bun.lock"), "");

    const input: TrustVerifierInput = {
      contract: makeContract({ scope: { filesExpected: ["**"], filesForbidden: [] } }),
      diff: {
        changedPaths: ["src/foo.ts", "bun.lock", "package.json"],
        addedLines: ["+const x = 1;"],
        base: "base-sha",
        head: "head-sha",
      },
      projectRoot: tmpDir,
    };

    const result = await runTrustVerifier(input, { gitSignatureProbe: stubProbe });
    expect(result.findings).toEqual([]);
  });

  it("diff violating scope + secrets + sensitive-paths — emits 3 check findings", async () => {
    // Write sensitive-paths policy
    await writeFile(
      join(tmpDir, ".maestro", "policies", "sensitive-paths.yaml"),
      'paths:\n  - "secrets/**"\n',
    );
    // No package.json / bun.lock so lockfile check stays clean

    const input: TrustVerifierInput = {
      contract: makeContract({
        scope: { filesExpected: ["src/**"], filesForbidden: [] },
      }),
      diff: {
        // out-of-scope AND sensitive path
        changedPaths: ["docs/README.md", "secrets/key.pem"],
        // AWS secret in diff
        addedLines: ["+const key = 'AKIAIOSFODNN7EXAMPLE';"],
        base: "base-sha",
        head: "head-sha",
      },
      projectRoot: tmpDir,
    };

    const result = await runTrustVerifier(input, { gitSignatureProbe: stubProbe });

    const checkNames = new Set(result.findings.map((f) => f.check));
    // scope finding (out-of-scope paths)
    expect(checkNames).toContain("scope");
    // secrets finding
    expect(checkNames).toContain("secrets-in-diff");
    // sensitive-paths finding
    expect(checkNames).toContain("sensitive-paths");
    // at least 3 distinct check categories
    expect(checkNames.size).toBeGreaterThanOrEqual(3);
  });

  it("lockfile violation — emits lockfile-parity finding", async () => {
    await writeFile(join(tmpDir, "package.json"), JSON.stringify({ scripts: {} }));
    await writeFile(join(tmpDir, "bun.lock"), "");

    const input: TrustVerifierInput = {
      contract: makeContract({ scope: { filesExpected: ["**"], filesForbidden: [] } }),
      diff: {
        changedPaths: ["package.json"],  // bun.lock missing from diff
        addedLines: [],
        base: "base-sha",
        head: "head-sha",
      },
      projectRoot: tmpDir,
    };

    const result = await runTrustVerifier(input, { gitSignatureProbe: stubProbe });
    const lockfileFindings = result.findings.filter((f) => f.check === "lockfile-parity");
    expect(lockfileFindings).toHaveLength(1);
    expect(lockfileFindings[0].severity).toBe("error");
  });

  it("empty diff — emits empty-diff warn finding", async () => {
    const input: TrustVerifierInput = {
      contract: makeContract({ scope: { filesExpected: ["**"], filesForbidden: [] } }),
      diff: {
        changedPaths: [],
        addedLines: [],
        base: "base-sha",
        head: "head-sha",
      },
      projectRoot: tmpDir,
    };

    const result = await runTrustVerifier(input, { gitSignatureProbe: stubProbe });
    const emptyDiffFindings = result.findings.filter((f) => f.check === "empty-diff");
    expect(emptyDiffFindings).toHaveLength(1);
    expect(emptyDiffFindings[0].severity).toBe("warn");
  });

  it("sync: script in package.json — emits generated-file-parity info finding", async () => {
    await writeFile(
      join(tmpDir, "package.json"),
      JSON.stringify({ scripts: { "sync:bundled-skills": "bun scripts/sync.ts" } }),
    );

    const input: TrustVerifierInput = {
      contract: makeContract({ scope: { filesExpected: ["**"], filesForbidden: [] } }),
      diff: {
        changedPaths: [],
        addedLines: [],
        base: "base-sha",
        head: "head-sha",
      },
      projectRoot: tmpDir,
    };

    const result = await runTrustVerifier(input, { gitSignatureProbe: stubProbe });
    const genFindings = result.findings.filter((f) => f.check === "generated-file-parity");
    expect(genFindings).toHaveLength(1);
    expect(genFindings[0].severity).toBe("info");
  });
});
