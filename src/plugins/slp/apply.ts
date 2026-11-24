import { readFileSync } from "node:fs";
import { join } from "node:path";
import {
  CliError,
  requiredPosition,
  stringOption,
  type CliGuard,
  type CliInvocation,
  type CliResult,
} from "../../kernel/cli.ts";
import type { PluginContext } from "../../kernel/loader.ts";
import { Store, tableExists } from "../../kernel/store.ts";
import type { DecisionSection } from "../decision.ts";
import type { InstallEvent } from "../install.ts";
import type { RecipeService } from "../recipe.ts";
import { installMachineFiles, removeMachineFiles, seatTokenCheck, syncSeatFiles } from "./plugin.ts";
import { writeSeatToken } from "./seats.ts";
import { HerdrSlpRuntime } from "./runtime.ts";
import { registerSessionCommand } from "../session-required.ts";
import { currentPaneId } from "../session-telemetry.ts";
import { runSlpRestore, runSlpRuntime, runtimeConfigFromEnvironment, slpRuntimeStatus } from "./attention.ts";
import { activeLocalTeam } from "./context.ts";
import { decide, slpDecisionResult } from "./decide.ts";
import type { PackModels } from "./pack.ts";
import { startTeam } from "./start.ts";
import { maybeHandleSlpStatus } from "./status.ts";
import { stopTeam } from "./stop.ts";
import { canonicalCheckoutRoot } from "./store.ts";
import {
  acceptWork,
  maybeHandleSlpWorkAdd,
  maybeHandleSlpWorkNote,
  pushDesignOpened,
  returnWork,
  takeWork,
} from "./work-ops.ts";

const nonTeamVerbs = new Map<string, string>([
  ["attention", "status"],
  ["brief", "status"],
  ["decision", "decide"],
  ["decision list", "status"],
  ["decision show", "status <decision-id>"],
  ["work done", "work return then work accept"],
  ["work list", "status"],
  ["work reclaim", "work take"],
  ["work release", "work return"],
  ["work show", "status <work-id>"],
  ["work start", "work take"],
]);

function paneGuard(store: Store, cwd = process.cwd()): CliGuard {
  return {
    beforeInvoke(command) {
      const replacement = [...nonTeamVerbs.entries()]
        .filter(([verb]) => command === verb || command.startsWith(`${verb} `))
        .sort(([left], [right]) => right.length - left.length)[0]?.[1];
      if (!replacement) return;
      if (!tableExists(store, "slp_local_teams") || !tableExists(store, "slp_local_roles")) return;
      const paneId = currentPaneId();
      if (!paneId) return;
      const activeRole = store.database
        .query<{ present: number }, [string, string]>(
          `SELECT 1 AS present
           FROM slp_local_teams AS team
           JOIN slp_local_roles AS role
             ON role.team_id = team.team_id AND role.generation = team.generation
           WHERE team.project_path = ? AND team.state = 'RUNNING' AND role.pane_id = ?
           LIMIT 1`,
        )
        .get(canonicalCheckoutRoot(cwd), paneId);
      if (activeRole) {
        throw new CliError(
          "SLP_V2_CUTOVER",
          `${command} was retired by SLP v2; use maestro ${replacement}`,
          { command, replacement },
        );
      }
    },
  };
}

const workAddFlags = {
  "--to": {
    description: "Assign SLP work to a Peer; without it the Lead opens an item for itself.",
    value: true,
  },
  "--model": {
    description: "Lead only: launch the --to Peer with this model on its launch line (default: the generation's --peer-model, else the peer seat file's model); passed through unvalidated (room d128), so name a model the peer seat's harness accepts, or its pane dies on the harness's own error.",
    value: true,
  },
  "--fresh": {
    description: "Lead only: reset a reused --to Peer pane to a fresh harness context (Claude /clear, Codex /new), proven by a new harness session, and re-check READY before the OPEN push; a no-op on a pane being opened, refused while the Peer holds ACTIVE work or when the reset does not take.",
  },
};

