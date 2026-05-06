import { mkdtemp, rm } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import type { Command } from "commander";
import { MaestroError } from "@/shared/errors.js";
import { output, resolveJsonFlag } from "@/shared/lib/output.js";
import { writeText, readText } from "@/shared/lib/fs.js";
import { getServices, type Services } from "@/services.js";
import { createSpec } from "../usecases/create-spec.usecase.js";
import { updateSpec } from "../usecases/update-spec.usecase.js";
import type { Spec, AcceptanceCriterion } from "../domain/types.js";
import type { SpecStorePort } from "../ports/storage.js";

interface SpecCommandDeps {
  readonly getServices: () => Pick<Services, "specStore" | "missions">;
}

export function registerSpecCommand(
  program: Command,
  deps: SpecCommandDeps = { getServices },
): void {
  const specCmd = program
    .command("spec")
    .description("Manage Mission specs (acceptance criteria, non-goals)")
    .option("--json", "Output as JSON");

  registerShowCommand(specCmd, program, deps);
  registerEditCommand(specCmd, program, deps);
}

function registerShowCommand(parent: Command, root: Command, deps: SpecCommandDeps): void {
  parent
    .command("show")
    .description("Show the Spec for a Mission")
    .requiredOption("--mission <id>", "Mission id")
    .option("--json", "Output as JSON")
    .action(async (opts: { mission: string; json?: boolean }) => {
      const services = deps.getServices();
      const isJson = resolveJsonFlag(opts as Record<string, unknown>, root)
        || (parent.opts().json as boolean | undefined) === true;

      const spec = await services.specStore.read(opts.mission);
      if (!spec) {
        throw new MaestroError(`No Spec found for mission: ${opts.mission}`, [
          `Create one with: maestro spec edit --mission ${opts.mission}`,
        ]);
      }

      output(isJson, spec, formatSpec);
    });
}

function registerEditCommand(parent: Command, root: Command, deps: SpecCommandDeps): void {
  parent
    .command("edit")
    .description("Create or edit the Spec for a Mission (opens $EDITOR)")
    .requiredOption("--mission <id>", "Mission id")
    .option("--json", "Output as JSON")
    .action(async (opts: { mission: string; json?: boolean }) => {
      const services = deps.getServices();
      const isJson = resolveJsonFlag(opts as Record<string, unknown>, root)
        || (parent.opts().json as boolean | undefined) === true;

      const editor = process.env["EDITOR"] ?? process.env["VISUAL"];
      if (!editor) {
        throw new MaestroError("$EDITOR is not set", [
          "Set EDITOR in your shell profile (e.g. export EDITOR=vim)",
          "Or set VISUAL instead",
        ]);
      }

      const mission = await services.missions.get(opts.mission);
      if (!mission) {
        throw new MaestroError(`Mission not found: ${opts.mission}`, [
          "List missions with: maestro mission list",
          "Create a mission first with: maestro mission new",
        ]);
      }

      const existing = await services.specStore.read(opts.mission);
      const initial = existing
        ? JSON.stringify(toEditableSpec(existing, opts.mission), null, 2)
        : JSON.stringify(emptyEditableSpec(opts.mission), null, 2);

      const edited = await openInEditor(initial, editor, opts.mission);

      let parsed: unknown;
      try {
        parsed = JSON.parse(edited);
      } catch (err) {
        throw new MaestroError("Failed to parse edited spec as JSON", [
          `Parse error: ${(err as Error).message}`,
          "Ensure your edits are valid JSON",
        ]);
      }

      const input = parseEditableSpec(parsed, opts.mission);

      const spec = existing
        ? await updateSpec(services.specStore, opts.mission, input)
        : await createSpec(services.specStore, input);

      output(isJson, spec, (s) => formatSpec(s));
    });
}

interface EditableSpec {
  readonly mission_id: string;
  readonly acceptance_criteria: readonly { id?: string; text: string }[];
  readonly non_goals: readonly { text: string }[];
}

function toEditableSpec(spec: Spec, missionId: string): EditableSpec {
  return {
    mission_id: missionId,
    acceptance_criteria: spec.acceptance_criteria.map((c) => ({ id: c.id, text: c.text })),
    non_goals: spec.non_goals.map((g) => ({ text: g.text })),
  };
}

