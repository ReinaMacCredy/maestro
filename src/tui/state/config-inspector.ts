import type { Feature } from "../../domain/mission-types.js";
import type { DoctorCheck, MaestroConfig } from "../../domain/types.js";
import type { ConfigLayers } from "../../ports/config.port.js";
import type {
  MissionControlConfigEditKind,
  MissionControlConfigInspector,
  MissionControlConfigRow,
  MissionControlConfigTab,
  MissionControlConfigValueSource,
} from "./types.js";

const KNOWN_TABS: readonly MissionControlConfigTab[] = [
  "overview",
  "effective",
  "project",
  "global",
  "defaults",
  "workers",
  "plan",
  "doctor",
];

const KNOWN_AGENT_OPTIONS = [
  "claude-code",
  "codex",
  "gemini",
  "opencode",
  "amp",
  "cline",
  "aider",
  "cursor",
] as const;

export function buildConfigInspector(
  layers: ConfigLayers,
  checks: readonly DoctorCheck[],
  features: readonly Feature[],
  configSource: "project" | "global" | "none",
): MissionControlConfigInspector {
  const effective = flattenConfig(layers.effective);
  const defaults = flattenConfig(layers.defaults);
  const project = flattenConfig(layers.project ?? {});
  const global = flattenConfig(layers.global ?? {});
  const allPaths = [...new Set([
    ...Object.keys(effective),
    ...Object.keys(defaults),
    ...Object.keys(project),
    ...Object.keys(global),
  ])].sort();

  const workerSlugs = Object.keys(layers.effective.workers ?? {});
  const rowsByTab = {
    overview: buildOverviewRows(layers, features, configSource),
    effective: allPaths.map((path) => buildEffectiveRow(path, layers, effective[path], defaults[path], global[path], project[path], workerSlugs)),
    project: buildScopeRows("project", project, effective, workerSlugs),
    global: buildScopeRows("global", global, effective, workerSlugs),
    defaults: buildScopeRows("default", defaults, effective, workerSlugs),
    workers: buildWorkerRows(layers.effective),
    plan: buildPlanRows(layers.effective, features),
    doctor: buildDoctorRows(checks, layers.errors),
  } satisfies Record<MissionControlConfigTab, readonly MissionControlConfigRow[]>;

  return {
    tabs: KNOWN_TABS,
    rowsByTab,
    hasProjectConfig: layers.project !== undefined,
    hasGlobalConfig: layers.global !== undefined,
    projectPath: layers.paths.project,
    globalPath: layers.paths.global,
    errors: layers.errors.map((error) => `${error.scope}: ${error.message}`),
  };
}

export function getConfigRowsForTab(
  inspector: MissionControlConfigInspector | null,
  tab: MissionControlConfigTab,
): readonly MissionControlConfigRow[] {
  if (!inspector) return [];
  return inspector.rowsByTab[tab] ?? [];
}

