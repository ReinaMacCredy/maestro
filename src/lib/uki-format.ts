/**
 * UKI v5.3 compressor/parser -- deterministic single-string compression of
 * structured handoff data for the maestro conductor model.
 *
 * New writes use the richer v5.3 contract, while the parser/validator remain
 * tolerant of legacy v5.2 strings so cached handoffs from older sessions can
 * still be picked up safely.
 */
import { MaestroError } from "../domain/errors.js";

export const UKI_VERSION = "5.3";
export const LEGACY_UKI_VERSION = "5.2";

/** Default value for STANCE_COLLAPSE when none is set. */
export const DEFAULT_STANCE_COLLAPSE = "NONE_DETECTED_LOW_FRICTION";

/** Placeholder emitted for empty list slots. */
export const EMPTY_LIST_SENTINEL = "NONE";

/** Maximum allowed SUMMARY length (exclusive upper bound). */
export const MAX_SUMMARY_LEN = 140;

/** Maximum allowed words per `_`-joined token half. */
export const MAX_WORDS_PER_TOKEN = 4;

export interface UkiSlots {
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
  readonly cs: { readonly work?: number; readonly summary?: number };
  readonly summary: string;
}

interface LegacyUkiSlots {
  readonly sessionCore: string;
  readonly causalDrivers: readonly string[];
  readonly divergences: readonly string[];
  readonly keyDecisions: readonly string[];
  readonly signalDelta: readonly string[];
  readonly artifacts: readonly string[];
  readonly executionState: string;
  readonly boundaryState: readonly string[];
  readonly stanceCollapse: string;
  readonly nextAction: string;
  readonly cs: { readonly work?: number; readonly summary?: number };
  readonly summary: string;
}

type SlotKind = "single" | "list" | "cs";

interface SlotDef {
  readonly name: string;
  readonly field?: string;
  readonly kind: SlotKind;
  readonly optional?: boolean;
}

interface LayoutMatch {
  readonly version: typeof LEGACY_UKI_VERSION | typeof UKI_VERSION;
  readonly defs: readonly SlotDef[];
}

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

