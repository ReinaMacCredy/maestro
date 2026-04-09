/**
 * UKI v5.4 transfer renderer/parser.
 *
 * New writes render from the canonical structured handoff payload.
 * Legacy v5.2/v5.3 strings still parse for compatibility reads.
 */
import { MaestroError } from "../domain/errors.js";
import type {
  ExecuteUkiHandoffContent,
  PlanUkiHandoffContent,
  UkiConfidenceScores,
  UkiHandoffContent,
  UkiMaestroRefs,
} from "../domain/uki-types.js";
import { UKI_ANCHOR_PREFIXES } from "./uki-token.js";

export const UKI_VERSION = "5.4";
export const LEGACY_UKI_VERSION = "5.3";
export const LEGACY_UKI_V52 = "5.2";
export const EMPTY_LIST_SENTINEL = "NONE";
export const MAX_SUMMARY_LEN = 140;
export const MAX_WORDS_PER_TOKEN = 6;

type SlotKind = "single" | "list" | "refs" | "cs";

interface SlotDef {
  readonly name: string;
  readonly field?: string;
  readonly kind: SlotKind;
  readonly optional?: boolean;
}

interface LayoutMatch {
  readonly version: "5.2" | "5.3" | "5.4";
  readonly mode?: "plan" | "execute";
  readonly defs: readonly SlotDef[];
}

interface ParsedUkiResult {
  readonly content: UkiHandoffContent;
  readonly layout: LayoutMatch;
}

interface LegacyV53Slots {
  readonly sessionCore: string;
  readonly causalDrivers: readonly string[];
  readonly divergences: readonly string[];
  readonly keyDecisions: readonly string[];
  readonly decisionBasis: readonly string[];
  readonly signalDelta: readonly string[];
  readonly validationState: readonly string[];
  readonly executionState: string;
  readonly boundaryState: readonly string[];
  readonly nextAction: string;
  readonly artifacts: readonly string[];
  readonly stanceCollapse?: string;
  readonly blindSpot?: string;
  readonly metaphor?: string;
  readonly cs: UkiConfidenceScores;
  readonly summary: string;
}

interface LegacyV52Slots {
  readonly sessionCore: string;
  readonly causalDrivers: readonly string[];
  readonly divergences: readonly string[];
  readonly keyDecisions: readonly string[];
  readonly signalDelta: readonly string[];
  readonly artifacts: readonly string[];
  readonly executionState: string;
  readonly boundaryState: readonly string[];
  readonly stanceCollapse?: string;
  readonly nextAction: string;
  readonly cs: UkiConfidenceScores;
  readonly summary: string;
}

const FIELD_FORBIDDEN_CHAR_PATTERN = /[:\n\r`*|]/;
const OUTPUT_FORBIDDEN_CHAR_PATTERN = /[:\n\r`*]/;
const CS_VALUE_PATTERN = /^(work_\d+(?:\.\d+)?(?:~summary_\d+(?:\.\d+)?)?|summary_\d+(?:\.\d+)?)$/;

const PLAN_SLOTS: readonly SlotDef[] = [
  { name: "MODE", field: "mode", kind: "single" },
  { name: "CURRENT_STATE", field: "currentState", kind: "single" },
  { name: "SESSION_CORE", field: "sessionCore", kind: "single" },
  { name: "CAUSAL_DRIVERS", field: "causalDrivers", kind: "list" },
  { name: "DIVERGENCES", field: "divergences", kind: "list" },
  { name: "MAESTRO_REFS", field: "maestroRefs", kind: "refs" },
  { name: "PLAN_PATHS", field: "planPaths", kind: "list" },
  { name: "MAESTRO_SYNC", field: "maestroSync", kind: "list" },
  { name: "DECISIONS", field: "decisions", kind: "list" },
  { name: "SIGNAL_DELTA", field: "signalDelta", kind: "list" },
  { name: "ARTIFACTS", field: "artifacts", kind: "list" },
  { name: "READ_MORE", field: "readMore", kind: "list" },
  { name: "NEXT_ACTION", field: "nextAction", kind: "single" },
  { name: "CS", kind: "cs" },
  { name: "SUMMARY", field: "summary", kind: "single" },
];

