import { resolve } from "node:path";
import { execArgv } from "@/shared/lib/shell.js";
import { estimateTokensAuto } from "@/shared/lib/token-estimate.js";

export interface TokenBudgetRow {
  readonly verb: string;
  readonly mode: "default" | "full";
  readonly bytes: number;
  readonly tokens: number;
  readonly ok: boolean;
}

export interface TokenBudgetResult {
  readonly rows: readonly TokenBudgetRow[];
  readonly totals: {
    readonly defaultBytes: number;
    readonly fullBytes: number;
    readonly defaultTokens: number;
    readonly fullTokens: number;
  };
}

interface VerbProbe {
  readonly verb: string;
  readonly args: readonly string[];
  readonly fullArgs?: readonly string[];
}

const PROBES: readonly VerbProbe[] = [
  { verb: "skills list", args: ["skills", "list", "--json"], fullArgs: ["skills", "list", "--json", "--full"] },
  { verb: "task list", args: ["task", "list", "--json"], fullArgs: ["task", "list", "--json", "--full", "--all"] },
  { verb: "task status", args: ["task", "status", "--json"], fullArgs: ["task", "status", "--json", "--full"] },
  { verb: "task ready", args: ["task", "ready", "--json"] },
  { verb: "task stuck", args: ["task", "stuck", "--json"], fullArgs: ["task", "stuck", "--json", "--full"] },
  { verb: "mission list", args: ["mission", "list", "--json"], fullArgs: ["mission", "list", "--json", "--full", "--all"] },
  { verb: "evidence list", args: ["evidence", "list", "--json"], fullArgs: ["evidence", "list", "--json", "--full", "--all"] },
  { verb: "handoff list", args: ["handoff", "list", "--json"], fullArgs: ["handoff", "list", "--json", "--full", "--all"] },
];

export async function inspectTokenBudget(): Promise<TokenBudgetResult> {
  const bin = resolveCliBin();
  const rows = await Promise.all(
    PROBES.flatMap((probe) => {
      const tasks = [measure(bin, probe.verb, "default", probe.args)];
      if (probe.fullArgs) tasks.push(measure(bin, probe.verb, "full", probe.fullArgs));
      return tasks;
    }),
  );
  const totals = { defaultBytes: 0, fullBytes: 0, defaultTokens: 0, fullTokens: 0 };
  for (const row of rows) {
    if (row.mode === "default") {
      totals.defaultBytes += row.bytes;
      totals.defaultTokens += row.tokens;
    } else {
      totals.fullBytes += row.bytes;
      totals.fullTokens += row.tokens;
    }
  }
  return { rows, totals };
}

async function measure(
  bin: string,
  verb: string,
  mode: "default" | "full",
  args: readonly string[],
): Promise<TokenBudgetRow> {
  const result = await execArgv([bin, ...args]);
  return {
    verb,
    mode,
    bytes: Buffer.byteLength(result.stdout, "utf8"),
    tokens: estimateTokensAuto(result.stdout),
    ok: result.exitCode === 0,
  };
}

function resolveCliBin(): string {
  const envBin = process.env.MAESTRO_BIN;
  if (envBin && envBin.length > 0) return envBin;
  if (process.argv[0]?.endsWith("maestro")) return process.argv[0];
  return resolve(process.cwd(), "dist/maestro");
}

export function formatTokenBudgetLines(result: TokenBudgetResult): readonly string[] {
  const lines: string[] = [];
  lines.push("Token budget (bytes / est. tokens):");
  lines.push("");
  const verbWidth = Math.max(...result.rows.map((r) => r.verb.length));
  lines.push(`  ${"verb".padEnd(verbWidth)}  mode     bytes    tokens`);
  lines.push(`  ${"-".repeat(verbWidth)}  -------  -------  -------`);
  for (const row of result.rows) {
    const verb = row.verb.padEnd(verbWidth);
    const mode = row.mode.padEnd(7);
    const bytes = String(row.bytes).padStart(7);
    const tokens = String(row.tokens).padStart(7);
    lines.push(`  ${verb}  ${mode}  ${bytes}  ${tokens}${row.ok ? "" : "  [err]"}`);
  }
  lines.push("");
  lines.push(
    `  totals (default): ${result.totals.defaultBytes} bytes, ~${result.totals.defaultTokens} tokens`,
  );
  if (result.totals.fullBytes > 0) {
    lines.push(
      `  totals (--full):  ${result.totals.fullBytes} bytes, ~${result.totals.fullTokens} tokens`,
    );
  }
  return lines;
}
