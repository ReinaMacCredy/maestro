export interface AgentCardDocument {
  readonly url?: string;
  readonly additionalInterfaces?: readonly {
    readonly transport?: string;
    readonly url: string;
  }[];
}

export async function fetchA2aAgentCard(
  baseUrl: string,
  options: {
    readonly agentCardPath?: string;
    readonly headers?: Readonly<Record<string, string>>;
    readonly signal?: AbortSignal;
  } = {},
): Promise<AgentCardDocument> {
  const cardUrl = new URL(
    options.agentCardPath ?? "/.well-known/agent-card.json",
    ensureTrailingSlash(baseUrl),
  ).toString();
  const response = await fetch(cardUrl, {
    headers: options.headers,
    signal: options.signal,
  });
  if (!response.ok) {
    throw new Error(`Failed to fetch agent card from ${cardUrl}: ${response.status}`);
  }

  return await response.json() as AgentCardDocument;
}

export function resolveA2aJsonRpcEndpoint(baseUrl: string, agentCard: AgentCardDocument): string {
  const jsonRpcInterface = agentCard.additionalInterfaces?.find((entry) => entry.transport === "JSONRPC");
  const endpoint = jsonRpcInterface?.url ?? agentCard.url;
  if (!endpoint) {
    throw new Error("Agent card does not expose a JSON-RPC endpoint");
  }

  return new URL(endpoint, ensureTrailingSlash(baseUrl)).toString();
}

function ensureTrailingSlash(value: string): string {
  return value.endsWith("/") ? value : `${value}/`;
}
