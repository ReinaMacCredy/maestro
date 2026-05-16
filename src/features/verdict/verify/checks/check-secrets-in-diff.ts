import type { TrustFinding } from "@/types/trust.js";

// Known credential patterns — each match is severity "error".
const KNOWN_SECRET_PATTERNS: Array<{ name: string; re: RegExp }> = [
  { name: "aws-access-key-id", re: /AKIA[0-9A-Z]{16}/ },
  { name: "github-pat", re: /ghp_[A-Za-z0-9]{36}/ },
  { name: "slack-token", re: /xox[baprs]-[A-Za-z0-9-]{10,}/ },
  { name: "pem-private-key", re: /-----BEGIN [A-Z ]+PRIVATE KEY-----/ },
];

// High-entropy heuristic:
//   Conservative — only flag base64-shaped strings (≥40 chars of [A-Za-z0-9+/=_-])
//   that appear within 60 characters of a keyword (key|token|secret|password).
//   Length floor and keyword proximity keep false-positive rate down.
//   We intentionally do NOT flag every long string — only likely-credential contexts.
const HIGH_ENTROPY_CONTEXT_RE =
  /(?:key|token|secret|password)[^\n]{0,60}[A-Za-z0-9+/=_-]{40,}/i;

/**
 * Scans added lines (lines starting with "+") in the diff for credential
 * patterns. Each match emits an error-severity finding.
 */
export function checkSecretsInDiff(
  addedLines: readonly string[],
): readonly TrustFinding[] {
  const findings: TrustFinding[] = [];
  const matchedNames = new Set<string>();
  const contextHit: string[] = [];

  for (const line of addedLines) {
    let lineMatchedKnown = false;
    for (const { name, re } of KNOWN_SECRET_PATTERNS) {
      if (!matchedNames.has(name) && re.test(line)) {
        matchedNames.add(name);
        lineMatchedKnown = true;
        findings.push({
          check: "secrets-in-diff",
          severity: "error",
          paths: [],
          details: `potential secret detected (${name}).`,
        });
      }
    }

    // Only run the heuristic if no known pattern already matched this line —
    // avoids emitting a duplicate finding for the same token.
    if (!lineMatchedKnown && contextHit.length === 0 && HIGH_ENTROPY_CONTEXT_RE.test(line)) {
      contextHit.push(line);
      findings.push({
        check: "secrets-in-diff",
        severity: "error",
        paths: [],
        details:
          "potential high-entropy credential detected (long token-shaped string near key/token/secret/password keyword).",
      });
    }
  }

  return findings;
}
