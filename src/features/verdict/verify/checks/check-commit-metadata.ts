import type { GitSignatureProbePort } from "../ports/git-signature.port.js";
import type { TrustFinding } from "@/v2/types/trust.js";

// Matches a commit line: "commit <sha>"
const COMMIT_LINE_RE = /^commit ([0-9a-f]{7,40})/;
// gpgsig or "Good signature" lines indicate a signed commit.
const SIGNED_INDICATOR_RE = /gpgsig|Good signature|Signature made/i;

/**
 * Parses raw `git log --show-signature` output and returns the set of commit
 * SHAs that have no gpg/ssh signature indicators.
 */
function parseUnsignedCommits(logOutput: string): readonly string[] {
  const unsigned: string[] = [];
  let currentSha: string | undefined;
  let currentSigned = false;

  for (const line of logOutput.split("\n")) {
    const commitMatch = COMMIT_LINE_RE.exec(line);
    if (commitMatch) {
      if (currentSha !== undefined && !currentSigned) {
        unsigned.push(currentSha);
      }
      currentSha = commitMatch[1];
      currentSigned = false;
      continue;
    }
    if (currentSha && SIGNED_INDICATOR_RE.test(line)) {
      currentSigned = true;
    }
  }

  // Flush last commit.
  if (currentSha !== undefined && !currentSigned) {
    unsigned.push(currentSha);
  }

  return unsigned;
}

/**
 * Checks for unsigned commits in the base..head range.
 * Advisory at L2 — severity is "info".
 */
export async function checkCommitMetadata(
  base: string,
  head: string,
  repoRoot: string,
  probe: GitSignatureProbePort,
): Promise<readonly TrustFinding[]> {
  const logOutput = await probe.showSignatureLog({ repoRoot, base, head });
  const unsigned = parseUnsignedCommits(logOutput);

  if (unsigned.length === 0) {
    return [];
  }

  return [
    {
      check: "commit-metadata",
      severity: "info",
      paths: [],
      details: `unsigned commits: ${unsigned.join(", ")}`,
    },
  ];
}