const ARTIFACT_PREFIX_PATTERN = /(?:^|-)(?:commit|branch|version|file)_/;
const CS_VALUE_PATTERN = /^(work_\d+(?:\.\d+)?(?:~summary_\d+(?:\.\d+)?)?|summary_\d+(?:\.\d+)?)$/;
const FORBIDDEN_CHAR_PATTERN = /[:\n\r`*]/;

export function compressUki(slots: UkiSlots): string {
  const defs = buildV53OutputDefs(slots);
  const parts: string[] = [];

  for (const def of defs) {
    parts.push(encodeSlot(def, slots));
  }

  const result = parts.join("|");
  assertCompressed(result, defs.length);
  return result;
}

function buildV53OutputDefs(slots: UkiSlots): readonly SlotDef[] {
  const defs = [...V53_HEAD_SLOTS];

  if (hasOptionalSingle(slots.blindSpot)) {
    defs.push(V53_BLIND_SPOT_SLOT);
  }
  if (hasOptionalSingle(slots.metaphor)) {
    defs.push(V53_METAPHOR_SLOT);
  }

  defs.push(...V53_TAIL_SLOTS);
  return defs;
}

function hasOptionalSingle(value: string | undefined): boolean {
  return typeof value === "string" && value.trim().length > 0;
}

function encodeSlot(def: SlotDef, slots: UkiSlots): string {
  switch (def.kind) {
    case "single":
      return encodeSingle(def, slots);
    case "list":
      return encodeList(def, slots);
    case "cs":
      return encodeCs(slots.cs);
  }
}

function encodeSingle(def: SlotDef, slots: UkiSlots): string {
  let raw: string;

  if (def.field === "stanceCollapse") {
    raw = slots.stanceCollapse?.trim() || DEFAULT_STANCE_COLLAPSE;
  } else {
    const value = def.field ? (slots as Record<string, unknown>)[def.field] : undefined;
    if (typeof value !== "string") {
      throw compressError(
        `Slot ${def.name} must be a string, got ${typeof value}`,
        [`Field: ${String(def.field)}`],
      );
    }
    raw = value.trim();
  }

  if (raw.length === 0) {
    throw compressError(`Slot ${def.name} must not be empty`, [
      `Field: ${String(def.field)}`,
      "Every required UKI v5.3 single slot needs a non-empty value",
    ]);
  }

  assertNoForbiddenChars(raw, def.name);

  if (def.name === "SUMMARY" && raw.length >= MAX_SUMMARY_LEN) {
    throw compressError(
      `SUMMARY exceeds v5.3 limit (${raw.length} >= ${MAX_SUMMARY_LEN})`,
      [
        "Rewrite the summary to under 140 characters",
        "Format hint: [Essence-Progress-Honest_Risk]",
      ],
    );
  }

  for (const subToken of raw.split("-")) {
    if (subToken.length === 0) {
      throw compressError(
        `Slot ${def.name} contains an empty sub-token (leading, trailing, or doubled '-')`,
        [`Offending value: ${raw}`],
      );
    }
    assertWordCount(subToken, def.name);
  }

  return `${def.name}-${raw}`;
}

function encodeList(def: SlotDef, slots: UkiSlots): string {
  const value = def.field ? (slots as Record<string, unknown>)[def.field] : undefined;
  if (!Array.isArray(value)) {
    throw compressError(
      `Slot ${def.name} must be an array, got ${typeof value}`,
      [`Field: ${String(def.field)}`],
    );
  }

  if (value.length === 0) {
    if (def.name === "ARTIFACTS") {
      throw compressError(
        "ARTIFACTS must contain at least one of commit_/branch_/version_/file_ (R6)",
        [
          "Add an artifact token like branch_<name> or commit_<sha>",
          "ARTIFACTS cannot be empty in UKI v5.3",
        ],
      );
    }
    return `${def.name}-${EMPTY_LIST_SENTINEL}`;
  }

  const normalized = value.map((raw) => {
    if (typeof raw !== "string") {
      throw compressError(
        `Slot ${def.name} must contain only strings`,
        [`Got element of type ${typeof raw}`],
      );
    }

    const token = raw.trim();
    if (token.length === 0) {
      throw compressError(
        `Slot ${def.name} contains an empty token`,
        ["Remove the empty token or merge it with its neighbor"],
      );
    }
    if (token.includes("-")) {
      throw compressError(
        `Slot ${def.name} token '${token}' contains '-' which clashes with the list separator`,
        [
          "Replace '-' with '_' inside list tokens",
          "Only single-string slots may contain '-' as a sub-token separator",
        ],
      );
    }

    assertNoForbiddenChars(token, def.name);
    assertWordCount(token, def.name);
    return token;
  });

  if (def.name === "ARTIFACTS" && !ARTIFACT_PREFIX_PATTERN.test(normalized.join("-"))) {
    throw compressError(
      "ARTIFACTS must contain at least one of commit_/branch_/version_/file_ (R6)",
      [
        "Add an artifact token like branch_<name> or commit_<sha>",
        "Or file_<path_with_underscores> / version_<x_y_z>",
      ],
    );
  }

  return `${def.name}-${normalized.join("-")}`;
}

function encodeCs(cs: UkiSlots["cs"]): string {
  const hasWork = typeof cs.work === "number";
  const hasSummary = typeof cs.summary === "number";

  if (!hasWork && !hasSummary) {
    throw compressError(
      "CS must have at least one of work or summary (R5)",
      [
        "Pass --confidence-work <n> or --confidence-summary <n>",
        "Bare CS-N.NN is not a valid UKI shape",
      ],
    );
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

export function parseUki(raw: string): UkiSlots {
  const violations = validateUki(raw);
  if (violations.length > 0) {
    throw new MaestroError(
      `UKI parse: invalid string (${violations.length} violation${violations.length === 1 ? "" : "s"})`,
      violations,
    );
  }

  return parseUkiUnchecked(raw);
}

function parseUkiUnchecked(raw: string): UkiSlots {
  const parts = raw.split("|");
  const layout = detectLayout(parts);

  if (!layout) {
    throw new Error("Unsupported UKI layout");
  }

  const valuesByName = new Map<string, string>();
  for (let i = 0; i < layout.defs.length; i++) {
    const def = layout.defs[i]!;
    const slot = parts[i]!;
    valuesByName.set(def.name, slot.slice(def.name.length + 1));
  }

  if (layout.version === LEGACY_UKI_VERSION) {
    return normalizeLegacySlots({
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
    });
  }

  return {
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
    stanceCollapse: valuesByName.get("STANCE_COLLAPSE")!,
    blindSpot: valuesByName.get("BLIND_SPOT"),
    metaphor: valuesByName.get("METAPHOR"),
    cs: parseCsValue(valuesByName.get("CS")!),
    summary: valuesByName.get("SUMMARY")!,
  };
}

function normalizeLegacySlots(legacy: LegacyUkiSlots): UkiSlots {
  return {
    sessionCore: legacy.sessionCore,
    causalDrivers: legacy.causalDrivers,
    divergences: legacy.divergences,
    keyDecisions: legacy.keyDecisions,
    decisionBasis: [],
    signalDelta: legacy.signalDelta,
    validationState: [],
    executionState: legacy.executionState,
    boundaryState: legacy.boundaryState,
    nextAction: legacy.nextAction,
    artifacts: legacy.artifacts,
    stanceCollapse: legacy.stanceCollapse,
    cs: legacy.cs,
    summary: legacy.summary,
  };
}

function parseListValue(value: string): readonly string[] {
  if (value === EMPTY_LIST_SENTINEL) return [];
  return value.split("-");
}

function parseCsValue(value: string): { work?: number; summary?: number } {
  const parts = value.split("~");
  const result: { work?: number; summary?: number } = {};

  for (const part of parts) {
    if (part.startsWith("work_")) {
      result.work = Number(part.slice("work_".length));
    } else if (part.startsWith("summary_")) {
      result.summary = Number(part.slice("summary_".length));
    }
  }

  return result;
}

export function validateUki(raw: string): readonly string[] {
  const violations: string[] = [];

  if (typeof raw !== "string") {
    violations.push("input is not a string");
    return violations;
  }
  if (raw.length === 0) {
    violations.push("input is empty");
    return violations;
  }
  if (FORBIDDEN_CHAR_PATTERN.test(raw)) {
    violations.push("input contains forbidden character (':', newline, '`', or '*')");
  }
  if (raw !== raw.trim()) {
    violations.push("input has leading or trailing whitespace");
  }

  const parts = raw.split("|");
  const layout = detectLayout(parts);
  if (!layout) {
    violations.push(
      `slot layout is unsupported (expected v5.2 with ${V52_SLOTS.length} slots or v5.3 with 14-16 ordered slots)`,
    );
    return violations;
  }

  const valuesByName = new Map<string, string>();
  for (let i = 0; i < layout.defs.length; i++) {
    const def = layout.defs[i]!;
    const slot = parts[i]!;
    if (!slot.startsWith(`${def.name}-`)) {
      violations.push(`slot ${i} must start with '${def.name}-'`);
      continue;
    }
    valuesByName.set(def.name, slot.slice(def.name.length + 1));
  }

  for (const def of layout.defs) {
    const value = valuesByName.get(def.name)!;
    if (value.length === 0) {
      violations.push(`slot ${def.name} is empty`);
    }
  }

  const stance = valuesByName.get("STANCE_COLLAPSE");
  if (layout.version === UKI_VERSION && (!stance || stance.length === 0)) {
    violations.push("STANCE_COLLAPSE slot must always be present");
  }

  const cs = valuesByName.get("CS");
  if (cs && !CS_VALUE_PATTERN.test(cs)) {
    violations.push(
      `CS value '${cs}' is not scoped (must be CS-work_X, CS-summary_Y, or CS-work_X~summary_Y)`,
    );
  }

  const artifacts = valuesByName.get("ARTIFACTS");
  if (!artifacts || artifacts === EMPTY_LIST_SENTINEL) {
    violations.push("ARTIFACTS must contain at least one token");
  } else if (!ARTIFACT_PREFIX_PATTERN.test(artifacts)) {
    violations.push("ARTIFACTS must contain at least one of commit_/branch_/version_/file_");
  }

  const summary = valuesByName.get("SUMMARY");
  if (summary !== undefined && summary.length >= MAX_SUMMARY_LEN) {
    violations.push(
      `SUMMARY length ${summary.length} exceeds limit ${MAX_SUMMARY_LEN}`,
    );
  }

  for (const def of layout.defs) {
    if (def.name === "CS") continue;
    const value = valuesByName.get(def.name);
    if (value === undefined || value === EMPTY_LIST_SENTINEL) continue;

    const tokens = value.split("-");
    for (const token of tokens) {
      const halves = token.split("~");
      for (const half of halves) {
        if (half.length === 0) continue;
        const words = half.split("_").filter((word) => word.length > 0);
        if (words.length > MAX_WORDS_PER_TOKEN) {
          violations.push(
            `slot ${def.name} token '${token}' exceeds word limit (${words.length} > ${MAX_WORDS_PER_TOKEN})`,
          );
        }
      }
    }
  }

  return violations;
}

