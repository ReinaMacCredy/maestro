import type { Feature } from "@/shared/domain/legacy-mission";
import { listIgnoredProjectConfigKeys, isGlobalOnlyConfigKey } from "@/tui/shared/ui-config.js";
import type { DoctorCheck } from "@/infra/domain/status-types.js";
import type { MaestroConfig } from "@/infra/domain/config-types.js";
import type { ConfigScope, ConfigLayers } from "@/infra/ports/config.port.js";
import type {
  MissionControlConfigEditKind,
  MissionControlConfigInspector,
  MissionControlConfigRow,
  MissionControlConfigSourceBadge,
  MissionControlConfigTab,
  MissionControlConfigValueSource,
} from "./types.js";

export { isGlobalOnlyConfigKey };

const KNOWN_TABS: readonly MissionControlConfigTab[] = [
  "overview",
  "effective",
  "project",
  "global",
  "defaults",
  "plan",
  "doctor",
  "memory",
];

const TAB_LABELS: Readonly<Record<MissionControlConfigTab, string>> = {
  overview: "overview",
  effective: "effective",
  project: "project",
  global: "global",
  defaults: "defaults",
  plan: "next",
  doctor: "problems",
  memory: "memory",
};

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

const HIDDEN_CONFIG_KEY_PATTERNS = [
  /^parallel\./,
] as const;

export function resolveConfigScopeForKey(keyPath: string, scope: ConfigScope): ConfigScope {
  return isGlobalOnlyConfigKey(keyPath) ? "global" : scope;
}

interface RowCopy {
  readonly label: string;
  readonly summary: string;
  readonly impactText: string;
  readonly section?: string;
}

