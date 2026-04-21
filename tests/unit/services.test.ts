import { describe, expect, it } from "bun:test";
import { getServices, initServices } from "@/services";

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

  it("throws when getServices is called before initialization", async () => {
    const freshServicesModule = await import(`@/services?uninitialized=${Date.now()}`);

    expect(() => freshServicesModule.getServices()).toThrow(
      "Services not initialized. Call initServices() first.",
    );
  });

  it("initializes and returns the shared service instance", () => {
    const services = initServices(process.cwd());

    expect(getServices()).toBe(services);
    expect(services).toMatchObject({
      config: expect.any(Object),
      git: expect.any(Object),
      sessionDetect: expect.any(Object),
      notesStore: expect.any(Object),
      missionStore: expect.any(Object),
      correctionStore: expect.any(Object),
      launchStore: expect.any(Object),
      handoffLaunchers: {
        codex: expect.any(Object),
        claude: expect.any(Object),
      },
      ratchetStore: expect.any(Object),
      projectGraphStore: expect.any(Object),
      taskStore: expect.any(Object),
      contractStore: expect.any(Object),
      gitAnchor: expect.any(Object),
      replyStore: expect.any(Object),
    });
  });
});