const EXECUTE_HEAD_SLOTS: readonly SlotDef[] = [
  { name: "MODE", field: "mode", kind: "single" },
  { name: "CURRENT_STATE", field: "currentState", kind: "single" },
  { name: "SESSION_CORE", field: "sessionCore", kind: "single" },
  { name: "CAUSAL_DRIVERS", field: "causalDrivers", kind: "list" },
  { name: "DIVERGENCES", field: "divergences", kind: "list" },
  { name: "MAESTRO_REFS", field: "maestroRefs", kind: "refs" },
  { name: "DECISIONS", field: "decisions", kind: "list" },
  { name: "SIGNAL_DELTA", field: "signalDelta", kind: "list" },
  { name: "TOUCHED_FILES", field: "touchedFiles", kind: "list" },
  { name: "COMPLETED_WORK", field: "completedWork", kind: "list" },
  { name: "VALIDATION", field: "validation", kind: "list" },
  { name: "ARTIFACTS", field: "artifacts", kind: "list" },
  { name: "READ_MORE", field: "readMore", kind: "list" },
  { name: "BOUNDARY_STATE", field: "boundaryState", kind: "list" },
  { name: "RISKS", field: "risks", kind: "list" },
];

const EXECUTE_BLIND_SPOT_SLOT: SlotDef = {
  name: "BLIND_SPOT",
  field: "blindSpot",
  kind: "single",
  optional: true,
};

const EXECUTE_METAPHOR_SLOT: SlotDef = {
  name: "METAPHOR",
  field: "metaphor",
  kind: "single",
  optional: true,
};

const EXECUTE_TAIL_SLOTS: readonly SlotDef[] = [
  { name: "NEXT_ACTION", field: "nextAction", kind: "single" },
  { name: "CS", kind: "cs" },
  { name: "SUMMARY", field: "summary", kind: "single" },
];

const V53_HEAD_SLOTS: readonly SlotDef[] = [
  { name: "SESSION_CORE", field: "sessionCore", kind: "single" },
  { name: "CAUSAL_DRIVERS", field: "causalDrivers", kind: "list" },
  { name: "DIVERGENCES", field: "divergences", kind: "list" },
  { name: "KEY_DECISIONS", field: "keyDecisions", kind: "list" },
  { name: "DECISION_BASIS", field: "decisionBasis", kind: "list" },
  { name: "SIGNAL_DELTA", field: "signalDelta", kind: "list" },
  { name: "VALIDATION_STATE", field: "validationState", kind: "list" },
  { name: "EXECUTION_STATE", field: "executionState", kind: "single" },
  { name: "BOUNDARY_STATE", field: "boundaryState", kind: "list" },
  { name: "NEXT_ACTION", field: "nextAction", kind: "single" },
  { name: "ARTIFACTS", field: "artifacts", kind: "list" },
  { name: "STANCE_COLLAPSE", field: "stanceCollapse", kind: "single" },
];

const V53_BLIND_SPOT_SLOT: SlotDef = {
  name: "BLIND_SPOT",
  field: "blindSpot",
  kind: "single",
  optional: true,
};

const V53_METAPHOR_SLOT: SlotDef = {
  name: "METAPHOR",
  field: "metaphor",
  kind: "single",
  optional: true,
};

const V53_TAIL_SLOTS: readonly SlotDef[] = [
  { name: "CS", kind: "cs" },
  { name: "SUMMARY", field: "summary", kind: "single" },
];

const V52_SLOTS: readonly SlotDef[] = [
  { name: "SESSION_CORE", field: "sessionCore", kind: "single" },
  { name: "CAUSAL_DRIVERS", field: "causalDrivers", kind: "list" },
  { name: "DIVERGENCES", field: "divergences", kind: "list" },
  { name: "KEY_DECISIONS", field: "keyDecisions", kind: "list" },
  { name: "SIGNAL_DELTA", field: "signalDelta", kind: "list" },
  { name: "ARTIFACTS", field: "artifacts", kind: "list" },
  { name: "EXECUTION_STATE", field: "executionState", kind: "single" },
  { name: "BOUNDARY_STATE", field: "boundaryState", kind: "list" },
  { name: "STANCE_COLLAPSE", field: "stanceCollapse", kind: "single" },
  { name: "NEXT_ACTION", field: "nextAction", kind: "single" },
  { name: "CS", kind: "cs" },
  { name: "SUMMARY", field: "summary", kind: "single" },
];