export function buildConfigInspector(
  layers: ConfigLayers,
  checks: readonly DoctorCheck[],
  features: readonly Feature[],
): MissionControlConfigInspector {
  const ignoredProjectConfigKeys = listIgnoredProjectConfigKeys(layers.project);
  const inspectionChecks = [
    ...checks,
    ...ignoredProjectConfigKeys.map((keyPath) => buildIgnoredProjectOverrideCheck(keyPath)),
  ];
  const effective = flattenConfig(layers.effective);
  const defaults = flattenConfig(layers.defaults);
  const project = flattenConfig(layers.project ?? {});
  const global = flattenConfig(layers.global ?? {});
  const allPaths = [...new Set([
    ...Object.keys(effective),
    ...Object.keys(defaults),
    ...Object.keys(project),
    ...Object.keys(global),
    ])]
    .filter((path) => isVisibleConfigPath(path))
    .sort();

  const planRows = buildPlanRows(features);

  const rowsByTab = {
    overview: buildOverviewRows(layers, inspectionChecks, planRows),
    effective: allPaths.map((path) =>
      buildConfigValueRow(
        path,
        effective[path],
        defaults[path],
        global[path],
        project[path],
        "effective",
      )
    ),
    project: buildScopeRows("project", project, effective),
    global: buildScopeRows("global", global, effective),
    defaults: buildScopeRows("default", defaults, effective),
    plan: planRows,
    doctor: buildDoctorRows(inspectionChecks, layers.errors),
    memory: buildMemoryConfigRows(effective, defaults, global, project),
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
  query?: string,
): readonly MissionControlConfigRow[] {
  if (!inspector) return [];
  const rows = inspector.rowsByTab[tab] ?? [];
  return filterConfigRows(rows, query);
}

export function getConfigTabDisplayLabel(tab: MissionControlConfigTab): string {
  return TAB_LABELS[tab] ?? tab;
}

function filterConfigRows(
  rows: readonly MissionControlConfigRow[],
  query?: string,
): readonly MissionControlConfigRow[] {
  const normalizedQuery = (query ?? "").trim().toLowerCase();
  if (normalizedQuery.length === 0) return rows;

  return rows.filter((row) =>
    [
      row.label,
      row.keyPath,
      row.summary,
      row.valueText,
      row.displayValueText,
      row.section,
    ]
      .filter(Boolean)
      .some((value) => value.toLowerCase().includes(normalizedQuery))
  );
}

function buildOverviewRows(
  layers: ConfigLayers,
  checks: readonly DoctorCheck[],
  planRows: readonly MissionControlConfigRow[],
): readonly MissionControlConfigRow[] {
  const overviewPlanRows = planRows.map((row) => ({
    ...row,
    section: "What happens next",
  }));

  const problemCount = checks.filter((check) => check.status !== "ok").length + layers.errors.length;
  const problemsRow = buildReadonlyRow({
    keyPath: "overview.problems",
    label: "Problems",
    section: "Problems",
    rawValue: problemCount > 0 ? `${problemCount}` : "none",
    displayValue: problemCount > 0 ? `${problemCount} ${problemCount === 1 ? "warning" : "issues"}` : "No problems",
    summary: "Warnings and errors that could affect config editing.",
    impactText: "Fix these before trusting the next run.",
    source: "none",
  });

  const quickRows = [
    buildConfigValueRow(
      "ui.missionControl.backgroundMode",
      layers.effective.ui?.missionControl?.backgroundMode,
      layers.defaults.ui?.missionControl?.backgroundMode,
      layers.global?.ui?.missionControl?.backgroundMode,
      layers.project?.ui?.missionControl?.backgroundMode,
      "overview",
    ),
  ];

  return [...quickRows, ...overviewPlanRows, problemsRow];
}

function buildConfigValueRow(
  keyPath: string,
  effectiveValue: unknown,
  defaultValue: unknown,
  globalValue: unknown,
  projectValue: unknown,
  tab: MissionControlConfigTab,
): MissionControlConfigRow {
  const editMeta = getEditMeta(keyPath, effectiveValue);
  const source = provenanceForValue(effectiveValue, defaultValue, globalValue, projectValue);
  const copy = getRowCopy(keyPath, tab);
  const section = copy.section ?? sectionForKey(keyPath);
  const effectiveRaw = stringifyConfigValue(keyPath, editMeta.editKind, effectiveValue);
  const effectiveDisplayValue = displayValueForKey(keyPath, editMeta.editKind, effectiveValue);
  const projectDisplayValue = displayValueForKey(keyPath, editMeta.editKind, projectValue);
  const globalDisplayValue = displayValueForKey(keyPath, editMeta.editKind, globalValue);
  const defaultDisplayValue = displayValueForKey(keyPath, editMeta.editKind, defaultValue);

  return {
    keyPath,
    label: copy.label,
    section,
    valueText: effectiveRaw,
    displayValueText: effectiveDisplayValue,
    source,
    sourceBadge: sourceBadgeForValueSource(source),
    editKind: editMeta.editKind,
    editKindLabel: editLabelForKind(editMeta.editKind),
    options: editMeta.options,
    description: editMeta.description,
    summary: copy.summary,
    impactText: copy.impactText,
    effectiveValueText: effectiveRaw,
    effectiveDisplayValueText: effectiveDisplayValue,
    projectValueText: stringifyConfigValue(keyPath, editMeta.editKind, projectValue),
    projectDisplayValueText: projectDisplayValue,
    globalValueText: stringifyConfigValue(keyPath, editMeta.editKind, globalValue),
    globalDisplayValueText: globalDisplayValue,
    defaultValueText: stringifyConfigValue(keyPath, editMeta.editKind, defaultValue),
    defaultDisplayValueText: defaultDisplayValue,
  };
}

function buildScopeRows(
  scope: "project" | "global" | "default",
  scopeValues: Readonly<Record<string, unknown>>,
  effectiveValues: Readonly<Record<string, unknown>>,
): readonly MissionControlConfigRow[] {
  const paths = Object.keys(scopeValues).sort();
  const visiblePaths = paths.filter((path) => isVisibleConfigPath(path));
  if (visiblePaths.length === 0) {
    return [buildReadonlyRow({
      keyPath: `${scope}.empty`,
      label: scope === "default" ? "Built-in defaults" : `No ${scope} settings`,
      section: scope === "default" ? "Defaults" : "Settings",
      rawValue: scope === "default" ? "available" : "empty",
      displayValue: scope === "default" ? "Built-in defaults are available" : "No settings saved here",
      summary: scope === "default"
        ? "These values are used when nothing overrides them."
        : `Settings saved in ${scope} config appear here.`,
      impactText: scope === "default"
        ? "These values are read-only in Mission Control."
        : `Save a change to ${scope} config to populate this tab.`,
      source: scope === "default" ? "default" : "none",
    })];
  }

  return visiblePaths.map((path) => {
    const editMeta = getEditMeta(path, scopeValues[path]);
    const copy = getRowCopy(path, scope);
    const ignoredProjectOverride = scope === "project" && isGlobalOnlyConfigKey(path);
    const editKind = scope === "default" || ignoredProjectOverride ? "readonly" : editMeta.editKind;
    const description = ignoredProjectOverride
      ? "This setting is global-only. Project config values are ignored."
      : editMeta.description;
    return {
      keyPath: path,
      label: copy.label,
      section: copy.section ?? sectionForKey(path),
      valueText: stringifyConfigValue(path, editMeta.editKind, scopeValues[path]),
      displayValueText: displayValueForKey(path, editMeta.editKind, scopeValues[path]),
      source: scope,
      sourceBadge: sourceBadgeForValueSource(scope),
      editKind,
      editKindLabel: editLabelForKind(editKind),
      options: editKind === "readonly" ? undefined : editMeta.options,
      description,
      summary: copy.summary,
      impactText: copy.impactText,
      effectiveValueText: stringifyConfigValue(path, editMeta.editKind, effectiveValues[path]),
      effectiveDisplayValueText: displayValueForKey(path, editMeta.editKind, effectiveValues[path]),
      defaultValueText: scope === "default" ? stringifyConfigValue(path, editMeta.editKind, scopeValues[path]) : undefined,
      defaultDisplayValueText: scope === "default" ? displayValueForKey(path, editMeta.editKind, scopeValues[path]) : undefined,
      globalValueText: scope === "global" ? stringifyConfigValue(path, editMeta.editKind, scopeValues[path]) : undefined,
      globalDisplayValueText: scope === "global" ? displayValueForKey(path, editMeta.editKind, scopeValues[path]) : undefined,
      projectValueText: scope === "project" ? stringifyConfigValue(path, editMeta.editKind, scopeValues[path]) : undefined,
      projectDisplayValueText: scope === "project" ? displayValueForKey(path, editMeta.editKind, scopeValues[path]) : undefined,
    };
  });
}

function buildPlanRows(
  features: readonly Feature[],
): readonly MissionControlConfigRow[] {
  const pending = features.filter((feature) => feature.status === "pending");
  const featureById = new Map(features.map((feature) => [feature.id, feature]));
  const ready = pending.find((feature) => feature.dependsOn.every((dependencyId) =>
    featureById.get(dependencyId)?.status === "done"
  ));

  return [
    buildReadonlyRow({
      keyPath: "plan.runMode",
      label: "Run mode",
      section: "What happens next",
      rawValue: "sequential",
      displayValue: "sequential",
      summary: "Shows whether Maestro would run tasks one at a time or in parallel.",
      impactText: "This changes how the next feature run will be scheduled.",
      source: "none",
    }),
    buildReadonlyRow({
      keyPath: "plan.nextTask",
      label: "Next task",
      section: "What happens next",
      rawValue: ready ? `${ready.id} ${ready.title}` : "none",
      displayValue: ready ? `${ready.id} ${ready.title}` : "No ready task",
      summary: "The next task Maestro would try to run with the current mission state.",
      impactText: ready
        ? `If you start a run now, Maestro will try ${ready.id} first.`
        : "No task is ready to run right now.",
      source: "none",
    }),
  ];
}

function buildDoctorRows(
  checks: readonly DoctorCheck[],
  errors: readonly { scope: string; message: string }[],
): readonly MissionControlConfigRow[] {
  const checkRows = checks
    .filter((check) => check.status !== "ok")
    .map((check) => buildReadonlyRow({
      keyPath: `doctor.${check.name}`,
      label: humanizeCheckName(check.name),
      section: "Problems",
      rawValue: check.message,
      displayValue: check.message,
      summary: check.fix ?? "Review this warning before trusting the next run.",
      impactText: check.message,
      source: "none",
    }));

  const errorRows = errors.map((error, index) => buildReadonlyRow({
    keyPath: `doctor.error.${index + 1}`,
    label: `${capitalize(error.scope)} config error`,
    section: "Problems",
    rawValue: error.message,
    displayValue: error.message,
    summary: "Mission Control cannot safely edit this config file until the YAML is fixed.",
    impactText: "Fix the config file first, then try editing again.",
    source: "none",
  }));

  return checkRows.length > 0 || errorRows.length > 0
    ? [...checkRows, ...errorRows]
    : [buildReadonlyRow({
      keyPath: "doctor.clear",
      label: "No problems",
      section: "Problems",
      rawValue: "clear",
      displayValue: "Everything looks good",
      summary: "Maestro did not detect config problems.",
      impactText: "You can change settings with confidence.",
      source: "none",
    })];
}

function buildReadonlyRow(options: {
  keyPath: string;
  label: string;
  section: string;
  rawValue: string;
  displayValue: string;
  summary: string;
  impactText: string;
  source: MissionControlConfigValueSource;
}): MissionControlConfigRow {
  return {
    keyPath: options.keyPath,
    label: options.label,
    section: options.section,
    valueText: options.rawValue,
    displayValueText: options.displayValue,
    source: options.source,
    sourceBadge: sourceBadgeForValueSource(options.source),
    editKind: "readonly",
    editKindLabel: editLabelForKind("readonly"),
    description: options.summary,
    summary: options.summary,
    impactText: options.impactText,
    effectiveValueText: options.rawValue,
    effectiveDisplayValueText: options.displayValue,
    projectValueText: undefined,
    projectDisplayValueText: undefined,
    globalValueText: undefined,
    globalDisplayValueText: undefined,
    defaultValueText: undefined,
    defaultDisplayValueText: undefined,
  };
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

function isVisibleConfigPath(keyPath: string): boolean {
  return !HIDDEN_CONFIG_KEY_PATTERNS.some((pattern) => pattern.test(keyPath));
}

function isPlainObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function sectionForKey(keyPath: string): string {
  if (keyPath.startsWith("ui.")) return "Interface";
  return "General";
}

function getEditMeta(
  keyPath: string,
  value: unknown,
): { editKind: MissionControlConfigEditKind; options?: readonly string[]; description: string } {
  if (typeof value === "boolean") {
    return {
      editKind: "toggle",
      options: ["off", "on"],
      description: `Toggle ${keyPath} between off and on.`,
    };
  }

  if (keyPath === "ui.missionControl.backgroundMode") {
    return {
      editKind: "enum",
      options: ["solid", "terminal"],
      description: "Choose whether Mission Control paints solid panel backgrounds or uses the terminal background.",
    };
  }

  if (keyPath === "defaultAgent") {
    return {
      editKind: "enum",
      options: [...KNOWN_AGENT_OPTIONS],
      description: "Choose the default agent slug.",
    };
  }

  if (keyPath === "memory.corrections.matching") {
    return {
      editKind: "enum",
      options: ["keyword", "ast-grep", "both"],
      description: "Choose how Maestro matches saved corrections to the current task.",
    };
  }

  if (keyPath === "memory.corrections.auto_capture") {
    return {
      editKind: "enum",
      options: ["prompt", "auto", "off"],
      description: "Choose when Maestro captures corrections automatically.",
    };
  }

  if (keyPath === "memory.corrections.severity_default") {
    return {
      editKind: "enum",
      options: ["soft", "hard"],
      description: "Choose the default severity for newly captured corrections.",
    };
  }

  if (keyPath === "memory.ratchet.enforcement") {
    return {
      editKind: "enum",
      options: ["warn", "block"],
      description: "Choose whether ratchet failures warn or block progress.",
    };
  }

  if (keyPath === "memory.learnings.compile_threshold") {
    return {
      editKind: "number-preset",
      options: ["1", "3", "5", "8", "10"],
      description: "How many raw learning entries should accumulate before prompting compilation.",
    };
  }

  if (keyPath === "memory.learnings.max_age_days") {
    return {
      editKind: "number-preset",
      options: ["3", "7", "14", "30"],
      description: "How long compiled learnings remain fresh before Maestro warns that they are stale.",
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

function getRowCopy(keyPath: string, tab: MissionControlConfigTab | "project" | "global" | "default"): RowCopy {
  switch (keyPath) {
    case "ui.missionControl.backgroundMode":
      return {
        label: "Background mode",
        summary: tab === "project"
          ? "This setting is global-only. Project config values are ignored."
          : "Choose whether Mission Control uses solid panel fills or the terminal background.",
        impactText: tab === "project"
          ? "Move this value to global config if you want it to affect Mission Control."
          : "Terminal mode shows your terminal background through normal dashboard chrome; modals stay solid.",
        section: tab === "overview" ? "Quick settings" : undefined,
      };
    case "memory.enabled":
      return {
        label: "Memory enabled",
        summary: "Master toggle for the memory system.",
        impactText: "Disabling this turns off correction recall, learnings, ratchet checks, and graph context.",
        section: "Memory",
      };
    case "memory.corrections.enabled":
      return {
        label: "Corrections enabled",
        summary: "Capture and recall corrections for future tasks.",
        impactText: "Turning this off stops Maestro from saving or matching corrections.",
        section: "Corrections",
      };
    case "memory.corrections.matching":
      return {
        label: "Matching",
        summary: "How Maestro matches saved corrections to the current task.",
        impactText: "Broader matching recalls more rules; narrower matching reduces noise.",
        section: "Corrections",
      };
    case "memory.corrections.auto_capture":
      return {
        label: "Auto capture",
        summary: "When Maestro should capture corrections automatically.",
        impactText: "Prompt is safer; auto is faster; off requires explicit capture commands.",
        section: "Corrections",
      };
    case "memory.corrections.severity_default":
      return {
        label: "Default severity",
        summary: "Default severity for new corrections.",
        impactText: "Hard corrections are always recalled even when the task match is weak.",
        section: "Corrections",
      };
    case "memory.learnings.enabled":
      return {
        label: "Learnings enabled",
        summary: "Store raw session learnings for later compilation.",
        impactText: "Turning this off stops the learning log from growing.",
        section: "Learnings",
      };
    case "memory.learnings.compile_threshold":
      return {
        label: "Compile threshold",
        summary: "How many raw learning entries should accumulate before compilation is suggested.",
        impactText: "Lower values compile sooner; higher values keep more raw history around.",
        section: "Learnings",
      };
    case "memory.learnings.max_age_days":
      return {
        label: "Max age",
        summary: "How long compiled learnings remain fresh.",
        impactText: "Older compiled learnings trigger stale warnings in linting and the TUI.",
        section: "Learnings",
      };
    case "memory.ratchet.enabled":
      return {
        label: "Ratchet enabled",
        summary: "Enable the regression ratchet system.",
        impactText: "Promoted corrections become tracked checks when the ratchet is enabled.",
        section: "Ratchet",
      };
    case "memory.ratchet.enforcement":
      return {
        label: "Enforcement",
        summary: "How ratchet failures are handled.",
        impactText: "Warn keeps the run moving; block stops progress until the regression is fixed.",
        section: "Ratchet",
      };
    case "memory.graph.enabled":
      return {
        label: "Project graph",
        summary: "Enable cross-project relationship context.",
        impactText: "Turning this off removes project-link context from memory and TUI graph views.",
        section: "Graph",
      };
    default:
      return {
        label: humanizeConfigKey(keyPath),
        summary: `Controls ${humanizeConfigKey(keyPath).toLowerCase()}.`,
        impactText: "Changing this will affect future Maestro behavior.",
      };
  }
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

function stringifyConfigValue(
  keyPath: string,
  editKind: MissionControlConfigEditKind,
  value: unknown,
): string {
  if (isSensitiveConfigKey(keyPath) && value !== undefined) {
    return "[hidden]";
  }
  if (editKind === "toggle") {
    return stringifyBoolean(value as boolean | undefined);
  }
  return stringifyValue(value);
}

function displayValueForKey(
  keyPath: string,
  editKind: MissionControlConfigEditKind,
  value: unknown,
): string {
  const raw = stringifyConfigValue(keyPath, editKind, value);
  if (keyPath === "ui.missionControl.backgroundMode" && raw === "terminal") return "terminal background";
  if (keyPath === "memory.learnings.compile_threshold" && raw !== "unset") return `${raw} entries`;
  if (keyPath === "memory.learnings.max_age_days" && raw !== "unset") return `${raw} days`;
  return raw;
}

function buildIgnoredProjectOverrideCheck(keyPath: string): DoctorCheck {
  return {
    name: `ignored-${keyPath.replaceAll(".", "-")}`,
    status: "warn",
    message: `${humanizeConfigKey(keyPath)} is set in project config but only global config is used`,
    fix: "Remove the project value or set it in ~/.maestro/config.yaml instead",
  };
}

function isSensitiveConfigKey(keyPath: string): boolean {
  return keyPath.includes(".env.")
    || keyPath.includes(".headers.")
    || /(?:token|secret|password|api[-_]?key)$/i.test(keyPath);
}

function editLabelForKind(editKind: MissionControlConfigEditKind): string {
  switch (editKind) {
    case "toggle":
      return "on/off";
    case "enum":
      return "choice";
    case "number-preset":
      return "number";
    case "readonly":
    default:
      return "read only";
  }
}

function sourceBadgeForValueSource(source: MissionControlConfigValueSource): MissionControlConfigSourceBadge {
  switch (source) {
    case "project":
      return "P";
    case "global":
      return "G";
    case "default":
      return "D";
    case "mixed":
      return "M";
    case "none":
    default:
      return "";
  }
}

function humanizeConfigKey(keyPath: string): string {
  const leaf = keyPath.split(".").at(-1) ?? keyPath;
  return capitalize(leaf.replace(/([A-Z])/g, " $1").replace(/[-_]/g, " ").trim());
}

function humanizeCheckName(name: string): string {
  return capitalize(name.replace(/[-_]/g, " "));
}

function capitalize(value: string): string {
  return value.length === 0 ? value : value[0]!.toUpperCase() + value.slice(1);
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

function buildMemoryConfigRows(
  effective: Readonly<Record<string, unknown>>,
  defaults: Readonly<Record<string, unknown>>,
  global: Readonly<Record<string, unknown>>,
  project: Readonly<Record<string, unknown>>,
): readonly MissionControlConfigRow[] {
  const memoryKeys = [
    "memory.enabled",
    "memory.corrections.enabled",
    "memory.corrections.matching",
    "memory.corrections.auto_capture",
    "memory.corrections.severity_default",
    "memory.learnings.enabled",
    "memory.learnings.compile_threshold",
    "memory.learnings.max_age_days",
    "memory.ratchet.enabled",
    "memory.ratchet.enforcement",
    "memory.graph.enabled",
  ] as const;

  if (!memoryKeys.some((keyPath) => effective[keyPath] !== undefined || defaults[keyPath] !== undefined || global[keyPath] !== undefined || project[keyPath] !== undefined)) {
    return [buildReadonlyRow({
      keyPath: "memory",
      label: "Memory system",
      section: "Memory",
      rawValue: "not configured",
      displayValue: "Not configured",
      summary: "Memory system is using defaults.",
      impactText: "Add memory config to customize behavior.",
      source: "default",
    })];
  }

  return memoryKeys.map((keyPath) =>
    buildConfigValueRow(
      keyPath,
      effective[keyPath],
      defaults[keyPath],
      global[keyPath],
      project[keyPath],
      "memory",
    )
  );
}
