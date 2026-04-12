import type { Command } from "commander";
import { getServices } from "@/services.js";
import { output, resolveJsonFlag } from "@/shared/lib/output.js";
import { MaestroError } from "@/shared/errors.js";
import type { Principle, CreatePrincipleInput } from "../domain/principle-types.js";
import { validateCreatePrincipleInput } from "../domain/principle-validators.js";

export function registerPrincipleCommand(program: Command): void {
  const principleCmd = program
    .command("principle")
    .description("Behavioral principle management")
    .option("--json", "Output as JSON");

  principleCmd
    .command("list")
    .description("List active principles")
    .option("--profile <profile>", "Filter by milestone profile")
    .option("--json", "Output as JSON")
    .action(async (opts) => {
      const services = getServices();
      const isJson = resolveJsonFlag(opts, program);

      const principles = opts.profile
        ? await services.principleStore.listByProfile(opts.profile)
        : await services.principleStore.list();

      output(isJson, principles, formatPrincipleList);
    });

  principleCmd
    .command("add")
    .description("Add a new behavioral principle")
    .requiredOption("--id <id>", "Principle id (lowercase, dashes)")
    .requiredOption("--name <name>", "Human-readable name")
    .requiredOption("--rule <rule>", "Rule text injected into worker prompts")
    .requiredOption("--profiles <profiles...>", "Milestone profiles this applies to")
    .requiredOption("--mode <mode>", "advisory or gate")
    .option("--gate-field <field>", "Handoff content field name (required for gate mode)")
    .option("--gate-check <check>", "Gate check expression (required for gate mode)")
    .option("--source <source>", "Source attribution (karpathy | custom)", "custom")
    .option("--json", "Output as JSON")
    .action(async (opts) => {
      const services = getServices();
      const isJson = resolveJsonFlag(opts, program);

      const raw: CreatePrincipleInput = {
        id: opts.id,
        name: opts.name,
        rule: opts.rule,
        profiles: opts.profiles,
        mode: opts.mode,
        ...(opts.gateField ? { gateField: opts.gateField } : {}),
        ...(opts.gateCheck ? { gateCheck: opts.gateCheck } : {}),
        ...(opts.source ? { source: opts.source } : {}),
      };

      let validated: CreatePrincipleInput;
      try {
        validated = validateCreatePrincipleInput(raw);
      } catch (err) {
        if (err instanceof Error) {
          throw new MaestroError(`Invalid principle: ${err.message}`, [
            "Gate-mode principles require --gate-field and --gate-check",
            "Profiles must be valid milestone profiles",
          ]);
        }
        throw err;
      }

      const principle = await services.principleStore.create(validated);
      output(isJson, principle, formatPrincipleCreated);
    });

  principleCmd
    .command("remove <id>")
    .description("Remove a principle by id")
    .option("--json", "Output as JSON")
    .action(async (id: string, opts) => {
      const services = getServices();
      const isJson = resolveJsonFlag(opts, program);

      const removed = await services.principleStore.remove(id);
      if (!removed) {
        throw new MaestroError(`Principle '${id}' not found`, [
          "List principles: maestro principle list",
        ]);
      }

      output(isJson, { id, removed: true }, formatPrincipleRemoved);
    });
}

function formatPrincipleList(principles: readonly Principle[]): string[] {
  if (principles.length === 0) {
    return ["No principles found"];
  }

  const lines: string[] = [`${principles.length} principle(s)`, ""];
  for (const p of principles) {
    const badge = p.mode === "gate" ? "[GATE]" : "[adv] ";
    const profiles = p.profiles.join(", ");
    lines.push(`${badge} ${p.id.padEnd(24)} ${p.name.padEnd(24)} (${profiles})`);
    if (p.mode === "gate" && p.gateField) {
      lines.push(`       gate: ${p.gateField} -> ${p.gateCheck}`);
    }
  }
  return lines;
}

function formatPrincipleCreated(principle: Principle): string[] {
  return [
    `[ok] Principle created: ${principle.id}`,
    `  Name: ${principle.name}`,
    `  Mode: ${principle.mode}`,
    `  Profiles: ${principle.profiles.join(", ")}`,
    ...(principle.gateField ? [`  Gate: ${principle.gateField} -> ${principle.gateCheck}`] : []),
  ];
}

function formatPrincipleRemoved(result: { id: string }): string[] {
  return [`[ok] Principle removed: ${result.id}`];
}
