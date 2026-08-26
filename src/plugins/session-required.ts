import { CliError, type CliHandler, type CommandOptions } from "../kernel/cli.ts";
import type { Disposer } from "../kernel/events.ts";
import type { PluginContext } from "../kernel/loader.ts";

export function registerSessionCommand(
  context: PluginContext,
  command: string,
  handler: CliHandler,
  options?: CommandOptions,
): Disposer {
  return context.cli.register(
    command,
    (invocation) => {
      if (process.env.MAESTRO_SESSION_NONE === "1") {
        throw new CliError(
          "SESSION_REQUIRED",
          "this command writes durable session-attributed state; remove MAESTRO_SESSION_NONE and retry",
        );
      }
      return handler(invocation);
    },
    options,
  );
}
