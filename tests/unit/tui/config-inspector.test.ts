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
    ui: {
      missionControl: {
        backgroundMode: "solid",
      },
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
        env: {
          API_TOKEN: "top-secret-token",
        },
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
    ui: {
      missionControl: {
        backgroundMode: "terminal",
      },
    },
  },
  global: {
    execution: {
      defaultWorker: "codex",
    },
    ui: {
      missionControl: {
        backgroundMode: "terminal",
      },
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
    ui: {
      missionControl: {
        backgroundMode: "solid",
      },
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
        const inspector = buildConfigInspector(layers, [], [], workerHealth);
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
      const features = [
        {
          id: "f1",
          missionId: "m1",
          milestoneId: "m1",
          status: "done",
          title: "Setup",
          description: "done",
          workerType: "test-skill",
          verificationSteps: [],
          dependsOn: [],
          fulfills: [],
          createdAt: "2026-04-02T12:00:00.000Z",
          updatedAt: "2026-04-02T12:00:00.000Z",
        },
        {
          id: "f2",
          missionId: "m1",
          milestoneId: "m1",
          status: "pending",
          title: "Ready after setup",
          description: "pending",
          workerType: "test-skill",
          verificationSteps: [],
          dependsOn: ["f1"],
          fulfills: [],
          createdAt: "2026-04-02T12:00:00.000Z",
          updatedAt: "2026-04-02T12:00:00.000Z",
        },
      ];

        const inspector = buildConfigInspector(
          {
          ...layers,
          errors: [{ scope: "project", path: ".maestro/config.yaml", message: "bad yaml" }],
          },
          [{ name: "git", status: "ok", message: "Git repository detected" }],
          features,
          workerHealth,
        );

      expect(inspector.rowsByTab.plan.length).toBeGreaterThan(0);
      expect(inspector.rowsByTab.plan.find((row) => row.keyPath === "plan.nextTask")?.displayValueText).toBe("f2 Ready after setup");
      expect(inspector.rowsByTab.doctor.some((row) => row.valueText.includes("bad yaml"))).toBe(true);
    });

    it("uses worker health as the shared source of truth for worker rows", () => {
        const inspector = buildConfigInspector(layers, [], [], workerHealth);
      const workerRow = inspector.rowsByTab.workers.find((row) => row.keyPath === "workers.codex");

      expect(workerRow).toMatchObject({
        label: "Codex",
        valueText: "busy",
      });
      expect(workerRow?.impactText).toContain("active on current mission");
    });

    it("masks sensitive worker config values", () => {
        const inspector = buildConfigInspector(layers, [], [], workerHealth);
      const row = inspector.rowsByTab.effective.find((item) => item.keyPath === "workers.codex.env.API_TOKEN");

      expect(row).toMatchObject({
        valueText: "[hidden]",
        displayValueText: "[hidden]",
      });
    });

    it("surfaces mission control background mode as a global-only setting", () => {
      const inspector = buildConfigInspector(layers, [], [], workerHealth);
      const overviewRow = inspector.rowsByTab.overview.find((row) => row.keyPath === "ui.missionControl.backgroundMode");
      const projectRow = inspector.rowsByTab.project.find((row) => row.keyPath === "ui.missionControl.backgroundMode");
      const doctorRow = inspector.rowsByTab.doctor.find((row) => row.keyPath === "doctor.ignored-ui-missionControl-backgroundMode");

      expect(overviewRow).toMatchObject({
        label: "Background mode",
        displayValueText: "terminal background",
        source: "global",
      });
      expect(projectRow).toMatchObject({
        editKind: "readonly",
      });
      expect(projectRow?.description).toContain("global-only");
      expect(doctorRow?.displayValueText).toContain("Background Mode");
    });
  });
