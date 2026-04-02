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

export function parseRawOutput(raw: string): string {
  return raw.trim();
}

export function parseStreamJsonOutput(raw: string, _workerSlug: string): string {
  const lines = raw.split(/\r?\n/);
  const extracted: string[] = [];

  for (const line of lines) {
    const trimmed = line.trim();
    if (trimmed.length === 0) {
      continue;
    }

    try {
      const parsed = JSON.parse(trimmed) as unknown;
      extracted.push(...extractText(parsed));
    } catch {
      continue;
    }
  }

  const normalized = extracted
    .map((value) => value.trim())
    .filter((value) => value.length > 0);

  if (normalized.length === 0) {
    return raw.trim();
  }

  return normalized.join("\n").trim();
}
