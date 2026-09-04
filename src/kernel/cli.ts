import { SQLiteError } from "bun:sqlite";
import type { Disposer } from "./events.ts";

export interface FlagDefinition {
  description?: string;
  multiple?: boolean;
  value?: boolean;
}

export interface CommandOptions {
  description?: string;
  flags?: Record<string, FlagDefinition>;
  json?: boolean;
  maxPositionals?: number;
  mutates?: boolean;
  positionals?: PositionalDefinition[];
  rootDescription?: string;
}

export interface CliOptions {
  beforeInvoke?: (command: string, mutates: boolean) => Promise<void> | void;
  beforeUnknown?: (args: readonly string[]) => Promise<void> | void;
  helpFooter?: string;
  readOnlyAdmits?: (owner: string | null, mutates: boolean) => boolean;
}

export interface PositionalDefinition {
  name: string;
  required: boolean;
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

export interface CliFlagDescriptor {
  description: string;
  multiple: boolean;
  name: string;
  value: boolean;
}

export interface CliCommandDescriptor {
  description: string;
  flags: CliFlagDescriptor[];
  name: string;
  positionals?: PositionalDefinition[];
}

export interface CliSuccessEnvelope {
  data: unknown;
  ok: true;
}

export interface CliFailureEnvelope {
  error: {
    code: string;
    message: string;
    [detail: string]: unknown;
  };
  ok: false;
}

export type CliHandler = (
  invocation: CliInvocation,
) => CliResult | string | void | Promise<CliResult | string | void>;

interface CommandDefinition {
  description: string;
  flags: Map<string, FlagDefinition>;
  handler: CliHandler;
  json: boolean;
  maxPositionals: number;
  mutates: boolean;
  owner: string | null;
  positionals: PositionalDefinition[];
  rootDescription?: string;
}

interface HelpRow {
  description: string;
  label: string;
}

export function editDistance(left: string, right: string): number {
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

export function requiredPosition(
  invocation: CliInvocation,
  index: number,
  label: string,
): string {
  const value = invocation.positionals[index];
  if (!value) throw new CliError("MISSING_ARGUMENT", `missing ${label}`);
  return value;
}

export function stringOption(invocation: CliInvocation, name: string): string | undefined {
  const value = invocation.options[name];
  return typeof value === "string" ? value : undefined;
}

export function stringOptions(invocation: CliInvocation, name: string): string[] {
  const value = invocation.options[name];
  if (Array.isArray(value)) return value;
  return typeof value === "string" ? [value] : [];
}

export function normalizeCliError(error: unknown): CliError {
  if (error instanceof CliError) return error;
  if (error instanceof SQLiteError && error.code?.startsWith("SQLITE_BUSY")) {
    return new CliError(
      "STORE_BUSY",
      "the Maestro store is busy; retry the command",
      { sqliteCode: error.code },
    );
  }
  return new CliError("INTERNAL", error instanceof Error ? error.message : String(error));
}

export function successEnvelope(result: CliResult): CliSuccessEnvelope {
  return { ok: true, data: result.data ?? null };
}

export function failureEnvelope(error: unknown): CliFailureEnvelope {
  const cliError = normalizeCliError(error);
  return {
    ok: false,
    error: {
      code: cliError.code,
      message: cliError.message,
      ...cliError.details,
    },
  };
}

export class Cli {
  private readonly commands = new Map<string, CommandDefinition>();
  private readonly extensions = new Map<string, Map<string, FlagDefinition>>();

  constructor(private readonly options: CliOptions = {}) {}

  // The loader names the plugin being applied so help can say which verbs
  // observer mode admits; the answer depends on the owning plugin, not the verb.
  owner: string | null = null;

  register(command: string, handler: CliHandler, options?: CommandOptions): Disposer;
  register(
    command: string,
    handler: CliHandler,
    flags?: Record<string, FlagDefinition>,
    maxPositionals?: number,
    description?: string,
  ): Disposer;
  register(
    command: string,
    handler: CliHandler,
    optionsOrFlags: CommandOptions | Record<string, FlagDefinition> = {},
    legacyMaxPositionals = 0,
    legacyDescription?: string,
  ): Disposer {
    if (this.commands.has(command)) {
      throw new Error(`command already registered: ${command}`);
    }
    const legacy =
      legacyDescription !== undefined ||
      legacyMaxPositionals !== 0 ||
      Object.keys(optionsOrFlags).some((key) => key.startsWith("--"));
    const options: CommandOptions = legacy
      ? {
          description: legacyDescription,
          flags: optionsOrFlags as Record<string, FlagDefinition>,
          maxPositionals: legacyMaxPositionals,
        }
      : (optionsOrFlags as CommandOptions);
    this.commands.set(command, {
      description: this.oneLine(options.description, `Run ${command}.`),
      handler,
      flags: new Map(Object.entries(options.flags ?? {})),
      json: options.json ?? false,
      maxPositionals: options.maxPositionals ?? options.positionals?.length ?? 0,
      mutates: options.mutates ?? true,
      owner: this.owner,
      positionals: options.positionals ?? [],
      rootDescription: options.rootDescription
        ? this.oneLine(options.rootDescription, options.rootDescription)
        : undefined,
    });
    return () => {
      this.commands.delete(command);
    };
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

  commandMutates(command: string): boolean | undefined {
    return this.commands.get(command)?.mutates;
  }

  describeCommands(): CliCommandDescriptor[] {
    return [...this.commands.entries()]
      .sort(([left], [right]) => left.localeCompare(right))
      .map(([name, definition]) => ({
        name,
        description: definition.description,
        ...(definition.positionals.length > 0
          ? { positionals: definition.positionals.map((positional) => ({ ...positional })) }
          : {}),
        flags: [...this.effectiveFlags(name, definition).entries()]
          .sort(([left], [right]) => left.localeCompare(right))
          .map(([flag, metadata]) => ({
            name: flag,
            description: this.oneLine(metadata.description, `Set ${flag}.`),
            multiple: metadata.multiple ?? false,
            value: metadata.value ?? false,
          })),
      }));
  }

  async execute(args: string[]): Promise<CliResult> {
    return (await this.invoke(args)).result;
  }

  async dispatch(args: string[]): Promise<number> {
    try {
      const { result, wantsJson } = await this.invoke(args);
      if (wantsJson) {
        process.stdout.write(`${JSON.stringify(successEnvelope(result))}\n`);
      } else if (result.text) {
        process.stdout.write(result.text.endsWith("\n") ? result.text : `${result.text}\n`);
      }
      return 0;
    } catch (error) {
      const cliError = normalizeCliError(error);
      process.stderr.write(`${JSON.stringify(failureEnvelope(cliError))}\n`);
      return cliError.code === "UNKNOWN_VERB" ||
        cliError.code === "UNKNOWN_FLAG" ||
        cliError.code === "UNKNOWN_ARGUMENT"
        ? 2
        : 1;
    }
  }

  private async invoke(args: string[]): Promise<{ result: CliResult; wantsJson: boolean }> {
    if (args.length === 0) {
      const help = this.helpText();
      return { result: { data: { help }, text: help }, wantsJson: false };
    }
    if (args[0] === "help") {
      if (args.length === 2 && args[1] === "--help") {
        const help = this.helpText();
        return { result: { data: { help }, text: help }, wantsJson: false };
      }
      const { target, unexpected } = this.helpTarget(args.slice(1));
      if (unexpected) {
        throw new CliError("UNKNOWN_ARGUMENT", `unknown argument: ${unexpected}`, {
          argument: unexpected,
        });
      }
      const help = this.helpText(target);
      return { result: { data: { help }, text: help }, wantsJson: false };
    }
    const helpIndex = args.indexOf("--help");
    if (helpIndex >= 0) {
      const unexpected = args[helpIndex + 1];
      if (unexpected) {
        throw new CliError("UNKNOWN_ARGUMENT", `unknown argument: ${unexpected}`, {
          argument: unexpected,
        });
      }
      const help = this.helpText(args.slice(0, helpIndex).join(" ") || undefined);
      return { result: { data: { help }, text: help }, wantsJson: false };
    }
    const found = this.findCommand(args);
    if (!found) {
      await this.options.beforeUnknown?.(args);
      const { attempted, suggestions } = this.nearestCommands(args);
      throw new CliError(
        "UNKNOWN_VERB",
        `unknown verb: ${attempted}; nearest: ${suggestions.join(", ")}`,
        { suggestions, verb: attempted },
      );
    }
    const { command, definition, consumed } = found;
    await this.options.beforeInvoke?.(command, this.commandMutates(command) ?? true);
    const remaining = args.slice(consumed);
    const jsonIndex = remaining.indexOf("--json");
    const wantsJson = jsonIndex >= 0;
    if (wantsJson) {
      if (!this.supportsJson(command)) {
        const helpCommand = `maestro help ${command.split(" ")[0]}`;
        throw new CliError("UNKNOWN_FLAG", `unknown flag: --json; run: ${helpCommand}`, {
          command: helpCommand,
          flag: "--json",
        });
      }
      remaining.splice(jsonIndex, 1);
    }
    const invocation = this.parse(
      command,
      remaining,
      this.effectiveFlags(command, definition),
      definition.maxPositionals,
    );
    const result = await definition.handler(invocation);
    const normalized: CliResult =
      typeof result === "string" ? { text: result, data: result } : result ?? {};
    return { result: normalized, wantsJson };
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
        const helpCommand = `maestro help ${command.split(" ")[0]}`;
        throw new CliError("UNKNOWN_FLAG", `unknown flag: ${token}; run: ${helpCommand}`, {
          command: helpCommand,
          flag: token,
        });
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
        description: `${this.rootDescription(verb)}${this.rootAdmittedReadOnly(verb) ? " *" : ""}`,
      }));
      const footer = this.options.helpFooter?.trim();
      const legend = this.options.readOnlyAdmits
        ? "\n  *  a verb it admits (on a root verb: at least one of its subverbs)"
        : "";
      return `verbs:\n${this.formatRows(rows, "  ")}\n${footer ? `\n${footer}${legend}\n` : ""}`;
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
    const description =
      direct?.description ??
      entries.find(([, definition]) => definition.rootDescription)?.[1].rootDescription ??
      entries[0]?.[1].description ??
      `Run ${target}.`;
    const lines = [`${target}  ${description}${direct && this.admittedReadOnly(direct) ? " *" : ""}`];
    if (direct) {
      lines.push(`usage: maestro ${this.usage(target, direct)}`);
      lines.push(...this.flagHelp(target, direct));
    }

    const nested = entries.filter(([command]) => command !== target);
    if (nested.length > 0) {
      lines.push("subverbs:");
      const rows = nested.map(([command, definition]): HelpRow => ({
        label: command.slice(target.length + 1),
        description: `${definition.description}${this.admittedReadOnly(definition) ? " *" : ""}`,
      }));
      const width = Math.max(...rows.map((row) => row.label.length));
      for (const [index, [command, definition]] of nested.entries()) {
        lines.push(this.formatRow(rows[index] ?? { label: command, description: definition.description }, width, "  "));
        if (definition.positionals.length > 0) {
          lines.push(`    usage: maestro ${this.usage(command, definition)}`);
        }
        const flags = this.flagHelp(command, definition);
        if (flags.length > 0) lines.push(...flags);
      }
    }
    if (entries.some(([, definition]) => this.admittedReadOnly(definition))) {
      lines.push("* runs under MAESTRO_READ_ONLY=1");
    }
    return `${lines.join("\n")}\n`;
  }

