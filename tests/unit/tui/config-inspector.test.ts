import { describe, expect, it } from "bun:test";
import { buildConfigInspector } from "../../../src/tui/state/config-inspector.js";
import type { ConfigLayers } from "../../../src/ports/config.port.js";

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

describe("buildConfigInspector", () => {
  it("builds effective rows with provenance", () => {
    const inspector = buildConfigInspector(layers, [], [], "project");
    const row = inspector.rowsByTab.effective.find((item) => item.keyPath === "execution.defaultWorker");

    expect(row?.valueText).toBe("claude-code");
    expect(row?.source).toBe("project");
    expect(row?.editKind).toBe("enum");
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
    );

    expect(inspector.rowsByTab.plan.length).toBeGreaterThan(0);
    expect(inspector.rowsByTab.doctor.some((row) => row.valueText.includes("bad yaml"))).toBe(true);
  });
});
