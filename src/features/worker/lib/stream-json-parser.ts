function extractText(value: unknown): string[] {
  if (typeof value === "string") {
    return [value];
  }

  if (Array.isArray(value)) {
    return value.flatMap(extractText);
  }

  if (value && typeof value === "object") {
    const obj = value as Record<string, unknown>;
    const prioritizedKeys = ["result", "text", "content", "message", "output"];
    const texts: string[] = [];

    for (const key of prioritizedKeys) {
      if (key in obj) {
        texts.push(...extractText(obj[key]));
      }
    }

    if (texts.length > 0) {
      return texts;
    }

    return Object.values(obj).flatMap(extractText);
  }

  return [];
}

function normalizeExtractedText(values: readonly string[]): string[] {
  return values
    .map((value) => value.trim())
    .filter((value) => value.length > 0);
}

export function parseRawOutput(raw: string): string {
  return raw.trim();
}

export function extractStreamJsonLineText(line: string): readonly string[] {
  const trimmed = line.trim();
  if (trimmed.length === 0) {
    return [];
  }

  try {
    const parsed = JSON.parse(trimmed) as unknown;
    return normalizeExtractedText(extractText(parsed));
  } catch {
    return [];
  }
}

export function parseStreamJsonOutput(raw: string, _workerSlug: string): string {
  const lines = raw.split(/\r?\n/);
  const extracted: string[] = [];

  for (const line of lines) {
    extracted.push(...extractStreamJsonLineText(line));
  }

  const normalized = normalizeExtractedText(extracted);
  if (normalized.length === 0) {
    return raw.trim();
  }

  return normalized.join("\n").trim();
}