  private admittedReadOnly(definition: CommandDefinition): boolean {
    return this.options.readOnlyAdmits?.(definition.owner, definition.mutates) ?? false;
  }

  private rootAdmittedReadOnly(root: string): boolean {
    if (root === "help") return this.options.readOnlyAdmits !== undefined;
    return [...this.commands.entries()].some(
      ([command, definition]) =>
        (command === root || command.startsWith(`${root} `)) && this.admittedReadOnly(definition),
    );
  }

  private flagHelp(command: string, definition: CommandDefinition): string[] {
    const flags = this.effectiveFlags(command, definition);
    if (this.supportsJson(command)) {
      flags.set("--json", { description: "Emit one compact JSON success envelope." });
    }
    if (flags.size === 0) return [];
    const rows = [...flags.entries()]
      .sort(([left], [right]) => left.localeCompare(right))
      .map(([flag, metadata]): HelpRow => ({
        label: `${flag}${metadata.value ? " <value>" : ""}${metadata.multiple ? " (repeatable)" : ""}`,
        description: this.oneLine(metadata.description, `Set ${flag}.`),
      }));
    return [this.formatRows(rows, "    ")];
  }

  private usage(command: string, definition: CommandDefinition): string {
    const positionals = definition.positionals.map((positional) =>
      positional.required ? `<${positional.name}>` : `[${positional.name}]`
    );
    return [command, ...positionals].join(" ");
  }

