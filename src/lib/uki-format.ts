/**
 * UKI v5.2 compressor/parser -- deterministic single-string compression of
 * structured handoff data for the maestro conductor model.
 *
 * Phase 2 of the 2026 conductor refactor. This is the artifact format that
 * external workers (Claude Code children, Codex, Gemini CLI) consume when
 * picking up a plan via `maestro handoff pickup --uki | <parser>`.
 *
 * ## v5.2 grammar (12 slots in fixed order, separated by `|`):
 *
 *    1. SESSION_CORE      -- single token, essence of the work
 *    2. CAUSAL_DRIVERS    -- list, why the work happened
 *    3. DIVERGENCES       -- list, immutable log of conflicts (R1)
 *    4. KEY_DECISIONS     -- list, design calls made during execution
 *    5. SIGNAL_DELTA      -- list, measurable change (tokens may use ~ for before->after)
 *    6. ARTIFACTS         -- list, must contain >=1 of commit_/branch_/version_/file_ (R7)
 *    7. EXECUTION_STATE   -- single token, final state of working tree
 *    8. BOUNDARY_STATE    -- list, what was deliberately NOT touched
 *    9. STANCE_COLLAPSE   -- single token, always present (R6), default NONE_DETECTED_LOW_FRICTION
 *   10. NEXT_ACTION       -- single token, concrete action for next agent
 *   11. CS                -- scoped confidence (R5): CS-work_X, CS-summary_Y, or CS-work_X~summary_Y
 *   12. SUMMARY           -- single token, <140 chars (R3)
 *
 * ## Slot encoding
 *
 * Each slot is `NAME-value`. For single-token slots, `value` is one token.
 * For list slots, `value` is tokens joined by `-`. Tokens internally use `_`
 * as a word separator (max 4 words per `_`-link, R2) and may contain `~`
 * for before->after deltas (e.g. `tests_27~41_green`).
 *
 * Empty list slots encode as `NAME-NONE`. The parser round-trips `NONE` to
 * an empty array.
 *
 * ## Forbidden characters (anywhere in the output)
 *
 *   - colons (`:`)
 *   - newlines (\n, \r)
 *   - markdown syntax (backticks, asterisks used for emphasis, etc.)
 *
 * ## Design decisions
 *
 * 1. A slot value is a `-`-joined sequence of one or more TOKENS. A token
 *    is a `_`-joined string (with optional `~` for before->after series),
 *    and each `~`-half is capped at 4 words (R2). Both single-string slots
 *    and list slots use the same encoding at the wire level -- the API
 *    difference is purely ergonomic:
 *      - Single-string slots accept a `string` that may itself contain
 *        `-` as a sub-token separator (e.g. `binary_verified-tree_clean`).
 *        On parse, the value is returned as the same raw string.
 *      - List slots accept a `string[]` where each element is a single
 *        token without embedded `-` (any `-` in list tokens is rejected
 *        at compression time to preserve the unambiguous list separator).
 *
 * 2. SUMMARY exceeding 140 chars is rejected at compression time via a
 *    MaestroError. We do not silently truncate -- callers choose to either
 *    rewrite or explicitly truncate, so no data is lost without their
 *    knowledge.
 *
 * 3. Empty STANCE_COLLAPSE is coerced to NONE_DETECTED_LOW_FRICTION at
 *    compression. This enforces R6 even if the caller forgets to pass a
 *    value. On parse, the value is read back verbatim.
 *
 * 4. CS with neither work nor summary is rejected at compression. At least
 *    one confidence scope must be present per R5.
 *
 * 5. Compression is PURE: no Date, no Math.random, no file I/O, no env
 *    reads. Slot iteration uses a fixed constant order so object-literal
 *    declaration order cannot affect output.
 */
import { MaestroError } from "../domain/errors.js";

export const UKI_VERSION = "5.2";

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
  readonly field: keyof UkiSlots;
  readonly kind: SlotKind;
}

/**
 * The 12 slots in fixed order. Do not reorder -- R4 requires this exact
 * sequence. Iteration during compression uses this list, not Object.keys,
 * so object literal order never affects output.
 */
