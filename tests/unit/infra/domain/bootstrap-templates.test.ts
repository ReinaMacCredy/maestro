import { describe, expect, it } from "bun:test";
import { AGENT_INSTRUCTION_BLOCK } from "@/infra/domain/bootstrap-templates.js";

describe("AGENT_INSTRUCTION_BLOCK", () => {
  it("documents the shared task coordination workflow", () => {
    expect(AGENT_INSTRUCTION_BLOCK).toContain("maestro task ready --json");
    expect(AGENT_INSTRUCTION_BLOCK).toContain("maestro task claim <id>");
    expect(AGENT_INSTRUCTION_BLOCK).toContain("maestro task unclaim <id>");
  });

  it("advertises the in_progress shortcut on task create", () => {
    expect(AGENT_INSTRUCTION_BLOCK).toContain("--status pending|in_progress");
    expect(AGENT_INSTRUCTION_BLOCK).toContain(
      "add --status in_progress to start immediately",
    );
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
});
