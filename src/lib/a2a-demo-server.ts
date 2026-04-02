import type { Server } from "node:http";
import express from "express";
import { AGENT_CARD_PATH, type AgentCard } from "@a2a-js/sdk";
import {
  DefaultRequestHandler,
  InMemoryTaskStore,
  type AgentExecutor,
  type ExecutionEventBus,
  type RequestContext,
} from "@a2a-js/sdk/server";
import { agentCardHandler, jsonRpcHandler, UserBuilder } from "@a2a-js/sdk/server/express";

export interface A2aDemoServerOptions {
  readonly host: string;
  readonly port: number;
  readonly delayMs: number;
  readonly steps: readonly string[];
  readonly finalMessage: string;
}

export interface A2aDemoServerHandle {
  readonly baseUrl: string;
  readonly agentCardUrl: string;
  readonly jsonRpcUrl: string;
  close(): Promise<void>;
}

export async function startA2aDemoServer(
  options: A2aDemoServerOptions,
): Promise<A2aDemoServerHandle> {
  const app = express();
  const executor: AgentExecutor = {
    execute: async (requestContext: RequestContext, eventBus: ExecutionEventBus) => {
      const submittedAt = new Date().toISOString();

      eventBus.publish({
        kind: "task",
        id: requestContext.taskId,
        contextId: requestContext.contextId,
        status: {
          state: "submitted",
          timestamp: submittedAt,
        },
        history: [requestContext.userMessage],
        artifacts: [],
      });

      eventBus.publish({
        kind: "status-update",
        taskId: requestContext.taskId,
        contextId: requestContext.contextId,
        status: {
          state: "working",
          timestamp: submittedAt,
        },
        final: false,
      });

      const artifactParts = options.steps.length > 0
        ? options.steps
        : [
            "Reading mission context",
            "Planning the implementation",
            options.finalMessage,
          ];

      for (const part of artifactParts) {
        await sleep(options.delayMs);
        eventBus.publish({
          kind: "artifact-update",
          taskId: requestContext.taskId,
          contextId: requestContext.contextId,
          artifact: {
            artifactId: "result",
            name: "result.txt",
            parts: [{ kind: "text", text: part }],
          },
        });
      }

      await sleep(options.delayMs);
      eventBus.publish({
        kind: "status-update",
        taskId: requestContext.taskId,
        contextId: requestContext.contextId,
        status: {
          state: "completed",
          timestamp: new Date().toISOString(),
        },
        final: true,
      });
      eventBus.finished();
    },
    cancelTask: async () => {},
  };

  const serverInfo = await new Promise<{
    server: Server;
    baseUrl: string;
    agentCardUrl: string;
    jsonRpcUrl: string;
  }>((resolve, reject) => {
    let server: Server | undefined;

    server = app.listen(options.port, options.host, () => {
      const address = server?.address();
      if (!address || typeof address === "string") {
        reject(new Error("Failed to start A2A demo server"));
        return;
      }

      const baseUrl = `http://${options.host}:${address.port}`;
      const jsonRpcUrl = `${baseUrl}/a2a/jsonrpc`;
      const agentCardUrl = `${baseUrl}/${AGENT_CARD_PATH}`;
      const card: AgentCard = {
        name: "Maestro A2A Demo Worker",
        description: "A local streaming A2A worker for watching live Mission Control updates.",
        protocolVersion: "0.3.0",
        version: "0.1.0",
        url: jsonRpcUrl,
        skills: [{
          id: "implement",
          name: "Implement",
          description: "Streams a few demo progress artifacts before completing the task.",
          tags: ["demo", "coding"],
        }],
        capabilities: {
          streaming: true,
          pushNotifications: false,
        },
        defaultInputModes: ["text"],
        defaultOutputModes: ["text"],
      };

      const requestHandler = new DefaultRequestHandler(card, new InMemoryTaskStore(), executor);
      app.use(`/${AGENT_CARD_PATH}`, agentCardHandler({ agentCardProvider: requestHandler }));
      app.use("/a2a/jsonrpc", jsonRpcHandler({ requestHandler, userBuilder: UserBuilder.noAuthentication }));

      resolve({
        server: server!,
        baseUrl,
        agentCardUrl,
        jsonRpcUrl,
      });
    });

    server.on("error", reject);
  });

  return {
    baseUrl: serverInfo.baseUrl,
    agentCardUrl: serverInfo.agentCardUrl,
    jsonRpcUrl: serverInfo.jsonRpcUrl,
    close: async () => {
      await new Promise<void>((resolve, reject) => {
        serverInfo.server.close((error) => {
          if (error) {
            reject(error);
            return;
          }

          resolve();
        });
      });
    },
  };
}

async function sleep(ms: number): Promise<void> {
  await new Promise((resolve) => setTimeout(resolve, ms));
}
