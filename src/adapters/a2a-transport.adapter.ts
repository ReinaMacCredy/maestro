import type { A2aWorkerConfig, WorkerResult } from "../domain/worker-types.js";
import { fetchA2aAgentCard, resolveA2aJsonRpcEndpoint } from "../lib/a2a.js";
import type { TransportPort } from "../ports/transport.port.js";

export class A2aTransportAdapter implements TransportPort {
  async spawn(
    workerConfig: A2aWorkerConfig,
    prompt: string,
    opts: {
      cwd: string;
      featureId: string;
      missionId: string;
      workerSlug: string;
      onEvent?: (event: {
        timestamp: string;
        kind: "status" | "stdout" | "stderr" | "heartbeat";
        worker: string;
        text?: string;
        sessionId?: string;
        runtimeState?: "starting" | "live" | "stale" | "failed" | "recoverable" | "completed";
      }) => void | Promise<void>;
    },
  ): Promise<WorkerResult> {
    const startedAt = Date.now();
    const rawEvents: string[] = [];
    const outputChunks: string[] = [];
    let finalState: string | undefined;
    let sessionId: string | undefined;
    let heartbeat: ReturnType<typeof setInterval> | undefined;

    const emitEvent = async (
      event: {
        timestamp: string;
        kind: "status" | "stdout" | "stderr" | "heartbeat";
        worker: string;
        text?: string;
        sessionId?: string;
        runtimeState?: "starting" | "live" | "stale" | "failed" | "recoverable" | "completed";
      },
    ): Promise<void> => {
      await opts.onEvent?.({
        ...event,
        sessionId: event.sessionId ?? sessionId,
      });
    };

    try {
      await emitEvent({
        timestamp: new Date().toISOString(),
        kind: "status",
        worker: opts.workerSlug,
        runtimeState: "starting",
        text: `Connecting to ${opts.workerSlug}`,
      });

      const agentCard = await fetchA2aAgentCard(workerConfig.url, {
        agentCardPath: workerConfig.agentCardPath,
        headers: workerConfig.headers,
      });
      const endpoint = resolveA2aJsonRpcEndpoint(workerConfig.url, agentCard);
      heartbeat = setInterval(() => {
        void emitEvent({
          timestamp: new Date().toISOString(),
          kind: "heartbeat",
          worker: opts.workerSlug,
          runtimeState: "live",
        });
      }, 15_000);
      const streamResponse = await fetch(endpoint, {
        method: "POST",
        headers: {
          Accept: "text/event-stream",
          "Content-Type": "application/json",
          ...workerConfig.headers,
        },
        body: JSON.stringify({
          jsonrpc: "2.0",
          id: crypto.randomUUID(),
          method: "message/stream",
          params: {
            message: {
              kind: "message",
              messageId: crypto.randomUUID(),
              role: "user",
              parts: [{ kind: "text", text: prompt }],
            },
          },
        }),
      });
      assertSseResponse(streamResponse, endpoint);

      for await (const event of parseJsonRpcSseStream(streamResponse)) {
        rawEvents.push(JSON.stringify(event));
        sessionId = extractRemoteHandle(event) ?? sessionId;
        const eventTexts = extractEventText(event);
        outputChunks.push(...eventTexts);
        if (event.kind === "task") {
          finalState = event.status.state;
        }
        if (event.kind === "status-update") {
          finalState = event.status.state;
        }
        if (finalState) {
          await emitEvent({
            timestamp: new Date().toISOString(),
            kind: "status",
            worker: opts.workerSlug,
            runtimeState: mapA2aRuntimeState(finalState),
            text: `${opts.workerSlug} task state ${finalState}`,
          });
        }
        for (const text of eventTexts) {
          const trimmed = text.trim();
          if (trimmed.length === 0) continue;
          await emitEvent({
            timestamp: new Date().toISOString(),
            kind: "stdout",
            worker: opts.workerSlug,
            runtimeState: "live",
            text: trimmed,
          });
        }
      }

      const parsedOutput = outputChunks.join("\n").trim();
      const success = finalState === undefined
        ? parsedOutput.length > 0
        : finalState === "completed";

      await emitEvent({
        timestamp: new Date().toISOString(),
        kind: "status",
        worker: opts.workerSlug,
        runtimeState: success ? "completed" : mapA2aRuntimeState(finalState),
        text: success
          ? `${opts.workerSlug} completed`
          : `${opts.workerSlug} finished with task state ${finalState ?? "unknown"}`,
      });

      return {
        success,
        exitCode: success ? 0 : 1,
        summary: summarizeA2aResult(parsedOutput, finalState, opts.workerSlug),
        stdoutRaw: rawEvents.join("\n"),
        stderrRaw: "",
        filesChanged: [],
        durationMs: Date.now() - startedAt,
        parsedOutput,
        failureClass: success ? undefined : classifyFailure(finalState),
      };
    } catch (error) {
      finalState = finalState ?? "failed";
      const detail = error instanceof Error ? error.message : String(error);
      await emitEvent({
        timestamp: new Date().toISOString(),
        kind: "status",
        worker: opts.workerSlug,
        runtimeState: "failed",
        text: detail,
      });
      return {
        success: false,
        exitCode: 1,
        summary: `Failed to communicate with A2A worker '${opts.workerSlug}': ${detail}`,
        stdoutRaw: rawEvents.join("\n"),
        stderrRaw: detail,
        filesChanged: [],
        durationMs: Date.now() - startedAt,
        failureClass: "infrastructure",
      };
    } finally {
      if (heartbeat) {
        clearInterval(heartbeat);
      }
    }
  }
}

