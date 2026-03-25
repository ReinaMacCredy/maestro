/**
 * Shared YAML frontmatter parser.
 * Handles the subset of YAML we need: simple `key: value` pairs.
 */

const FRONTMATTER_RE = /^---\r?\n([\s\S]*?)\r?\n---/;
const FRONTMATTER_BLOCK_RE = /^---\r?\n[\s\S]*?\r?\n---\r?\n?/;

/**
 * Parse YAML frontmatter between `---` markers.
 * Returns null if no valid frontmatter is found.
 * Returns Record<string, string> -- DO NOT widen this type (5 callers depend on it).
 */
export function parseFrontmatter(content: string): Record<string, string> | null {
  const match = content.match(FRONTMATTER_RE);
  if (!match) return null;

  const body = match[1];
  const result: Record<string, string> = {};

  for (const line of body.split(/\r?\n/)) {
    const trimmed = line.trim();
    if (!trimmed || trimmed.startsWith('#')) continue;

    const colonIdx = trimmed.indexOf(':');
    if (colonIdx < 1) continue;

    const key = trimmed.slice(0, colonIdx).trim();
    let value = trimmed.slice(colonIdx + 1).trim();
    if (
      (value.startsWith('"') && value.endsWith('"')) ||
      (value.startsWith("'") && value.endsWith("'"))
    ) {
      value = value.slice(1, -1);
    }
    result[key] = value;
  }

  return Object.keys(result).length > 0 ? result : null;
}

/**
 * Rich frontmatter parser -- supports inline arrays `[a, b]` and numeric values.
 * Multi-line YAML lists degrade to raw string (not crash).
 */
export function parseFrontmatterRich(content: string): Record<string, string | string[] | number> | null {
  const match = content.match(FRONTMATTER_RE);
  if (!match) return null;

  const body = match[1];
  const result: Record<string, string | string[] | number> = {};

  for (const line of body.split(/\r?\n/)) {
    const trimmed = line.trim();
    if (!trimmed || trimmed.startsWith('#')) continue;
    // Skip YAML list items (multi-line arrays)
    if (trimmed.startsWith('- ')) continue;

    const colonIdx = trimmed.indexOf(':');
    if (colonIdx < 1) continue;

    const key = trimmed.slice(0, colonIdx).trim();
    let raw = trimmed.slice(colonIdx + 1).trim();

    // Strip quotes
    if (
      (raw.startsWith('"') && raw.endsWith('"')) ||
      (raw.startsWith("'") && raw.endsWith("'"))
    ) {
      raw = raw.slice(1, -1);
    }

    // Inline array: [a, b, c]
    if (raw.startsWith('[') && raw.endsWith(']')) {
      const inner = raw.slice(1, -1);
      result[key] = inner
        .split(',')
        .map(s => s.trim())
        .filter(s => s.length > 0);
      continue;
    }

    // Numeric value
    if (raw !== '' && !isNaN(Number(raw))) {
      result[key] = Number(raw);
      continue;
    }

    result[key] = raw;
  }

  return Object.keys(result).length > 0 ? result : null;
}

/**
 * Returns the content body without the frontmatter block.
 */
export function stripFrontmatter(content: string): string {
  const match = content.match(FRONTMATTER_BLOCK_RE);
  if (!match) return content;
  return content.slice(match[0].length);
}

/**
 * Serialize a metadata record into YAML frontmatter.
 */
export function serializeFrontmatter(meta: Record<string, unknown>): string {
  const lines: string[] = ['---'];
  for (const [key, value] of Object.entries(meta)) {
    if (value === undefined || value === null) continue;
    if (Array.isArray(value)) {
      lines.push(`${key}: [${value.join(', ')}]`);
    } else {
      lines.push(`${key}: ${value}`);
    }
  }
  lines.push('---');
  return lines.join('\n');
}

/**
 * Prepend YAML frontmatter with metadata to content.
 * Returns content unchanged if no metadata fields are provided.
 */
export function prependMetadataFrontmatter(
  content: string,
  opts: { tags?: string[]; priority?: number; category?: string; selectionCount?: number; lastSelectedAt?: string },
): string {
  const meta: Record<string, unknown> = {};
  if (opts.tags?.length) meta.tags = opts.tags;
  if (opts.priority !== undefined) meta.priority = opts.priority;
  if (opts.category) meta.category = opts.category;
  if (opts.selectionCount !== undefined) meta.selectionCount = opts.selectionCount;
  if (opts.lastSelectedAt) meta.lastSelectedAt = opts.lastSelectedAt;
  if (Object.keys(meta).length === 0) return content;
  return serializeFrontmatter(meta) + '\n' + content;
}
