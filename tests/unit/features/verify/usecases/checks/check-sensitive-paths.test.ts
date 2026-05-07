import { describe, expect, it, beforeEach, afterEach } from "bun:test";
import { mkdtemp, mkdir, writeFile, rm } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { checkSensitivePaths } from "@/features/verify/usecases/checks/check-sensitive-paths.js";

let tmpDir: string;

beforeEach(async () => {
  tmpDir = await mkdtemp(join(tmpdir(), "sensitive-paths-"));
  await mkdir(join(tmpDir, ".maestro", "policies"), { recursive: true });
});

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true });
});

async function writeSensitivePaths(globs: string[]): Promise<void> {
  const content = `paths:\n${globs.map((g) => `  - "${g}"`).join("\n")}\n`;
  await writeFile(join(tmpDir, ".maestro", "policies", "sensitive-paths.yaml"), content);
}

describe("checkSensitivePaths", () => {
  it("no policy file — returns empty findings", async () => {
    const findings = await checkSensitivePaths(["secrets/key.pem"], tmpDir);
    expect(findings).toEqual([]);
  });

  it("policy file with matching glob — emits warn finding", async () => {
    await writeSensitivePaths(["secrets/**", "*.pem"]);
    const findings = await checkSensitivePaths(["secrets/key.pem", "src/foo.ts"], tmpDir);
    expect(findings).toHaveLength(1);
    expect(findings[0].check).toBe("sensitive-paths");
    expect(findings[0].severity).toBe("warn");
    expect(findings[0].paths).toContain("secrets/key.pem");
  });

  it("policy file present but no diff path matches — empty findings", async () => {
    await writeSensitivePaths(["secrets/**"]);
    const findings = await checkSensitivePaths(["src/foo.ts", "docs/README.md"], tmpDir);
    expect(findings).toEqual([]);
  });

  it("multiple sensitive paths matched — all included in single finding", async () => {
    await writeSensitivePaths(["*.pem", "*.key"]);
    const findings = await checkSensitivePaths(["cert.pem", "id.key", "src/ok.ts"], tmpDir);
    expect(findings).toHaveLength(1);
    expect(findings[0].paths).toContain("cert.pem");
    expect(findings[0].paths).toContain("id.key");
    expect(findings[0].paths).not.toContain("src/ok.ts");
  });

  it("empty paths list in policy — returns empty findings", async () => {
    await writeFile(
      join(tmpDir, ".maestro", "policies", "sensitive-paths.yaml"),
      "paths: []\n",
    );
    const findings = await checkSensitivePaths(["secrets/key.pem"], tmpDir);
    expect(findings).toEqual([]);
  });
});
