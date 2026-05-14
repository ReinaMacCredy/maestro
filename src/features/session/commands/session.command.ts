import type { Command } from "commander";
import { type Services } from "@/services.js";
import { detectSession } from "../usecases/detect-session.usecase.js";
import { output } from "@/shared/lib/output.js";
import { MaestroError } from "@/shared/errors.js";
import type { AgentSession } from "../domain/types.js";
import { registerSessionStartCommand } from "./session-start.command.js";
import { registerSessionExitCommand } from "./session-exit.command.js";

interface SessionCommandDeps {
  readonly getServices: () => Services;
}

export function registerSessionCommand(
  program: Command,
  deps: SessionCommandDeps,
): void {
  // The parent `session` command keeps the legacy `whoami` behavior when
  // invoked bare (`maestro session`, `maestro session --json`,
  // `maestro session --quiet`) so existing CI/automation scripts do not
  // need an immediate update. The Phase 1 pivot adds `start`/`exit`
  // subcommands; `whoami` is the canonical name going forward.
  const sessionCmd = program
    .command("session")
    .description("Session lifecycle and detection verbs (bare invocation = whoami)")
    .option("--json", "Output as JSON")
    .option("-q, --quiet", "Output just the session ID")
    .action(async (opts): Promise<void> => {
      await runWhoami(deps, opts, program);
    })
    .addHelpText(
      "after",
      `
Subcommands:
  whoami                 detect the current agent session
  start <taskId>         open a task with an orient digest + baseline verify
  exit <taskId>          close a task with regression check + progress digest

Examples:
  maestro session whoami --json     # full session info as JSON
  maestro session whoami -q         # just the session ID (for scripting)
  maestro session start tsk-abc123  # write orient digest, anchor commit
  maestro session exit tsk-abc123   # exit 0 if clean, 2 if lint regressed
`,
    );

  sessionCmd
    .command("whoami")
    .description("Detect the current agent session")
    .option("--json", "Output as JSON")
    .option("-q, --quiet", "Output just the session ID")
    .action(async (opts): Promise<void> => {
      await runWhoami(deps, opts, program);
    });

  registerSessionStartCommand(sessionCmd, program, deps);
  registerSessionExitCommand(sessionCmd, program, deps);
}

async function runWhoami(
  deps: SessionCommandDeps,
  opts: { json?: boolean; quiet?: boolean },
  program: Command,
): Promise<void> {
  const services = deps.getServices();
  const isJson = opts.json ?? program.opts().json;

  const result = await detectSession(services.sessionDetect, {
    cwd: process.cwd(),
  });

  if (!result) {
    if (opts.quiet) process.exit(1);
    throw new MaestroError("No session detected", [
      "Run inside Claude Code, Codex, or another supported agent",
      "The conductor reads CLAUDECODE / CODEX_THREAD_ID env vars only",
      "Subcommands: `maestro session start <taskId>` / `exit <taskId>` (see `maestro session --help`)",
    ]);
  }

  if (opts.quiet) {
    console.log(result.session.sessionId);
    return;
  }

  output(isJson, result, (r) => formatText(r.session));
}

function formatText(s: AgentSession): string[] {
  return [
    `Agent:     ${s.agent}`,
    `Session:   ${s.sessionId}`,
    `Source:    ${s.sourcePath}`,
    ...(s.startedAt
      ? [`Started:   ${new Date(s.startedAt).toLocaleString()}`]
      : []),
  ];
}
