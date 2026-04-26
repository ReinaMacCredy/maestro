import type { Task, TaskType } from "./task-types.js";
import { isTaskId } from "./task-id.js";
import {
  invalidTaskField,
  slugNotFound,
  taskNotFound,
} from "./task-errors.js";

export const ALLOWED_VERBS = [
  "implement",
  "fix",
  "chore",
  "spike",
  "epic",
] as const;

export type SlugVerb = (typeof ALLOWED_VERBS)[number];

export const SLUG_MAX_LENGTH = 60;
export const SLUG_DERIVE_MAX_SUFFIX = 9;
/** Soft cap applied to derived slugs so the kebab stays scannable. */
export const SLUG_DERIVE_TAIL_MAX = 32;
/** Word count cap applied to derived slugs after stop-word filtering. */
export const SLUG_DERIVE_MAX_WORDS = 4;

const VERB_SET: ReadonlySet<string> = new Set(ALLOWED_VERBS);

const SLUG_TAIL_PATTERN = /^[a-z0-9]+(?:-[a-z0-9]+)*$/;

const STOP_WORDS: ReadonlySet<string> = new Set([
  "a", "an", "the",
  "and", "or", "but",
  "of", "for", "to", "in", "on", "at", "by", "with", "from", "into", "onto",
  "after", "before", "as",
  "is", "are", "was", "were", "be", "been", "being",
  "this", "that", "these", "those", "it", "its",
]);

/** Drop hex commit hashes and pure-digit tokens — they're noise in display slugs. */
function isNoiseToken(word: string): boolean {
  if (/^[0-9]+$/.test(word)) return true;
  if (/^[0-9a-f]{6,}$/.test(word)) return true;
  return false;
}
/**
 * Single source of truth for the slug regex pattern. Used both for runtime
 * shape checks (via {@link isValidSlugShape}) and for the JSON Schema
 * `pattern` property exposed by `task plan --schema`.
 */
export const SLUG_PATTERN_SOURCE =
  `^(?:${ALLOWED_VERBS.join("|")})/[a-z0-9]+(?:-[a-z0-9]+)*$`;

/**
 * Validate the printable shape of a slug. Returns true when the slug is
 * `<verb>/<kebab>`, the verb is in ALLOWED_VERBS, the kebab tail uses only
 * lowercase ASCII alphanumerics and single hyphens, and the total length is
 * at most SLUG_MAX_LENGTH characters.
 */
export function isValidSlugShape(slug: string): boolean {
  if (typeof slug !== "string") return false;
  if (slug.length === 0 || slug.length > SLUG_MAX_LENGTH) return false;
  const slashIdx = slug.indexOf("/");
  if (slashIdx <= 0 || slashIdx === slug.length - 1) return false;
  if (slug.indexOf("/", slashIdx + 1) !== -1) return false;
  const verb = slug.slice(0, slashIdx);
  const tail = slug.slice(slashIdx + 1);
  if (!VERB_SET.has(verb)) return false;
  return SLUG_TAIL_PATTERN.test(tail);
}

/**
 * Validate a slug supplied by an external caller and return it verbatim, or
 * throw a MaestroError describing the violation.
 */
export function parseSlug(input: string): string {
  if (!isValidSlugShape(input)) {
    throw invalidTaskField(
      "slug",
      `'${input}' must be '<verb>/<kebab>' where verb is one of ${ALLOWED_VERBS.join(
        ", ",
      )} and the tail is a lowercase kebab string up to ${SLUG_MAX_LENGTH} chars total`,
    );
  }
  return input;
}

/**
 * Lowercase, transliterate, and kebab-case a free-form title for use as a
 * display slug. Drops English stop-words and pure-hex/digit noise tokens
 * (commit shas, line numbers), keeps at most {@link SLUG_DERIVE_MAX_WORDS}
 * significant words, and truncates at word boundaries so a slug never ends
 * mid-word.
 *
 * Returns the empty string when no kebab can be derived (e.g. punctuation
 * only).
 */
export function kebabFromTitle(title: string, max: number = SLUG_MAX_LENGTH): string {
  if (typeof title !== "string" || title.length === 0) return "";
  const transliterated = title
    .normalize("NFKD")
    .replace(/[̀-ͯ]/g, "");
  const lower = transliterated.toLowerCase();
  const collapsed = lower
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/-+/g, "-")
    .replace(/^-+|-+$/g, "");
  if (collapsed.length === 0) return "";

  const words = collapsed.split("-").filter((w) => w.length > 0);
  const meaningful = words.filter((w) => !STOP_WORDS.has(w) && !isNoiseToken(w));
  const useWords = (meaningful.length > 0 ? meaningful : words).slice(
    0,
    SLUG_DERIVE_MAX_WORDS,
  );

  const picked: string[] = [];
  for (const word of useWords) {
    const next = picked.length === 0 ? word : `${picked.join("-")}-${word}`;
    if (next.length > max) {
      if (picked.length === 0) return word.slice(0, max).replace(/-+$/g, "");
      break;
    }
    picked.push(word);
  }
  return picked.join("-");
}

/**
 * Choose the canonical verb for a task type. Default is `implement`.
 */