const workNoteFlags = {
  "--blocked": {
    description: "Flag the note as blocked and push it one SLP seat up.",
  },
  "--owner": {
    description: "Record an owner walk-in typed into this seat's pane (room d123): provenance only, pushed one SLP seat up, never a blocked flag.",
  },
  "--rework": {
    description: "Grant the current SLP return revision one reviewer-authorized retake.",
  },
};

function registerCoreSeams(context: PluginContext): void {
  context.effect(() => context.cli.guard(paneGuard(context.store)));
  for (const [flag, definition] of Object.entries(workAddFlags)) {
    context.effect(() => context.cli.registerFlag("work add", flag, definition));
  }
  for (const [flag, definition] of Object.entries(workNoteFlags)) {
    context.effect(() => context.cli.registerFlag("work note", flag, definition));
  }
  context.effect(() =>
    context.cli.registerFlag("install", "--seat-token", {
      description: "Read one claude setup-token token from stdin and store it for every Claude seat (d846).",
    })
  );
  context.effect(() =>
    context.events.on<CliInvocation, CliResult>("work.add.invoke", async (invocation, next) => {
      if (invocation.options.start === true && invocation.options.to !== undefined) {
        throw new CliError("INVALID_OPTION", "--start is not available for SLP work add --to <peer>; the Peer takes its card with work take");
      }
      const slp = await maybeHandleSlpWorkAdd(context, invocation);
      if (slp) return slp;
      // Hub finding F1 (g7, w734): an option only a Lead's Peer add can honour refuses before any store write.
      if (invocation.options.model !== undefined) {
        throw new CliError("INVALID_OPTION", "--model is available only for SLP work add --to <peer>");
      }
      if (invocation.options.fresh !== undefined) {
        throw new CliError("INVALID_OPTION", "--fresh is available only for SLP work add --to <peer>");
      }
      return next();
    })
  );
  context.effect(() =>
    context.events.on<CliInvocation, CliResult>("work.note.invoke", async (invocation, next) => {
      const slp = await maybeHandleSlpWorkNote(context, invocation);
      if (slp) return slp;
      if (invocation.options.rework === true) {
        throw new CliError("INVALID_OPTION", "--rework is available only for SLP RETURNED work");
      }
      if (invocation.options.blocked === true) {
        throw new CliError("INVALID_OPTION", "--blocked is available only for SLP work");
      }
      if (invocation.options.owner === true) {
        throw new CliError("INVALID_OPTION", "--owner is available only for SLP work held or reviewed by a seat");
      }
      return next();
    })
  );
  context.effect(() =>
    context.cli.register(
      "status",
      async (invocation): Promise<CliResult> => {
        const status = await maybeHandleSlpStatus(context, invocation);
        if (status) return status;
        throw new CliError(
          "NO_ACTIVE_TEAM",
          invocation.positionals[0]
            ? "status <work-id> reads SLP team work from a role pane; outside a running team use: maestro work show <id>"
            : "status reads a running SLP team from a role pane or the Hub room; for tracked work run: maestro work list",
        );
      },
      {
        description: "Show the SLP team readout: the Hub's teams, a seat's work, one work item or one ruling.",
        flags: { "--all": { description: "List DONE work instead of counting it." } },
        mutates: false,
        positionals: [{ name: "work-id", required: false }],
      },
    )
  );
  context.effect(() =>
    context.events.on<{ id: string }, DecisionSection | null>("decision.show", ({ id }) => {
      const slp = slpDecisionResult(context, id);
      return slp ? { data: { slpDecisions: slp.decisions }, label: "SLP rulings", text: slp.text } : null;
    })
  );
  context.effect(() =>
    context.events.on<{ home: string; invocation: CliInvocation }>("install.begin", async ({ home, invocation }) => {
      // The re-executed install inherits the flag, but the token is already written and stdin is spent.
      if (invocation.options["seat-token"] !== true || process.env.MAESTRO_INSTALL_REEXEC === "1") return;
      await writeSeatToken(home, await Bun.stdin.text());
    })
  );
  context.effect(() => context.events.on<InstallEvent, string>("install", (event) => installMachineFiles(event)));
  context.effect(() =>
    context.events.on<{ home: string; repo: string; retired: string[] }, string>("update", async ({ home, repo, retired }) => {
      await syncSeatFiles(home, repo, retired);
      return "";
    })
  );
  context.effect(() => context.events.on<{ home: string }, string[]>("uninstall", ({ home }) => removeMachineFiles(home)));
  context.effect(() => context.events.on<{ home: string }, string>("doctor", ({ home }) => seatTokenCheck(home)));
}

