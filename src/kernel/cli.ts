import type { Disposer } from "./events.ts";

export interface FlagDefinition {
  multiple?: boolean;
  value?: boolean;
}

export interface CliInvocation {
  command: string;
  options: Record<string, boolean | string | string[]>;
  positionals: string[];
}

export interface CliResult {
  data?: unknown;
  text?: string;
}

export type CliHandler = (
  invocation: CliInvocation,
) => CliResult | string | void | Promise<CliResult | string | void>;

interface CommandDefinition {
  flags: Map<string, FlagDefinition>;
  handler: CliHandler;
}

export class CliError extends Error {
  constructor(
    readonly code: string,
    message: string,
    readonly details: Record<string, unknown> = {},
  ) {
    super(message);
  }
}

export class Cli {
  private readonly commands = new Map<string, CommandDefinition>();
  private readonly extensions = new Map<string, Map<string, FlagDefinition>>();

  register(
    command: string,
    handler: CliHandler,
    flags: Record<string, FlagDefinition> = {},
  ): Disposer {
    if (this.commands.has(command)) {
      throw new Error(`command already registered: ${command}`);
    }
    this.commands.set(command, { handler, flags: new Map(Object.entries(flags)) });
    return () => this.commands.delete(command);
  }

  registerFlag(command: string, flag: string, definition: FlagDefinition): Disposer {
    const flags = this.extensions.get(command) ?? new Map<string, FlagDefinition>();
    if (flags.has(flag)) throw new Error(`flag already registered: ${command} ${flag}`);
    flags.set(flag, definition);
    this.extensions.set(command, flags);
    return () => {
      const current = this.extensions.get(command);
      current?.delete(flag);
      if (current?.size === 0) this.extensions.delete(command);
    };
  }

  async dispatch(args: string[]): Promise<number> {
    try {
      const found = this.findCommand(args);
      if (!found) {
        throw new CliError("UNKNOWN_VERB", `unknown verb: ${args.join(" ") || "<none>"}`);
      }
      const { command, definition, consumed } = found;
      const remaining = args.slice(consumed);
      const jsonIndex = remaining.indexOf("--json");
      const wantsJson = jsonIndex >= 0;
      if (wantsJson) {
        if (!(command === "status" || command.endsWith(" list"))) {
          throw new CliError("UNKNOWN_FLAG", `unknown flag: --json`, { flag: "--json" });
        }
        remaining.splice(jsonIndex, 1);
      }
      const invocation = this.parse(
        command,
        remaining,
        new Map([
          ...definition.flags,
          ...(this.extensions.get(command) ?? new Map<string, FlagDefinition>()),
        ]),
      );
      const result = await definition.handler(invocation);
      const normalized: CliResult =
        typeof result === "string" ? { text: result, data: result } : result ?? {};
      if (wantsJson) {
        process.stdout.write(`${JSON.stringify({ ok: true, data: normalized.data ?? null })}\n`);
      } else if (normalized.text) {
        process.stdout.write(normalized.text.endsWith("\n") ? normalized.text : `${normalized.text}\n`);
      }
      return 0;
    } catch (error) {
      const cliError =
        error instanceof CliError
          ? error
          : new CliError("INTERNAL", error instanceof Error ? error.message : String(error));
      process.stderr.write(
        `${JSON.stringify({
          ok: false,
          error: {
            code: cliError.code,
            message: cliError.message,
            ...cliError.details,
          },
        })}\n`,
      );
      return cliError.code === "UNKNOWN_VERB" || cliError.code === "UNKNOWN_FLAG" ? 2 : 1;
    }
  }

  private findCommand(args: string[]): {
    command: string;
    consumed: number;
    definition: CommandDefinition;
  } | null {
    const candidates = [...this.commands.entries()]
      .map(([command, definition]) => ({ command, definition, parts: command.split(" ") }))
      .sort((left, right) => right.parts.length - left.parts.length);
    const match = candidates.find(({ parts }) => parts.every((part, index) => args[index] === part));
    return match
      ? { command: match.command, definition: match.definition, consumed: match.parts.length }
      : null;
  }

  private parse(
    command: string,
    args: string[],
    flags: Map<string, FlagDefinition>,
  ): CliInvocation {
    const positionals: string[] = [];
    const options: Record<string, boolean | string | string[]> = {};
    for (let index = 0; index < args.length; index += 1) {
      const token = args[index] as string;
      if (!token.startsWith("--")) {
        positionals.push(token);
        continue;
      }
      const definition = flags.get(token);
      if (!definition) {
        throw new CliError("UNKNOWN_FLAG", `unknown flag: ${token}`, { flag: token });
      }
      const key = token.slice(2);
      if (definition.value) {
        const value = args[index + 1];
        if (value === undefined || value.startsWith("--")) {
          throw new CliError("MISSING_VALUE", `missing value for ${token}`, { flag: token });
        }
        index += 1;
        if (definition.multiple) {
          const current = options[key];
          options[key] = Array.isArray(current) ? [...current, value] : [value];
        } else {
          options[key] = value;
        }
      } else {
        options[key] = true;
      }
    }
    return { command, options, positionals };
  }
}