const SLOTS: readonly SlotDef[] = [
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
  { name: "CS", field: "cs", kind: "cs" },
  { name: "SUMMARY", field: "summary", kind: "single" },
];

const ARTIFACT_PREFIX_PATTERN = /(?:^|-)(?:commit|branch|version|file)_/;
const CS_VALUE_PATTERN = /^(work_\d+(?:\.\d+)?(?:~summary_\d+(?:\.\d+)?)?|summary_\d+(?:\.\d+)?)$/;
const FORBIDDEN_CHAR_PATTERN = /[:\n\r`*]/;

// ---------- compressUki ----------

/**
 * Compress a structured UkiSlots into a v5.2 single-string representation.
 *
 * Throws MaestroError with actionable hints on any rule violation (invalid
 * CS, SUMMARY over the limit, ARTIFACTS missing the required prefix, etc.).
 * Pure and deterministic: byte-identical output for byte-identical input.
 */
export function compressUki(slots: UkiSlots): string {
  const parts: string[] = new Array(SLOTS.length);
  for (let i = 0; i < SLOTS.length; i++) {
    const def = SLOTS[i]!;
    parts[i] = encodeSlot(def, slots);
  }
  const result = parts.join("|");
  assertCompressed(result);
  return result;
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
  // Resolve the raw value from the field.
  let raw: string;
  if (def.field === "stanceCollapse") {
    const value = slots.stanceCollapse?.trim();
    raw = value && value.length > 0 ? value : DEFAULT_STANCE_COLLAPSE;
  } else {
    const value = slots[def.field];
    if (typeof value !== "string") {
      throw compressError(
        `Slot ${def.name} must be a string, got ${typeof value}`,
        [`Field: ${String(def.field)}`],
      );
    }
    raw = value;
  }

  if (raw.length === 0) {
    throw compressError(
      `Slot ${def.name} must not be empty`,
      [
        `Field: ${String(def.field)}`,
        "Every v5.2 slot requires a non-empty value (STANCE_COLLAPSE defaults to NONE_DETECTED_LOW_FRICTION)",
      ],
    );
  }

  // Single-string slots may contain `-` as a sub-token separator. We
  // trim and forbid colons/newlines/markdown, then validate each sub-token
  // against R2. The value is kept as-is (no `-` normalization).
  const value = raw.trim();
  assertNoForbiddenChars(value, def.name);

  if (def.name === "SUMMARY" && value.length >= MAX_SUMMARY_LEN) {
    throw compressError(
      `SUMMARY exceeds v5.2 limit (${value.length} >= ${MAX_SUMMARY_LEN})`,
      [
        "Rewrite the summary to under 140 characters",
        "Format hint: [Essence-Progress-Honest_Risk]",
      ],
    );
  }

  for (const subToken of value.split("-")) {
    if (subToken.length === 0) {
      throw compressError(
        `Slot ${def.name} contains an empty sub-token (leading, trailing, or doubled '-')`,
        [`Offending value: ${value}`],
      );
    }
    assertWordCount(subToken, def.name);
  }
  return `${def.name}-${value}`;
}

function encodeList(def: SlotDef, slots: UkiSlots): string {
  const value = slots[def.field];
  if (!Array.isArray(value)) {
    throw compressError(
      `Slot ${def.name} must be an array, got ${typeof value}`,
      [`Field: ${String(def.field)}`],
    );
  }

  if (value.length === 0) {
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

  // R7: ARTIFACTS must contain at least one of commit_/branch_/version_/file_
  if (def.name === "ARTIFACTS") {
    const joined = normalized.join("-");
    if (!ARTIFACT_PREFIX_PATTERN.test(joined)) {
      throw compressError(
        "ARTIFACTS must contain at least one of commit_/branch_/version_/file_ (R7)",
        [
          "Add an artifact token like branch_<name> or commit_<sha>",
          "Or file_<path_with_underscores> / version_<x_y_z>",
        ],
      );
    }
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
        "Bare CS-N.NN is not a valid v5.2 shape",
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
    // Defensive -- this should be unreachable if the branches above are correct.
    throw compressError(`CS produced invalid value: ${value}`);
  }
  return `CS-${value}`;
}

function assertNoForbiddenChars(token: string, slotName: string): void {
  if (FORBIDDEN_CHAR_PATTERN.test(token)) {
    throw compressError(
      `Slot ${slotName} contains forbidden character (colon, newline, or markdown syntax)`,
      [
        "UKI v5.2 forbids ':', newlines, '`', and '*' in any slot",
        `Offending value: ${token}`,
      ],
    );
  }
}

