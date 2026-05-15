import { describe, expect, it } from "bun:test";
import { createServices } from "@/services";

describe("services composition root", () => {
  it("imports feature service builders directly instead of feature public surfaces", async () => {
    const source = await Bun.file(new URL("../../src/services.ts", import.meta.url)).text();

    expect(source).toContain('./features/session/services.js');
    expect(source).toContain('./features/notes/services.js');
    expect(source).toContain('./features/mission/services.js');
    expect(source).toContain('./features/handoff/services.js');
    expect(source).toContain('./features/graph/services.js');
    expect(source).toContain('./features/task/services.js');

    expect(source).not.toContain('./features/session/index.js');
    expect(source).not.toContain('./features/notes/index.js');
    expect(source).not.toContain('./features/mission/index.js');
    expect(source).not.toContain('./features/handoff/index.js');
    expect(source).not.toContain('./features/graph/index.js');
    expect(source).not.toContain('./features/task/index.js');

    // v1 memory + memory-ratchet were retired in Phase 4 (ADR-0015 absorbs
    // them into `principle` + provider gates).
    expect(source).not.toContain('./features/memory/');
    expect(source).not.toContain('./features/memory-ratchet/');
    expect(source).not.toContain('./features/agent/');
  });

  it("createServices returns a fresh, fully-populated Services graph", () => {
    const services = createServices(process.cwd());

    expect(services).toMatchObject({
      config: expect.any(Object),
      git: expect.any(Object),
      sessionDetect: expect.any(Object),
      notesStore: expect.any(Object),
      missionStore: expect.any(Object),
      missions: expect.any(Object),
      handoffStore: expect.any(Object),
      handoffLaunchers: {
        codex: expect.any(Object),
        claude: expect.any(Object),
      },
      projectGraphStore: expect.any(Object),
      taskStore: expect.any(Object),
      contractStore: expect.any(Object),
      gitAnchor: expect.any(Object),
      replyStore: expect.any(Object),
    });
  });

  it("createServices applies overrides on top of the base graph", () => {
    const customGit = { sentinel: true } as never;
    const services = createServices(process.cwd(), { git: customGit });

    expect(services.git).toBe(customGit);
  });

  it("createServices returns a fresh instance each call (no module-level cache)", () => {
    const a = createServices(process.cwd());
    const b = createServices(process.cwd());
    expect(a).not.toBe(b);
  });
});
