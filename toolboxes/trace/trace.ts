#!/usr/bin/env node
import { Server } from "@modelcontextprotocol/sdk/server/index.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import {
  CallToolRequestSchema,
  ListToolsRequestSchema,
} from "@modelcontextprotocol/sdk/types.js";
import { readFileSync, existsSync } from "fs";
import { join } from "path";

const TRACE_FILE = join(process.cwd(), ".maestro", "trace.jsonl");

interface TraceEvent {
  timestamp: string;
  event_type: string;
  tool_name: string;
  agent_name: string;
  success: boolean;
  summary: string;
}

function readTraceEvents(): TraceEvent[] {
  if (!existsSync(TRACE_FILE)) return [];
  const content = readFileSync(TRACE_FILE, "utf-8").trim();
  if (!content) return [];
  return content
    .split("\n")
    .map((line) => {
      try {
        return JSON.parse(line) as TraceEvent;
      } catch {
        return null;
      }
    })
    .filter((e): e is TraceEvent => e !== null);
}

function formatTimeline(
  events: TraceEvent[],
  filter?: string,
  last?: number
): string {
  let filtered = events;
  if (filter) {
    filtered = events.filter(
      (e) =>
        e.tool_name.includes(filter) ||
        e.agent_name.includes(filter) ||
        e.event_type.includes(filter)
    );
  }
  if (last && last > 0) {
    filtered = filtered.slice(-last);
  }
  if (filtered.length === 0) return "No trace events found.";

  const lines = filtered.map((e) => {
    const status = e.success ? "OK" : "FAIL";
    const summary = e.summary ? ` — ${e.summary}` : "";
    return `[${e.timestamp}] ${status} ${e.agent_name}/${e.tool_name}${summary}`;
  });
  return `## Trace Timeline (${filtered.length} events)\n\n` + lines.join("\n");
}

function formatSummary(events: TraceEvent[]): string {
  if (events.length === 0) return "No trace events to summarize.";

  const byTool: Record<string, number> = {};
  const byAgent: Record<string, number> = {};
  let successes = 0;
  let failures = 0;

  for (const e of events) {
    byTool[e.tool_name] = (byTool[e.tool_name] || 0) + 1;
    byAgent[e.agent_name] = (byAgent[e.agent_name] || 0) + 1;
    if (e.success) successes++;
    else failures++;
  }

  const toolLines = Object.entries(byTool)
    .sort((a, b) => b[1] - a[1])
    .map(([name, count]) => `  ${name}: ${count}`)
    .join("\n");

  const agentLines = Object.entries(byAgent)
    .sort((a, b) => b[1] - a[1])
    .map(([name, count]) => `  ${name}: ${count}`)
    .join("\n");

  return [
    `## Trace Summary`,
    ``,
    `**Total events**: ${events.length}`,
    `**Success**: ${successes} | **Failure**: ${failures}`,
    ``,
    `### By Tool`,
    toolLines,
    ``,
    `### By Agent`,
    agentLines,
  ].join("\n");
}

const server = new Server(
  { name: "trace", version: "1.0.0" },
  { capabilities: { tools: {} } }
);

server.setRequestHandler(ListToolsRequestSchema, async () => ({
  tools: [
    {
      name: "trace_timeline",
      description:
        "Show chronological timeline of tool events from .maestro/trace.jsonl",
      inputSchema: {
        type: "object" as const,
        properties: {
          filter: {
            type: "string",
            description:
              "Filter by tool_name, agent_name, or event_type (substring match)",
          },
          last: {
            type: "number",
            description: "Limit to N most recent events",
          },
        },
      },
    },
    {
      name: "trace_summary",
      description: "Show aggregate statistics from .maestro/trace.jsonl",
      inputSchema: {
        type: "object" as const,
        properties: {},
      },
    },
  ],
}));

server.setRequestHandler(CallToolRequestSchema, async (request) => {
  const { name, arguments: args } = request.params;
  const events = readTraceEvents();

  if (name === "trace_timeline") {
    const filter = (args?.filter as string) || undefined;
    const last = (args?.last as number) || undefined;
    return {
      content: [
        { type: "text" as const, text: formatTimeline(events, filter, last) },
      ],
    };
  }

  if (name === "trace_summary") {
    return {
      content: [{ type: "text" as const, text: formatSummary(events) }],
    };
  }

  return {
    content: [{ type: "text" as const, text: `Unknown tool: ${name}` }],
    isError: true,
  };
});

async function main() {
  const transport = new StdioServerTransport();
  await server.connect(transport);
}

main().catch(console.error);
