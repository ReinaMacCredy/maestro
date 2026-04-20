import { describe, expect, it } from "bun:test";
import { AGENT_INSTRUCTION_BLOCK } from "@/infra/domain/bootstrap-templates.js";

describe("AGENT_INSTRUCTION_BLOCK", () => {
  it("documents the shared task coordination workflow", () => {
    expect(AGENT_INSTRUCTION_BLOCK).toContain("maestro task ready --json --compact --limit 5");
    expect(AGENT_INSTRUCTION_BLOCK).not.toContain("maestro task ready --json --limit 5");
    expect(AGENT_INSTRUCTION_BLOCK).toContain("maestro task claim <id>");
    expect(AGENT_INSTRUCTION_BLOCK).toContain("maestro task unclaim <id>");
  });

  it("advertises the in_progress shortcut on task create", () => {
    expect(AGENT_INSTRUCTION_BLOCK).toContain("--status pending|in_progress");
    expect(AGENT_INSTRUCTION_BLOCK).toContain(
      "add --status in_progress to start immediately",
    );
  });

  it("teaches the task plan batch pattern with name-slot references", () => {
    expect(AGENT_INSTRUCTION_BLOCK).toContain("Plan a batch of tasks upfront");
    expect(AGENT_INSTRUCTION_BLOCK).toContain("maestro task plan --file");
    expect(AGENT_INSTRUCTION_BLOCK).toContain('"blockedBy": ["first"]');
    expect(AGENT_INSTRUCTION_BLOCK).toContain("--start <name>");
    expect(AGENT_INSTRUCTION_BLOCK).toContain("batchId");
  });

  it("keeps the two non-obvious rules agents can't derive from --help", () => {
    expect(AGENT_INSTRUCTION_BLOCK).toContain("blockedBy");
    expect(AGENT_INSTRUCTION_BLOCK).toContain("persisted verbatim");
  });

  it("points completion at update --status completed with a reason", () => {
    expect(AGENT_INSTRUCTION_BLOCK).toContain(
      "maestro task update <id> --status completed --reason",
    );
  });

  it("documents the native handoff launcher and launch artifact path", () => {
    expect(AGENT_INSTRUCTION_BLOCK).toContain('maestro handoff "Implement <featureId> for mission <id>"');
    expect(AGENT_INSTRUCTION_BLOCK).toContain(".maestro/launches/<id>/");
  });
});
