import { describe, expect, it } from "bun:test";
import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { parseYaml } from "@/shared/lib/yaml.js";

interface CompositeAction {
  readonly runs?: {
    readonly steps?: Array<{
      readonly shell?: string;
      readonly run?: string;
    }>;
  };
}

const ACTION_PATH = join(
  import.meta.dir,
  "..",
  "..",
  "..",
  ".github",
  "actions",
  "detect-maestro-changes",
  "action.yml",
);

describe("detect-maestro-changes composite action", () => {
  it("forces full CI when shared composite actions change", async () => {
    const action = parseYaml<CompositeAction>(await readFile(ACTION_PATH, "utf8"));
    const classifyStep = action.runs?.steps?.find((step) =>
      step.run?.includes(".github/workflows/**|.github/actions/**"),
    );

    expect(classifyStep?.shell).toBe("bash");
    expect(classifyStep?.run).toContain(".github/workflows/**|.github/actions/**)");
    expect(classifyStep?.run).toContain("github_changed=true");
    expect(classifyStep?.run).toContain("full_ci=true");
  });
});