export function compressUki(content: UkiHandoffContent): string {
  const defs = buildOutputDefs(content);
  const parts = defs.map((def) => encodeSlot(def, content));
  const result = parts.join("|");
  assertCompressed(result, defs.length);
  return result;
}

export function parseUki(raw: string): UkiHandoffContent {
  return parseUkiResult(raw).content;
}

export function validateUki(raw: string): string[] {
  try {
    const parsed = parseUkiResult(raw);
    if (parsed.layout.version === UKI_VERSION && compressUki(parsed.content) !== raw) {
      return ["UKI validation failed deterministic round-trip for v5.4 payload"];
    }
    return [];
  } catch (error) {
    if (error instanceof MaestroError) {
      return [error.message, ...error.hints];
    }
    return [error instanceof Error ? error.message : String(error)];
  }
}

function buildOutputDefs(content: UkiHandoffContent): readonly SlotDef[] {
  if (content.mode === "plan") {
    return PLAN_SLOTS;
  }

  const defs = [...EXECUTE_HEAD_SLOTS];
  if (hasOptionalSingle(content.blindSpot)) {
    defs.push(EXECUTE_BLIND_SPOT_SLOT);
  }
  if (hasOptionalSingle(content.metaphor)) {
    defs.push(EXECUTE_METAPHOR_SLOT);
  }
  defs.push(...EXECUTE_TAIL_SLOTS);
  return defs;
}

function encodeSlot(def: SlotDef, content: UkiHandoffContent): string {
  switch (def.kind) {
    case "single":
      return encodeSingle(def, content);
    case "list":
      return encodeList(def, content);
    case "refs":
      return encodeRefs(content.maestroRefs);
    case "cs":
      return encodeCs(content.cs);
  }
}

function encodeSingle(def: SlotDef, content: UkiHandoffContent): string {
  const value = def.field ? (content as Record<string, unknown>)[def.field] : undefined;
  if (typeof value !== "string") {
    throw compressError(`Slot ${def.name} must be a string`, [`Field: ${String(def.field)}`]);
  }

  const raw = value.trim();
  if (raw.length === 0) {
    throw compressError(`Slot ${def.name} must not be empty`, [`Field: ${String(def.field)}`]);
  }

  assertNoForbiddenChars(raw, def.name);
  if (def.name === "SUMMARY" && raw.length >= MAX_SUMMARY_LEN) {
    throw compressError(`SUMMARY exceeds v5.4 limit (${raw.length} >= ${MAX_SUMMARY_LEN})`, [
      "Rewrite the summary to under 140 characters",
    ]);
  }

  for (const subToken of raw.split("-")) {
    if (subToken.length === 0) {
      throw compressError(`Slot ${def.name} contains an empty sub-token`, [`Offending value: ${raw}`]);
    }
    assertWordCount(subToken, def.name);
  }

  return `${def.name}-${raw}`;
}

function encodeList(def: SlotDef, content: UkiHandoffContent): string {
  const value = def.field ? (content as Record<string, unknown>)[def.field] : undefined;
  if (!Array.isArray(value)) {
    throw compressError(`Slot ${def.name} must be an array`, [`Field: ${String(def.field)}`]);
  }

  if (def.name === "READ_MORE" && value.length === 0) {
    throw compressError("READ_MORE must contain at least one anchor", [
      "Provide --read-more or rely on auto-collected touched files",
    ]);
  }

  if (def.name === "ARTIFACTS" && value.length === 0) {
    throw compressError("ARTIFACTS must contain at least one anchor", [
      "Provide --artifact or rely on auto-collected branch/file anchors",
    ]);
  }

  if (value.length === 0) {
    return `${def.name}-${EMPTY_LIST_SENTINEL}`;
  }

  const normalized = value.map((raw) => {
    if (typeof raw !== "string") {
      throw compressError(`Slot ${def.name} must contain only strings`, [`Got ${typeof raw}`]);
    }

    const token = raw.trim();
    if (token.length === 0) {
      throw compressError(`Slot ${def.name} contains an empty token`);
    }
    if (token.includes("-")) {
      throw compressError(`Slot ${def.name} token '${token}' contains '-'`, [
        "Replace '-' with '_' inside list tokens",
      ]);
    }

    assertNoForbiddenChars(token, def.name);
    assertWordCount(token, def.name);
    return token;
  });

  return `${def.name}-${normalized.join("-")}`;
}

