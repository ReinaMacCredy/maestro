import { describe, it, expect } from "bun:test";
import type { GitPort } from "@/infra/ports/git.port.js";
import { ShellGitAdapter } from "@/infra/adapters/git.adapter.js";

describe("GitPort interface completeness", () => {
  it("ShellGitAdapter implements all GitPort methods", () => {
    const adapter = new ShellGitAdapter();
    
    // Check that all required methods exist
    expect(typeof adapter.getState).toBe("function");
    expect(typeof adapter.isRepo).toBe("function");
    expect(typeof adapter.getCurrentBranch).toBe("function");
    expect(typeof adapter.createWorktree).toBe("function");
  });

  it("GitPort interface has all expected methods", () => {
    // This test will fail at compile time if the interface is incomplete
    const portMethods: Array<keyof GitPort> = [
      "getState",
      "isRepo",
      "getCurrentBranch",
      "createWorktree",
    ];
    
    expect(portMethods.length).toBe(4);
  });
});
