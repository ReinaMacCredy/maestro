import { readFile } from "node:fs/promises";
import { join } from "node:path";
import YAML from "yaml";
import {
  ArchitectureRulesNotFoundError,
  ArchitectureRulesParseError,
  type ArchitectureRules,
  type ArchitectureRulesPort,
  type PassiveHarnessRules,
} from "./architecture-rules.port.js";

const DEFAULT_PATH = "docs/architecture.yaml";

export interface YamlArchitectureRulesOptions {
  readonly repoRoot: string;
  readonly relPath?: string;
}

export function parseArchitectureRules(raw: string, path: string): ArchitectureRules {
  let parsed: unknown;
  try {
    parsed = YAML.parse(raw);
  } catch (err) {
    throw new ArchitectureRulesParseError(
      `Architecture rules at ${path} has invalid YAML: ${(err as Error).message}`,
    );
  }
  if (parsed === null || typeof parsed !== "object" || Array.isArray(parsed)) {
    throw new ArchitectureRulesParseError(`Architecture rules at ${path} must be a YAML mapping`);
  }
  const obj = parsed as Record<string, unknown>;

  if (obj.version !== 1) {
    throw new ArchitectureRulesParseError(
      `Architecture rules at ${path} requires version: 1`,
      "version",
    );
  }

  if (typeof obj.forward_only !== "boolean") {
    throw new ArchitectureRulesParseError(
      `Architecture rules at ${path} requires forward_only: boolean`,
      "forward_only",
    );
  }

  if (!isNonEmptyStringArray(obj.layers)) {
    throw new ArchitectureRulesParseError(
      `Architecture rules at ${path} requires layers: non-empty string array`,
      "layers",
    );
  }

  const crossCutting = obj.cross_cutting ?? [];
  if (!isStringArray(crossCutting)) {
    throw new ArchitectureRulesParseError(
      `Architecture rules at ${path} cross_cutting must be a string array when present`,
      "cross_cutting",
    );
  }

  const lintScope = obj.lint_scope ?? [];
  if (!isStringArray(lintScope)) {
    throw new ArchitectureRulesParseError(
      `Architecture rules at ${path} lint_scope must be a string array when present`,
      "lint_scope",
    );
  }

  const passive = parsePassiveHarness(obj.passive_harness, path);

  return {
    version: 1,
    forward_only: obj.forward_only,
    layers: obj.layers,
    cross_cutting: crossCutting,
    lint_scope: lintScope,
    passive_harness: passive,
  };
}

function parsePassiveHarness(value: unknown, path: string): PassiveHarnessRules {
  if (value === undefined || value === null) {
    return { forbidden_patterns: [] };
  }
  if (typeof value !== "object") {
    throw new ArchitectureRulesParseError(
      `Architecture rules at ${path} passive_harness must be a mapping`,
      "passive_harness",
    );
  }
  const obj = value as Record<string, unknown>;
  const patterns = obj.forbidden_patterns ?? [];
  if (!isStringArray(patterns)) {
    throw new ArchitectureRulesParseError(
      `Architecture rules at ${path} passive_harness.forbidden_patterns must be a string array`,
      "passive_harness.forbidden_patterns",
    );
  }
  return { forbidden_patterns: patterns };
}

function isStringArray(value: unknown): value is string[] {
  return Array.isArray(value) && value.every((v) => typeof v === "string");
}

function isNonEmptyStringArray(value: unknown): value is string[] {
  return isStringArray(value) && value.length > 0;
}

export class YamlArchitectureRules implements ArchitectureRulesPort {
  readonly #path: string;

  constructor(options: YamlArchitectureRulesOptions) {
    this.#path = join(options.repoRoot, options.relPath ?? DEFAULT_PATH);
  }

  async load(): Promise<ArchitectureRules> {
    let raw: string;
    try {
      raw = await readFile(this.#path, "utf8");
    } catch (err) {
      if ((err as NodeJS.ErrnoException).code === "ENOENT") {
        throw new ArchitectureRulesNotFoundError(this.#path);
      }
      throw err;
    }
    return parseArchitectureRules(raw, this.#path);
  }
}
