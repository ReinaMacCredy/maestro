import { describe, expect, it } from "bun:test";
import { getMissionControlCommandSpecs } from "@/tui/state/mission-control-commands.js";

describe("getMissionControlCommandSpecs", () => {
  it("omits mission-scoped commands in home mode but keeps principles scoreboard", () => {
    const commandIds = getMissionControlCommandSpecs("home").map((command) => command.id);

    expect(commandIds).toEqual([
      "features",
      "agent-grid",
      "handoffs",
      "event-stream",
      "task-board",
      "config",
      "memory",
      "graph",
      "principle-review",
      "help",
      "exit",
    ]);
  });

  it("keeps graph available in mission mode with a dedicated dependencies hotkey", () => {
    const commands = getMissionControlCommandSpecs("mission");

    expect(commands.map((command) => command.id)).toEqual([
      "features",
      "agent-grid",
      "dispatch",
      "event-stream",
      "dependencies",
      "handoffs",
      "timeline",
      "task-board",
      "config",
      "memory",
      "graph",
      "principle-review",
      "help",
      "exit",
    ]);
    expect(commands.find((command) => command.id === "dependencies")?.key).toBe("B");
    expect(commands.find((command) => command.id === "graph")?.key).toBe("G");
    expect(commands.find((command) => command.id === "principle-review")?.key).toBe("R");
  });
});