function encodeRefs(refs: UkiMaestroRefs): string {
  const tokens: string[] = [];

  if (refs.missionId) tokens.push(`${UKI_ANCHOR_PREFIXES.mission}${refs.missionId}`);
  if (refs.featureId) tokens.push(`${UKI_ANCHOR_PREFIXES.feature}${refs.featureId}`);
  if (refs.milestoneId) tokens.push(`${UKI_ANCHOR_PREFIXES.milestone}${refs.milestoneId}`);
  if (refs.planPath) tokens.push(`${UKI_ANCHOR_PREFIXES.plan}${refs.planPath}`);
  if (refs.specPath) tokens.push(`${UKI_ANCHOR_PREFIXES.spec}${refs.specPath}`);

  if (tokens.length === 0) {
    return `MAESTRO_REFS-${EMPTY_LIST_SENTINEL}`;
  }

  for (const token of tokens) {
    if (token.includes("-")) {
      throw compressError(`MAESTRO_REFS token '${token}' contains '-'`, [
        "Store Maestro refs as token-safe values before rendering UKI",
      ]);
    }
    assertNoForbiddenChars(token, "MAESTRO_REFS");
    assertWordCount(token, "MAESTRO_REFS");
  }

  return `MAESTRO_REFS-${tokens.join("-")}`;
}

function encodeCs(cs: UkiConfidenceScores): string {
  const hasWork = typeof cs.work === "number";
  const hasSummary = typeof cs.summary === "number";

  if (!hasWork && !hasSummary) {
    throw compressError("CS must have at least one of work or summary", [
      "Pass --confidence-work <n> or --confidence-summary <n>",
    ]);
  }
  if (hasWork && !isFiniteNumber(cs.work)) {
    throw compressError("CS.work must be a finite number", [`Got ${cs.work}`]);
  }
  if (hasSummary && !isFiniteNumber(cs.summary)) {
    throw compressError("CS.summary must be a finite number", [`Got ${cs.summary}`]);
  }

  const parts: string[] = [];
  if (hasWork) parts.push(`work_${formatConfidence(cs.work as number)}`);
  if (hasSummary) parts.push(`summary_${formatConfidence(cs.summary as number)}`);
  const value = parts.join("~");

  if (!CS_VALUE_PATTERN.test(value)) {
    throw compressError(`CS produced invalid value: ${value}`);
  }

  return `CS-${value}`;
}

