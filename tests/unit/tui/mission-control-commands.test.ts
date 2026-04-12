import { describe, expect, it } from "bun:test";
import { getMissionControlCommandSpecs } from "@/tui/state/mission-control-commands.js";

describe("getMissionControlCommandSpecs", () => {
  it("omits mission-scoped commands in home mode", () => {
    const commandIds = getMissionControlCommandSpecs("home").map((command) => command.id);

    expect(commandIds).toEqual(["features", "agent-grid", "handoffs", "event-stream", "task-board", "config", "memory", "graph", "help", "exit"]);
  });
});