function detectLayout(parts: readonly string[]): LayoutMatch | undefined {
  if (parts.length === V52_SLOTS.length && matchesOrderedDefs(parts, V52_SLOTS)) {
    return { version: LEGACY_UKI_VERSION, defs: V52_SLOTS };
  }

  return detectV53Layout(parts);
}

function detectV53Layout(parts: readonly string[]): LayoutMatch | undefined {
  let index = 0;
  const defs: SlotDef[] = [];

  for (const def of V53_HEAD_SLOTS) {
    if (!slotStartsWith(parts[index], def.name)) {
      return undefined;
    }
    defs.push(def);
    index += 1;
  }

  if (slotStartsWith(parts[index], V53_BLIND_SPOT_SLOT.name)) {
    defs.push(V53_BLIND_SPOT_SLOT);
    index += 1;
  }

  if (slotStartsWith(parts[index], V53_METAPHOR_SLOT.name)) {
    defs.push(V53_METAPHOR_SLOT);
    index += 1;
  }

  for (const def of V53_TAIL_SLOTS) {
    if (!slotStartsWith(parts[index], def.name)) {
      return undefined;
    }
    defs.push(def);
    index += 1;
  }

  if (index !== parts.length) {
    return undefined;
  }

  return { version: UKI_VERSION, defs };
}