function parseUkiResult(raw: string): ParsedUkiResult {
  const parts = raw.split("|");
  const layout = detectLayout(parts);
  if (!layout) {
    throw new MaestroError("UKI parse: unsupported UKI layout");
  }

  const valuesByName = new Map<string, string>();
  for (let index = 0; index < layout.defs.length; index += 1) {
    const def = layout.defs[index]!;
    const slot = parts[index]!;
    valuesByName.set(def.name, slot.slice(def.name.length + 1));
  }

  if (layout.version === LEGACY_UKI_V52) {
    return {
      layout,
      content: normalizeLegacyV52({
        sessionCore: valuesByName.get("SESSION_CORE")!,
        causalDrivers: parseListValue(valuesByName.get("CAUSAL_DRIVERS")!),
        divergences: parseListValue(valuesByName.get("DIVERGENCES")!),
      keyDecisions: parseListValue(valuesByName.get("KEY_DECISIONS")!),
      signalDelta: parseListValue(valuesByName.get("SIGNAL_DELTA")!),
      artifacts: parseListValue(valuesByName.get("ARTIFACTS")!),
      executionState: valuesByName.get("EXECUTION_STATE")!,
      boundaryState: parseListValue(valuesByName.get("BOUNDARY_STATE")!),
        stanceCollapse: valuesByName.get("STANCE_COLLAPSE")!,
        nextAction: valuesByName.get("NEXT_ACTION")!,
        cs: parseCsValue(valuesByName.get("CS")!),
        summary: valuesByName.get("SUMMARY")!,
      }),
    };
  }

  if (layout.version === LEGACY_UKI_VERSION) {
    return {
      layout,
      content: normalizeLegacyV53({
        sessionCore: valuesByName.get("SESSION_CORE")!,
        causalDrivers: parseListValue(valuesByName.get("CAUSAL_DRIVERS")!),
        divergences: parseListValue(valuesByName.get("DIVERGENCES")!),
      keyDecisions: parseListValue(valuesByName.get("KEY_DECISIONS")!),
      decisionBasis: parseListValue(valuesByName.get("DECISION_BASIS")!),
      signalDelta: parseListValue(valuesByName.get("SIGNAL_DELTA")!),
      validationState: parseListValue(valuesByName.get("VALIDATION_STATE")!),
      executionState: valuesByName.get("EXECUTION_STATE")!,
      boundaryState: parseListValue(valuesByName.get("BOUNDARY_STATE")!),
      nextAction: valuesByName.get("NEXT_ACTION")!,
      artifacts: parseListValue(valuesByName.get("ARTIFACTS")!),
      stanceCollapse: valuesByName.get("STANCE_COLLAPSE"),
        blindSpot: valuesByName.get("BLIND_SPOT"),
        metaphor: valuesByName.get("METAPHOR"),
        cs: parseCsValue(valuesByName.get("CS")!),
        summary: valuesByName.get("SUMMARY")!,
      }),
    };
  }

  const mode = layout.mode!;
  if (mode === "plan") {
    return {
      layout,
      content: {
        mode,
        currentState: valuesByName.get("CURRENT_STATE")!,
        sessionCore: valuesByName.get("SESSION_CORE")!,
        causalDrivers: parseListValue(valuesByName.get("CAUSAL_DRIVERS")!),
        divergences: parseListValue(valuesByName.get("DIVERGENCES")!),
        maestroRefs: parseMaestroRefs(valuesByName.get("MAESTRO_REFS")!),
        planPaths: parseListValue(valuesByName.get("PLAN_PATHS")!),
        maestroSync: parseListValue(valuesByName.get("MAESTRO_SYNC")!),
        decisions: parseListValue(valuesByName.get("DECISIONS")!),
        signalDelta: parseListValue(valuesByName.get("SIGNAL_DELTA")!),
        artifacts: parseListValue(valuesByName.get("ARTIFACTS")!),
        readMore: parseListValue(valuesByName.get("READ_MORE")!),
        nextAction: valuesByName.get("NEXT_ACTION")!,
        cs: parseCsValue(valuesByName.get("CS")!),
        summary: valuesByName.get("SUMMARY")!,
        boundaryState: [],
        risks: [],
      },
    };
  }

  const blindSpot = valuesByName.get("BLIND_SPOT");
  const metaphor = valuesByName.get("METAPHOR");

  return {
    layout,
    content: {
      mode,
      currentState: valuesByName.get("CURRENT_STATE")!,
      sessionCore: valuesByName.get("SESSION_CORE")!,
      causalDrivers: parseListValue(valuesByName.get("CAUSAL_DRIVERS")!),
      divergences: parseListValue(valuesByName.get("DIVERGENCES")!),
      maestroRefs: parseMaestroRefs(valuesByName.get("MAESTRO_REFS")!),
      decisions: parseListValue(valuesByName.get("DECISIONS")!),
      signalDelta: parseListValue(valuesByName.get("SIGNAL_DELTA")!),
      touchedFiles: parseListValue(valuesByName.get("TOUCHED_FILES")!),
      completedWork: parseListValue(valuesByName.get("COMPLETED_WORK")!),
      validation: parseListValue(valuesByName.get("VALIDATION")!),
      artifacts: parseListValue(valuesByName.get("ARTIFACTS")!),
      readMore: parseListValue(valuesByName.get("READ_MORE")!),
      boundaryState: parseListValue(valuesByName.get("BOUNDARY_STATE")!),
      risks: parseListValue(valuesByName.get("RISKS")!),
      ...(blindSpot ? { blindSpot } : {}),
      ...(metaphor ? { metaphor } : {}),
      nextAction: valuesByName.get("NEXT_ACTION")!,
      cs: parseCsValue(valuesByName.get("CS")!),
      summary: valuesByName.get("SUMMARY")!,
    },
  };
}

function detectLayout(parts: readonly string[]): LayoutMatch | undefined {
  const first = parts[0];
  if (!first) return undefined;

  if (first.startsWith("MODE-plan")) {
    return detectV54Layout(parts, "plan");
  }
  if (first.startsWith("MODE-execute")) {
    return detectV54Layout(parts, "execute");
  }

  const v53 = detectV53Layout(parts);
  if (v53) return v53;
  return detectV52Layout(parts);
}

