import { describe, expect, it } from "bun:test";
import {
  validateExecutionRecord,
  validateWorkerConfig,
} from "../../../src/domain/worker-validators.js";

describe("worker validators", () => {
  it("accepts a valid cli worker config", () => {
    const result = validateWorkerConfig({
      enabled: true,
      transport: "cli",
      command: "codex",
      args: ["exec"],
      outputMode: "raw",
    });

    expect(result.command).toBe("codex");
    expect(result.outputMode).toBe("raw");
  });

  it("accepts a valid a2a worker config", () => {
    const result = validateWorkerConfig({
      enabled: true,
      transport: "a2a",
      url: "http://127.0.0.1:4123",
      agentCardPath: "/.well-known/agent-card.json",
    });

    expect(result.transport).toBe("a2a");
    expect(result.url).toBe("http://127.0.0.1:4123");
  });

  it("rejects invalid transport-specific config", () => {
    expect(() =>
      validateWorkerConfig({
        enabled: true,
        transport: "a2a",
      })).toThrow("Invalid worker config");
  });

  it("accepts a valid execution record", () => {
    const record = validateExecutionRecord({
      id: "attempt-1",
      missionId: "mission-1",
      featureId: "feature-1",
      worker: "codex",
      transport: "a2a",
      attemptId: "attempt-1",
      startedAt: "2026-04-02T10:00:00.000Z",
      completedAt: "2026-04-02T10:00:05.000Z",
      durationMs: 5000,
      success: true,
      exitCode: 0,
      summary: "done",
      stdoutRaw: "{}",
      stderrRaw: "",
      filesChanged: ["src/index.ts"],
    });

    expect(record.success).toBe(true);
  });

  it("rejects malformed execution records", () => {
    expect(() =>
      validateExecutionRecord({
        id: "attempt-1",
      })).toThrow("Invalid execution record");
  });
});
