import { describe, expect, it } from "bun:test";

describe("services composition root", () => {
  it("imports feature service builders directly instead of feature public surfaces", async () => {
    const source = await Bun.file(new URL("../../src/services.ts", import.meta.url)).text();

    expect(source).toContain('./features/session/services.js');
    expect(source).toContain('./features/notes/services.js');
    expect(source).toContain('./features/mission/services.js');
    expect(source).toContain('./features/memory/services.js');
    expect(source).toContain('./features/handoff/services.js');
    expect(source).toContain('./features/ratchet/services.js');
    expect(source).toContain('./features/graph/services.js');
    expect(source).toContain('./features/task/services.js');

    expect(source).not.toContain('./features/session/index.js');
    expect(source).not.toContain('./features/notes/index.js');
    expect(source).not.toContain('./features/mission/index.js');
    expect(source).not.toContain('./features/memory/index.js');
    expect(source).not.toContain('./features/handoff/index.js');
    expect(source).not.toContain('./features/ratchet/index.js');
    expect(source).not.toContain('./features/graph/index.js');
    expect(source).not.toContain('./features/task/index.js');
  });
});
