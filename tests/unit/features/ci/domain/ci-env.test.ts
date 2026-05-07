import { describe, it, expect } from "bun:test";
import { readCiEnv } from "@/features/ci/domain/ci-env.js";

describe("readCiEnv", () => {
  it("detects GitHub Actions when GITHUB_ACTIONS=true", () => {
    const env = readCiEnv({ GITHUB_ACTIONS: "true" });
    expect(env.provider).toBe("github-actions");
  });

  it("returns provider=unknown when GITHUB_ACTIONS is unset", () => {
    const env = readCiEnv({});
    expect(env.provider).toBe("unknown");
  });

  it("returns provider=unknown when GITHUB_ACTIONS is not 'true'", () => {
    const env = readCiEnv({ GITHUB_ACTIONS: "false" });
    expect(env.provider).toBe("unknown");
  });

  it("parses pr from GITHUB_REF refs/pull/42/merge", () => {
    const env = readCiEnv({
      GITHUB_ACTIONS: "true",
      GITHUB_REF: "refs/pull/42/merge",
    });
    expect(env.pr).toBe(42);
  });

  it("parses pr from GITHUB_REF refs/pull/7/head", () => {
    const env = readCiEnv({
      GITHUB_ACTIONS: "true",
      GITHUB_REF: "refs/pull/7/head",
    });
    expect(env.pr).toBe(7);
  });

  it("does not set pr when GITHUB_REF is a branch ref", () => {
    const env = readCiEnv({
      GITHUB_ACTIONS: "true",
      GITHUB_REF: "refs/heads/main",
    });
    expect(env.pr).toBeUndefined();
  });

  it("parses pr from event JSON when REF doesn't match", () => {
    const eventJson = JSON.stringify({ pull_request: { number: 99 } });
    const env = readCiEnv(
      { GITHUB_ACTIONS: "true", GITHUB_EVENT_PATH: "/tmp/event.json" },
      { readEvent: (_path) => eventJson },
    );
    expect(env.pr).toBe(99);
  });

  it("GITHUB_REF takes precedence over event JSON", () => {
    const eventJson = JSON.stringify({ pull_request: { number: 99 } });
    const env = readCiEnv(
      {
        GITHUB_ACTIONS: "true",
        GITHUB_REF: "refs/pull/42/merge",
        GITHUB_EVENT_PATH: "/tmp/event.json",
      },
      { readEvent: (_path) => eventJson },
    );
    expect(env.pr).toBe(42);
  });

  it("tolerates missing token — returns token: undefined, no throw", () => {
    const env = readCiEnv({ GITHUB_ACTIONS: "true" });
    expect(env.token).toBeUndefined();
  });

  it("parses all standard GHA env vars", () => {
    const env = readCiEnv({
      GITHUB_ACTIONS: "true",
      GITHUB_REF: "refs/pull/5/merge",
      GITHUB_SHA: "abc1234",
      GITHUB_BASE_REF: "main",
      GITHUB_EVENT_PATH: "/tmp/event.json",
      GITHUB_OUTPUT: "/tmp/github-output",
      GITHUB_TOKEN: "ghs_fake",
      GITHUB_REPOSITORY: "owner/repo",
    });
    expect(env.headSha).toBe("abc1234");
    expect(env.baseRef).toBe("main");
    expect(env.eventPath).toBe("/tmp/event.json");
    expect(env.outputPath).toBe("/tmp/github-output");
    expect(env.token).toBe("ghs_fake");
    expect(env.repository).toBe("owner/repo");
  });

  it("does not set pr when event JSON has no pull_request key", () => {
    const eventJson = JSON.stringify({ issue: { number: 5 } });
    const env = readCiEnv(
      { GITHUB_ACTIONS: "true", GITHUB_EVENT_PATH: "/tmp/event.json" },
      { readEvent: (_path) => eventJson },
    );
    expect(env.pr).toBeUndefined();
  });

  it("does not throw when readEvent returns undefined", () => {
    const env = readCiEnv(
      { GITHUB_ACTIONS: "true", GITHUB_EVENT_PATH: "/tmp/missing.json" },
      { readEvent: (_path) => undefined },
    );
    expect(env.pr).toBeUndefined();
  });

  it("does not throw when readEvent throws", () => {
    const env = readCiEnv(
      { GITHUB_ACTIONS: "true", GITHUB_EVENT_PATH: "/tmp/event.json" },
      {
        readEvent: (_path) => {
          throw new Error("read failed");
        },
      },
    );
    expect(env.pr).toBeUndefined();
  });
});
