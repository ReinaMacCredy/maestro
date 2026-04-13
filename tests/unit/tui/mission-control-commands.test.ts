import { describe, expect, it } from "bun:test";
import { getMissionControlCommandSpecs } from "@/tui/state/mission-control-commands.js";

describe("getMissionControlCommandSpecs", () => {
  it("omits mission-scoped commands in home mode", () => {
    const commandIds = getMissionControlCommandSpecs("home").map((command) => command.id);

    expect(commandIds).toEqual(["features", "agent-grid", "handoffs", "event-stream", "task-board", "config", "memory", "graph", "help", "exit"]);
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
      "help",
      "exit",
    ]);
    expect(commands.find((command) => command.id === "dependencies")?.key).toBe("B");
    expect(commands.find((command) => command.id === "graph")?.key).toBe("G");
  });
});
