import { readFileSync } from "node:fs";
import { join } from "node:path";
import { CliError, type CliInvocation, type CliResult } from "../kernel/cli.ts";
import type { Disposer } from "../kernel/events.ts";
import type { BuiltInPlugin } from "../kernel/loader.ts";
import type { BriefService } from "./coordination.ts";

export interface RecipeEntry {
  body: string | (() => string);
  description: string;
  name: string;
}

export interface RecipeService {
  get(name: string): RecipeEntry | null;
  list(): RecipeEntry[];
  register(entry: RecipeEntry): Disposer;
}

const catalog = [
  ["design", "Settle one design fork at a time before implementation."],
  ["work", "Implement one accepted unit through proof and the next gate."],
  ["audit", "Review a bounded surface and record evidence-backed findings."],
  ["ship", "Cross close and delivery gates only with evidence and authority."],
  ["unattended", "Let an external driver advance one safe ready item at a time."],
  ["learning", "Turn sourced corrections into durable, reusable project knowledge."],
  ["worktree", "Isolate concurrent work and return a verified branch for merge-back."],
  ["conflict-handoff", "Coordinate overlap through terminal panes, dispatches, and handbacks."],
  ["style-cpp", "Apply modern C++ ownership, interface, and verification conventions."],
  ["style-csharp", "Apply clear C# contracts, nullability, async, and testing conventions."],
  ["style-dart", "Apply idiomatic Dart typing, async, package, and testing conventions."],
  ["style-general", "Apply language-neutral clarity, correctness, and change-discipline conventions."],
  ["style-go", "Apply idiomatic Go APIs, errors, concurrency, and testing conventions."],
  ["style-html-css", "Build semantic, accessible, responsive HTML and CSS."],
  ["style-javascript", "Apply modern JavaScript module, async, error, and testing conventions."],
  ["style-python", "Apply explicit Python interfaces, errors, resources, and testing conventions."],
  ["style-rust", "Apply idiomatic Rust ownership, errors, APIs, and testing conventions."],
  ["style-typescript", "Apply strict TypeScript boundaries, types, modules, and bun verification."],
] as const;

class Recipes implements RecipeService {
  private readonly entries = new Map<string, RecipeEntry>();

  get(name: string): RecipeEntry | null {
    return this.entries.get(name) ?? null;
  }

  list(): RecipeEntry[] {
    return [...this.entries.values()];
  }

  register(entry: RecipeEntry): Disposer {
    if (this.entries.has(entry.name)) throw new Error(`recipe already registered: ${entry.name}`);
    this.entries.set(entry.name, entry);
    return () => {
      this.entries.delete(entry.name);
    };
  }
}

function shippedRecipe(name: string, description: string): RecipeEntry {
  return {
    name,
    description,
    body: () => readFileSync(join(import.meta.dir, "recipes", `${name}.md`), "utf8"),
  };
}

function requiredName(invocation: CliInvocation): string {
  const name = invocation.positionals[0];
  if (!name) throw new CliError("MISSING_ARGUMENT", "missing recipe name");
  return name;
}

export const recipePlugin: BuiltInPlugin = {
  name: "recipe",
  inject: ["brief"],
  apply(context) {
    const recipes = new Recipes();
    const brief = context.brief as BriefService;
    context.effect(() => context.provide("recipe", recipes));
    for (const [name, description] of catalog) {
      context.effect(() => recipes.register(shippedRecipe(name, description)));
    }
    context.effect(() =>
      context.cli.register(
        "recipe list",
        (): CliResult => {
          const entries = recipes.list();
          return {
            data: { recipes: entries.map(({ name, description }) => ({ name, description })) },
            text: entries.map((entry) => `${entry.name}\t${entry.description}`).join("\n"),
          };
        },
        {
          description: "List the shipped workflow recipes.",
          rootDescription: "Browse and read shipped workflow recipes.",
        },
      ),
    );
    context.effect(() =>
      context.cli.register(
        "recipe show",
        (invocation): CliResult => {
          const name = requiredName(invocation);
          const entry = recipes.get(name);
          if (!entry) {
            throw new CliError(
              "RECIPE_NOT_FOUND",
              `recipe not found: ${name}; available: ${recipes.list().map((recipe) => recipe.name).join(", ")}`,
            );
          }
          const body = typeof entry.body === "function" ? entry.body() : entry.body;
          return { data: { name, description: entry.description, body }, text: body };
        },
        {
          description: "Show one workflow recipe by name.",
          positionals: [{ name: "name", required: true }],
        },
      ),
    );
    context.effect(() =>
      brief.register(() => "recipes: maestro recipe list; maestro recipe show <name>"),
    );
  },
};
