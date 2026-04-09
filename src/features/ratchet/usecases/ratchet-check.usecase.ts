import { readdir } from "node:fs/promises";
import { join } from "node:path";
import type { RatchetAssertion, RatchetBaseline } from "../domain/types.js";
import type { RatchetStorePort } from "../ports/ratchet-store.port.js";
import { readText } from "@/lib/fs.js";

export interface AssertionResult {
  readonly assertion: RatchetAssertion;
  readonly passed: boolean;
  readonly detail?: string;
}

export interface RatchetCheckResult {
  readonly results: readonly AssertionResult[];
  readonly passCount: number;
  readonly totalCount: number;
  readonly passed: boolean;
  readonly previousBaseline?: RatchetBaseline;
}

export async function checkRatchet(
  store: RatchetStorePort,
  projectDir: string,
): Promise<RatchetCheckResult> {
  const suite = await store.getSuite();
  const previousBaseline = await store.getBaseline();
  const results: AssertionResult[] = [];

  for (const assertion of suite.assertions) {
    const result = await runAssertion(assertion, projectDir);
    results.push(result);
  }

  const passCount = results.filter((r) => r.passed).length;
  const totalCount = results.length;
  const passed = passCount === totalCount;

  const baseline: RatchetBaseline = {
    passCount,
    lastRunAt: new Date().toISOString(),
  };
  await store.writeBaseline(baseline);

  return { results, passCount, totalCount, passed, previousBaseline };
}

async function runAssertion(
  assertion: RatchetAssertion,
  projectDir: string,
): Promise<AssertionResult> {
  try {
    const pattern = assertion.check;
    const matches = await grepProject(projectDir, pattern);

    if (matches.length === 0) {
      return { assertion, passed: true };
    }

    return {
      assertion,
      passed: false,
      detail: `Found ${matches.length} violation(s): ${matches.slice(0, 3).join(", ")}`,
    };
  } catch (err) {
    return {
      assertion,
      passed: false,
      detail: `Check error: ${err instanceof Error ? err.message : String(err)}`,
    };
  }
}

async function grepProject(dir: string, pattern: string): Promise<string[]> {
  const matches: string[] = [];
  const regex = new RegExp(pattern, "gi");

  async function walk(current: string): Promise<void> {
    try {
      const entries = await readdir(current, { withFileTypes: true });
      for (const entry of entries) {
        if (entry.name.startsWith(".") || entry.name === "node_modules" || entry.name === "dist") continue;
        const full = join(current, entry.name);
        if (entry.isDirectory()) {
          await walk(full);
        } else if (entry.isFile() && /\.(ts|js|json|yaml|yml|sh)$/.test(entry.name)) {
          const content = await readText(full);
          if (content && regex.test(content)) {
            matches.push(full);
            regex.lastIndex = 0;
          }
        }
      }
    } catch {
      // skip unreadable dirs
    }
  }

  await walk(dir);
  return matches;
}
