/**
 * AgentMailHandoffAdapter -- HandoffPort implementation.
 *
 * Writes handoff documents to local files (.maestro/features/<name>/handoffs/)
 * as the primary artifact, then sends via Agent Mail HTTP API as notification.
 * File is always written; Agent Mail is best-effort.
 */

import type { HandoffPort, HandoffDocument, HandoffResult } from '../../../../../domain/ports/handoff.ts';
import type { TaskPort, RichTaskFields } from '../../../../../domain/ports/task.ts';
import type { MemoryPort } from '../../../../../domain/ports/memory.ts';
import type { SettingsPort } from '../../../../../domain/ports/settings.ts';
import { selectMemories } from '../../../../../app/dcp/selector.ts';
import { scoreByGoal } from '../../../../../app/handoff/scorer.ts';
import { resolveDcpConfig } from '../../../../../app/dcp/config.ts';
import { getHandoffPath, getHandoffsPath } from '../../../../utils/paths.ts';
import { ensureDir, writeText, readText } from '../../../../utils/fs-io.ts';
import { getModifiedFiles, extractTitle, formatHandoffMessage } from '../../../../adapters/handoff/shared.ts';
import { HttpTransport } from '../../../sdk/http-transport.ts';
import * as fs from 'node:fs';
import * as path from 'node:path';
import type { AdapterContext, AdapterFactory } from '../../../types.ts';

const DEFAULT_AGENT_MAIL_URL = 'http://localhost:8765';
const ALLOWED_SCHEMES = new Set(['http:', 'https:']);
const LOCALHOST_HOSTS = new Set(['localhost', '127.0.0.1', '[::1]']);

/**
 * Validate an Agent Mail base URL: must be http/https, and warn if a bearer
 * token will be sent to a non-localhost endpoint.
 */
function validateAgentMailUrl(baseUrl: string, hasBearerToken: boolean): string {
  let parsed: URL;
  try {
    parsed = new URL(baseUrl);
  } catch {
    throw new Error(`Invalid AGENT_MAIL_URL: "${baseUrl}" is not a valid URL`);
  }

  if (!ALLOWED_SCHEMES.has(parsed.protocol)) {
    throw new Error(
      `Unsupported AGENT_MAIL_URL scheme "${parsed.protocol}" -- only http: and https: are allowed`,
    );
  }

  if (hasBearerToken && !LOCALHOST_HOSTS.has(parsed.hostname)) {
    console.warn(
      `[!] Agent Mail: bearer token will be sent to non-localhost URL "${parsed.origin}". ` +
      'Verify this endpoint is trusted.',
    );
  }

  return baseUrl;
}

interface AgentMailIdentity {
  agentName: string;
  projectSlug: string;
}

export class AgentMailHandoffAdapter implements HandoffPort {
  private baseUrl: string;
  private projectRoot: string;
  private taskPort: TaskPort;
  private memoryAdapter: MemoryPort;
  private settingsPort: SettingsPort;
  private taskBackend: 'fs' | 'br';
  private transport: HttpTransport;
  private identity: AgentMailIdentity | undefined;

  constructor(
    projectRoot: string,
    taskPort: TaskPort,
    memoryAdapter: MemoryPort,
    settingsPort: SettingsPort,
    taskBackend: 'fs' | 'br',
    agentMailUrl?: string,
    transport?: HttpTransport,
  ) {
    this.projectRoot = projectRoot;
    this.taskPort = taskPort;
    this.memoryAdapter = memoryAdapter;
    this.settingsPort = settingsPort;
    this.taskBackend = taskBackend;
    const hasBearerToken = !!process.env.HTTP_BEARER_TOKEN;
    this.baseUrl = validateAgentMailUrl(
      agentMailUrl ?? process.env.AGENT_MAIL_URL ?? DEFAULT_AGENT_MAIL_URL,
      hasBearerToken,
    );
    this.transport = transport ?? new HttpTransport({
      baseUrl: this.baseUrl,
      timeout: 5000,
      bestEffort: true,
      retryDelays: [],
      authHeaders: process.env.HTTP_BEARER_TOKEN
        ? { Authorization: `Bearer ${process.env.HTTP_BEARER_TOKEN}` }
        : undefined,
    });
  }