function detectV54Layout(parts: readonly string[], mode: "plan" | "execute"): LayoutMatch | undefined {
  if (mode === "plan") {
    return matchesLayout(parts, PLAN_SLOTS)
      ? { version: UKI_VERSION, mode, defs: PLAN_SLOTS }
      : undefined;
  }

  const defs = detectOptionalLayout(parts, EXECUTE_HEAD_SLOTS, EXECUTE_TAIL_SLOTS, {
    blindSpot: EXECUTE_BLIND_SPOT_SLOT,
    metaphor: EXECUTE_METAPHOR_SLOT,
  });

  return matchesLayout(parts, defs)
    ? { version: UKI_VERSION, mode, defs }
    : undefined;
}

function detectV53Layout(parts: readonly string[]): LayoutMatch | undefined {
  const defs = detectOptionalLayout(parts, V53_HEAD_SLOTS, V53_TAIL_SLOTS, {
    blindSpot: V53_BLIND_SPOT_SLOT,
    metaphor: V53_METAPHOR_SLOT,
  });

  return matchesLayout(parts, defs)
    ? { version: LEGACY_UKI_VERSION, defs }
    : undefined;
}

function detectV52Layout(parts: readonly string[]): LayoutMatch | undefined {
  return matchesLayout(parts, V52_SLOTS)
    ? { version: LEGACY_UKI_V52, defs: V52_SLOTS }
    : undefined;
}

function matchesLayout(parts: readonly string[], defs: readonly SlotDef[]): boolean {
  if (parts.length !== defs.length) {
    return false;
  }

  return defs.every((def, index) => parts[index]?.startsWith(`${def.name}-`) === true);
}

function detectOptionalLayout(
  parts: readonly string[],
  head: readonly SlotDef[],
  tail: readonly SlotDef[],
  optional: {
    readonly blindSpot: SlotDef;
    readonly metaphor: SlotDef;
  },
): readonly SlotDef[] {
  const defs = [...head];
  let offset = head.length;
  if (parts[offset]?.startsWith(`${optional.blindSpot.name}-`)) {
    defs.push(optional.blindSpot);
    offset += 1;
  }
  if (parts[offset]?.startsWith(`${optional.metaphor.name}-`)) {
    defs.push(optional.metaphor);
  }
  defs.push(...tail);
  return defs;
}

function parseListValue(raw: string): readonly string[] {
  if (raw === EMPTY_LIST_SENTINEL) {
    return [];
  }
  return raw.split("-").filter(Boolean);
}

function parseMaestroRefs(raw: string): UkiMaestroRefs {
  const refs: UkiMaestroRefs = {};
  if (raw === EMPTY_LIST_SENTINEL) {
    return refs;
  }

  for (const token of raw.split("-")) {
    if (token.startsWith(UKI_ANCHOR_PREFIXES.mission)) {
      refs.missionId = token.slice(UKI_ANCHOR_PREFIXES.mission.length);
    } else if (token.startsWith(UKI_ANCHOR_PREFIXES.feature)) {
      refs.featureId = token.slice(UKI_ANCHOR_PREFIXES.feature.length);
    } else if (token.startsWith(UKI_ANCHOR_PREFIXES.milestone)) {
      refs.milestoneId = token.slice(UKI_ANCHOR_PREFIXES.milestone.length);
    } else if (token.startsWith(UKI_ANCHOR_PREFIXES.plan)) {
      refs.planPath = token.slice(UKI_ANCHOR_PREFIXES.plan.length);
    } else if (token.startsWith(UKI_ANCHOR_PREFIXES.spec)) {
      refs.specPath = token.slice(UKI_ANCHOR_PREFIXES.spec.length);
    }
  }

  return refs;
}

function parseCsValue(raw: string): UkiConfidenceScores {
  if (!CS_VALUE_PATTERN.test(raw)) {
    throw compressError(`CS has invalid value: ${raw}`);
  }

  const result: UkiConfidenceScores = {};
  for (const token of raw.split("~")) {
    if (token.startsWith("work_")) {
      result.work = Number(token.slice("work_".length));
    } else if (token.startsWith("summary_")) {
      result.summary = Number(token.slice("summary_".length));
    }
  }
  return result;
}

