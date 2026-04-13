export type ReleaseBumpKind = "feature" | "patch";

const BREAKING_SUBJECT = /^[a-z]+(\(.+\))?!:/;
const BREAKING_BODY = /(^|\n)BREAKING[ -]CHANGE:\s+/m;
const FEAT_SUBJECT = /^feat(\(.+\))?[!:]/;

export function splitCommitMessages(raw: string): string[] {
  return raw
    .split("\0")
    .map((entry) => entry.trim())
    .filter((entry) => entry.length > 0);
}

export function classifyCommitBump(message: string): ReleaseBumpKind {
  const subject = message.split("\n", 1)[0]?.trim() ?? "";

  if (BREAKING_SUBJECT.test(subject) || BREAKING_BODY.test(message) || FEAT_SUBJECT.test(subject)) {
    return "feature";
  }

  return "patch";
}

export function summarizeCommitBumps(messages: readonly string[]): {
  readonly bump: ReleaseBumpKind;
  readonly featureCount: number;
  readonly patchCount: number;
} {
  let featureCount = 0;

  for (const message of messages) {
    if (classifyCommitBump(message) === "feature") {
      featureCount += 1;
    }
  }

  return {
    bump: featureCount > 0 ? "feature" : "patch",
    featureCount,
    patchCount: messages.length - featureCount,
  };
}
