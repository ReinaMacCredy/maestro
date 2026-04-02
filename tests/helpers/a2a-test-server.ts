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

export interface TestA2aServer {
  readonly baseUrl: string;
  close(): Promise<void>;
}

export async function startA2aTestServer(responseText: string): Promise<TestA2aServer> {
  const app = express();
  const executor: AgentExecutor = {
    execute: async (requestContext: RequestContext, eventBus: ExecutionEventBus) => {
      const timestamp = new Date().toISOString();

      eventBus.publish({
        kind: "task",
        id: requestContext.taskId,
        contextId: requestContext.contextId,
        status: {
          state: "submitted",
          timestamp,
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
          timestamp,
        },
        final: false,
      });
      eventBus.publish({
        kind: "artifact-update",
        taskId: requestContext.taskId,
        contextId: requestContext.contextId,
        artifact: {
          artifactId: "result",
          name: "result.txt",
          parts: [{ kind: "text", text: responseText }],
        },
      });
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

  let server: Server | undefined;
  const baseUrl = await new Promise<string>((resolve, reject) => {
    server = app.listen(0, "127.0.0.1", () => {
      const address = server?.address();
      if (!address || typeof address === "string") {
        reject(new Error("Failed to start test A2A server"));
        return;
      }

      const card: AgentCard = {
        name: "Test A2A Worker",
        description: "A local test worker for Maestro A2A integration coverage.",
        protocolVersion: "0.3.0",
        version: "0.1.0",
        url: `http://127.0.0.1:${address.port}/a2a/jsonrpc`,
        skills: [{
          id: "implement",
          name: "Implement",
          description: "Completes the submitted task.",
          tags: ["coding"],
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
      resolve(`http://127.0.0.1:${address.port}`);
    });
    server?.on("error", reject);
  });

  return {
    baseUrl,
    close: async () => {
      await new Promise<void>((resolve, reject) => {
        server?.close((error) => {
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
