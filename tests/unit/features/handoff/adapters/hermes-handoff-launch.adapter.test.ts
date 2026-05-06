import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { chmod, mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { HermesHandoffLaunchAdapter } from "@/features/handoff/adapters/hermes-handoff-launch.adapter.js";

describe("HermesHandoffLaunchAdapter", () => {
  let tmpDir: string;
  let originalPath: string | undefined;

  beforeEach(async () => {
    tmpDir = await mkdtemp(join(tmpdir(), "maestro-hermes-launch-"));
    originalPath = process.env.PATH;
  });

  afterEach(async () => {
    if (originalPath === undefined) {
      delete process.env.PATH;
    } else {
      process.env.PATH = originalPath;
    }
    await rm(tmpDir, { recursive: true, force: true });
  });

  it("constructs the Hermes chat command, optional model flag, and launch env", async () => {
    const binDir = join(tmpDir, "bin");
    const argsPath = join(tmpDir, "args.txt");
    const envPath = join(tmpDir, "env.txt");
    await mkdir(binDir, { recursive: true });
    await writeFile(
      join(binDir, "hermes"),
      [
        "#!/bin/sh",
        "printf '%s\\n' \"$@\" > \"$FAKE_HERMES_ARGS\"",
        "printf '%s\\n%s\\n' \"$MAESTRO_AGENT\" \"$MAESTRO_SESSION_ID\" > \"$FAKE_HERMES_ENV\"",
      ].join("\n"),
    );
    await chmod(join(binDir, "hermes"), 0o755);
    process.env.PATH = `${binDir}:${originalPath ?? ""}`;

    const adapter = new HermesHandoffLaunchAdapter();
    const result = await adapter.launch({
      prompt: "Do the work",
      targetDir: tmpDir,
      model: "nous/hermes",
      modelProvided: true,
      name: "Handoff",
      wait: true,
      logPath: join(tmpDir, "launch.log"),
      env: {
        FAKE_HERMES_ARGS: argsPath,
        FAKE_HERMES_ENV: envPath,
        MAESTRO_AGENT: "hermes",
        MAESTRO_SESSION_ID: "handoff-123",
      },
    });

    expect(result.exitCode).toBe(0);
    expect(result.command).toEqual([
      "hermes",
      "chat",
      "--quiet",
      "--yolo",
      "--toolsets",
      "terminal,skills",
      "--source",
      "maestro",
      "--model",
      "nous/hermes",
      "-q",
      "Do the work",
    ]);
    expect((await readFile(argsPath, "utf8")).trim().split("\n")).toEqual(result.command.slice(1));
    expect((await readFile(envPath, "utf8")).trim().split("\n")).toEqual(["hermes", "handoff-123"]);
  });

  it("omits --model when the handoff did not explicitly provide one", async () => {
    const binDir = join(tmpDir, "bin-no-model");
    await mkdir(binDir, { recursive: true });
    await writeFile(join(binDir, "hermes"), "#!/bin/sh\nexit 0\n");
    await chmod(join(binDir, "hermes"), 0o755);
    process.env.PATH = `${binDir}:${originalPath ?? ""}`;

    const result = await new HermesHandoffLaunchAdapter().launch({
      prompt: "Default model",
      targetDir: tmpDir,
      model: "default",
      modelProvided: false,
      name: "Handoff",
      wait: true,
      logPath: join(tmpDir, "launch-no-model.log"),
    });

    expect(result.exitCode).toBe(0);
    expect(result.command).not.toContain("--model");
  });
});
