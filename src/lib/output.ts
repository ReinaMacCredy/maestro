/**
 * Dual-mode output: JSON for machines, text for humans.
 * All commands route through this to ensure consistent --json behavior.
 */
export function output<T>(
  json: boolean,
  data: T,
  formatter: (d: T) => string[],
): void {
  if (json) {
    console.log(JSON.stringify(data, null, 2));
  } else {
    for (const line of formatter(data)) {
      console.log(line);
    }
  }
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
