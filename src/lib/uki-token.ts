export const UKI_ANCHOR_PREFIXES = {
  branch: "branch_",
  feature: "feature_",
  file: "file_",
  milestone: "milestone_",
  mission: "mission_",
  plan: "plan_",
  spec: "spec_",
} as const;

interface NormalizeUkiTokenOptions {
  readonly fallback?: string;
  readonly maxSegments?: number;
}

export function normalizeUkiToken(
  value: string | undefined,
  options: NormalizeUkiTokenOptions = {},
): string {
  const {
    fallback = "unknown",
    maxSegments = 12,
  } = options;

  const normalized = (value ?? "")
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "_")
    .replace(/^_+|_+$/g, "");

  if (normalized.length === 0) {
    return fallback;
  }

  return normalized
    .split("_")
    .filter(Boolean)
    .slice(0, maxSegments)
    .join("_");
}
