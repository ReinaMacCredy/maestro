import { describe, expect, it } from "bun:test";
import { buildConfigInspector } from "@/tui/state/config-inspector.js";
import type { ConfigLayers } from "@/ports/config.port.js";
import type { Feature } from "@/features/mission/domain/mission-types.js";

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
      memory: {
        enabled: true,
        corrections: {
          enabled: true,
          matching: "keyword",
          auto_capture: "prompt",
          severity_default: "soft",
        },
        learnings: {
          enabled: true,
          compile_threshold: 5,
          max_age_days: 7,
        },
        ratchet: {
          enabled: false,
          enforcement: "warn",
        },
        graph: {
          enabled: true,
        },
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
      memory: {
        enabled: true,
        corrections: {
          enabled: true,
          matching: "keyword",
          auto_capture: "prompt",
          severity_default: "soft",
        },
        learnings: {
          enabled: true,
          compile_threshold: 8,
          max_age_days: 14,
        },
        ratchet: {
          enabled: false,
          enforcement: "warn",
        },
        graph: {
          enabled: true,
        },
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
      memory: {
        enabled: true,
        corrections: {
          enabled: true,
          matching: "both",
          auto_capture: "auto",
          severity_default: "hard",
        },
        learnings: {
          enabled: true,
          compile_threshold: 5,
          max_age_days: 7,
        },
        ratchet: {
          enabled: true,
          enforcement: "block",
        },
        graph: {
          enabled: true,
        },
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

describe("buildConfigInspector", () => {
    it("builds effective rows with provenance", () => {
        const inspector = buildConfigInspector(layers, [], []);
        const row = inspector.rowsByTab.effective.find((item) => item.keyPath === "execution.defaultWorker");

      expect(row?.label).toBe("Default worker");
      expect(row?.displayValueText).toBe("claude-code");
        expect(row?.source).toBe("project");
        expect(row?.sourceBadge).toBe("P");
        expect(row?.editKind).toBe("enum");
        expect(row?.editKindLabel).toBe("choice");
        // Phase 3 strip: availability now derives from `cachedWhich` so
        // the choice status depends on whether `codex` is on PATH. Only
        // assert the static label fields here.
        const codexChoice = row?.workerChoices?.find((choice) => choice.slug === "codex");
        expect(codexChoice?.label).toBe("Codex");
        expect(codexChoice?.slug).toBe("codex");
      });

    it("builds plan and doctor tabs", () => {
      const features: Feature[] = [
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
        );

      expect(inspector.rowsByTab.plan.length).toBeGreaterThan(0);
      expect(inspector.rowsByTab.plan.find((row) => row.keyPath === "plan.nextTask")?.displayValueText).toBe("f2 Ready after setup");
      expect(inspector.rowsByTab.doctor.some((row) => row.valueText.includes("bad yaml"))).toBe(true);
    });

    it("derives worker row labels from CLI config without the Phase 1 health pane", () => {
        const inspector = buildConfigInspector(layers, [], []);
      const workerRow = inspector.rowsByTab.workers.find((row) => row.keyPath === "workers.codex");

      expect(workerRow?.label).toBe("Codex");
      // Phase 3 strip: valueText/impactText now reflect `cachedWhich`
      // output instead of an injected worker health row. We assert
      // only the stable label fields.
      expect(workerRow?.section).toBe("Workers");
    });

    it("masks sensitive worker config values", () => {
        const inspector = buildConfigInspector(layers, [], []);
      const row = inspector.rowsByTab.effective.find((item) => item.keyPath === "workers.codex.env.API_TOKEN");

      expect(row).toMatchObject({
        valueText: "[hidden]",
        displayValueText: "[hidden]",
      });
    });

  it("surfaces mission control background mode as a global-only setting", () => {
      const inspector = buildConfigInspector(layers, [], []);
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

  it("builds editable memory rows in the memory tab", () => {
    const inspector = buildConfigInspector(layers, [], []);
    const matchingRow = inspector.rowsByTab.memory.find((row) => row.keyPath === "memory.corrections.matching");
    const thresholdRow = inspector.rowsByTab.memory.find((row) => row.keyPath === "memory.learnings.compile_threshold");
    const ratchetRow = inspector.rowsByTab.memory.find((row) => row.keyPath === "memory.ratchet.enforcement");

    expect(matchingRow).toMatchObject({
      editKind: "enum",
      displayValueText: "keyword",
      source: "global",
    });
    expect(thresholdRow).toMatchObject({
      editKind: "number-preset",
      displayValueText: "5 entries",
    });
    expect(ratchetRow).toMatchObject({
      editKind: "enum",
      displayValueText: "warn",
    });
  });
});