function emptyEditableSpec(missionId: string): EditableSpec {
  return {
    mission_id: missionId,
    acceptance_criteria: [
      { text: "Describe what must be true for this mission to be complete" },
    ],
    non_goals: [
      { text: "Describe what is explicitly out of scope" },
    ],
  };
}

function parseEditableSpec(
  value: unknown,
  missionId: string,
): { mission_id: string; acceptance_criteria: { id?: string; text: string }[]; non_goals: { text: string }[] } {
  if (typeof value !== "object" || value === null) {
    throw new MaestroError("Spec must be a JSON object");
  }
  const v = value as Record<string, unknown>;
  if (typeof v["mission_id"] !== "string") {
    throw new MaestroError("Spec must have a string mission_id field");
  }
  if (v["mission_id"] !== missionId) {
    throw new MaestroError(`mission_id in spec (${v["mission_id"]}) does not match --mission ${missionId}`);
  }
  if (!Array.isArray(v["acceptance_criteria"])) {
    throw new MaestroError("Spec must have an acceptance_criteria array");
  }
  const criteria = v["acceptance_criteria"].map((c: unknown, i: number) => {
    if (typeof c !== "object" || c === null) {
      throw new MaestroError(`acceptance_criteria[${i}] must be an object`);
    }
    const item = c as Record<string, unknown>;
    if (typeof item["text"] !== "string") {
      throw new MaestroError(`acceptance_criteria[${i}].text must be a string`);
    }
    return {
      ...(typeof item["id"] === "string" ? { id: item["id"] } : {}),
      text: item["text"],
    };
  });
  const nonGoals = Array.isArray(v["non_goals"])
    ? v["non_goals"].map((g: unknown, i: number) => {
        if (typeof g !== "object" || g === null) {
          throw new MaestroError(`non_goals[${i}] must be an object`);
        }
        const item = g as Record<string, unknown>;
        if (typeof item["text"] !== "string") {
          throw new MaestroError(`non_goals[${i}].text must be a string`);
        }
        return { text: item["text"] };
      })
    : [];
  return { mission_id: missionId, acceptance_criteria: criteria, non_goals: nonGoals };
}

async function openInEditor(content: string, editorCommand: string, missionId: string): Promise<string> {
  const draftDir = await mkdtemp(join(tmpdir(), "maestro-spec-edit-"));
  const draftPath = join(draftDir, `spec-${missionId}.json`);
  await writeText(draftPath, content);

  try {
    const editorArgv = editorCommand.trim().split(/\s+/);
    const result = Bun.spawnSync([...editorArgv, draftPath], {
      stdio: ["inherit", "inherit", "inherit"],
    });
    if ((result.exitCode ?? 1) !== 0) {
      throw new MaestroError(`Editor command failed: ${editorCommand}`, [
        "Retry with a working editor command",
        "Or set $EDITOR to a valid editor",
      ]);
    }
    return (await readText(draftPath)) ?? "";
  } finally {
    await rm(draftDir, { recursive: true, force: true });
  }
}

function formatSpec(spec: Spec): string[] {
  const lines: string[] = [
    `Spec for mission: ${spec.mission_id}`,
    `  Schema version: ${spec.schema_version}`,
    `  Created: ${spec.created_at}`,
    `  Updated: ${spec.updated_at}`,
    "",
    `Acceptance Criteria (${spec.acceptance_criteria.length}):`,
  ];

  if (spec.acceptance_criteria.length === 0) {
    lines.push("  (none)");
  } else {
    for (const c of spec.acceptance_criteria) {
      lines.push(`  [${c.id}] ${c.text}`);
    }
  }

  lines.push("", `Non-Goals (${spec.non_goals.length}):`);
  if (spec.non_goals.length === 0) {
    lines.push("  (none)");
  } else {
    for (const g of spec.non_goals) {
      lines.push(`  - ${g.text}`);
    }
  }

  return lines;
}

// Re-export for testing
export type { SpecCommandDeps };
export { formatSpec };
