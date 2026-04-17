import { describe, expect, it } from "bun:test";
import { AGENT_INSTRUCTION_BLOCK } from "@/infra/domain/bootstrap-templates.js";

describe("AGENT_INSTRUCTION_BLOCK", () => {
  it("documents the shared task coordination workflow", () => {
    expect(AGENT_INSTRUCTION_BLOCK).toContain("maestro task ready --json");
    expect(AGENT_INSTRUCTION_BLOCK).toContain("maestro task claim <id>");
    expect(AGENT_INSTRUCTION_BLOCK).toContain("maestro task unclaim <id>");
  });

  it("warns agents that task create has no --status option", () => {
    expect(AGENT_INSTRUCTION_BLOCK).toContain("do NOT pass --status on create");
    expect(AGENT_INSTRUCTION_BLOCK).toContain("`task create` has no `--status` option");
  });

  it("enumerates the three valid statuses and rejects legacy values", () => {
    expect(AGENT_INSTRUCTION_BLOCK).toContain("`pending`, `in_progress`, `completed`");
    expect(AGENT_INSTRUCTION_BLOCK).toContain("`open`, `blocked`, `deferred`, `closed`");
  });

  it("points completion at update --status completed with a reason", () => {
    expect(AGENT_INSTRUCTION_BLOCK).toContain(
      "maestro task update <id> --status completed --reason",
    );
    expect(AGENT_INSTRUCTION_BLOCK).toContain("no `task close`");
    expect(AGENT_INSTRUCTION_BLOCK).toContain("no `task update --claim`");
  });
});
