import type { Disposer } from "./events.ts";

export interface FlagDefinition {
  description?: string;
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
  description: string;
  flags: Map<string, FlagDefinition>;
  handler: CliHandler;
  maxPositionals: number;
}

interface HelpRow {
  description: string;
  label: string;
}

function editDistance(left: string, right: string): number {
  const previous = Array.from({ length: right.length + 1 }, (_, index) => index);
  for (let leftIndex = 1; leftIndex <= left.length; leftIndex += 1) {
    const current = [leftIndex];
    for (let rightIndex = 1; rightIndex <= right.length; rightIndex += 1) {
      const substitution =
        (previous[rightIndex - 1] ?? 0) +
        (left[leftIndex - 1] === right[rightIndex - 1] ? 0 : 1);
      current[rightIndex] = Math.min(
        (current[rightIndex - 1] ?? 0) + 1,
        (previous[rightIndex] ?? 0) + 1,
        substitution,
      );
    }
    previous.splice(0, previous.length, ...current);
  }
  return previous[right.length] ?? 0;
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
    maxPositionals = 0,
    description = `Run ${command}.`,
  ): Disposer {
    if (this.commands.has(command)) {
      throw new Error(`command already registered: ${command}`);
    }
    this.commands.set(command, {
      description: this.oneLine(description, `Run ${command}.`),
      handler,
      flags: new Map(Object.entries(flags)),
      maxPositionals,
    });
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
      if (args.length === 0) {
        process.stdout.write(this.helpText());
        return 0;
      }
      if (args[0] === "help") {
        const unexpected = args[2];
        if (unexpected) {
          throw new CliError("UNKNOWN_ARGUMENT", `unknown argument: ${unexpected}`, {
            argument: unexpected,
          });
        }
        process.stdout.write(this.helpText(args[1]));
        return 0;
      }
      const helpIndex = args.indexOf("--help");
      if (helpIndex >= 0) {
        const unexpected = args[helpIndex + 1];
        if (unexpected) {
          throw new CliError("UNKNOWN_ARGUMENT", `unknown argument: ${unexpected}`, {
            argument: unexpected,
          });
        }
        process.stdout.write(this.helpText(args.slice(0, helpIndex).join(" ") || undefined));
        return 0;
      }
      const found = this.findCommand(args);
      if (!found) {
        const { attempted, suggestions } = this.nearestCommands(args);
        throw new CliError(
          "UNKNOWN_VERB",
          `unknown verb: ${attempted}; nearest: ${suggestions.join(", ")}`,
          { suggestions, verb: attempted },
        );
      }
      const { command, definition, consumed } = found;
      const remaining = args.slice(consumed);
      const jsonIndex = remaining.indexOf("--json");
      const wantsJson = jsonIndex >= 0;
      if (wantsJson) {
        if (!(command === "status" || command === "msg send" || command.endsWith(" list"))) {
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
        definition.maxPositionals,
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
      return cliError.code === "UNKNOWN_VERB" ||
        cliError.code === "UNKNOWN_FLAG" ||
        cliError.code === "UNKNOWN_ARGUMENT"
        ? 2
        : 1;
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
    maxPositionals: number,
  ): CliInvocation {
    const positionals: string[] = [];
    const options: Record<string, boolean | string | string[]> = {};
    for (let index = 0; index < args.length; index += 1) {
      const token = args[index] ?? "";
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
    const unexpected = positionals[maxPositionals];
    if (unexpected !== undefined) {
      throw new CliError("UNKNOWN_ARGUMENT", `unknown argument: ${unexpected}`, {
        argument: unexpected,
      });
    }
    return { command, options, positionals };
  }

  private helpText(target?: string): string {
    if (!target) {
      const rows = this.rootVerbs().map((verb): HelpRow => ({
        label: verb,
        description: this.rootDescription(verb),
      }));
      return `verbs:\n${this.formatRows(rows, "  ")}\n`;
    }

    const entries = [...this.commands.entries()]
      .filter(([command]) => command === target || command.startsWith(`${target} `))
      .sort(([left], [right]) => left.localeCompare(right));
    if (entries.length === 0) {
      const { attempted, suggestions } = this.nearestCommands(target.split(" "));
      throw new CliError(
        "UNKNOWN_VERB",
        `unknown verb: ${attempted}; nearest: ${suggestions.join(", ")}`,
        { suggestions, verb: attempted },
      );
    }

    const direct = entries.find(([command]) => command === target)?.[1];
    const description = direct?.description ?? entries[0]?.[1].description ?? `Run ${target}.`;
    const lines = [`${target}  ${description}`];
    if (direct) lines.push(...this.flagHelp(target, direct));

    const nested = entries.filter(([command]) => command !== target);
    if (nested.length > 0) {
      lines.push("subverbs:");
      const rows = nested.map(([command, definition]): HelpRow => ({
        label: command.slice(target.length + 1),
        description: definition.description,
      }));
      const commandLines = this.formatRows(rows, "  ").split("\n");
      for (const [index, [command, definition]] of nested.entries()) {
        lines.push(commandLines[index] ?? command);
        const flags = this.flagHelp(command, definition);
        if (flags.length > 0) lines.push(...flags);
      }
    }
    return `${lines.join("\n")}\n`;
  }

  private flagHelp(command: string, definition: CommandDefinition): string[] {
    const flags = new Map([
      ...definition.flags,
      ...(this.extensions.get(command) ?? new Map<string, FlagDefinition>()),
    ]);
    if (flags.size === 0) return [];
    const rows = [...flags.entries()]
      .sort(([left], [right]) => left.localeCompare(right))
      .map(([flag, metadata]): HelpRow => ({
        label: `${flag}${metadata.value ? " <value>" : ""}${metadata.multiple ? " (repeatable)" : ""}`,
        description: this.oneLine(metadata.description, `Set ${flag}.`),
      }));
    return [this.formatRows(rows, "    ")];
  }

  private formatRows(rows: HelpRow[], indent: string): string {
    const width = Math.max(...rows.map((row) => row.label.length));
    return rows
      .map((row) => `${indent}${row.label.padEnd(width + 2)}${row.description}`)
      .join("\n");
  }

  private rootDescription(root: string): string {
    if (root === "help") return "Show top-level or per-verb help.";
    const direct = this.commands.get(root);
    if (direct) return direct.description;
    const nested = [...this.commands.entries()]
      .filter(([command]) => command.startsWith(`${root} `))
      .sort(([left], [right]) => left.localeCompare(right));
    return nested[0]?.[1].description ?? `Run ${root} commands.`;
  }

  private oneLine(description: string | undefined, fallback: string): string {
    const value = description?.trim() || fallback;
    if (/\r|\n/.test(value)) throw new Error("CLI descriptions must fit on one line");
    return value;
  }

  private nearestCommands(args: string[]): { attempted: string; suggestions: string[] } {
    const root = args[0] ?? "";
    const nested = [...this.commands.keys()].filter((command) => command.startsWith(`${root} `));
    const candidates = nested.length > 0 ? nested : this.rootVerbs();
    const attempted = nested.length > 0 ? args.slice(0, 2).join(" ") : root;
    const suggestions = candidates
      .map((command) => ({ command, distance: editDistance(attempted, command) }))
      .sort((left, right) => left.distance - right.distance || left.command.localeCompare(right.command))
      .slice(0, 3)
      .map(({ command }) => command);
    return { attempted, suggestions };
  }

  private rootVerbs(): string[] {
    return [
      ...new Set([
        "help",
        ...[...this.commands.keys()].map((command) => command.split(" ", 1)[0] ?? command),
      ]),
    ].sort();
  }
}