  async buildHandoff(feature: string, taskId: string, goal?: string): Promise<HandoffDocument> {
    const task = await this.taskPort.get(feature, taskId);
    const richFields: RichTaskFields | null = this.taskPort.getRichFields
      ? await this.taskPort.getRichFields(feature, taskId)
      : null;

    const cfg = resolveDcpConfig(this.settingsPort.get().dcp);

    let decisions: Array<{ key: string; value: string }>;

    if (goal && cfg.enabled) {
      // Goal-based scoring: rank memories by relevance to the handoff goal
      const allMemories = this.memoryAdapter.listWithMeta(feature);
      const scored = scoreByGoal(allMemories, goal);
      decisions = scored.map(s => ({
        key: s.name,
        value: s.memory.bodyContent.slice(0, 500),
      }));
    } else if (cfg.enabled && task) {
      // DCP task-based scoring (default when no goal)
      const allMemories = this.memoryAdapter.listWithMeta(feature);
      const selected = selectMemories(
        allMemories, task, null, cfg.handoffDecisionBudgetTokens,
        cfg.relevanceThreshold,
      );
      decisions = selected.memories.map(m => ({
        key: m.name,
        value: m.bodyContent.slice(0, 500),
      }));
    } else {
      const memories = this.memoryAdapter.list(feature);
      decisions = memories.map(mf => ({
        key: mf.name,
        value: mf.content.slice(0, 500),
      }));
    }

    const modifiedFiles = getModifiedFiles(this.projectRoot);

    return {
      beadId: taskId,
      beadState: {
        title: task?.planTitle ?? task?.name ?? taskId,
        status: task?.status ?? 'unknown',
        description: richFields?.description,
        design: richFields?.design,
        acceptanceCriteria: richFields?.acceptanceCriteria,
      },
      decisions,
      modifiedFiles,
      blockers: task?.status === 'blocked' ? [task.summary ?? 'Unknown blocker'] : [],
      openQuestions: [],
      nextSteps: [],
      criticalContext: '',
      cassPointer: `Search prior sessions: maestro search-sessions --query "${task?.name ?? taskId}"`,
      goal,
    };
  }

  private identityPath(): string {
    return path.join(this.projectRoot, '.maestro', '.agent-mail.json');
  }

  private async rpc(tool: string, args: Record<string, unknown>): Promise<{ isError: boolean; text?: string }> {
    return this.transport.rpc(tool, args);
  }

  private async ensureIdentity(): Promise<AgentMailIdentity | undefined> {
    if (this.identity) return this.identity;

    try {
      const raw = readText(this.identityPath());
      if (raw) {
        const cached = JSON.parse(raw) as AgentMailIdentity;
        if (cached.agentName && cached.projectSlug) {
          this.identity = cached;
          return this.identity;
        }
      }
    } catch { /* no cache or corrupt */ }

    const proj = await this.rpc('ensure_project', { human_key: this.projectRoot });
    if (proj.isError || !proj.text) return undefined;
    const projectSlug = JSON.parse(proj.text).slug as string;

    const agent = await this.rpc('register_agent', {
      project_key: this.projectRoot,
      program: 'maestro',
      model: 'orchestrator',
    });
    if (agent.isError || !agent.text) return undefined;
    const agentName = JSON.parse(agent.text).name as string;

    this.identity = { agentName, projectSlug };

    try {
      ensureDir(path.dirname(this.identityPath()));
      writeText(this.identityPath(), JSON.stringify(this.identity, null, 2));
    } catch { /* best-effort persistence */ }

    return this.identity;
  }

  async sendHandoff(feature: string, handoff: HandoffDocument, targetAgent?: string): Promise<HandoffResult> {
    const body = formatHandoffMessage(handoff, feature, this.taskBackend);

    const filePath = getHandoffPath(this.projectRoot, feature, handoff.beadId);
    ensureDir(path.dirname(filePath));
    writeText(filePath, body);

    let agentMailSent = false;
    let threadId: string | undefined;

    try {
      threadId = `bead-${handoff.beadId}`;
      const id = await this.ensureIdentity();
      if (id) {
        const subject = targetAgent
          ? `[${handoff.beadId}] Handoff for ${targetAgent}: ${handoff.beadState.title}`
          : `[${handoff.beadId}] Handoff: ${handoff.beadState.title}`;
        const result = await this.rpc('send_message', {
          project_key: this.projectRoot,
          sender_name: id.agentName,
          to: targetAgent ? [targetAgent] : [],
          subject,
          body_md: body,
          thread_id: threadId,
          importance: 'high',
          broadcast: !targetAgent,
        });
        agentMailSent = !result.isError;
        if (agentMailSent && result.text) {
          try {
            const parsed = JSON.parse(result.text);
            const msgId = parsed.deliveries?.[0]?.payload?.id ?? parsed.id;
            threadId = String(msgId ?? threadId);
          } catch { /* keep string threadId */ }
        }
      }
    } catch {
      // Agent Mail unreachable -- file was still written
    }

    return { filePath, threadId, agentMailSent };
  }