function buildOverviewRows(
  layers: ConfigLayers,
  features: readonly Feature[],
  configSource: "project" | "global" | "none",
): readonly MissionControlConfigRow[] {
  const workerEntries = Object.entries(layers.effective.workers ?? {});
  return [
    {
      keyPath: "overview.configSource",
      label: "Config source",
      section: "Status",
      valueText: configSource,
      source: "none",
      editKind: "readonly",
      description: "Which config layer is currently active.",
      effectiveValueText: configSource,
    },
    {
      keyPath: "overview.projectPath",
      label: "Project config",
      section: "Paths",
      valueText: layers.project ? layers.paths.project : "missing",
      source: layers.project ? "project" : "none",
      editKind: "readonly",
      description: "Project-level config file path.",
      effectiveValueText: layers.project ? layers.paths.project : "missing",
    },
    {
      keyPath: "overview.globalPath",
      label: "Global config",
      section: "Paths",
      valueText: layers.global ? layers.paths.global : "missing",
      source: layers.global ? "global" : "none",
      editKind: "readonly",
      description: "Global config file path.",
      effectiveValueText: layers.global ? layers.paths.global : "missing",
    },
    {
      keyPath: "execution.defaultWorker",
      label: "Default worker",
      section: "Execution",
      valueText: stringifyValue(layers.effective.execution?.defaultWorker),
      source: provenanceForValue(
        layers.effective.execution?.defaultWorker,
        layers.defaults.execution?.defaultWorker,
        layers.global?.execution?.defaultWorker,
        layers.project?.execution?.defaultWorker,
      ),
      editKind: "enum",
      options: workerEntries.map(([slug]) => slug),
      description: "Worker used by feature run when no override is passed.",
      effectiveValueText: stringifyValue(layers.effective.execution?.defaultWorker),
      defaultValueText: stringifyValue(layers.defaults.execution?.defaultWorker),
      globalValueText: stringifyValue(layers.global?.execution?.defaultWorker),
      projectValueText: stringifyValue(layers.project?.execution?.defaultWorker),
    },
    {
      keyPath: "execution.stopOnFailure",
      label: "Stop on failure",
      section: "Execution",
      valueText: stringifyBoolean(layers.effective.execution?.stopOnFailure),
      source: provenanceForValue(
        layers.effective.execution?.stopOnFailure,
        layers.defaults.execution?.stopOnFailure,
        layers.global?.execution?.stopOnFailure,
        layers.project?.execution?.stopOnFailure,
      ),
      editKind: "toggle",
      options: ["off", "on"],
      description: "Whether the sequential run loop stops after the first failed feature.",
      effectiveValueText: stringifyBoolean(layers.effective.execution?.stopOnFailure),
      defaultValueText: stringifyBoolean(layers.defaults.execution?.stopOnFailure),
      globalValueText: stringifyBoolean(layers.global?.execution?.stopOnFailure),
      projectValueText: stringifyBoolean(layers.project?.execution?.stopOnFailure),
    },
    {
      keyPath: "overview.pendingFeatures",
      label: "Runnable features",
      section: "Plan",
      valueText: String(features.filter((feature) => feature.status === "pending").length),
      source: "none",
      editKind: "readonly",
      description: "Pending features that may participate in the next run.",
      effectiveValueText: String(features.filter((feature) => feature.status === "pending").length),
    },
  ];
}

function buildEffectiveRow(
  keyPath: string,
  layers: ConfigLayers,
  effectiveValue: unknown,
  defaultValue: unknown,
  globalValue: unknown,
  projectValue: unknown,
  workerSlugs: readonly string[],
): MissionControlConfigRow {
  const meta = getEditMeta(keyPath, effectiveValue, workerSlugs);
  return {
    keyPath,
    label: keyPath,
    section: sectionForKey(keyPath),
    valueText: displayValue(meta.editKind, effectiveValue),
    source: provenanceForValue(effectiveValue, defaultValue, globalValue, projectValue),
    editKind: meta.editKind,
    options: meta.options,
    description: meta.description,
    effectiveValueText: displayValue(meta.editKind, effectiveValue),
    defaultValueText: displayValue(meta.editKind, defaultValue),
    globalValueText: displayValue(meta.editKind, globalValue),
    projectValueText: displayValue(meta.editKind, projectValue),
  };
}

