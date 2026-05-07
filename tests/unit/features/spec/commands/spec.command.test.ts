import { afterEach, describe, it, expect } from "bun:test";
import { Command } from "commander";
import { registerSpecCommand } from "@/features/spec/commands/spec.command.js";
import { createSpec } from "@/features/spec/usecases/create-spec.usecase.js";
import { MaestroError } from "@/shared/errors.js";
import type { SpecStorePort } from "@/features/spec/ports/storage.js";
import type { Spec } from "@/features/spec/domain/types.js";

const originalConsoleLog = console.log;
const originalConsoleError = console.error;

function captureConsole(): { readonly logs: string[]; readonly errors: string[] } {
  const logs: string[] = [];
  const errors: string[] = [];
  console.log = (...args: unknown[]) => {
    logs.push(args.map(String).join(" "));
  };
  console.error = (...args: unknown[]) => {
    errors.push(args.map(String).join(" "));
  };
  return { logs, errors };
}

afterEach(() => {
  console.log = originalConsoleLog;
  console.error = originalConsoleError;
});

function mockSpecStore(initial: Spec[] = []): SpecStorePort {
  const store = new Map(initial.map((s) => [s.mission_id, s]));
  return {
    write: async (spec) => { store.set(spec.mission_id, spec); },
    read: async (missionId) => store.get(missionId),
    list: async () => [...store.values()].sort((a, b) => a.mission_id.localeCompare(b.mission_id)),
  };
}

function makeProgram(specStore: SpecStorePort): Command {
  const program = new Command().name("maestro").option("--json", "Output as JSON").exitOverride();
  // spec show tests don't touch missions; cast a stub through unknown.
  const missions = { get: async () => undefined } as unknown as ReturnType<
    Parameters<typeof registerSpecCommand>[1]["getServices"]
  >["missions"];
  registerSpecCommand(program, { getServices: () => ({ specStore, missions }) });
  return program;
}

describe("spec show", () => {
  it("throws MaestroError when no spec exists", async () => {
    const store = mockSpecStore();
    const program = makeProgram(store);
    await expect(
      program.parseAsync(["spec", "show", "--mission", "2026-05-04-001"], { from: "user" }),
    ).rejects.toBeInstanceOf(MaestroError);
  });

  it("outputs JSON that is parseable and contains the spec", async () => {
    const store = mockSpecStore();
    await createSpec(store, {
      mission_id: "2026-05-04-001",
      acceptance_criteria: [
        { text: "Tests pass" },
        { text: "Build succeeds" },
      ],
    });
    const { logs } = captureConsole();
    const program = makeProgram(store);
    await program.parseAsync(["spec", "show", "--mission", "2026-05-04-001", "--json"], { from: "user" });

    expect(logs.length).toBeGreaterThan(0);
    const parsed = JSON.parse(logs.join("\n")) as Spec;
    expect(parsed.mission_id).toBe("2026-05-04-001");
    expect(parsed.acceptance_criteria).toHaveLength(2);
    expect(parsed.schema_version).toBe(2);
  });

  it("text mode lists criteria with ids", async () => {
    const store = mockSpecStore();
    await createSpec(store, {
      mission_id: "2026-05-04-002",
      acceptance_criteria: [
        { text: "Alpha" },
        { text: "Beta" },
        { text: "Gamma" },
      ],
    });
    const { logs } = captureConsole();
    const program = makeProgram(store);
    await program.parseAsync(["spec", "show", "--mission", "2026-05-04-002"], { from: "user" });

    const all = logs.join("\n");
    expect(all).toContain("Alpha");
    expect(all).toContain("Beta");
    expect(all).toContain("Gamma");
    // Each criterion line should include the criterion id (crt-...)
    expect(all).toMatch(/crt-\d{13}-[0-9a-f]{8}/);
  });
});
