// Re-runs the CLI with HERDR_PANE_ID removed from the child's environment while
// this process keeps it: the shape Codex produced for the Observer in lab g9,
// where the pane identity has to come from an ancestor (d770). The parent must
// be a third-party binary such as bun; macOS ps hides the environment of Apple
// platform binaries (sh, zsh, sleep), so a shell parent would not be seen.
const [cli, ...args] = process.argv.slice(2);
const env = { ...process.env };
delete env.HERDR_PANE_ID;
const child = Bun.spawnSync([process.execPath, cli as string, ...args], {
  env,
  stdin: "inherit",
  stdout: "inherit",
  stderr: "inherit",
});
process.exit(child.exitCode ?? 1);
