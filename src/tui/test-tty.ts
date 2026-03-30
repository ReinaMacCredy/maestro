#!/usr/bin/env bun
/**
 * Minimal TTY test -- enter alt screen, show message, wait for q, exit.
 * Run: bun run src/tui/test-tty.ts
 * Or compiled: bun build src/tui/test-tty.ts --compile --outfile /tmp/test-tty && /tmp/test-tty
 */
const ESC = "\x1b";

process.stdout.write(`${ESC}[?1049h${ESC}[?25l`); // alt screen + hide cursor
process.stdout.write(`${ESC}[H`); // home
process.stdout.write("Minimal TTY test. Press q to quit.\r\n");
process.stdout.write(`stdout.isTTY=${process.stdout.isTTY} stdin.isTTY=${process.stdin.isTTY}\r\n`);
process.stdout.write(`cols=${process.stdout.columns} rows=${process.stdout.rows}\r\n`);

if (process.stdin.isTTY && typeof process.stdin.setRawMode === "function") {
  process.stdin.setRawMode(true);
  process.stdin.resume();
  process.stdout.write("Raw mode enabled. Listening for keys...\r\n");
} else {
  process.stdout.write("Not a TTY -- raw mode unavailable.\r\n");
}

process.stdin.on("data", (data: Buffer) => {
  const byte = data[0];
  if (byte === 0x71) { // 'q'
    process.stdout.write(`\r\n${ESC}[?25h${ESC}[?1049l`); // show cursor + exit alt
    if (process.stdin.isTTY) process.stdin.setRawMode(false);
    process.exit(0);
  }
  process.stdout.write(`Key: 0x${byte?.toString(16).padStart(2, "0")}\r\n`);
});
