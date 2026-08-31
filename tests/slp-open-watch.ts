#!/usr/bin/env bun

export {};

const [supervisorPaneId, projectPath, teamId, generation, workspaceId, watchEntry] =
  process.argv.slice(2);
if (!supervisorPaneId || !projectPath || !teamId || !generation || !workspaceId || !watchEntry) {
  process.stderr.write(
    "usage: slp-open-watch <supervisor-pane> <project> <team> <generation> <workspace> <watch-entry>\n",
  );
  process.exit(2);
}

async function herdr(args: string[], allowEmpty = false): Promise<Record<string, unknown>> {
  const child = Bun.spawn(["herdr", ...args], {
    cwd: projectPath,
    env: process.env,
    stderr: "pipe",
    stdout: "pipe",
  });
  const [stdout, stderr, exitCode] = await Promise.all([
    new Response(child.stdout).text(),
    new Response(child.stderr).text(),
    child.exited,
  ]);
  if (exitCode !== 0) throw new Error(`herdr ${args.join(" ")} failed: ${stderr.trim()}`);
  if (allowEmpty && stdout.trim() === "") return {};
  try {
    return JSON.parse(stdout) as Record<string, unknown>;
  } catch {
    throw new Error(
      `herdr ${args.join(" ")} returned invalid JSON: stdout=${JSON.stringify(stdout)} ` +
        `stderr=${JSON.stringify(stderr)}`,
    );
  }
}

const split = await herdr([
  "pane",
  "split",
  "--pane",
  supervisorPaneId,
  "--direction",
  "right",
  "--cwd",
  projectPath,
  "--env",
  `HERDR_WORKSPACE_ID=${workspaceId}`,
  "--no-focus",
]);
const result = split.result as Record<string, unknown> | undefined;
const pane = result?.pane as Record<string, unknown> | undefined;
const paneId = pane?.pane_id;
if (typeof paneId !== "string") throw new Error("Herdr pane split returned no pane id");
await herdr([
  "pane",
  "run",
  paneId,
  process.execPath,
  watchEntry,
  "--team",
  teamId,
  "--generation",
  generation,
  "--interval-ms",
  "100",
], true);
process.stdout.write(`${JSON.stringify({ paneId })}\n`);
