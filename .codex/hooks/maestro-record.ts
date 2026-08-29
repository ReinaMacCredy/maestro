#!/usr/bin/env bun
const raw = await Bun.stdin.text();
const input = raw.trim() ? JSON.parse(raw) : {};
const event = typeof input.hook_event_name === "string" ? input.hook_event_name : "SessionStart";
const sessionId = typeof input.session_id === "string" ? input.session_id : undefined;
const child = Bun.spawn(["maestro", "hook", "record", "--event", event, "--harness", "codex"], {
  cwd: typeof input.cwd === "string" ? input.cwd : process.cwd(),
  env: { ...process.env, ...(sessionId ? { MAESTRO_SESSION_ID: sessionId } : {}) },
  stdin: new TextEncoder().encode(raw),
  stdout: "pipe",
  stderr: "pipe",
});
const [stdout, stderr, exitCode] = await Promise.all([
  new Response(child.stdout).text(),
  new Response(child.stderr).text(),
  child.exited,
]);
if (stdout) process.stdout.write(stdout);
if (stderr) process.stderr.write(stderr);
process.exitCode = exitCode;
