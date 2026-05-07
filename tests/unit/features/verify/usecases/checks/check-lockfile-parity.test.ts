import { describe, expect, it, beforeEach, afterEach } from "bun:test";
import { mkdtemp, writeFile, rm } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { checkLockfileParity } from "@/features/verify/usecases/checks/check-lockfile-parity.js";

let tmpDir: string;

beforeEach(async () => {
  tmpDir = await mkdtemp(join(tmpdir(), "lockfile-parity-"));
});

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true });
});

describe("checkLockfileParity", () => {
  it("package.json without bun.lock in diff — emits error finding", async () => {
    await writeFile(join(tmpDir, "package.json"), "{}");
    await writeFile(join(tmpDir, "bun.lock"), "");
    const findings = await checkLockfileParity(["package.json", "src/foo.ts"], tmpDir);
    expect(findings).toHaveLength(1);
    expect(findings[0].check).toBe("lockfile-parity");
    expect(findings[0].severity).toBe("error");
    expect(findings[0].details).toMatch(/bun\.lock/);
  });

  it("bun.lock without package.json in diff — emits error finding", async () => {
    await writeFile(join(tmpDir, "package.json"), "{}");
    await writeFile(join(tmpDir, "bun.lock"), "");
    const findings = await checkLockfileParity(["bun.lock"], tmpDir);
    expect(findings).toHaveLength(1);
    expect(findings[0].check).toBe("lockfile-parity");
    expect(findings[0].severity).toBe("error");
  });

  it("both package.json and bun.lock in diff — clean", async () => {
    await writeFile(join(tmpDir, "package.json"), "{}");
    await writeFile(join(tmpDir, "bun.lock"), "");
    const findings = await checkLockfileParity(["package.json", "bun.lock"], tmpDir);
    expect(findings).toEqual([]);
  });

  it("neither package.json nor bun.lock in diff — clean", async () => {
    await writeFile(join(tmpDir, "package.json"), "{}");
    await writeFile(join(tmpDir, "bun.lock"), "");
    const findings = await checkLockfileParity(["src/foo.ts"], tmpDir);
    expect(findings).toEqual([]);
  });

  it("lockfile files don't exist at project root — no false-positive findings", async () => {
    // No lockfile files created at all
    const findings = await checkLockfileParity(["package.json"], tmpDir);
    expect(findings).toEqual([]);
  });

  it("pnpm-lock.yaml without package.json — emits error finding", async () => {
    await writeFile(join(tmpDir, "package.json"), "{}");
    await writeFile(join(tmpDir, "pnpm-lock.yaml"), "");
    const findings = await checkLockfileParity(["pnpm-lock.yaml"], tmpDir);
    expect(findings).toHaveLength(1);
    expect(findings[0].severity).toBe("error");
  });
});