  async receiveHandoffs(feature: string | undefined, agentId?: string): Promise<HandoffDocument[]> {
    const handoffs: HandoffDocument[] = [];
    const seenBeadIds = new Set<string>();

    if (!feature) return handoffs;

    const identityPromise = agentId
      ? this.ensureIdentity().catch(() => undefined)
      : Promise.resolve(undefined);

    const handoffsDir = getHandoffsPath(this.projectRoot, feature);
    try {
      const files = fs.readdirSync(handoffsDir).filter(f => f.endsWith('.md'));
      for (const file of files) {
        const content = readText(path.join(handoffsDir, file));
        if (!content) continue;
        const beadId = file.replace(/\.md$/, '');
        seenBeadIds.add(beadId);
        handoffs.push({
          beadId,
          beadState: { title: extractTitle(content), status: 'unknown' },
          decisions: [],
          modifiedFiles: [],
          blockers: [],
          openQuestions: [],
          nextSteps: [],
          criticalContext: content,
        });
      }
    } catch {
      // No handoffs directory yet
    }

    if (agentId) {
      try {
        const id = await identityPromise;
        if (id) {
          const result = await this.rpc('fetch_inbox', {
            project_key: this.projectRoot,
            agent_name: agentId,
          });
          if (!result.isError && result.text) {
            const parsed = JSON.parse(result.text);
            const messages = Array.isArray(parsed) ? parsed : parsed.messages ?? [];
            for (const msg of messages) {
              const beadMatch = msg.subject?.match(/^\[([^\]]+)\]\s*Handoff/);
              const beadId = beadMatch?.[1] ?? `mail-${msg.id}`;
              if (seenBeadIds.has(beadId)) continue;
              seenBeadIds.add(beadId);
              handoffs.push({
                beadId,
                beadState: { title: msg.subject ?? 'Unknown', status: 'unknown' },
                decisions: [],
                modifiedFiles: [],
                blockers: [],
                openQuestions: [],
                nextSteps: [],
                criticalContext: msg.body_md ?? '',
                agentMailThread: String(msg.thread_id ?? msg.id),
              });
            }
          }
        }
      } catch { /* Agent Mail unreachable */ }
    }

    return handoffs;
  }

  async acknowledgeHandoff(threadId: string): Promise<void> {
    const msgId = parseInt(threadId, 10);
    if (isNaN(msgId)) return;

    try {
      const id = await this.ensureIdentity();
      if (!id) return;
      await this.rpc('acknowledge_message', {
        project_key: this.projectRoot,
        message_id: msgId,
        agent_name: id.agentName,
      });
    } catch {
      // Best-effort
    }
  }

}

export const createAdapter: AdapterFactory<HandoffPort> = (ctx: AdapterContext) => {
  const taskPort = ctx.ports.taskPort as TaskPort;
  const memoryPort = ctx.ports.memoryPort as MemoryPort;
  const settingsPort = ctx.ports.settingsPort as SettingsPort;
  const taskBackend = (ctx.ports.taskBackend as 'fs' | 'br') ?? 'fs';
  const hasBearerToken = !!process.env.HTTP_BEARER_TOKEN;
  const baseUrl = validateAgentMailUrl(
    ctx.manifest.baseUrl ?? process.env.AGENT_MAIL_URL ?? DEFAULT_AGENT_MAIL_URL,
    hasBearerToken,
  );
  const transport = new HttpTransport({
    baseUrl,
    timeout: 5000,
    bestEffort: true,
    retryDelays: [],
    authHeaders: process.env.HTTP_BEARER_TOKEN
      ? { Authorization: `Bearer ${process.env.HTTP_BEARER_TOKEN}` }
      : undefined,
  });
  return new AgentMailHandoffAdapter(ctx.projectRoot, taskPort, memoryPort, settingsPort, taskBackend, undefined, transport);
};
