import { describe, expect, it } from "bun:test";
import { validateWorkerConfig } from "@/features/worker";

describe("worker validators", () => {
  it("accepts a valid cli worker config", () => {
    const result = validateWorkerConfig({
      enabled: true,
      transport: "cli",
      command: "codex",
      args: ["exec"],
      outputMode: "raw",
    });

    if (result.transport !== "cli") {
      throw new Error("expected cli transport");
    }
    expect(result.command).toBe("codex");
    expect(result.outputMode).toBe("raw");
  });

  it("rejects a config without a command", () => {
    expect(() =>
      validateWorkerConfig({
        enabled: true,
        transport: "cli",
      })).toThrow("Invalid worker config");
  });

  it("rejects unknown transports (Phase 1 strip removed a2a)", () => {
    expect(() =>
      validateWorkerConfig({
        enabled: true,
        transport: "a2a",
        url: "http://127.0.0.1:4123",
      })).toThrow("Invalid worker config");
  });
});
