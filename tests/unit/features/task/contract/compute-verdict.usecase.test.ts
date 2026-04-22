import { describe, expect, it } from "bun:test";
import type { Contract } from "@/features/task/domain/contract/contract-types.js";
import { computeContractVerdictForTask } from "@/features/task/usecases/contract/compute-verdict.usecase.js";
import type { ContractStoreQueryPort } from "@/features/task/ports/contract-store.port.js";
import type { GitAnchorPort, GitTouchedFilesResult } from "@/features/task/ports/git-anchor.port.js";

function contractFixture(overrides: Partial<Contract> = {}): Contract {
  return {
    schemaVersion: 1,
    id: "c-a1b2c3",
    taskId: "tsk-a1b2c3",
    repoRoot: "/tmp/repo",
    status: "locked",
    createdAt: "2026-04-21T00:00:00.000Z",
    lockedAt: "2026-04-21T00:05:00.000Z",
    intent: "Keep the change inside README.",
    scope: {
      filesExpected: ["README.md"],
      filesForbidden: [],
    },
    doneWhen: [],
    claimedAtCommit: "1111111111111111111111111111111111111111",
    amendments: [],
    createdBy: "user",
    lockedBy: "session:test",
    configSnapshot: {
      strict: false,
      overlapPolicy: "annotate",
      rebaseFallback: "best-effort",
      staleReclaimContractPolicy: "inherit",
    },
    ...overrides,
  };
}

const baseGitResult: GitTouchedFilesResult = {
  gitAvailable: true,
  actualFilesTouched: ["README.md"],
  closedAtCommit: "2222222222222222222222222222222222222222",
  anchorFallback: "direct",
};

function contractStore(contracts: readonly Contract[]): ContractStoreQueryPort {
  return {
    async get(id) {
      return contracts.find((contract) => contract.id === id);
    },
    async getByTaskId(taskId) {
      return contracts.find((contract) => contract.taskId === taskId);
    },
    async all() {
      return contracts;
    },
    async readIndex() {
      return [];
    },
  };
}

describe("computeContractVerdictForTask", () => {
  it("ignores timestamp-only overlap and uses git commit windows instead", async () => {
    const current = contractFixture({
      id: "c-current",
      claimedAtCommit: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
      lockedAt: "2026-04-21T02:00:00.000Z",
    });
    const older = contractFixture({
      id: "c-older",
      taskId: "tsk-older00",
      status: "fulfilled",
      claimedAtCommit: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
      closedAtCommit: "cccccccccccccccccccccccccccccccccccccccc",
      lockedAt: "2026-04-21T00:00:00.000Z",
      closedAt: "2026-04-21T04:00:00.000Z",
    });

    const gitAnchor: GitAnchorPort = {
      async resolveRepoRoot(cwd) {
        return cwd;
      },
      async resolveHeadCommit() {
        return baseGitResult.closedAtCommit;
      },
      async collectTouchedFiles() {
        return baseGitResult;
      },
      async windowsOverlap(input) {
        expect(input.left.claimedAtCommit).toBe(current.claimedAtCommit);
        expect(input.right.closedAtCommit).toBe(older.closedAtCommit);
        return false;
      },
    };

    const computed = await computeContractVerdictForTask(
      contractStore([current, older]),
      gitAnchor,
      current,
      {
        updatedAt: "2026-04-21T03:00:00.000Z",
        assignee: "session:test",
      },
    );

    expect(computed.verdict.overlapDetected).toBeUndefined();
    expect(computed.verdict.fulfilled).toBe(true);
  });

  it("records overlap when git commit windows intersect", async () => {
    const current = contractFixture({
      id: "c-current",
      claimedAtCommit: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    });
    const sibling = contractFixture({
      id: "c-sibling",
      taskId: "tsk-sibling",
      status: "amended",
      claimedAtCommit: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
      lockedAt: "2026-04-21T00:10:00.000Z",
    });

    const gitAnchor: GitAnchorPort = {
      async resolveRepoRoot(cwd) {
        return cwd;
      },
      async resolveHeadCommit() {
        return baseGitResult.closedAtCommit;
      },
      async collectTouchedFiles() {
        return baseGitResult;
      },
      async windowsOverlap() {
        return true;
      },
    };

    const computed = await computeContractVerdictForTask(
      contractStore([current, sibling]),
      gitAnchor,
      current,
      {
        updatedAt: "2026-04-21T03:00:00.000Z",
        assignee: "session:test",
      },
    );

    expect(computed.verdict.overlapDetected).toEqual({
      otherContractIds: ["c-sibling"],
      policy: "annotate",
    });
  });

  it("uses the trusted runtime repo root instead of the stored contract path", async () => {
    const current = contractFixture({
      id: "c-current",
      repoRoot: "/tmp/untrusted-contract-path",
      claimedAtCommit: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    });
    const trustedRepoRoot = "/tmp/trusted-runtime-root";

    const gitAnchor: GitAnchorPort = {
      async resolveRepoRoot(cwd) {
        return cwd;
      },
      async resolveHeadCommit() {
        return baseGitResult.closedAtCommit;
      },
      async collectTouchedFiles(input) {
        expect(input.repoRoot).toBe(trustedRepoRoot);
        return baseGitResult;
      },
      async windowsOverlap(input) {
        expect(input.repoRoot).toBe(trustedRepoRoot);
        return false;
      },
    };

    await computeContractVerdictForTask(
      contractStore([current]),
      gitAnchor,
      current,
      {
        updatedAt: "2026-04-21T03:00:00.000Z",
        assignee: "session:test",
      },
      undefined,
      trustedRepoRoot,
    );
  });
});
