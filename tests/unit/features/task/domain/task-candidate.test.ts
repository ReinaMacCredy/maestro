import { describe, expect, it } from "bun:test";
import {
  validateTaskCandidate,
  type TaskCandidate,
} from "@/features/task/domain/task-candidate.js";

function fixture(overrides: Partial<TaskCandidate> = {}): TaskCandidate {
  return {
    id: "tsk-abc123",
    sourceTaskId: "tsk-abc123",
    sourceType: "task-close",
    title: "Sample task",
    reason: "done",
    keywords: ["sample", "task"],
    capturedAt: "2026-04-10T00:00:00.000Z",
    ...overrides,
  };
}

describe("validateTaskCandidate", () => {
  it("accepts a well-formed candidate", () => {
    const result = validateTaskCandidate(fixture());
    expect(result).toBeDefined();
    expect(result?.id).toBe("tsk-abc123");
    expect(result?.sourceType).toBe("task-close");
  });

  it("rejects non-object input", () => {
    expect(validateTaskCandidate(null)).toBeUndefined();
    expect(validateTaskCandidate(undefined)).toBeUndefined();
    expect(validateTaskCandidate("not an object")).toBeUndefined();
    expect(validateTaskCandidate(42)).toBeUndefined();
  });

  it("rejects missing id", () => {
    expect(validateTaskCandidate({ ...fixture(), id: "" })).toBeUndefined();
    expect(validateTaskCandidate({ ...fixture(), id: "not-a-task-id" })).toBeUndefined();
    const { id: _id, ...noId } = fixture();
    expect(validateTaskCandidate(noId)).toBeUndefined();
  });

  it("rejects missing sourceTaskId", () => {
    expect(
      validateTaskCandidate({ ...fixture(), sourceTaskId: "" }),
    ).toBeUndefined();
    expect(
      validateTaskCandidate({ ...fixture(), sourceTaskId: "bad-id" }),
    ).toBeUndefined();
  });

  it("rejects unknown sourceType", () => {
    expect(
      validateTaskCandidate({ ...fixture(), sourceType: "task-reopen" as never }),
    ).toBeUndefined();
  });

  it("rejects non-string title / reason", () => {
    expect(
      validateTaskCandidate({ ...fixture(), title: 42 as never }),
    ).toBeUndefined();
    expect(
      validateTaskCandidate({ ...fixture(), reason: null as never }),
    ).toBeUndefined();
  });

  it("rejects non-array keywords", () => {
    expect(
      validateTaskCandidate({ ...fixture(), keywords: "auth" as never }),
    ).toBeUndefined();
  });

  it("rejects keywords containing non-strings", () => {
    expect(
      validateTaskCandidate({ ...fixture(), keywords: ["ok", 42] as never }),
    ).toBeUndefined();
  });

  it("rejects missing capturedAt", () => {
    expect(
      validateTaskCandidate({ ...fixture(), capturedAt: 0 as never }),
    ).toBeUndefined();
  });

  it("returns a new object rather than the input", () => {
    const input = fixture();
    const result = validateTaskCandidate(input);
    expect(result).not.toBe(input);
    expect(result).toEqual(input);
  });
});
