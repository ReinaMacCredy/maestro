import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdirSync, mkdtempSync, realpathSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { findMaestroProjectRoot } from "@/features/mcp/server/project.js";

describe("findMaestroProjectRoot", () => {
  let root: string;

  beforeEach(() => {
    root = realpathSync(mkdtempSync(join(tmpdir(), "maestro-mcp-project-")));
  });

  afterEach(() => {
    rmSync(root, { recursive: true, force: true });
  });

  it("walks up from the start directory to find .maestro/", () => {
    mkdirSync(join(root, ".maestro"));
    const nested = join(root, "a", "b", "c");
    mkdirSync(nested, { recursive: true });
    const found = findMaestroProjectRoot(nested, {} as NodeJS.ProcessEnv);
    expect(found).toBe(root);
  });

  it("returns the start directory itself when .maestro/ is here", () => {
    mkdirSync(join(root, ".maestro"));
    const found = findMaestroProjectRoot(root, {} as NodeJS.ProcessEnv);
    expect(found).toBe(root);
  });

  it("respects MAESTRO_PROJECT_ROOT when it points at a maestro project", () => {
    mkdirSync(join(root, ".maestro"));
    const elsewhere = realpathSync(mkdtempSync(join(tmpdir(), "maestro-mcp-elsewhere-")));
    try {
      const found = findMaestroProjectRoot(elsewhere, {
        MAESTRO_PROJECT_ROOT: root,
      } as NodeJS.ProcessEnv);
      expect(found).toBe(root);
    } finally {
      rmSync(elsewhere, { recursive: true, force: true });
    }
  });

  it("ignores MAESTRO_PROJECT_ROOT when it does not contain .maestro/ and walks up from start", () => {
    mkdirSync(join(root, ".maestro"));
    const nested = join(root, "deep");
    mkdirSync(nested, { recursive: true });
    const bogus = realpathSync(mkdtempSync(join(tmpdir(), "maestro-mcp-bogus-")));
    try {
      const found = findMaestroProjectRoot(nested, {
        MAESTRO_PROJECT_ROOT: bogus,
      } as NodeJS.ProcessEnv);
      expect(found).toBe(root);
    } finally {
      rmSync(bogus, { recursive: true, force: true });
    }
  });

  it("throws when no .maestro/ ancestor exists", () => {
    const lonely = realpathSync(mkdtempSync(join(tmpdir(), "maestro-mcp-lonely-")));
    try {
      expect(() =>
        findMaestroProjectRoot(lonely, {} as NodeJS.ProcessEnv),
      ).toThrow(/Not in a maestro project/);
    } finally {
      rmSync(lonely, { recursive: true, force: true });
    }
  });
});
