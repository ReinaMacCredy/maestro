import { afterEach, describe, expect, it } from "bun:test";
import { Command } from "commander";
import { registerIntakeCommand } from "@/features/intake/index.js";
import { DEFAULT_RISK_POLICY } from "@/features/policy/index.js";

const originalConsoleLog = console.log;
afterEach(() => { console.log = originalConsoleLog; });

function captureConsole(): { logs: string[] } {
  const logs: string[] = [];
  console.log = (...args: unknown[]) => { logs.push(args.map(String).join(" ")); };
  return { logs };
}

function makeProgram(sensitivePaths: readonly string[] = [".maestro/policies/**"]): Command {
  const program = new Command()
    .name("maestro")
    .exitOverride()
    .option("--json", "Output as JSON");
  registerIntakeCommand(program, {
    getServices: () => ({
      getEffectiveRiskPolicy: async () => DEFAULT_RISK_POLICY,
      getEffectiveSensitivePathsGlobs: async () => sensitivePaths,
    }),
  });
  return program;
}

describe("intake command", () => {
  it("emits JSON output with the lane and derived risk class", async () => {
    const program = makeProgram();
    const { logs } = captureConsole();

    await program.parseAsync(["node", "maestro", "intake", "--paths", "README.md", "--json"]);

    expect(logs.length).toBeGreaterThan(0);
    const parsed = JSON.parse(logs.join("\n"));
    expect(parsed.lane).toBe("tiny");
    expect(parsed.derivedRiskClass).toBeDefined();
    expect(parsed.recommendedNextStep).toBeDefined();
  });

  it("escalates to high-risk when an auth path is intended", async () => {
    const program = makeProgram();
    const { logs } = captureConsole();

    await program.parseAsync(["node", "maestro", "intake", "--paths", "src/auth/session.ts", "--json"]);

    const parsed = JSON.parse(logs.join("\n"));
    expect(parsed.lane).toBe("high-risk");
    expect(parsed.autoDetectedFlags).toContain("auth");
    expect(parsed.hardGatesTriggered).toContain("auth");
  });

  it("accepts a declared --flag and includes it in the result", async () => {
    const program = makeProgram();
    const { logs } = captureConsole();

    await program.parseAsync([
      "node", "maestro", "intake",
      "--paths", "src/foo.ts",
      "--flag", "weak-proof",
      "--flag", "existing-behavior",
      "--json",
    ]);

    const parsed = JSON.parse(logs.join("\n"));
    expect(parsed.declaredFlags).toEqual(["weak-proof", "existing-behavior"]);
    expect(parsed.lane).toBe("normal");
  });

  it("rejects an unknown --flag", async () => {
    const program = makeProgram();
    let err: unknown;
    try {
      await program.parseAsync(["node", "maestro", "intake", "--paths", "src/foo.ts", "--flag", "bogus", "--json"]);
    } catch (e) {
      err = e;
    }
    expect(err).toBeDefined();
    expect(String(err)).toContain("Unknown intake flag");
  });

  it("requires --paths", async () => {
    const program = makeProgram();
    let err: unknown;
    try {
      await program.parseAsync(["node", "maestro", "intake", "--json"]);
    } catch (e) {
      err = e;
    }
    expect(err).toBeDefined();
    expect(String(err)).toContain("--paths is required");
  });

  it("prints human-readable output without --json", async () => {
    const program = makeProgram();
    const { logs } = captureConsole();

    await program.parseAsync(["node", "maestro", "intake", "--paths", "src/auth/session.ts"]);

    const all = logs.join("\n");
    expect(all).toContain("[!! high-risk]");
    expect(all).toContain("auth");
  });
});