function buildScopeRows(
  scope: "project" | "global" | "default",
  scopeValues: Readonly<Record<string, unknown>>,
  effectiveValues: Readonly<Record<string, unknown>>,
  workerSlugs: readonly string[],
): readonly MissionControlConfigRow[] {
  const paths = Object.keys(scopeValues).sort();
  if (paths.length === 0) {
    return [{
      keyPath: `${scope}.empty`,
      label: scope === "default" ? "Defaults" : `No ${scope} config`,
      section: "Status",
      valueText: scope === "default" ? "Built-in defaults available" : "Not configured",
      source: scope === "default" ? "default" : "none",
      editKind: "readonly",
      description: `Values defined in the ${scope} scope.`,
      effectiveValueText: scope === "default" ? "Built-in defaults available" : "Not configured",
    }];
  }

  return paths.map((path) => {
    const meta = getEditMeta(path, scopeValues[path], workerSlugs);
    return {
      keyPath: path,
      label: path,
      section: sectionForKey(path),
      valueText: displayValue(meta.editKind, scopeValues[path]),
      source: scope,
      editKind: scope === "default" ? "readonly" : meta.editKind,
      options: scope === "default" ? undefined : meta.options,
      description: meta.description,
      effectiveValueText: displayValue(meta.editKind, effectiveValues[path]),
      defaultValueText: scope === "default" ? displayValue(meta.editKind, scopeValues[path]) : undefined,
      globalValueText: scope === "global" ? displayValue(meta.editKind, scopeValues[path]) : undefined,
      projectValueText: scope === "project" ? displayValue(meta.editKind, scopeValues[path]) : undefined,
    };
  });
}

function buildWorkerRows(config: MaestroConfig): readonly MissionControlConfigRow[] {
  return Object.entries(config.workers ?? {}).map(([slug, worker]) => ({
    keyPath: `workers.${slug}`,
    label: slug,
    section: "Workers",
    valueText: `${worker.enabled ? "enabled" : "disabled"} · ${worker.transport} · ${worker.command} · ${worker.outputMode ?? "raw"} · ${Bun.which(worker.command) ? "ready" : "missing"}`,
    source: "mixed",
    editKind: "readonly",
    description: "Resolved worker profile and binary availability.",
    effectiveValueText: `${worker.enabled ? "enabled" : "disabled"} · ${worker.transport} · ${worker.command} · ${worker.outputMode ?? "raw"}`,
  }));
}

function buildPlanRows(
  config: MaestroConfig,
  features: readonly Feature[],
): readonly MissionControlConfigRow[] {
  const pending = features.filter((feature) => feature.status === "pending");
  const ready = pending.find((feature) => feature.dependsOn.length === 0);
  const lines = [
    `Mode: ${config.parallel?.enabled ? "parallel" : "sequential"}`,
    `Default worker: ${config.execution?.defaultWorker ?? "unset"}`,
    `Stop on failure: ${stringifyBoolean(config.execution?.stopOnFailure)}`,
    `Retry budget: ${config.execution?.retryBudget ?? 0}`,
    ready
      ? `Next ready feature: ${ready.id} · ${ready.title}`
      : "Next ready feature: none",
  ];

  return lines.map((line, index) => ({
    keyPath: `plan.${index + 1}`,
    label: line,
    section: "Execution Plan",
    valueText: line,
    source: "none",
    editKind: "readonly",
    description: "How feature run will behave with the current config.",
    effectiveValueText: line,
  }));
}

function buildDoctorRows(
  checks: readonly DoctorCheck[],
  errors: readonly { scope: string; message: string }[],
): readonly MissionControlConfigRow[] {
  const checkRows = checks.map((check) => ({
    keyPath: `doctor.${check.name}`,
    label: check.name,
    section: "Checks",
    valueText: `${check.status} · ${check.message}`,
    source: "none" as const,
    editKind: "readonly" as const,
    description: check.fix ?? "No fix available",
    effectiveValueText: `${check.status} · ${check.message}`,
  }));
  const errorRows = errors.map((error, index) => ({
    keyPath: `doctor.error.${index + 1}`,
    label: `${error.scope} config`,
    section: "YAML",
    valueText: error.message,
    source: "none" as const,
    editKind: "readonly" as const,
    description: "Fix the YAML before editing this scope from Mission Control.",
    effectiveValueText: error.message,
  }));

  return [...checkRows, ...errorRows];
}

function flattenConfig(
  input: MaestroConfig | Record<string, unknown>,
  prefix = "",
): Record<string, unknown> {
  const source = input as Record<string, unknown>;
  const result: Record<string, unknown> = {};

  for (const [key, value] of Object.entries(source)) {
    const keyPath = prefix ? `${prefix}.${key}` : key;
    if (isPlainObject(value)) {
      Object.assign(result, flattenConfig(value as Record<string, unknown>, keyPath));
    } else {
      result[keyPath] = value;
    }
  }

  return result;
}

function isPlainObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function sectionForKey(keyPath: string): string {
  if (keyPath.startsWith("execution.")) return "Execution";
  if (keyPath.startsWith("workers.")) return "Workers";
  if (keyPath.startsWith("supervision.")) return "Supervision";
  if (keyPath.startsWith("parallel.")) return "Parallel";
  if (keyPath.startsWith("sessionDetection.")) return "Session Detection";
  return "General";
}

function getEditMeta(
  keyPath: string,
  value: unknown,
  workerSlugs: readonly string[],
): { editKind: MissionControlConfigEditKind; options?: readonly string[]; description: string } {
  if (typeof value === "boolean") {
    return {
      editKind: "toggle",
      options: ["off", "on"],
      description: `Toggle ${keyPath} between off and on.`,
    };
  }

  if (keyPath === "execution.defaultWorker") {
    return {
      editKind: "enum",
      options: workerSlugs,
      description: "Choose the default worker profile for feature run.",
    };
  }

  if (keyPath === "defaultAgent") {
    return {
      editKind: "enum",
      options: [...KNOWN_AGENT_OPTIONS],
      description: "Choose the default agent slug.",
    };
  }

  if (keyPath === "supervision.level") {
    return {
      editKind: "enum",
      options: ["low", "mid", "high"],
      description: "Adjust supervision aggressiveness.",
    };
  }

  if (keyPath === "sessionDetection.staleMinutes") {
    return {
      editKind: "number-preset",
      options: ["5", "10", "15", "30", "60"],
      description: "Preset stale-session windows in minutes.",
    };
  }

  if (keyPath === "execution.retryBudget") {
    return {
      editKind: "number-preset",
      options: ["0", "1", "2", "3"],
      description: "Retry attempts allowed for feature execution.",
    };
  }

  if (keyPath === "parallel.maxConcurrent") {
    return {
      editKind: "number-preset",
      options: ["1", "2", "3", "4"],
      description: "Parallel worker cap for future execution modes.",
    };
  }

  if (keyPath.endsWith(".outputMode")) {
    return {
      editKind: "enum",
      options: ["raw", "stream-json"],
      description: "How worker stdout should be interpreted.",
    };
  }

  if (typeof value === "number") {
    return {
      editKind: "number-preset",
      options: [String(value)],
      description: `Numeric config value for ${keyPath}.`,
    };
  }

  return {
    editKind: "readonly",
    description: `Inspect the current value for ${keyPath}.`,
  };
}

function provenanceForValue(
  effectiveValue: unknown,
  defaultValue: unknown,
  globalValue: unknown,
  projectValue: unknown,
): MissionControlConfigValueSource {
  if (projectValue !== undefined && areEqual(projectValue, effectiveValue)) return "project";
  if (globalValue !== undefined && areEqual(globalValue, effectiveValue)) return "global";
  if (defaultValue !== undefined && areEqual(defaultValue, effectiveValue)) return "default";
  if (projectValue !== undefined || globalValue !== undefined) return "mixed";
  return "none";
}

function areEqual(left: unknown, right: unknown): boolean {
  return JSON.stringify(left) === JSON.stringify(right);
}

function displayValue(editKind: MissionControlConfigEditKind, value: unknown): string {
  if (editKind === "toggle") {
    return stringifyBoolean(value as boolean | undefined);
  }
  return stringifyValue(value);
}

function stringifyBoolean(value: boolean | undefined): string {
  if (value === undefined) return "unset";
  return value ? "on" : "off";
}

function stringifyValue(value: unknown): string {
  if (value === undefined) return "unset";
  if (typeof value === "string") return value;
  if (typeof value === "number" || typeof value === "boolean") return String(value);
  return JSON.stringify(value);
}
