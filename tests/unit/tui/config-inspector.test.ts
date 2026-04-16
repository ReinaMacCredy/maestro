import { describe, expect, it } from "bun:test";
import { buildConfigInspector } from "@/tui/state/config-inspector.js";
import type { MaestroConfig } from "@/infra/domain/config-types.js";
import type { ConfigLayers } from "@/infra/ports/config.port.js";
import type { Feature } from "@/features/mission";

const layers: ConfigLayers = {
  defaults: {
    ui: {
      missionControl: {
        backgroundMode: "solid",
      },
    },
  } satisfies MaestroConfig,
  effective: {
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
    ui: {
      missionControl: {
        backgroundMode: "terminal",
      },
    },
  } satisfies MaestroConfig,
  global: {
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
  } satisfies MaestroConfig,
  project: {
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
    ui: {
      missionControl: {
        backgroundMode: "solid",
      },
    },
  } satisfies MaestroConfig,
  errors: [],
  paths: {
    project: ".maestro/config.yaml",
    global: "~/.maestro/config.yaml",
  },
};

describe("buildConfigInspector", () => {
  it("builds effective rows with provenance", () => {
    const inspector = buildConfigInspector(layers, [], []);
    const row = inspector.rowsByTab.effective.find(
      (item) => item.keyPath === "memory.corrections.matching",
    );

    expect(row?.label).toBe("Matching");
    expect(row?.displayValueText).toBe("keyword");
    expect(row?.source).toBe("global");
    expect(row?.sourceBadge).toBe("G");
    expect(row?.editKind).toBe("enum");
    expect(row?.editKindLabel).toBe("choice");
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
    expect(
      inspector.rowsByTab.plan.find((row) => row.keyPath === "plan.nextTask")?.displayValueText,
    ).toBe("f2 Ready after setup");
    expect(inspector.rowsByTab.doctor.some((row) => row.valueText.includes("bad yaml"))).toBe(true);
  });

  it("surfaces mission control background mode as a global-only setting", () => {
    const inspector = buildConfigInspector(layers, [], []);
    const overviewRow = inspector.rowsByTab.overview.find(
      (row) => row.keyPath === "ui.missionControl.backgroundMode",
    );
    const projectRow = inspector.rowsByTab.project.find(
      (row) => row.keyPath === "ui.missionControl.backgroundMode",
    );
    const doctorRow = inspector.rowsByTab.doctor.find(
      (row) => row.keyPath === "doctor.ignored-ui-missionControl-backgroundMode",
    );

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
    const matchingRow = inspector.rowsByTab.memory.find(
      (row) => row.keyPath === "memory.corrections.matching",
    );
    const thresholdRow = inspector.rowsByTab.memory.find(
      (row) => row.keyPath === "memory.learnings.compile_threshold",
    );
    const ratchetRow = inspector.rowsByTab.memory.find(
      (row) => row.keyPath === "memory.ratchet.enforcement",
    );

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

  it("hides parallel.* keys from all tabs", () => {
    const inspectorWithParallel = buildConfigInspector(
      {
        ...layers,
        effective: {
          ...layers.effective,
          // @ts-expect-error -- parallel.* is an unknown legacy key; the filter must hide it
          parallel: { enabled: true, maxConcurrent: 4 },
        },
      },
      [],
      [],
    );
    for (const tab of ["effective", "project", "global", "defaults"] as const) {
      expect(
        inspectorWithParallel.rowsByTab[tab].some((row) => row.keyPath.startsWith("parallel.")),
      ).toBe(false);
    }
  });
});
