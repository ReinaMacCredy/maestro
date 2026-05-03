import { describe, expect, it } from "bun:test";
import { checkCommitMetadata } from "@/features/verify/usecases/checks/check-commit-metadata.js";
import type { GitSignatureProbePort } from "@/features/verify/ports/git-signature.port.js";

function makeProbe(output: string): GitSignatureProbePort {
  return {
    showSignatureLog: async () => output,
  };
}

// Sample git log --show-signature output for a signed commit
const SIGNED_OUTPUT = `commit abc1234
gpgsig -----BEGIN PGP SIGNATURE-----
 iQEzBAABCAAdFiEE...
 -----END PGP SIGNATURE-----
Author: Alice <alice@example.com>
Date:   Mon Jan 1 00:00:00 2026 +0000

    feat: signed commit
`;

// Unsigned commit output
const UNSIGNED_OUTPUT = `commit def5678
Author: Bob <bob@example.com>
Date:   Mon Jan 1 00:00:00 2026 +0000

    feat: unsigned commit
`;

// Multiple commits — first signed, second unsigned
const MIXED_OUTPUT = `commit abc1234
gpgsig -----BEGIN PGP SIGNATURE-----
 iQEzBAABCAAdFiEE...
 -----END PGP SIGNATURE-----
Author: Alice <alice@example.com>
Date:   Mon Jan 1 00:00:00 2026 +0000

    feat: signed

commit def5678
Author: Bob <bob@example.com>
Date:   Mon Jan 1 00:00:00 2026 +0000

    feat: unsigned
`;

describe("checkCommitMetadata", () => {
  it("all commits signed — empty findings", async () => {
    const findings = await checkCommitMetadata(
      "base-sha",
      "head-sha",
      "/repo",
      makeProbe(SIGNED_OUTPUT),
    );
    expect(findings).toEqual([]);
  });

  it("unsigned commit — emits info finding listing sha", async () => {
    const findings = await checkCommitMetadata(
      "base-sha",
      "head-sha",
      "/repo",
      makeProbe(UNSIGNED_OUTPUT),
    );
    expect(findings).toHaveLength(1);
    expect(findings[0].check).toBe("commit-metadata");
    expect(findings[0].severity).toBe("info");
    expect(findings[0].details).toMatch(/def5678/);
  });

  it("mixed output — only lists unsigned commits", async () => {
    const findings = await checkCommitMetadata(
      "base-sha",
      "head-sha",
      "/repo",
      makeProbe(MIXED_OUTPUT),
    );
    expect(findings).toHaveLength(1);
    expect(findings[0].details).toMatch(/def5678/);
    expect(findings[0].details).not.toMatch(/abc1234/);
  });

  it("empty log output — empty findings", async () => {
    const findings = await checkCommitMetadata("base-sha", "head-sha", "/repo", makeProbe(""));
    expect(findings).toEqual([]);
  });
});