/**
 * Enforce R2: each `_`-joined half of a `~`-separated token is limited to
 * four words.
 */
function assertWordCount(token: string, slotName: string): void {
  const halves = token.split("~");
  for (const half of halves) {
    if (half.length === 0) continue;
    const words = half.split("_").filter((w) => w.length > 0);
    if (words.length > MAX_WORDS_PER_TOKEN) {
      throw compressError(
        `Slot ${slotName} token exceeds word limit (${words.length} > ${MAX_WORDS_PER_TOKEN})`,
        [
          "UKI v5.2 caps _-joined tokens at 4 words per half (R2)",
          `Offending token: ${token}`,
        ],
      );
    }
  }
}

function assertCompressed(result: string): void {
  if (FORBIDDEN_CHAR_PATTERN.test(result)) {
    throw compressError("Compressed string contains forbidden character", [
      "Internal invariant violation",
    ]);
  }
  const pipes = countChar(result, "|");
  if (pipes !== SLOTS.length - 1) {
    throw compressError(
      `Compressed string has wrong pipe count: ${pipes} (expected ${SLOTS.length - 1})`,
    );
  }
}

function countChar(s: string, ch: string): number {
  let count = 0;
  for (const c of s) {
    if (c === ch) count++;
  }
  return count;
}

function isFiniteNumber(n: number | undefined): n is number {
  return typeof n === "number" && Number.isFinite(n);
}

/**
 * Format a confidence scalar deterministically. We want `0.9` not `0.900000`
 * and we want the same scalar to produce the same string every time. We use
 * the default `Number.prototype.toString()` which gives a minimal
 * representation.
 */
function formatConfidence(n: number): string {
  return String(n);
}

function compressError(message: string, hints: readonly string[] = []): MaestroError {
  return new MaestroError(`UKI compress: ${message}`, hints);
}

// ---------- parseUki ----------

/**
 * Parse a v5.2 compressed string back into UkiSlots. Throws MaestroError
 * with actionable hints if the input violates any of R1-R7.
 *
 * For valid inputs produced by compressUki, this function is the byte-exact
 * inverse: `compressUki(parseUki(x))` equals `x`.
 */
export function parseUki(raw: string): UkiSlots {
  const violations = validateUki(raw);
  if (violations.length > 0) {
    throw new MaestroError(
      `UKI parse: invalid v5.2 string (${violations.length} violation${violations.length === 1 ? "" : "s"})`,
      violations,
    );
  }
  return parseUkiUnchecked(raw);
}

/**
 * Parse a v5.2 compressed string that has already been validated. This
 * helper exists so the validator and parser do not double-walk the string.
 */
function parseUkiUnchecked(raw: string): UkiSlots {
  const parts = raw.split("|");
  const valuesByName = new Map<string, string>();
  for (let i = 0; i < SLOTS.length; i++) {
    const def = SLOTS[i]!;
    const slot = parts[i]!;
    const value = slot.slice(def.name.length + 1); // after "NAME-"
    valuesByName.set(def.name, value);
  }

  return {
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
  };
}

function parseListValue(value: string): readonly string[] {
  if (value === EMPTY_LIST_SENTINEL) return [];
  return value.split("-");
}