function matchesOrderedDefs(parts: readonly string[], defs: readonly SlotDef[]): boolean {
  if (parts.length !== defs.length) return false;
  return defs.every((def, index) => slotStartsWith(parts[index], def.name));
}

function slotStartsWith(slot: string | undefined, name: string): boolean {
  return typeof slot === "string" && slot.startsWith(`${name}-`);
}

function assertNoForbiddenChars(token: string, slotName: string): void {
  if (FORBIDDEN_CHAR_PATTERN.test(token)) {
    throw compressError(
      `Slot ${slotName} contains forbidden character (colon, newline, or markdown syntax)`,
      [
        "UKI forbids ':', newlines, '`', and '*' in any slot",
        `Offending value: ${token}`,
      ],
    );
  }
}

function assertWordCount(token: string, slotName: string): void {
  const halves = token.split("~");
  for (const half of halves) {
    if (half.length === 0) continue;
    const words = half.split("_").filter((word) => word.length > 0);
    if (words.length > MAX_WORDS_PER_TOKEN) {
      throw compressError(
        `Slot ${slotName} token exceeds word limit (${words.length} > ${MAX_WORDS_PER_TOKEN})`,
        [
          "UKI caps _-joined tokens at 4 words per half",
          `Offending token: ${token}`,
        ],
      );
    }
  }
}

function assertCompressed(result: string, slotCount: number): void {
  if (FORBIDDEN_CHAR_PATTERN.test(result)) {
    throw compressError("Compressed string contains forbidden character", [
      "Internal invariant violation",
    ]);
  }

  const pipes = countChar(result, "|");
  if (pipes !== slotCount - 1) {
    throw compressError(
      `Compressed string has wrong pipe count: ${pipes} (expected ${slotCount - 1})`,
    );
  }
}

function countChar(s: string, ch: string): number {
  let count = 0;
  for (const c of s) {
    if (c === ch) count += 1;
  }
  return count;
}

function isFiniteNumber(n: number | undefined): n is number {
  return typeof n === "number" && Number.isFinite(n);
}

function formatConfidence(n: number): string {
  return String(n);
}

function compressError(message: string, hints: readonly string[] = []): MaestroError {
  return new MaestroError(`UKI compress: ${message}`, hints);
}
