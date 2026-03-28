import { describe, expect, it } from "bun:test";
import { generatePrompt } from "../../../src/usecases/generate-prompt.usecase.js";
import type { MaestroConfig } from "../../../src/domain/types.js";
import { DEFAULT_PROMPT_TEMPLATE } from "../../../src/domain/defaults.js";

const baseConfig: MaestroConfig = {
  sessionDetection: { enabled: true, agents: ["claude-code"] },
};

describe("generatePrompt", () => {
  it("uses default template with agent from opts", () => {
    const result = generatePrompt(baseConfig, {
      agent: "codex",
      handoffId: "2026-03-28-001",
    });
    expect(result).toContain("--agent codex");
    expect(result).toContain("handoff-pickup --claim");
    expect(result).not.toContain("Your task:");
  });

  it("falls back to config.defaultAgent when opts.agent is undefined", () => {
    const config: MaestroConfig = { ...baseConfig, defaultAgent: "gemini" };
    const result = generatePrompt(config, { handoffId: "2026-03-28-001" });
    expect(result).toContain("--agent gemini");
  });

  it("falls back to TARGET_AGENT when no agent specified anywhere", () => {
    const result = generatePrompt(baseConfig, { handoffId: "2026-03-28-001" });
    expect(result).toContain("--agent TARGET_AGENT");
  });

  it("includes task when provided", () => {
    const result = generatePrompt(baseConfig, {
      agent: "codex",
      task: "implement note command",
      handoffId: "2026-03-28-001",
    });
    expect(result).toContain("Your task: implement note command");
  });

  it("omits task block when task is not provided", () => {
    const result = generatePrompt(baseConfig, {
      agent: "codex",
      handoffId: "2026-03-28-001",
    });
    expect(result).not.toContain("Your task:");
    // Should not have double newlines from stripped conditional
    expect(result).not.toContain("\n\n\n");
  });

  it("uses custom template from config", () => {
    const config: MaestroConfig = {
      ...baseConfig,
      promptTemplate: "Agent {{agent}} pick up {{handoffId}}{{#task}} do {{task}}{{/task}}",
    };
    const result = generatePrompt(config, {
      agent: "codex",
      task: "deploy",
      handoffId: "2026-03-28-005",
    });
    expect(result).toBe("Agent codex pick up 2026-03-28-005 do deploy");
  });

  it("custom template without task", () => {
    const config: MaestroConfig = {
      ...baseConfig,
      promptTemplate: "{{agent}} picks up {{handoffId}}{{#task}} -- {{task}}{{/task}}",
    };
    const result = generatePrompt(config, {
      agent: "claude",
      handoffId: "2026-03-28-002",
    });
    expect(result).toBe("claude picks up 2026-03-28-002");
  });

  it("includes instructions when provided", () => {
    const result = generatePrompt(baseConfig, {
      agent: "codex",
      instructions: "Deploy to staging first",
      handoffId: "2026-03-28-001",
    });
    expect(result).toContain("Your instructions: Deploy to staging first");
  });

  it("omits instructions block when not provided", () => {
    const result = generatePrompt(baseConfig, {
      agent: "codex",
      handoffId: "2026-03-28-001",
    });
    expect(result).not.toContain("Your instructions:");
  });

  it("includes handoff-dig with --session when sessionId provided", () => {
    const result = generatePrompt(baseConfig, {
      agent: "codex",
      sessionId: "abc-session-123",
      handoffId: "2026-03-28-001",
    });
    expect(result).toContain("handoff-dig");
    expect(result).toContain("--session abc-session-123");
    expect(result).not.toContain("--id 2026-03-28-001");
  });

  it("omits dig hint when sessionId not provided", () => {
    const result = generatePrompt(baseConfig, {
      agent: "codex",
      handoffId: "2026-03-28-001",
    });
    expect(result).not.toContain("handoff-dig");
  });

  it("agent priority: opts.agent > config.defaultAgent > TARGET_AGENT", () => {
    const config: MaestroConfig = { ...baseConfig, defaultAgent: "gemini" };

    // opts.agent wins
    const r1 = generatePrompt(config, { agent: "codex", handoffId: "x" });
    expect(r1).toContain("--agent codex");

    // config.defaultAgent next
    const r2 = generatePrompt(config, { handoffId: "x" });
    expect(r2).toContain("--agent gemini");

    // fallback
    const r3 = generatePrompt(baseConfig, { handoffId: "x" });
    expect(r3).toContain("--agent TARGET_AGENT");
  });
});