function verbForType(type: TaskType | undefined): SlugVerb {
  switch (type) {
    case "bug":
      return "fix";
    case "chore":
      return "chore";
    case "epic":
      return "epic";
    case "feature":
    case "task":
    default:
      return "implement";
  }
}

/**
 * Derive a slug `<verb>/<kebab>` from a title and task type. The verb is
 * picked from the type; the tail is `kebabFromTitle` capped so that the full
 * slug stays within SLUG_MAX_LENGTH. Throws when no kebab can be derived.
 */
export function deriveSlugFromTitle(title: string, type: TaskType | undefined): string {
  const verb = verbForType(type);
  const totalCap = SLUG_MAX_LENGTH - verb.length - 1;
  const tailMax = Math.min(totalCap, SLUG_DERIVE_TAIL_MAX);
  if (tailMax <= 0) {
    throw invalidTaskField(
      "slug",
      `verb '${verb}' leaves no room for a kebab tail under the ${SLUG_MAX_LENGTH}-char cap`,
    );
  }
  const tail = kebabFromTitle(title, tailMax);
  if (tail.length === 0) {
    throw invalidTaskField(
      "slug",
      `cannot derive a kebab slug from title '${title}'; pass --slug explicitly or rename the task`,
    );
  }
  const slug = `${verb}/${tail}`;
  if (!isValidSlugShape(slug)) {
    throw invalidTaskField(
      "slug",
      `derived slug '${slug}' did not pass shape validation`,
    );
  }
  return slug;
}

export interface TaskRefStore {
  all(): Promise<readonly Task[]>;
  get(id: string): Promise<Task | undefined>;
}

/**
 * Resolve a CLI-style identifier to a stored Task. Accepts either a `tsk-XXX`
 * id (R1: id-shaped strings short-circuit to `store.get`) or a slug (R3:
 * lowercase exact match against top-level slugs). Throws `slugNotFound` with
 * a Levenshtein-1 suggestion (R2) on miss.
 */
export async function resolveTaskRef(
  store: TaskRefStore,
  input: string,
): Promise<Task> {
  if (typeof input !== "string" || input.length === 0) {
    throw invalidTaskField("ref", "must be a non-empty task id or slug");
  }

  if (isTaskId(input)) {
    const task = await store.get(input);
    if (!task) {
      throw taskNotFound(input);
    }
    return task;
  }

  const all = await store.all();
  const slugs: string[] = [];
  for (const task of all) {
    if (task.parentId !== undefined || task.slug === undefined) continue;
    if (task.slug === input) {
      return task;
    }
    slugs.push(task.slug);
  }

  const suggestion = closestSlugSuggestion(input, slugs);
  throw slugNotFound(input, suggestion);
}

/** Return the closest known slug if it is within Levenshtein distance 1. */
export function closestSlugSuggestion(
  input: string,
  candidates: readonly string[],
): string | undefined {
  let best: string | undefined;
  for (const candidate of candidates) {
    if (levenshteinAtMostOne(input, candidate)) {
      if (best === undefined || candidate < best) {
        best = candidate;
      }
    }
  }
  return best;
}

/**
 * Collect every top-level (parentId-less) task's slug into a set. Shared by
 * `task create` and `task plan` to seed the on-disk collision check before
 * the lock-held write.
 */
export async function collectExistingTopLevelSlugs(
  store: Pick<TaskRefStore, "all">,
): Promise<Set<string>> {
  const slugs = new Set<string>();
  const all = await store.all();
  for (const task of all) {
    if (task.parentId === undefined && task.slug !== undefined) {
      slugs.add(task.slug);
    }
  }
  return slugs;
}

/**
 * Return the first free slug from `<base>`, `<base>-2`, ... `<base>-N` where
 * a candidate is "free" iff it appears in neither `existing` nor
 * `usedInBatch`. Returns undefined when every candidate is taken.
 */
export function pickFreeDerivedSlug(
  base: string,
  existing: ReadonlySet<string>,
  usedInBatch: ReadonlySet<string> = new Set(),
): string | undefined {
  const isFree = (slug: string): boolean => !existing.has(slug) && !usedInBatch.has(slug);
  if (isFree(base)) return base;
  for (let suffix = 2; suffix <= SLUG_DERIVE_MAX_SUFFIX; suffix++) {
    const candidate = `${base}-${suffix}`;
    if (isFree(candidate)) return candidate;
  }
  return undefined;
}

function levenshteinAtMostOne(a: string, b: string): boolean {
  if (a === b) return true;
  if (Math.abs(a.length - b.length) > 1) return false;

  if (a.length === b.length) {
    let diffs = 0;
    for (let i = 0; i < a.length; i++) {
      if (a[i] !== b[i]) {
        diffs++;
        if (diffs > 1) return false;
      }
    }
    return diffs === 1;
  }

  const shorter = a.length < b.length ? a : b;
  const longer = a.length < b.length ? b : a;
  let i = 0;
  let j = 0;
  let edits = 0;
  while (i < shorter.length && j < longer.length) {
    if (shorter[i] === longer[j]) {
      i++;
      j++;
      continue;
    }
    edits++;
    if (edits > 1) return false;
    j++;
  }
  return true;
}
