import { describe, expect, it } from "bun:test";
import type { Contract } from "@/features/task/domain/contract/contract-types.js";
import {
  AMENDMENT_ID_PATTERN,
  CONTRACT_ID_PATTERN,
  DONE_WHEN_ID_PATTERN,
  canAmendContract,
  canCloseContract,
  canDiscardContract,
  canReopenContract,
  generateContractAmendmentId,
  generateContractId,
  generateDoneWhenId,
  isContractLockable,
  validateContract,
} from "@/features/task/domain/contract/contract-state.js";

function contract(overrides: Partial<Contract> = {}): Contract {
  return {
    schemaVersion: 1,
    id: "c-a1b2c3",
    taskId: "tsk-a1b2c3",
    repoRoot: "/repo",
    status: "draft",
    createdAt: "2026-04-21T00:00:00.000Z",
    intent: "Tighten task claim flow",
    scope: {
      filesExpected: ["src/features/task/**"],
      filesForbidden: ["src/features/mission/**"],
    },
    doneWhen: [
      {
        id: "dw-a1b2c3",
        text: "task claim captures the lock anchor",
        kind: "manual",
      },
    ],
    amendments: [],
    createdBy: "user",
    configSnapshot: {
      strict: false,
      overlapPolicy: "fail",
      rebaseFallback: "best-effort",
      staleReclaimContractPolicy: "inherit",
    },
    ...overrides,
  };
}

describe("contract-state", () => {
  it("generates ids using the task short-id shape", () => {
    for (let i = 0; i < 25; i++) {
      expect(generateContractId()).toMatch(CONTRACT_ID_PATTERN);
      expect(generateDoneWhenId()).toMatch(DONE_WHEN_ID_PATTERN);
      expect(generateContractAmendmentId()).toMatch(AMENDMENT_ID_PATTERN);
    }
  });

  it("treats only populated draft contracts as lockable", () => {
    expect(isContractLockable(contract())).toBe(true);
    expect(isContractLockable(contract({ intent: "   " }))).toBe(false);
    expect(isContractLockable(contract({ scope: { filesExpected: [], filesForbidden: [] } }))).toBe(false);
    expect(isContractLockable(contract({ doneWhen: [] }))).toBe(false);
    expect(isContractLockable(contract({
      doneWhen: [{ id: "dw-a1b2c3", text: "   ", kind: "manual" }],
    }))).toBe(false);
    expect(isContractLockable(contract({ status: "locked" }))).toBe(false);
  });

  it("enforces the state transition guards", () => {
    const draft = contract();
    const locked = contract({
      status: "locked",
      lockedAt: "2026-04-21T01:00:00.000Z",
      lockedBy: "session:codex:a",
    });
    const fulfilled = contract({
      status: "fulfilled",
      lockedAt: "2026-04-21T01:00:00.000Z",
      lockedBy: "session:codex:a",
      closedAt: "2026-04-21T02:00:00.000Z",
      closedBy: "session:codex:a",
    });
    const discarded = contract({
      status: "discarded",
      discardedAt: "2026-04-21T01:30:00.000Z",
    });

    expect(canDiscardContract(draft)).toBe(true);
    expect(canAmendContract(draft)).toBe(false);
    expect(canCloseContract(draft)).toBe(false);

    expect(canDiscardContract(locked)).toBe(false);
    expect(canAmendContract(locked)).toBe(true);
    expect(canCloseContract(locked)).toBe(true);
    expect(canReopenContract(locked)).toBe(false);

    expect(canReopenContract(fulfilled)).toBe(true);
    expect(canAmendContract(fulfilled)).toBe(false);
    expect(canCloseContract(fulfilled)).toBe(false);

    expect(canDiscardContract(discarded)).toBe(false);
    expect(canReopenContract(discarded)).toBe(false);
  });

  it("accepts well-formed stored contracts", () => {
    expect(validateContract(contract())).toEqual(contract());
  });

  it("rejects hand-edited invalid contracts", () => {
    expect(validateContract({ ...contract(), id: "nope" })).toBeUndefined();
    expect(validateContract({ ...contract(), createdAt: "2026-04-21" })).toBeUndefined();
    expect(validateContract({ ...contract(), doneWhen: [{ id: "dw-a1b2c3", text: 1, kind: "manual" }] })).toBeUndefined();
    expect(validateContract({ ...contract(), configSnapshot: { strict: false } })).toBeUndefined();
    expect(validateContract({ ...contract(), verdict: { fulfilled: "nope" } })).toBeUndefined();
  });
});