function normalizeLegacyV52(legacy: LegacyV52Slots): ExecuteUkiHandoffContent {
  // STANCE_COLLAPSE existed in v5.2 only; v5.4 keeps no direct field for it.
  return {
    mode: "execute",
    currentState: legacy.executionState,
    sessionCore: legacy.sessionCore,
    decisions: [...legacy.keyDecisions],
    artifacts: legacy.artifacts,
    readMore: deriveReadMore(legacy.artifacts, legacy.nextAction),
    nextAction: legacy.nextAction,
    summary: legacy.summary,
    maestroRefs: {},
    cs: legacy.cs,
    signalDelta: legacy.signalDelta,
    boundaryState: legacy.boundaryState,
    risks: [],
    blindSpot: undefined,
    metaphor: undefined,
    causalDrivers: legacy.causalDrivers,
    divergences: legacy.divergences,
    touchedFiles: deriveTouchedFiles(legacy.artifacts),
    completedWork: legacy.signalDelta.length > 0 ? legacy.signalDelta : legacy.keyDecisions,
    validation: [],
  };
}

function normalizeLegacyV53(legacy: LegacyV53Slots): ExecuteUkiHandoffContent {
  // STANCE_COLLAPSE remains a legacy-only compatibility signal in v5.3 reads.
  return {
    mode: "execute",
    currentState: legacy.executionState,
    sessionCore: legacy.sessionCore,
    decisions: [...legacy.keyDecisions, ...legacy.decisionBasis],
    artifacts: legacy.artifacts,
    readMore: deriveReadMore(legacy.artifacts, legacy.nextAction),
    nextAction: legacy.nextAction,
    summary: legacy.summary,
    maestroRefs: {},
    cs: legacy.cs,
    signalDelta: legacy.signalDelta,
    boundaryState: legacy.boundaryState,
    risks: legacy.blindSpot ? [legacy.blindSpot] : [],
    blindSpot: legacy.blindSpot,
    metaphor: legacy.metaphor,
    causalDrivers: legacy.causalDrivers,
    divergences: legacy.divergences,
    touchedFiles: deriveTouchedFiles(legacy.artifacts),
    completedWork: legacy.signalDelta.length > 0 ? legacy.signalDelta : legacy.keyDecisions,
    validation: legacy.validationState,
  };
}

function deriveTouchedFiles(artifacts: readonly string[]): readonly string[] {
  return artifacts.filter((token) => token.startsWith("file_"));
}

function deriveReadMore(artifacts: readonly string[], nextAction: string): readonly string[] {
  const fileArtifacts = deriveTouchedFiles(artifacts);
  if (fileArtifacts.length > 0) {
    return fileArtifacts;
  }
  if (artifacts.length > 0) {
    return artifacts.slice(0, 3);
  }
  return [nextAction];
}

function hasOptionalSingle(value: string | undefined): boolean {
  return typeof value === "string" && value.trim().length > 0;
}

function assertCompressed(raw: string, expectedSlots: number): void {
  const actual = raw.length === 0 ? 0 : raw.split("|").length;
  if (actual !== expectedSlots) {
    throw compressError(`Compressed UKI slot count mismatch (${actual} !== ${expectedSlots})`);
  }
  if (OUTPUT_FORBIDDEN_CHAR_PATTERN.test(raw)) {
    throw compressError("Compressed UKI contains forbidden characters", [raw]);
  }
}

function assertNoForbiddenChars(raw: string, slotName: string): void {
  if (FIELD_FORBIDDEN_CHAR_PATTERN.test(raw)) {
    throw compressError(`Slot ${slotName} contains a forbidden character`, [
      "UKI forbids :, newlines, pipes, backticks, and asterisks",
    ]);
  }
}

function assertWordCount(raw: string, slotName: string): void {
  const halves = raw.split("~");
  for (const half of halves) {
    const words = half.split("_").filter(Boolean).length;
    if (words > MAX_WORDS_PER_TOKEN) {
      throw compressError(
        `Slot ${slotName} token exceeds word limit (${words} > ${MAX_WORDS_PER_TOKEN})`,
        [`Offending token: ${half}`],
      );
    }
  }
}

function isFiniteNumber(value: number | undefined): boolean {
  return typeof value === "number" && Number.isFinite(value);
}

function formatConfidence(value: number): string {
  return Number.parseFloat(value.toFixed(2)).toString();
}

function compressError(message: string, hints: readonly string[] = []): MaestroError {
  return new MaestroError(`UKI compress: ${message}`, [...hints]);
}