export function applySlp(context: PluginContext): void {
  const runtime = new HerdrSlpRuntime();
  registerCoreSeams(context);
  const recipes = context.recipe as RecipeService | undefined;
  if (recipes) {
    context.effect(() =>
      recipes.register({
        name: "slp",
        description: "Know which SLP role a session holds (Supervisor, Lead, Peer) and what it owns.",
        body: () => readFileSync(join(import.meta.dir, "..", "recipes", "slp.md"), "utf8"),
      })
    );
  }
  context.effect(() =>
    registerSessionCommand(
      context,
      "team start",
      (invocation): Promise<CliResult> => {
        // room d128: a seat's model is a launch-time choice. Each flag is
        // passed through unvalidated (the harness judges the name); only an
        // empty value is refused here, since it would launch `--model ""`.
        const models: PackModels = {};
        for (const [flag, seat] of [
          ["lead-model", "lead"],
          ["peer-model", "peer"],
        ] as const) {
          const value = stringOption(invocation, flag);
          if (value === undefined) continue;
          if (value.trim() === "") {
            throw new CliError("INVALID_OPTION", `--${flag} needs a model name; the harness validates it, maestro passes it through`);
          }
          models[seat] = value;
        }
        return startTeam(
          context,
          runtime,
          requiredPosition(invocation, 0, "project"),
          requiredPosition(invocation, 1, "objective"),
          models,
        );
      },
      {
        description: "Start or restore one SLP team generation: one Lead pane plus the runtime pane; the Hub Supervisor reviews the Lead's work (room d117).",
        flags: {
          "--lead-model": {
            description: "Launch the Lead with this model (`claude --model` / `codex --model` on its launch line); the lead seat file's model is the default; recorded on the team row (room d128).",
            value: true,
          },
          "--peer-model": {
            description: "Launch every Peer this generation opens with this model unless work add --model names another; recorded on the team row (room d128); name a model the peer seat's harness accepts, or its pane dies on the harness's own error.",
            value: true,
          },
        },
        positionals: [
          { name: "project", required: true },
          { name: "objective", required: true },
        ],
        rootDescription: "Run the simplified supervised-team lifecycle.",
      },
    ),
  );
  context.effect(() =>
    registerSessionCommand(
      context,
      "team stop",
      (invocation): Promise<CliResult> => stopTeam(context, runtime, invocation),
      {
        description: "Stop one complete SLP team, or abandon unfinished work from Hub.",
        flags: {
          "--emergency": { description: "Use Hub owner authority and abandon unfinished work." },
          "--reason": {
            description:
              "The closing report shown on the Hub ledger (w722); with --emergency, why unfinished work is abandoned.",
            value: true,
          },
        },
        positionals: [{ name: "team-id", required: true }],
        rootDescription: "Run the simplified supervised-team lifecycle.",
      },
    ),
  );
  context.effect(() =>
    registerSessionCommand(
      context,
      "work take",
      (invocation): CliResult =>
        takeWork(context, requiredPosition(invocation, 0, "work id")),
      {
        description: "Take assigned OPEN work or reviewer-granted RETURNED SLP work.",
        positionals: [{ name: "work-id", required: true }],
        rootDescription: "Move supervised work through its four-state lifecycle.",
      },
    ),
  );
  context.effect(() =>
    registerSessionCommand(
      context,
      "work return",
      (invocation): Promise<CliResult> =>
        returnWork(
          context,
          requiredPosition(invocation, 0, "work id"),
          requiredPosition(invocation, 1, "return body"),
        ),
      {
        description: "Return ACTIVE SLP work with its bounded result body.",
        positionals: [
          { name: "work-id", required: true },
          { name: "body", required: true },
        ],
        rootDescription: "Move supervised work through its four-state lifecycle.",
      },
    ),
  );
  context.effect(() =>
    registerSessionCommand(
      context,
      "work accept",
      (invocation): Promise<CliResult> =>
        acceptWork(
          context,
          requiredPosition(invocation, 0, "work id"),
          stringOption(invocation, "outcome") ?? "accepted",
        ),
      {
        description: "Accept RETURNED SLP work at the reviewer boundary.",
        flags: {
          "--outcome": {
            description: "Record accepted or cancelled; cancellation may close OPEN work.",
            value: true,
          },
        },
        positionals: [{ name: "work-id", required: true }],
        rootDescription: "Move supervised work through its four-state lifecycle.",
      },
    ),
  );
  // Hub d96/d97: the plugin entrypoints and the runtime's own readout. None
  // is an SLP operation; Herdr launches the first two from the manifest.
  context.effect(() =>
    context.cli.register(
      "slp runtime",
      async (): Promise<CliResult> => {
        const exitCode = await runSlpRuntime(runtimeConfigFromEnvironment());
        if (exitCode !== 0) throw new CliError("RUNTIME_EXIT", `maestro slp runtime exited with ${exitCode}`);
        return { data: { exitCode }, text: "runtime stopped" };
      },
      {
        description: "Run the team runtime pane for MAESTRO_SLP_TEAM and MAESTRO_SLP_GENERATION (opened by team start through the maestro Herdr plugin).",
        rootDescription: "Maestro's Herdr plugin entrypoints and the runtime readout.",
      },
    ),
  );
  context.effect(() =>
    context.cli.register(
      "slp restore",
      (): Promise<CliResult> => runSlpRestore(),
      { description: "Herdr startup hook: reopen the runtime pane of every RUNNING generation whose role panes survived." },
    ),
  );
  context.effect(() =>
    context.cli.register(
      "slp status",
      (): Promise<CliResult> => {
        const team = activeLocalTeam(context);
        if (!team) throw new CliError("NO_ACTIVE_TEAM", "no running SLP team is bound to this workspace");
        return slpRuntimeStatus(team.project_path, team.team_id, team.generation);
      },
      { description: "Read the runtime's pending wakes for the running generation.", mutates: false },
    ),
  );
  // w740 (d898): bundle open says a design has begun; the SLP plugin owns
  // the push, so the bundle plugin only announces the event.
  context.effect(() =>
    context.events.on<{ bundleId: string; workIds: string[] }>(
      "bundle.open",
      async (opened, next) => {
        await pushDesignOpened(context, opened.bundleId, opened.workIds);
        return next();
      },
    ),
  );
  context.effect(() =>
    registerSessionCommand(context, "decide", (invocation): Promise<CliResult> => decide(context, invocation), {
      description: "Record one immutable settled decision.",
      flags: {
        "--replaces": { description: "Link the decision it replaces.", value: true },
        "--scope": {
          description: "Select owner or cross-team scope when acting as Hub Supervisor.",
          value: true,
        },
        "--why": { description: "Record why this choice is settled.", value: true },
        "--work": { description: "Link the decision to SLP work.", value: true },
      },
      positionals: [{ name: "choice", required: true }],
      rootDescription: "Record a settled SLP decision in one operation.",
    }),
  );
}
