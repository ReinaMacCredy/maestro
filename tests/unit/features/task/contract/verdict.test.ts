import { describe, expect, it } from "bun:test";
import { computeContractVerdict } from "@/features/task/domain/contract/verdict.js";
import type { Contract } from "@/features/task/domain/contract/contract-types.js";
import type { GitTouchedFilesResult } from "@/features/task/ports/git-anchor.port.js";

function contractFixture(overrides: Partial<Contract> = {}): Contract {
  return {
    schemaVersion: 1,
    id: "c-a1b2c3",
    taskId: "tsk-a1b2c3",
    repoRoot: "/tmp/repo",
    status: "locked",
    createdAt: "2026-04-21T00:00:00.000Z",
    lockedAt: "2026-04-21T00:05:00.000Z",
    intent: "Keep the change inside the scoped files.",
    scope: {
      filesExpected: ["README.md"],
      filesForbidden: [],
    },
    doneWhen: [
      {
        id: "dw-a1b2c3",
        text: "manual",
        kind: "receipt-hint",
      },
    ],
    claimedAtCommit: "0123456789abcdef0123456789abcdef01234567",
    amendments: [],
    createdBy: "user",
    lockedBy: "session:test",
    configSnapshot: {
      strict: false,
      overlapPolicy: "fail",
      rebaseFallback: "best-effort",
      staleReclaimContractPolicy: "inherit",
    },
    ...overrides,
  };
}

