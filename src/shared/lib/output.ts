/**
 * Dual-mode output: JSON for machines, text for humans.
 * All commands route through this to ensure consistent --json behavior.
 */
import { sanitizeTerminalText } from "@/shared/lib/sanitize.js";

export function output<T>(
  json: boolean,
  data: T,
  formatter: (d: T) => string[],
): void {
  if (json) {
    console.log(JSON.stringify(data, null, 2));
  } else {
    for (const line of formatter(data)) {
      console.log(sanitizeTerminalText(line));
    }
  }
}

/** Resolve --json flag from leaf option, group option, or root program option. */
export function resolveJsonFlag(opts: Record<string, unknown>, program: { opts(): Record<string, unknown> }): boolean {
  if (opts.json !== undefined) return opts.json as boolean;
  if (opts.jsonGroup !== undefined) return opts.jsonGroup as boolean;
  return program.opts().json as boolean ?? false;
}

/** Write to stderr without affecting stdout (for warnings in --json mode). */
export function warn(message: string): void {
  console.error(`[!] ${message}`);
}

/** Format agent inject/remove results for text output. */
export function formatAgentResults(
  agents: ReadonlyArray<{ agent: string; action: string; configPath: string }>,
): string[] {
  return agents.map((a) => `  ${a.agent}: ${a.action} (${a.configPath})`);
}