function mapA2aRuntimeState(
  finalState: string | undefined,
): "starting" | "live" | "stale" | "failed" | "recoverable" | "completed" {
  switch (finalState) {
    case "submitted":
      return "starting";
    case "completed":
      return "completed";
    case "failed":
    case "rejected":
    case "canceled":
      return "failed";
    default:
      return "live";
  }
}

function extractEventText(event: A2aEvent): readonly string[] {
  if (event.kind === "message") {
    return extractPartTexts(event.parts);
  }

  if (event.kind === "task") {
    return (event.artifacts ?? []).flatMap((artifact) => extractPartTexts(artifact.parts));
  }

  if (event.kind === "artifact-update") {
    return extractPartTexts(event.artifact.parts);
  }

  return [];
}

function extractRemoteHandle(event: A2aEvent): string | undefined {
  if (event.kind === "task") {
    return event.id ?? event.contextId;
  }

  if (event.kind === "status-update" || event.kind === "artifact-update") {
    return event.taskId ?? event.contextId;
  }

  return undefined;
}

function extractPartTexts(parts: readonly unknown[]): readonly string[] {
  return parts.flatMap((part) => {
    if (typeof part !== "object" || part === null) {
      return [];
    }

    const maybeText = part as { kind?: unknown; text?: unknown };
    return maybeText.kind === "text" && typeof maybeText.text === "string"
      ? [maybeText.text]
      : [];
  });
}

function summarizeA2aResult(parsedOutput: string, finalState: string | undefined, workerSlug: string): string {
  const firstLine = parsedOutput
    .split(/\r?\n/)
    .map((line) => line.trim())
    .find((line) => line.length > 0);

  if (firstLine) {
    return firstLine;
  }

  if (finalState) {
    return `${workerSlug} finished with task state ${finalState}`;
  }

  return `${workerSlug} completed without textual output`;
}

function classifyFailure(finalState: string | undefined): WorkerResult["failureClass"] {
  switch (finalState) {
    case "input-required":
    case "auth-required":
      return "validation";
    case "failed":
    case "rejected":
    case "canceled":
      return "worker-crash";
    case "unknown":
    default:
      return "unknown";
  }
}

interface A2aMessage {
  readonly kind: "message";
  readonly parts: readonly unknown[];
}

interface A2aTask {
  readonly kind: "task";
  readonly id?: string;
  readonly contextId?: string;
  readonly status: {
    readonly state: string;
  };
  readonly artifacts?: readonly {
    readonly parts: readonly unknown[];
  }[];
}

interface A2aStatusUpdate {
  readonly kind: "status-update";
  readonly taskId?: string;
  readonly contextId?: string;
  readonly status: {
    readonly state: string;
  };
}

interface A2aArtifactUpdate {
  readonly kind: "artifact-update";
  readonly taskId?: string;
  readonly contextId?: string;
  readonly artifact: {
    readonly parts: readonly unknown[];
  };
}

type A2aEvent = A2aMessage | A2aTask | A2aStatusUpdate | A2aArtifactUpdate;

function assertSseResponse(response: Response, endpoint: string): void {
  if (!response.ok) {
    throw new Error(`A2A stream request failed for ${endpoint}: ${response.status}`);
  }

  const contentType = response.headers.get("content-type");
  if (!contentType?.startsWith("text/event-stream")) {
    throw new Error(`A2A stream request returned unexpected content-type: ${contentType ?? "missing"}`);
  }
}

async function* parseJsonRpcSseStream(response: Response): AsyncGenerator<A2aEvent, void, undefined> {
  if (!response.body) {
    throw new Error("A2A stream response did not include a body");
  }

  let buffer = "";
  let eventType = "message";
  let eventData = "";
  const stream = response.body.pipeThrough(new TextDecoderStream());

  for await (const chunk of readFrom(stream)) {
    buffer += chunk;

    while (true) {
      const lineBreakIndex = buffer.indexOf("\n");
      if (lineBreakIndex < 0) {
        break;
      }

      const line = buffer.slice(0, lineBreakIndex).trim();
      buffer = buffer.slice(lineBreakIndex + 1);

      if (line === "") {
        if (eventData.length > 0) {
          yield parseEventPayload(eventType, eventData);
          eventType = "message";
          eventData = "";
        }
        continue;
      }

      if (line.startsWith("event:")) {
        eventType = line.slice("event:".length).trim();
        continue;
      }

      if (line.startsWith("data:")) {
        eventData = line.slice("data:".length).trim();
      }
    }
  }

  if (eventData.length > 0) {
    yield parseEventPayload(eventType, eventData);
  }
}

async function* readFrom(stream: ReadableStream<string>): AsyncGenerator<string, void, undefined> {
  const reader = stream.getReader();

  try {
    while (true) {
      const { done, value } = await reader.read();
      if (done) {
        return;
      }

      yield value;
    }
  } finally {
    reader.releaseLock();
  }
}

function parseEventPayload(eventType: string, eventData: string): A2aEvent {
  const payload = JSON.parse(eventData) as {
    readonly result?: A2aEvent;
    readonly error?: {
      readonly message?: string;
    };
  };

  if (eventType === "error" || payload.error) {
    throw new Error(payload.error?.message ?? "A2A stream emitted an error event");
  }

  if (!payload.result) {
    throw new Error("A2A stream event did not include a result payload");
  }

  return payload.result;
}
