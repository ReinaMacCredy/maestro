import type { Feature } from "../../domain/mission-types.js";
import type { DoctorCheck, MaestroConfig } from "../../domain/types.js";
import type { WorkerConfig } from "../../domain/worker-types.js";
import { formatWorkerLabel, getWorkerGuidance } from "../../domain/worker-presentation.js";
import type { ConfigLayers } from "../../ports/config.port.js";
import type {
  MissionControlConfigEditKind,
  MissionControlConfigInspector,
  MissionControlConfigRow,
  MissionControlConfigWorkerChoice,
  MissionControlConfigSourceBadge,
  MissionControlConfigTab,
  MissionControlConfigValueSource,
  MissionControlWorkerHealthRow,
  MissionControlWorkerHealthStatus,
} from "./types.js";
import { recommendWorkerFit } from "../../usecases/worker-fit-recommendation.usecase.js";

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

const TAB_LABELS: Readonly<Record<MissionControlConfigTab, string>> = {
  overview: "overview",
  effective: "effective",
  project: "project",
  global: "global",
  defaults: "defaults",
  workers: "workers",
  plan: "next",
  doctor: "problems",
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
  workerHealth: readonly MissionControlWorkerHealthRow[] = [],
): MissionControlConfigInspector {
  const workerHealthBySlug = new Map(workerHealth.map((row) => [row.slug, row]));
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

  const workerSlugs = [...new Set([
    ...Object.keys(layers.effective.workers ?? {}),
    ...workerHealth.map((row) => row.slug),
  ])].sort();
  const rowsByTab = {
    overview: buildOverviewRows(layers, checks, features, workerHealthBySlug),
    effective: allPaths.map((path) =>
      buildConfigValueRow(
        path,
        effective[path],
        defaults[path],
        global[path],
        project[path],
        workerSlugs,
        "effective",
        layers.effective.workers,
        features,
        workerHealthBySlug,
      )
    ),
    project: buildScopeRows("project", project, effective, workerSlugs, layers.effective.workers, features, workerHealthBySlug),
    global: buildScopeRows("global", global, effective, workerSlugs, layers.effective.workers, features, workerHealthBySlug),
    defaults: buildScopeRows("default", defaults, effective, workerSlugs, layers.effective.workers, features, workerHealthBySlug),
    workers: buildWorkerRows(layers.effective, features, workerHealthBySlug),
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
  features: readonly Feature[],
  workerHealthBySlug: ReadonlyMap<string, MissionControlWorkerHealthRow>,
): readonly MissionControlConfigRow[] {
  const workerRows = buildWorkerRows(layers.effective, features, workerHealthBySlug).map((row) => ({
    ...row,
    section: "Workers",
  }));
  const planRows = buildPlanRows(layers.effective, features).map((row) => ({
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
    summary: "Warnings and errors that could affect config editing or worker choice.",
    impactText: "Fix these before trusting the next run.",
    source: "none",
  });

  const quickRows = [
    buildConfigValueRow(
      "execution.defaultWorker",
      layers.effective.execution?.defaultWorker,
      layers.defaults.execution?.defaultWorker,
      layers.global?.execution?.defaultWorker,
      layers.project?.execution?.defaultWorker,
      Object.keys(layers.effective.workers ?? {}),
      "overview",
      layers.effective.workers,
      features,
      workerHealthBySlug,
    ),
    buildConfigValueRow(
      "execution.stopOnFailure",
      layers.effective.execution?.stopOnFailure,
      layers.defaults.execution?.stopOnFailure,
      layers.global?.execution?.stopOnFailure,
      layers.project?.execution?.stopOnFailure,
      Object.keys(layers.effective.workers ?? {}),
      "overview",
    ),
    buildConfigValueRow(
      "execution.retryBudget",
      layers.effective.execution?.retryBudget,
      layers.defaults.execution?.retryBudget,
      layers.global?.execution?.retryBudget,
      layers.project?.execution?.retryBudget,
      Object.keys(layers.effective.workers ?? {}),
      "overview",
    ),
    buildConfigValueRow(
      "supervision.level",
      layers.effective.supervision?.level,
      layers.defaults.supervision?.level,
      layers.global?.supervision?.level,
      layers.project?.supervision?.level,
      Object.keys(layers.effective.workers ?? {}),
      "overview",
    ),
    buildConfigValueRow(
      "parallel.enabled",
      layers.effective.parallel?.enabled,
      layers.defaults.parallel?.enabled,
      layers.global?.parallel?.enabled,
      layers.project?.parallel?.enabled,
      Object.keys(layers.effective.workers ?? {}),
      "overview",
    ),
  ];

  return [...quickRows, ...workerRows, ...planRows, problemsRow];
}

function buildConfigValueRow(
  keyPath: string,
  effectiveValue: unknown,
  defaultValue: unknown,
  globalValue: unknown,
  projectValue: unknown,
  workerSlugs: readonly string[],
  tab: MissionControlConfigTab,
  workers?: MaestroConfig["workers"],
  features: readonly Feature[] = [],
  workerHealthBySlug: ReadonlyMap<string, MissionControlWorkerHealthRow> = new Map(),
): MissionControlConfigRow {
  const editMeta = getEditMeta(keyPath, effectiveValue, workerSlugs);
  const source = provenanceForValue(effectiveValue, defaultValue, globalValue, projectValue);
  const copy = getRowCopy(keyPath, tab);
  const section = copy.section ?? sectionForKey(keyPath);
  const effectiveDisplayValue = displayValueForKey(keyPath, editMeta.editKind, effectiveValue);
  const projectDisplayValue = displayValueForKey(keyPath, editMeta.editKind, projectValue);
  const globalDisplayValue = displayValueForKey(keyPath, editMeta.editKind, globalValue);
  const defaultDisplayValue = displayValueForKey(keyPath, editMeta.editKind, defaultValue);

  return {
    keyPath,
    label: copy.label,
    section,
    valueText: stringifyConfigValue(keyPath, editMeta.editKind, effectiveValue),
    displayValueText: effectiveDisplayValue,
    source,
    sourceBadge: sourceBadgeForValueSource(source),
    editKind: editMeta.editKind,
    editKindLabel: editLabelForKind(editMeta.editKind),
    options: editMeta.options,
    description: editMeta.description,
    summary: copy.summary,
    impactText: copy.impactText,
    effectiveValueText: stringifyConfigValue(keyPath, editMeta.editKind, effectiveValue),
    effectiveDisplayValueText: effectiveDisplayValue,
    projectValueText: stringifyConfigValue(keyPath, editMeta.editKind, projectValue),
    projectDisplayValueText: projectDisplayValue,
    globalValueText: stringifyConfigValue(keyPath, editMeta.editKind, globalValue),
    globalDisplayValueText: globalDisplayValue,
    defaultValueText: stringifyConfigValue(keyPath, editMeta.editKind, defaultValue),
    defaultDisplayValueText: defaultDisplayValue,
    workerChoices: keyPath === "execution.defaultWorker"
      ? buildWorkerChoices(workerSlugs, workers, features, workerHealthBySlug)
      : undefined,
  };
}

function buildScopeRows(
  scope: "project" | "global" | "default",
  scopeValues: Readonly<Record<string, unknown>>,
  effectiveValues: Readonly<Record<string, unknown>>,
  workerSlugs: readonly string[],
  workers?: MaestroConfig["workers"],
  features: readonly Feature[] = [],
  workerHealthBySlug: ReadonlyMap<string, MissionControlWorkerHealthRow> = new Map(),
): readonly MissionControlConfigRow[] {
  const paths = Object.keys(scopeValues).sort();
  if (paths.length === 0) {
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

  return paths.map((path) => {
    const editMeta = getEditMeta(path, scopeValues[path], workerSlugs);
    const copy = getRowCopy(path, scope);
    return {
      keyPath: path,
      label: copy.label,
      section: copy.section ?? sectionForKey(path),
      valueText: stringifyConfigValue(path, editMeta.editKind, scopeValues[path]),
      displayValueText: displayValueForKey(path, editMeta.editKind, scopeValues[path]),
      source: scope,
      sourceBadge: sourceBadgeForValueSource(scope),
      editKind: scope === "default" ? "readonly" : editMeta.editKind,
      editKindLabel: scope === "default" ? editLabelForKind("readonly") : editLabelForKind(editMeta.editKind),
      options: scope === "default" ? undefined : editMeta.options,
      description: editMeta.description,
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
      workerChoices: path === "execution.defaultWorker"
        ? buildWorkerChoices(workerSlugs, workers, features, workerHealthBySlug)
        : undefined,
    };
  });
}

function buildWorkerRows(
  config: MaestroConfig,
  features: readonly Feature[],
  workerHealthBySlug: ReadonlyMap<string, MissionControlWorkerHealthRow>,
): readonly MissionControlConfigRow[] {
  const nextFeature = features.find((feature) => feature.status === "pending");
  return Object.entries(config.workers ?? {}).map(([slug, worker]) => {
    const health = workerHealthBySlug.get(slug) ?? fallbackWorkerHealth(slug, worker);
    const stateLabel = health.status;
    const copy: RowCopy = {
      label: health.label,
      summary: `${health.label} is a worker Maestro can choose for task execution.`,
      impactText: workerImpactText(health.status, nextFeature?.id),
      section: "Workers",
    };
      return {
        keyPath: `workers.${slug}`,
        label: copy.label,
        section: copy.section ?? "Workers",
        valueText: stateLabel,
        displayValueText: stateLabel,
        source: "mixed",
      sourceBadge: sourceBadgeForValueSource("mixed"),
      editKind: "readonly",
      editKindLabel: editLabelForKind("readonly"),
      description: health.detail,
      summary: health.summary || copy.summary,
      impactText: health.detail === health.status ? copy.impactText : `${copy.impactText} ${health.detail}`.trim(),
        effectiveValueText: stateLabel,
        effectiveDisplayValueText: stateLabel,
        projectValueText: undefined,
        globalValueText: undefined,
        defaultValueText: undefined,
      };
  });
}

function buildPlanRows(
  config: MaestroConfig,
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
      rawValue: config.parallel?.enabled ? "parallel" : "sequential",
      displayValue: config.parallel?.enabled ? "parallel" : "sequential",
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
      summary: "Maestro did not detect config or worker problems.",
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

function isPlainObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function sectionForKey(keyPath: string): string {
  if (keyPath.startsWith("execution.")) return "Execution";
  if (keyPath.startsWith("workers.")) return "Workers";
  if (keyPath.startsWith("supervision.")) return "Supervision";
  if (keyPath.startsWith("parallel.")) return "Parallel";
  if (keyPath.startsWith("sessionDetection.")) return "Session detection";
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

function getRowCopy(keyPath: string, tab: MissionControlConfigTab | "project" | "global" | "default"): RowCopy {
  switch (keyPath) {
    case "execution.defaultWorker":
      return {
        label: "Default worker",
        summary: "Maestro uses this worker unless you choose a different one for a run.",
        impactText: "This changes which worker runs the next task by default.",
        section: tab === "overview" ? "Quick settings" : undefined,
      };
    case "execution.stopOnFailure":
      return {
        label: "Stop on failure",
        summary: "Choose whether Maestro stops after the first failed task.",
        impactText: "If this is on, the run stops on the first failure.",
        section: tab === "overview" ? "Quick settings" : undefined,
      };
    case "execution.retryBudget":
      return {
        label: "Retry attempts",
        summary: "How many retry attempts Maestro is allowed to make.",
        impactText: "Higher numbers allow more retries before a task is marked blocked.",
        section: tab === "overview" ? "Quick settings" : undefined,
      };
    case "supervision.level":
      return {
        label: "Watch level",
        summary: "How closely Maestro watches running workers.",
        impactText: "Higher levels check more aggressively for stale or failed workers.",
        section: tab === "overview" ? "Quick settings" : undefined,
      };
    case "parallel.enabled":
      return {
        label: "Run in parallel",
        summary: "Choose whether independent tasks can run at the same time.",
        impactText: "Turning this on can speed up runs when tasks do not conflict.",
        section: tab === "overview" ? "Quick settings" : undefined,
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
  if (keyPath === "supervision.level" && raw === "mid") return "medium";
  return raw;
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

function buildWorkerChoices(
  workerSlugs: readonly string[],
  workers: MaestroConfig["workers"] | undefined,
  features: readonly Feature[],
  workerHealthBySlug: ReadonlyMap<string, MissionControlWorkerHealthRow>,
): readonly MissionControlConfigWorkerChoice[] {
  return workerSlugs.map((slug) => {
    const worker = workers?.[slug];
    const health = workerHealthBySlug.get(slug) ?? fallbackWorkerHealth(slug, worker);
    return {
      slug,
      label: health.label,
      availability: health.status,
      availabilityDetail: health.detail,
      summary: health.summary,
      bestFor: health.bestFor,
      tradeoffs: health.tradeoffs,
      recommendation: recommendWorkerFit(slug, features),
    };
  });
}

function fallbackWorkerHealth(
  slug: string,
  worker: WorkerConfig | undefined,
): MissionControlWorkerHealthRow {
  const guidance = getWorkerGuidance(slug);
  if (!worker) {
    return {
      slug,
      label: formatWorkerLabel(slug),
      status: "missing",
      detail: "Worker profile is missing from config.",
      lastCheckedAt: "",
      checks: [],
      summary: guidance.summary,
      bestFor: guidance.bestFor,
      tradeoffs: guidance.tradeoffs,
    };
  }

  if (!worker.enabled) {
    return {
      slug,
      label: formatWorkerLabel(slug),
      status: "disabled",
      detail: "Worker is disabled in config.",
      lastCheckedAt: "",
      checks: [],
      summary: guidance.summary,
      bestFor: guidance.bestFor,
      tradeoffs: guidance.tradeoffs,
    };
  }

  if (worker.transport === "cli") {
    return {
      slug,
      label: formatWorkerLabel(slug),
      status: Bun.which(worker.command) ? "ready" : "missing",
      detail: Bun.which(worker.command) ? "ready to run" : `Command not found: ${worker.command}`,
      lastCheckedAt: "",
      checks: [],
      summary: guidance.summary,
      bestFor: guidance.bestFor,
      tradeoffs: guidance.tradeoffs,
    };
  }

  return {
    slug,
    label: formatWorkerLabel(slug),
    status: worker.url ? "degraded" : "missing",
    detail: worker.url ? "Health has not been checked yet." : "Missing agent endpoint.",
    lastCheckedAt: "",
    checks: [],
    summary: guidance.summary,
    bestFor: guidance.bestFor,
    tradeoffs: guidance.tradeoffs,
  };
}

function workerImpactText(
  status: MissionControlWorkerHealthStatus,
  nextFeatureId?: string,
): string {
  switch (status) {
    case "ready":
      return `Available for future runs${nextFeatureId ? `, including ${nextFeatureId}` : ""}.`;
    case "busy":
      return "Already active on this mission. Maestro can still use it later.";
    case "degraded":
      return "This worker responds, but something looks unhealthy. Check it before relying on it.";
    case "missing":
      return "Install or repair this worker command before expecting Maestro to use it.";
    case "disabled":
      return "Disabled workers will not be selected.";
    default:
      return "Review this worker before using it.";
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
