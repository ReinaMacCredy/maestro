/**
 * Structured text output with JSON bypass.
 *
 * - `setOutputMode` / `getOutputMode` control the module-level mode.
 * - `output()` prints via textFormatter in text mode, JSON.stringify in json mode.
 * - Auto-detects `--json` in process.argv on first call if mode not explicitly set.
 */

type OutputMode = "text" | "json";

let mode: OutputMode | undefined;

export function setOutputMode(m: OutputMode): void {
  mode = m;
}

export function getOutputMode(): OutputMode {
  return mode ?? "text";
}

function autoDetect(): void {
  if (mode !== undefined) return;
  mode = process.argv.includes("--json") ? "json" : "text";
}

export function output<T = unknown>(data: T, textFormatter: (data: T) => string): void {
  autoDetect();
  if (mode === "json") {
    console.log(JSON.stringify(data));
  } else {
    console.log(textFormatter(data));
  }
}

/**
 * Render an aligned table string.
 * Each column is padded to the max width in that column.
 * A dash line separates header from body rows.
 */
export function renderTable(headers: string[], rows: string[][]): string {
  const colWidths: number[] = headers.map((h, i) => {
    const cellWidths = rows.map((r) => (r[i] ?? "").length);
    return Math.max(h.length, ...cellWidths);
  });

  const pad = (s: string, w: number) => s + " ".repeat(Math.max(0, w - s.length));

  const headerLine = colWidths.map((w, i) => pad(headers[i], w)).join("  ");
  const separator = colWidths.map((w) => "-".repeat(w)).join("  ");
  const bodyLines = rows.map((row) =>
    colWidths.map((w, i) => pad(row[i] ?? "", w)).join("  "),
  );

  return [headerLine, separator, ...bodyLines].join("\n");
}

export function renderStatusLine(label: string, value: string): string {
  return `${label}: ${value}`;
}

export function renderList(items: string[]): string {
  return items.map((item) => `- ${item}`).join("\n");
}

/** Render a task/subtask list as a table with standard columns. */
export function renderTaskTable(tasks: { id: string; name: string; status: string; origin: string }[]): string {
  const headers = ["ID", "Name", "Status", "Origin"];
  const rows = tasks.map((t) => [t.id, t.name, t.status, t.origin]);
  return renderTable(headers, rows);
}