function parseCsValue(value: string): { work?: number; summary?: number } {
  // Value is the part after "CS-" -- e.g. "work_0.95", "summary_0.9",
  // or "work_0.95~summary_0.92".
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

// ---------- validateUki ----------

/**
 * Return a list of rule violations for a v5.2 compressed string. An empty
 * array indicates the string is valid. Does not throw. Use `parseUki` for
 * a throwing variant.
 */
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

  // Forbidden-character check (R4)
  if (FORBIDDEN_CHAR_PATTERN.test(raw)) {
    violations.push(
      "input contains forbidden character (':', newline, '`', or '*')",
    );
  }

  if (raw !== raw.trim()) {
    violations.push("input has leading or trailing whitespace");
  }

  // Slot count (R4)
  const parts = raw.split("|");
  if (parts.length !== SLOTS.length) {
    violations.push(
      `slot count is ${parts.length} but v5.2 requires exactly ${SLOTS.length}`,
    );
    // Return early -- nothing else can be meaningfully checked.
    return violations;
  }

  // Slot order (R4)
  for (let i = 0; i < SLOTS.length; i++) {
    const def = SLOTS[i]!;
    const slot = parts[i]!;
    if (!slot.startsWith(`${def.name}-`)) {
      violations.push(`slot ${i} must start with '${def.name}-' (R4)`);
    }
  }
  if (violations.some((v) => v.includes("R4"))) {
    return violations;
  }

  const valuesByName = new Map<string, string>();
  for (let i = 0; i < SLOTS.length; i++) {
    const def = SLOTS[i]!;
    const slot = parts[i]!;
    valuesByName.set(def.name, slot.slice(def.name.length + 1));
  }

  // Each slot must be non-empty (STANCE_COLLAPSE always present per R6)
  for (const def of SLOTS) {
    const value = valuesByName.get(def.name)!;
    if (value.length === 0) {
      violations.push(`slot ${def.name} is empty`);
    }
  }

  // STANCE_COLLAPSE present (R6)
  const stance = valuesByName.get("STANCE_COLLAPSE");
  if (!stance || stance.length === 0) {
    violations.push("STANCE_COLLAPSE slot must always be present (R6)");
  }

  // CS scoped format (R5)
  const cs = valuesByName.get("CS");
  if (cs && !CS_VALUE_PATTERN.test(cs)) {
    violations.push(
      `CS value '${cs}' is not scoped (R5: must be CS-work_X, CS-summary_Y, or CS-work_X~summary_Y)`,
    );
  }

  // ARTIFACTS must contain a required prefix (R7)
  const artifacts = valuesByName.get("ARTIFACTS");
  if (artifacts && artifacts !== EMPTY_LIST_SENTINEL) {
    if (!ARTIFACT_PREFIX_PATTERN.test(artifacts)) {
      violations.push(
        "ARTIFACTS must contain at least one of commit_/branch_/version_/file_ (R7)",
      );
    }
  } else if (artifacts === EMPTY_LIST_SENTINEL) {
    // Empty artifacts also fails R7 -- there is nothing to point at.
    violations.push("ARTIFACTS must contain at least one token (R7)");
  }

  // SUMMARY length (R3)
  const summary = valuesByName.get("SUMMARY");
  if (summary !== undefined && summary.length >= MAX_SUMMARY_LEN) {
    violations.push(
      `SUMMARY length ${summary.length} exceeds v5.2 limit ${MAX_SUMMARY_LEN} (R3)`,
    );
  }

  // Word-count per token (R2). Apply to every slot except CS. Both single
  // and list slots use `-` internally as a sub-token separator, so the
  // validator splits uniformly.
  for (const def of SLOTS) {
    if (def.name === "CS") continue;
    const value = valuesByName.get(def.name);
    if (value === undefined || value === EMPTY_LIST_SENTINEL) continue;
    const tokens = value.split("-");
    for (const token of tokens) {
      const halves = token.split("~");
      for (const half of halves) {
        if (half.length === 0) continue;
        const words = half.split("_").filter((w) => w.length > 0);
        if (words.length > MAX_WORDS_PER_TOKEN) {
          violations.push(
            `slot ${def.name} token '${token}' exceeds word limit (${words.length} > ${MAX_WORDS_PER_TOKEN}) (R2)`,
          );
        }
      }
    }
  }

  return violations;
}
