import { describe, expect, it } from "bun:test";
import { getWorkerHealthRows } from "../../../src/usecases/worker-health.usecase.js";

describe("getWorkerHealthRows", () => {
  it("marks enabled workers ready when probes succeed", async () => {
    const rows = await getWorkerHealthRows(
      {
        codex: {
          enabled: true,
          transport: "cli",
          command: "codex",
          outputMode: "raw",
        },
      },
      {
        probeCli: async () => ({
          status: "ready",
          checks: [
            { label: "command found", ok: true },
            { label: "launch test", ok: true },
          ],
        }),
      },
    );

    expect(rows[0]).toMatchObject({
      slug: "codex",
      status: "ready",
    });
    expect(rows[0]?.checks.some((check) => check.label === "launch test" && check.ok)).toBe(true);
  });

  it("marks workers degraded when the probe fails after the command is found", async () => {
    const rows = await getWorkerHealthRows(
      {
        codex: {
          enabled: true,
          transport: "cli",
          command: "codex",
          outputMode: "raw",
        },
      },
      {
        probeCli: async () => ({
          status: "degraded",
          detail: "auth/session check failed",
          checks: [
            { label: "command found", ok: true },
            { label: "auth/session", ok: false, detail: "auth/session check failed" },
          ],
        }),
      },
    );

    expect(rows[0]).toMatchObject({
      slug: "codex",
      status: "degraded",
      detail: "auth/session check failed",
    });
  });

  it("marks disabled workers without probing them", async () => {
    const rows = await getWorkerHealthRows(
      {
        gemini: {
          enabled: false,
          transport: "cli",
          command: "gemini",
          outputMode: "stream-json",
        },
      },
      {
        probeCli: async () => {
          throw new Error("should not be called");
        },
      },
    );

    expect(rows[0]).toMatchObject({
      slug: "gemini",
      status: "disabled",
    });
  });
});