  private helpTarget(args: string[]): { target?: string; unexpected?: string } {
    if (args.length === 0) return {};
    const targets = [...new Set([...this.commands.keys(), ...this.rootVerbs()])]
      .map((target) => ({ parts: target.split(" "), target }))
      .sort((left, right) => right.parts.length - left.parts.length);
    const matched = targets.find(({ parts }) =>
      parts.every((part, index) => args[index] === part)
    );
    if (!matched) return { target: args[0], unexpected: args[1] };
    return { target: matched.target, unexpected: args[matched.parts.length] };
  }

  private formatRows(rows: HelpRow[], indent: string): string {
    const width = Math.max(...rows.map((row) => row.label.length));
    return rows.map((row) => this.formatRow(row, width, indent)).join("\n");
  }

  private formatRow(row: HelpRow, width: number, indent: string): string {
    return `${indent}${row.label.padEnd(width + 2)}${row.description}`;
  }

  private rootDescription(root: string): string {
    if (root === "help") return "Show top-level or per-verb help.";
    const direct = this.commands.get(root);
    if (direct) return direct.description;
    const nested = [...this.commands.entries()]
      .filter(([command]) => command.startsWith(`${root} `))
      .sort(([left], [right]) => left.localeCompare(right));
    return (
      nested.find(([, definition]) => definition.rootDescription)?.[1].rootDescription ??
      nested[0]?.[1].description ??
      `Run ${root} commands.`
    );
  }

  private effectiveFlags(
    command: string,
    definition: CommandDefinition,
  ): Map<string, FlagDefinition> {
    return new Map([
      ...definition.flags,
      ...(this.extensions.get(command) ?? new Map<string, FlagDefinition>()),
    ]);
  }

  private oneLine(description: string | undefined, fallback: string): string {
    const value = description?.trim() || fallback;
    if (/\r|\n/.test(value)) throw new Error("CLI descriptions must fit on one line");
    return value;
  }

  private supportsJson(command: string): boolean {
    return this.commands.get(command)?.json === true ||
      command === "status" ||
      command === "ready" ||
      command === "handoff" ||
      command === "bundle show" ||
      command === "work show" ||
      command === "search" ||
      command.endsWith(" list");
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
