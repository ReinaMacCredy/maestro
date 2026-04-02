import { describe, expect, it } from "bun:test";
import { buildConfigInspector } from "../../../src/tui/state/config-inspector.js";
import type { ConfigLayers } from "../../../src/ports/config.port.js";
import type { MissionControlWorkerHealthRow } from "../../../src/tui/state/types.js";

const layers: ConfigLayers = {
  defaults: {
    execution: {
      defaultWorker: "codex",
      stopOnFailure: true,
    },
    supervision: {
      level: "mid",
    },
  },
  effective: {
    execution: {
      defaultWorker: "claude-code",
      stopOnFailure: false,
    },
    workers: {
      codex: {
        enabled: true,
        transport: "cli",
        command: "codex",
        outputMode: "raw",
      },
      "claude-code": {
        enabled: true,
        transport: "cli",
        command: "claude",
        outputMode: "stream-json",
      },
    },
    supervision: {
      level: "high",
    },
  },
  global: {
    execution: {
      defaultWorker: "codex",
    },
  },
  project: {
    execution: {
      defaultWorker: "claude-code",
      stopOnFailure: false,
    },
    supervision: {
      level: "high",
    },
  },
  errors: [],
  paths: {
    project: ".maestro/config.yaml",
    global: "~/.maestro/config.yaml",
  },
};

const workerHealth: readonly MissionControlWorkerHealthRow[] = [
  {
    slug: "codex",
    label: "Codex",
    status: "busy",
    detail: "active on current mission",
    lastCheckedAt: "2026-04-02T12:00:00.000Z",
    checks: [{ label: "command found", ok: true }],
    summary: "Fast, strong general-purpose coding.",
    bestFor: "everyday implementation",
    tradeoffs: "less exhaustive than Claude Code",
  },
  {
    slug: "claude-code",
    label: "Claude Code",
    status: "ready",
    detail: "ready",
    lastCheckedAt: "2026-04-02T12:00:00.000Z",
    checks: [{ label: "command found", ok: true }],
    summary: "Highest quality, slower and pricier.",
    bestFor: "hard bugs",
    tradeoffs: "slower",
  },
];

describe("buildConfigInspector", () => {
    it("builds effective rows with provenance", () => {
        const inspector = buildConfigInspector(layers, [], [], "project", workerHealth);
        const row = inspector.rowsByTab.effective.find((item) => item.keyPath === "execution.defaultWorker");

      expect(row?.label).toBe("Default worker");
      expect(row?.displayValueText).toBe("claude-code");
        expect(row?.source).toBe("project");
        expect(row?.sourceBadge).toBe("P");
        expect(row?.editKind).toBe("enum");
        expect(row?.editKindLabel).toBe("choice");
        expect(row?.workerChoices?.find((choice) => choice.slug === "codex")).toMatchObject({
          availability: "busy",
          availabilityDetail: "active on current mission",
        });
      });

    it("builds plan and doctor tabs", () => {
      const inspector = buildConfigInspector(
        {
        ...layers,
        errors: [{ scope: "project", path: ".maestro/config.yaml", message: "bad yaml" }],
        },
        [{ name: "git", status: "ok", message: "Git repository detected" }],
        [],
        "project",
        workerHealth,
      );

      expect(inspector.rowsByTab.plan.length).toBeGreaterThan(0);
      expect(inspector.rowsByTab.doctor.some((row) => row.valueText.includes("bad yaml"))).toBe(true);
    });

    it("uses worker health as the shared source of truth for worker rows", () => {
      const inspector = buildConfigInspector(layers, [], [], "project", workerHealth);
      const workerRow = inspector.rowsByTab.workers.find((row) => row.keyPath === "workers.codex");

      expect(workerRow).toMatchObject({
        label: "Codex",
        valueText: "busy",
      });
      expect(workerRow?.impactText).toContain("active on current mission");
    });
  });
