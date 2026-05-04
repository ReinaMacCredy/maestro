import { describe, it, expect, spyOn, afterEach } from "bun:test";
import { Command } from "commander";
import { registerPolicyCommand } from "@/features/policy/commands/policy.command.js";
import type { PendingLoosening } from "@/features/policy/usecases/detect-pending-loosenings.usecase.js";

function makeProgram(): Command {
  return new Command().name("maestro").option("--json", "Output as JSON").exitOverride();
}

function makePendingLoosening(): PendingLoosening {
  const effectiveAt = new Date(Date.now() + 29 * 24 * 60 * 60 * 1000).toISOString();
  return {
    commitSha: "abc123def456abc123def456abc123def456abc1",
    commitTime: new Date().toISOString(),
    effectiveAt,
    oldYaml: "required_witness_level:\n  high: witnessed-by-maestro",
    kind: "autopilot",
    file: ".maestro/policies/autopilot.yaml",
    edit: {
      description: "required_witness_level.high lowered from 'witnessed-by-maestro' to 'agent-claimed-locally'",
      path: "requiredWitnessLevel.high",
      oldValue: "witnessed-by-maestro",
      newValue: "agent-claimed-locally",
    },
  };
}

// Capture console.log calls
function captureLog(fn: () => Promise<void>): Promise<string[]> {
  const lines: string[] = [];
  const spy = spyOn(console, "log").mockImplementation((...args: unknown[]) => {
    lines.push(args.map(String).join(" "));
  });
  return fn().then(() => {
    spy.mockRestore();
    return lines;
  }).catch((err) => {
    spy.mockRestore();
    throw err;
  });
}

describe("maestro policy pending", () => {
  afterEach(() => {
    // Nothing to clean up
  });

  it("lists a pending loosening in text mode", async () => {
    const loosening = makePendingLoosening();
    const deps = {
      detectPendingLoosenings: async () => [loosening] as readonly PendingLoosening[],
    };

    const program = makeProgram();
    registerPolicyCommand(program, deps);

    const lines = await captureLog(() =>
      program.parseAsync(["node", "maestro", "policy", "pending"]),
    );

    expect(lines.length).toBeGreaterThan(0);
    expect(lines[0]).toContain(".maestro/policies/autopilot.yaml");
    expect(lines[0]).toContain("[autopilot]");
    expect(lines[0]).toContain("lowered");
    expect(lines[0]).toContain("effective");
  });

  it("outputs JSON array in --json mode", async () => {
    const loosening = makePendingLoosening();
    const deps = {
      detectPendingLoosenings: async () => [loosening] as readonly PendingLoosening[],
    };

    const program = makeProgram();
    registerPolicyCommand(program, deps);

    const lines = await captureLog(() =>
      program.parseAsync(["node", "maestro", "policy", "pending", "--json"]),
    );

    const json = JSON.parse(lines.join("\n")) as PendingLoosening[];
    expect(Array.isArray(json)).toBe(true);
    expect(json).toHaveLength(1);
    expect(json[0].kind).toBe("autopilot");
    expect(json[0].commitSha).toBe(loosening.commitSha);
  });

  it("shows 'No pending loosenings.' when list is empty (text mode)", async () => {
    const deps = {
      detectPendingLoosenings: async () => [] as readonly PendingLoosening[],
    };

    const program = makeProgram();
    registerPolicyCommand(program, deps);

    const lines = await captureLog(() =>
      program.parseAsync(["node", "maestro", "policy", "pending"]),
    );

    expect(lines).toHaveLength(1);
    expect(lines[0]).toContain("No pending loosenings");
  });

  it("outputs empty array in --json mode when no loosenings", async () => {
    const deps = {
      detectPendingLoosenings: async () => [] as readonly PendingLoosening[],
    };

    const program = makeProgram();
    registerPolicyCommand(program, deps);

    const lines = await captureLog(() =>
      program.parseAsync(["node", "maestro", "policy", "pending", "--json"]),
    );

    const json = JSON.parse(lines.join("\n")) as unknown[];
    expect(json).toHaveLength(0);
  });
});
