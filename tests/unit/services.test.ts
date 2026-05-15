import { describe, expect, it } from "bun:test";
import { createServices } from "@/services";

describe("services composition root", () => {
  it("imports feature service builders directly instead of feature public surfaces", async () => {
    const source = await Bun.file(new URL("../../src/services.ts", import.meta.url)).text();

    expect(source).not.toContain('./features/mission/');
    expect(source).toContain('./features/principle/services.js');
    expect(source).toContain('./features/reply/services.js');
    expect(source).toContain('./features/handoff/services.js');
    expect(source).toContain('./features/task/services.js');

    expect(source).not.toContain('./features/principle/index.js');
    expect(source).not.toContain('./features/reply/index.js');
    expect(source).not.toContain('./features/handoff/index.js');
    expect(source).not.toContain('./features/task/index.js');

    // v1 modules retired in Phase 4 (ADR-0015 + ADR-0018):
    //   memory / memory-ratchet / agent  -> absorbed by `principle`
    //   session / notes / graph / intake  -> dropped from the harness OS
    for (const retired of [
      "memory", "memory-ratchet", "agent",
      "session", "notes", "graph", "intake",
    ]) {
      expect(source).not.toContain(`./features/${retired}/`);
    }
  });

  it("createServices returns a fresh, fully-populated Services graph", () => {
    const services = createServices(process.cwd());

    expect(services).toMatchObject({
      config: expect.any(Object),
      git: expect.any(Object),
      missionStore: expect.any(Object),
      missions: expect.any(Object),
      handoffStore: expect.any(Object),
      handoffLaunchers: {
        codex: expect.any(Object),
        claude: expect.any(Object),
      },
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
