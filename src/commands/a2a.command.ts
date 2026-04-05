import type { Command } from "commander";
import { startA2aDemoServer } from "../lib/a2a-demo-server.js";
import { resolveJsonFlag } from "../lib/output.js";

// [WIP] A2A command group -- demo server only; feature execution still requires execution.allowA2a
export function registerA2aCommand(program: Command): void {
  const a2aCmd = program
    .command("a2a")
    .description("A2A transport tools (under development)");

  a2aCmd
    .command("serve-demo")
    .description("Start a local streaming A2A demo worker you can watch in Mission Control")
    .option("--host <host>", "Host to bind", "127.0.0.1")
    .option("--public", "Allow binding the demo server to a non-loopback host")
    .option("--port <port>", "Port to bind (0 chooses a random free port)", parseInteger, 4123)
    .option("--delay-ms <ms>", "Delay between streamed demo updates", parseInteger, 1500)
    .option("--step <text>", "Artifact text chunk to stream in order", collectValues, [])
    .option("--message <text>", "Final completion text", "A2A demo worker completed the task.")
    .option("--json", "Output startup info as JSON")
    .addHelpText("after", `
Examples:
  maestro a2a serve-demo
  maestro a2a serve-demo --port 0 --delay-ms 800
  maestro a2a serve-demo --step "Reading mission" --step "Applying patch" --step "Done"
`)
    .action(async (opts) => {
      if (!isLoopbackHost(opts.host as string) && opts.public !== true) {
        throw new Error("Refusing to bind demo A2A server to a non-loopback host without --public");
      }

      const isJson = resolveJsonFlag(opts, program);
      const server = await startA2aDemoServer({
        host: opts.host as string,
        port: opts.port as number,
        delayMs: opts.delayMs as number,
        steps: ((opts.step as string[] | undefined) ?? []).map((value) => value.trim()).filter((value) => value.length > 0),
        finalMessage: opts.message as string,
      });

        const payload = {
        baseUrl: server.baseUrl,
        agentCardUrl: server.agentCardUrl,
        jsonRpcUrl: server.jsonRpcUrl,
        configSnippet: [
          "execution:",
          "  defaultWorker: demo-a2a",
          "  allowA2a: true",
          "workers:",
          "  demo-a2a:",
          "    enabled: true",
          "    transport: a2a",
          `    url: ${server.baseUrl}`,
        ].join("\n"),
      };

      if (isJson) {
        console.log(JSON.stringify(payload));
      } else {
        console.log("[ok] A2A demo worker ready");
        console.log(`  Base URL: ${payload.baseUrl}`);
        console.log(`  Agent card: ${payload.agentCardUrl}`);
        console.log(`  JSON-RPC: ${payload.jsonRpcUrl}`);
        console.log("");
        console.log("Add this to .maestro/config.yaml:");
        console.log(payload.configSnippet);
        console.log("");
        console.log("Watch it live:");
        console.log("  1. Run `maestro feature run --mission <id> --worker demo-a2a`");
        console.log("  2. In another terminal, run `maestro mission-control --mission <id>`");
        console.log("");
        console.log("Press Ctrl+C to stop the demo server.");
      }

      await waitForShutdown(server.close);
    });
}

function parseInteger(value: string): number {
  const parsed = Number.parseInt(value, 10);
  if (!Number.isFinite(parsed) || Number.isNaN(parsed)) {
    throw new Error(`Expected an integer but received '${value}'`);
  }
  return parsed;
}

function collectValues(value: string, previous: string[]): string[] {
  return [...previous, value];
}

function isLoopbackHost(host: string): boolean {
  return host === "127.0.0.1" || host === "localhost" || host === "::1";
}

async function waitForShutdown(close: () => Promise<void>): Promise<void> {
  await new Promise<void>((resolve, reject) => {
    let closed = false;

    const shutdown = () => {
      if (closed) return;
      closed = true;
      void close()
        .then(resolve)
        .catch(reject)
        .finally(() => {
          process.off("SIGINT", shutdown);
          process.off("SIGTERM", shutdown);
        });
    };

    process.on("SIGINT", shutdown);
    process.on("SIGTERM", shutdown);
  });
}
