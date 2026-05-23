import type { Command } from "commander";
import { type Services } from "@/services.js";
import { output, resolveJsonFlag, stringifyForOutput } from "@/shared/lib/output.js";
import { MaestroError } from "@/shared/errors.js";
import type {
  Principle,
  CreatePrincipleInput,
  PrincipleEffectiveness,
  MilestoneProfile,
} from "../domain/types.js";
import { MilestoneProfileSchema } from "../domain/validators.js";
import { validateCreatePrincipleInput } from "../domain/validators.js";
import {
  buildPrincipleEffectiveness,
  PRINCIPLE_SMALL_SAMPLE_THRESHOLD,
} from "../usecases/principle-effectiveness.usecase.js";

interface PrincipleCommandDeps {
  readonly getServices: () => Pick<Services, "principleStore" | "principlesStore">;
}

export function registerPrincipleCommand(
  program: Command,
  deps: PrincipleCommandDeps,
): void {
  const principleCmd = program
    .command("principle")
    .description("Behavioral principle management")
    .option("--json", "Output as JSON");

  principleCmd
    .command("list")
    .description("List active principles (behavioral + lint packs)")
    .option("--profile <profile>", "Filter by milestone profile")
    .option("--json", "Output as JSON")
    .action(async (opts): Promise<void> => {
      const services = deps.getServices();
      const isJson = resolveJsonFlag(opts, program);
      const profile = typeof opts.profile === "string" ? parsePrincipleProfile(opts.profile) : undefined;

      const principles = profile
        ? await services.principleStore.listByProfile(profile)
        : await services.principleStore.list();

      // Profile filter only applies to behavioral principles; skip the lint
      // pack when the caller narrowed the query.
      const lintPack = profile
        ? []
        : await services.principlesStore.list();

      if (isJson) {
        output(true, { behavioral: principles, lint: lintPack }, () => []);
        return;
      }

      output(false, principles, (p) => formatPrincipleList(p, lintPack));
    });

  principleCmd
    .command("add")
    .description("Add a new behavioral principle")
    .requiredOption("--id <id>", "Principle id (lowercase, dashes)")
    .requiredOption("--name <name>", "Human-readable name")
    .requiredOption("--rule <rule>", "Rule text injected into agent prompts")
    .requiredOption("--profiles <profiles...>", "Milestone profiles this applies to")
    .requiredOption("--mode <mode>", "advisory or gate")
    .option("--gate-field <field>", "Handoff content field name (required for gate mode)")
    .option("--gate-check <check>", "Gate check expression (required for gate mode)")
    .option("--source <source>", "Source attribution (karpathy | custom)", "custom")
    .option("--json", "Output as JSON")
    .action(async (opts): Promise<void> => {
      const services = deps.getServices();
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
    .command("effectiveness")
    .alias("stats")
    .description("Per-principle helpful/unhelpful scoreboard (worst first)")
    .option("--json", "Output as JSON")
    .option("--all", "Include principles that fall below the small-sample threshold")
    .action(async (opts): Promise<void> => {
      const services = deps.getServices();
      const isJson = resolveJsonFlag(opts, program);

      const [principles, outcomes] = await Promise.all([
        services.principleStore.list(),
        services.principleStore.listOutcomes(),
      ]);
      const rollup = buildPrincipleEffectiveness(principles, outcomes);
      const principleById = new Map(principles.map((p) => [p.id, p]));
      const showAll = opts.all === true;
      const rows = [...rollup.values()]
        .filter((row) => showAll || row.helpful + row.unhelpful >= PRINCIPLE_SMALL_SAMPLE_THRESHOLD)
        .sort(sortWorstFirst);

      if (isJson) {
        const payload = rows.map((row) => ({
          ...row,
          name: principleById.get(row.principleId)?.name,
          lowSample: row.helpful + row.unhelpful < PRINCIPLE_SMALL_SAMPLE_THRESHOLD,
        }));
        console.log(stringifyForOutput(payload));
        return;
      }
      output(false, rows, (r) => formatEffectivenessTable(r, principleById));
    });

  principleCmd
    .command("remove <id>")
    .description("Remove a principle by id")
    .option("--json", "Output as JSON")
    .action(async (id: string, opts): Promise<void> => {
      const services = deps.getServices();
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

function formatPrincipleList(
  principles: readonly Principle[],
  lintPack: readonly { readonly slug: string }[] = [],
): string[] {
  const lines: string[] = [];

  lines.push(`Behavioral principles (.maestro/principles.jsonl): ${principles.length}`);
  if (principles.length === 0) {
    lines.push("  (none — register one with `maestro principle add`)");
  } else {
    for (const p of principles) {
      const badge = p.mode === "gate" ? "[GATE]" : "[adv] ";
      const profiles = p.profiles.join(", ");
      lines.push(`  ${badge} ${p.id.padEnd(24)} ${p.name.padEnd(24)} (${profiles})`);
      if (p.mode === "gate" && p.gateField) {
        lines.push(`         gate: ${p.gateField} -> ${p.gateCheck}`);
      }
    }
  }

  lines.push("");
  lines.push(`Lint principles (docs/principles/*.md): ${lintPack.length}`);
  if (lintPack.length === 0) {
    lines.push("  (none — `maestro init` seeds the default pack)");
  } else {
    for (const p of lintPack) {
      lines.push(`  ${p.slug}`);
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

function sortWorstFirst(a: PrincipleEffectiveness, b: PrincipleEffectiveness): number {
  const ae = a.effectiveness ?? Number.POSITIVE_INFINITY;
  const be = b.effectiveness ?? Number.POSITIVE_INFINITY;
  if (ae !== be) return ae - be; // worst first
  // Secondary sort: more decided outcomes first (higher-signal rows surface).
  const aDecided = a.helpful + a.unhelpful;
  const bDecided = b.helpful + b.unhelpful;
  return bDecided - aDecided;
}

function formatEffectivenessTable(
  rows: readonly PrincipleEffectiveness[],
  principleById: ReadonlyMap<string, Principle>,
): string[] {
  if (rows.length === 0) {
    return [
      "No principles with recorded outcomes yet.",
      "Run `maestro principle effectiveness --all` to include low-sample entries.",
    ];
  }
  const lines: string[] = [
    `${rows.length} principle(s)  (sorted by effectiveness, worst first)`,
    "",
    "  eff%    helpful / total   principle",
  ];
  for (const row of rows) {
    const effStr = row.effectiveness === undefined
      ? "   -  "
      : `${(row.effectiveness * 100).toFixed(0).padStart(4)}%`;
    const decided = row.helpful + row.unhelpful;
    const lowSample = decided < PRINCIPLE_SMALL_SAMPLE_THRESHOLD ? " [low]" : "";
    const name = principleById.get(row.principleId)?.name ?? row.principleId;
    const pendingStr = row.pending > 0 ? ` +${row.pending} pending` : "";
    lines.push(`  ${effStr}   ${String(row.helpful).padStart(3)} / ${String(decided).padStart(3)}      ${row.principleId.padEnd(30)} ${name}${pendingStr}${lowSample}`);
  }
  return lines;
}

function parsePrincipleProfile(raw: string): MilestoneProfile {
  const result = MilestoneProfileSchema.safeParse(raw);
  if (result.success) {
    return result.data;
  }

  throw new MaestroError(`Invalid principle profile: ${raw}`, [
    `Allowed values: ${MilestoneProfileSchema.options.join(" | ")}`,
  ]);
}