describe("computeContractVerdict", () => {
  it("fulfills a contract when touched files stay in scope and receipt hints satisfy criteria", () => {
    const contract = contractFixture({
      scope: {
        filesExpected: ["README.md", "src/**"],
        filesForbidden: [],
      },
    });
    const gitResult: GitTouchedFilesResult = {
      gitAvailable: true,
      actualFilesTouched: ["README.md"],
      closedAtCommit: "89abcdef0123456789abcdef0123456789abcdef",
      anchorFallback: "direct",
    };

    const computed = computeContractVerdict(
      contract,
      gitResult,
      {
        summary: "updated docs",
        verifiedBy: ["manual"],
        capturedAt: "2026-04-21T00:10:00.000Z",
      },
      "session:test",
      "2026-04-21T00:10:00.000Z",
    );

    expect(computed.verdict.fulfilled).toBe(true);
    expect(computed.verdict.expectedFilesMatched).toEqual(["README.md"]);
    expect(computed.verdict.outOfScopeFiles).toEqual([]);
    expect(computed.verdict.filesExpectedUnused).toEqual(["src/**"]);
    expect(computed.verdict.metCriteria).toHaveLength(1);
    expect(computed.criteria[0]).toEqual(
      expect.objectContaining({
        met: true,
        metEvidence: "receipt.verifiedBy:manual",
      }),
    );
  });

  it("only auto-marks receipt-hint criteria from receipt verification", () => {
    const contract = contractFixture({
      doneWhen: [
        {
          id: "dw-hint01",
          text: "manual",
          kind: "receipt-hint",
        },
        {
          id: "dw-manual",
          text: "manual",
          kind: "manual",
        },
      ],
    });

    const computed = computeContractVerdict(
      contract,
      {
        gitAvailable: true,
        actualFilesTouched: ["README.md"],
        closedAtCommit: "89abcdef0123456789abcdef0123456789abcdef",
        anchorFallback: "direct",
      },
      {
        summary: "verified",
        verifiedBy: ["manual"],
        capturedAt: "2026-04-21T00:10:00.000Z",
      },
      "session:test",
      "2026-04-21T00:10:00.000Z",
    );

    expect(computed.criteria.find((criterion) => criterion.id === "dw-hint01")).toEqual(
      expect.objectContaining({ met: true }),
    );
    expect(computed.criteria.find((criterion) => criterion.id === "dw-manual")?.met).not.toBe(true);
  });

  it("avoids false-positive receipt hint matches from short verifier substrings", () => {
    const contract = contractFixture({
      doneWhen: [
        {
          id: "dw-hint01",
          text: "manual review",
          kind: "receipt-hint",
        },
      ],
    });

    const computed = computeContractVerdict(
      contract,
      {
        gitAvailable: true,
        actualFilesTouched: ["README.md"],
        closedAtCommit: "89abcdef0123456789abcdef0123456789abcdef",
        anchorFallback: "direct",
      },
      {
        summary: "verified",
        verifiedBy: ["manual"],
        capturedAt: "2026-04-21T00:10:00.000Z",
      },
      "session:test",
      "2026-04-21T00:10:00.000Z",
    );

    expect(computed.criteria[0]?.met).not.toBe(true);
    expect(computed.verdict.fulfilled).toBe(false);
  });

  it("marks the verdict broken for lost anchors, forbidden files, out-of-scope files, and unmet criteria", () => {
    const contract = contractFixture({
      scope: {
        filesExpected: ["src/**"],
        filesForbidden: ["secret.env"],
        maxFilesTouched: 1,
      },
      doneWhen: [
        {
          id: "dw-bad123",
          text: "manual review",
          kind: "manual",
        },
      ],
    });
    const gitResult: GitTouchedFilesResult = {
      gitAvailable: true,
      actualFilesTouched: ["README.md", "secret.env"],
      closedAtCommit: "89abcdef0123456789abcdef0123456789abcdef",
      anchorFallback: "lost",
      notes: "Claim anchor could not be recovered after history rewriting.",
    };

    const computed = computeContractVerdict(
      contract,
      gitResult,
      undefined,
      "session:test",
      "2026-04-21T00:10:00.000Z",
    );

    expect(computed.verdict.fulfilled).toBe(false);
    expect(computed.verdict.outOfScopeFiles).toEqual(["README.md"]);
    expect(computed.verdict.forbiddenTouched).toEqual(["secret.env"]);
    expect(computed.verdict.capExceeded).toEqual({ cap: 1, actual: 2 });
    expect(computed.verdict.unmetCriteria).toHaveLength(1);
    expect(computed.verdict.anchorFallback).toBe("lost");
  });

  it("records overlap annotations without failing when policy is annotate", () => {
    const contract = contractFixture({
      configSnapshot: {
        strict: false,
        overlapPolicy: "annotate",
        rebaseFallback: "best-effort",
        staleReclaimContractPolicy: "inherit",
      },
      doneWhen: [],
    });
    const gitResult: GitTouchedFilesResult = {
      gitAvailable: true,
      actualFilesTouched: ["README.md"],
      closedAtCommit: "89abcdef0123456789abcdef0123456789abcdef",
      anchorFallback: "direct",
    };

    const computed = computeContractVerdict(
      contract,
      gitResult,
      undefined,
      "session:test",
      "2026-04-21T00:10:00.000Z",
      {
        overlapDetected: {
          otherContractIds: ["c-b2c3d4"],
          policy: "annotate",
        },
      },
    );

    expect(computed.verdict.fulfilled).toBe(true);
    expect(computed.verdict.overlapDetected).toEqual({
      otherContractIds: ["c-b2c3d4"],
      policy: "annotate",
    });
  });

  it("fails the verdict when overlap policy is fail", () => {
    const contract = contractFixture({
      doneWhen: [],
    });
    const gitResult: GitTouchedFilesResult = {
      gitAvailable: true,
      actualFilesTouched: ["README.md"],
      closedAtCommit: "89abcdef0123456789abcdef0123456789abcdef",
      anchorFallback: "direct",
    };

    const computed = computeContractVerdict(
      contract,
      gitResult,
      undefined,
      "session:test",
      "2026-04-21T00:10:00.000Z",
      {
        overlapDetected: {
          otherContractIds: ["c-b2c3d4"],
          policy: "fail",
        },
      },
    );

    expect(computed.verdict.fulfilled).toBe(false);
    expect(computed.verdict.overlapDetected).toEqual({
      otherContractIds: ["c-b2c3d4"],
      policy: "fail",
    });
  });

  it("stores a capped touched-file list and keeps the truncation metadata", () => {
    const contract = contractFixture({
      doneWhen: [],
    });
    const gitResult: GitTouchedFilesResult = {
      gitAvailable: true,
      actualFilesTouched: ["README.md", "src/index.ts", "tests/unit/sample.test.ts"],
      actualFilesTouchedTruncated: {
        stored: 2,
        actual: 3,
      },
      closedAtCommit: "89abcdef0123456789abcdef0123456789abcdef",
      anchorFallback: "direct",
    };

    const computed = computeContractVerdict(
      contract,
      gitResult,
      undefined,
      "session:test",
      "2026-04-21T00:10:00.000Z",
    );

    expect(computed.verdict.actualFilesTouched).toEqual(["README.md", "src/index.ts"]);
    expect(computed.verdict.actualFilesTouchedTruncated).toEqual({
      stored: 2,
      actual: 3,
    });
    expect(computed.verdict.outOfScopeFiles).toEqual(["src/index.ts", "tests/unit/sample.test.ts"]);
  });

  it("notes when out-of-scope files were previously allowed by amendment history", () => {
    const contract = contractFixture({
      scope: {
        filesExpected: ["src/**"],
        filesForbidden: [],
      },
      doneWhen: [],
      amendments: [
        {
          id: "a-a1b2c3",
          at: "2026-04-21T00:06:00.000Z",
          by: "session:test",
          reason: "narrowed scope after the docs edit landed",
          before: {
            scope: {
              filesExpected: ["README.md", "src/**"],
              filesForbidden: [],
            },
          },
          after: {
            scope: {
              filesExpected: ["src/**"],
              filesForbidden: [],
            },
          },
        },
      ],
    });
    const gitResult: GitTouchedFilesResult = {
      gitAvailable: true,
      actualFilesTouched: ["README.md"],
      closedAtCommit: "89abcdef0123456789abcdef0123456789abcdef",
      anchorFallback: "direct",
    };

    const computed = computeContractVerdict(
      contract,
      gitResult,
      undefined,
      "session:test",
      "2026-04-21T00:10:00.000Z",
    );

    expect(computed.verdict.fulfilled).toBe(false);
    expect(computed.verdict.outOfScopeFiles).toEqual(["README.md"]);
    expect(computed.verdict.notes).toContain("Previously in scope under amendments: README.md");
  });
});
