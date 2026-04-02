import { describe, expect, it } from "bun:test";
import { DEFAULT_RUNTIME_LEASE_MS } from "../../../src/domain/defaults.js";
import type { WorkerRuntime } from "../../../src/domain/runtime-types.js";
import { applyWorkerProgressEvent } from "../../../src/usecases/live-runtime-tracking.usecase.js";

function makeRuntime(overrides: Partial<WorkerRuntime> = {}): WorkerRuntime {
  return {
    featureId: "f1",
    attemptId: "attempt-1",
    attempt: 1,
    agent: "unknown",
    runtimeState: "starting",
    startedAt: "2026-04-02T12:00:00.000Z",
    lastSeenAt: "2026-04-02T12:00:00.000Z",
    leaseExpiresAt: "2026-04-02T12:02:00.000Z",
    recoveryMetadata: {
      retryCount: 0,
      history: [],
    },
    ...overrides,
  };
}

describe("applyWorkerProgressEvent", () => {
  it("marks runtime live and refreshes lease metadata for output activity", () => {
    const runtime = makeRuntime();
    const eventTime = "2026-04-02T12:00:15.000Z";

    const updated = applyWorkerProgressEvent(runtime, {
      timestamp: eventTime,
      kind: "stdout",
      text: "Running tests",
      worker: "codex",
      sessionId: "session-123",
    });

    expect(updated.runtimeState).toBe("live");
    expect(updated.agent).toBe("codex");
    expect(updated.sessionId).toBe("session-123");
    expect(updated.lastSeenAt).toBe(eventTime);
    expect(updated.leaseExpiresAt).toBe(
      new Date(new Date(eventTime).getTime() + DEFAULT_RUNTIME_LEASE_MS).toISOString(),
    );
  });

  it("keeps runtime live on heartbeat events without clearing session info", () => {
    const runtime = makeRuntime({
      agent: "claude-code",
      sessionId: "session-1",
      runtimeState: "live",
      lastSeenAt: "2026-04-02T12:00:10.000Z",
    });

    const updated = applyWorkerProgressEvent(runtime, {
      timestamp: "2026-04-02T12:00:20.000Z",
      kind: "heartbeat",
      worker: "claude-code",
    });

    expect(updated.runtimeState).toBe("live");
    expect(updated.agent).toBe("claude-code");
    expect(updated.sessionId).toBe("session-1");
    expect(updated.lastSeenAt).toBe("2026-04-02T12:00:20.000Z");
  });

  it("captures failure state updates from transport progress events", () => {
    const runtime = makeRuntime({
      agent: "gemini",
      runtimeState: "live",
    });

    const updated = applyWorkerProgressEvent(runtime, {
      timestamp: "2026-04-02T12:00:30.000Z",
      kind: "status",
      worker: "gemini",
      runtimeState: "failed",
      text: "worker exited unexpectedly",
    });

    expect(updated.runtimeState).toBe("failed");
    expect(updated.failureReason).toBe("worker exited unexpectedly");
    expect(updated.lastSeenAt).toBe("2026-04-02T12:00:30.000Z");
  });
});
