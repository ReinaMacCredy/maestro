import type { CassSearchResponse, CassSearchResult } from "../domain/types.js";
import type { CassPort } from "../ports/cass.port.js";
import { exec } from "../lib/shell.js";
import { warn } from "../lib/output.js";

export class ShellCassAdapter implements CassPort {
  private readonly cassPath: string;

  constructor(cassPath?: string) {
    this.cassPath = cassPath ?? "cass";
  }

  async isAvailable(): Promise<boolean> {
    const result = await exec(`${this.cassPath} health --json`);
    return result.exitCode === 0;
  }

  async indexOnce(sessionPaths: readonly string[]): Promise<void> {
    if (sessionPaths.length === 0) return;
    const paths = sessionPaths.join(",");
    const result = await exec(
      `${this.cassPath} index --watch-once ${paths} --json`,
      { timeout: 60_000 },
    );
    if (result.exitCode !== 0) {
      warn(`CASS indexing failed: ${result.stderr}`);
    }
  }

  async search(
    query: string,
    options: {
      agent?: string;
      workspace?: string;
      limit?: number;
    },
  ): Promise<CassSearchResponse> {
    const args = [`search`, `"${escapeShell(query)}"`, "--json"];

    if (options.agent) {
      args.push("--agent", options.agent);
    }
    if (options.workspace) {
      args.push("--workspace", options.workspace);
    }
    args.push("--limit", String(options.limit ?? 10));

    const result = await exec(`${this.cassPath} ${args.join(" ")}`);

    if (result.exitCode !== 0) {
      return { query, count: 0, totalMatches: 0, hits: [] };
    }

    return parseCassSearchOutput(query, result.stdout);
  }
}

function escapeShell(s: string): string {
  return s.replace(/'/g, "'\\''");
}

function parseCassSearchOutput(
  query: string,
  stdout: string,
): CassSearchResponse {
  try {
    const raw = JSON.parse(stdout) as {
      count?: number;
      total_matches?: number;
      hits?: Array<{
        title?: string;
        snippet?: string;
        content?: string;
        score?: number;
        source_path?: string;
        agent?: string;
        line_number?: number;
        created_at?: number;
      }>;
    };

    const hits: CassSearchResult[] = (raw.hits ?? []).map((h) => ({
      title: h.title ?? "",
      snippet: h.snippet ?? "",
      content: h.content ?? "",
      score: h.score ?? 0,
      sourcePath: h.source_path ?? "",
      agent: h.agent ?? "",
      lineNumber: h.line_number ?? 0,
      createdAt: h.created_at ?? 0,
    }));

    return {
      query,
      count: raw.count ?? hits.length,
      totalMatches: raw.total_matches ?? hits.length,
      hits,
    };
  } catch {
    return { query, count: 0, totalMatches: 0, hits: [] };
  }
}
