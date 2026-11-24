// Read model over maestro CLI JSON envelopes (attention, work list, decision
// list). Pure functions only; the app never mutates the store.

export type WorkState = "open" | "active" | "done" | "cancelled";

export interface Work {
  id: string;
  title: string;
  kind: string;
  state: WorkState;
  parentId: string | null;
  updatedAt: string;
  blockedBy: { id: string; state: string }[];
  holder: Holder | null;
  gate: Gate | null;
}

export interface Holder {
  id: string;
  harness: string | null;
  live: boolean;
  lastSeen: string | null;
}

export interface Gate {
  origin: string;
  reason: string;
  command: string | null;
}

export interface WorkCounts {
  open: number;
  active: number;
  done: number;
  cancelled: number;
}

export interface Decision {
  id: string;
  text: string;
  rationale: string | null;
  state: string;
  workId: string | null;
  updatedAt: string;
}

export interface Finding {
  kind: string;
  packet: string;
  subjectSession: string | null;
  subjectWork: string | null;
  targets: string[];
  raisedAt: string;
  raised: boolean;
}

export interface RepoSnapshot {
  repo: string;
  path: string;
  at: string;
  works: Work[];
  counts: WorkCounts;
  decisions: Decision[];
  findings: Finding[];
  error?: string;
}

export type RowStatus = WorkState | "gated";

export interface WorkRow {
  work: Work;
  status: RowStatus;
  depth: number;
  blockers: string[];
}

export type CardVariant = "attention" | "decision" | "gated";

export interface Card {
  key: string;
  variant: CardVariant;
  repo: string;
  title: string;
  sub: string;
  body: string;
  command: string;
  at: string | null;
  hot: boolean;
}

export const DECISION_HOT_HOURS = 24;

export function shortId(id: string | null | undefined): string {
  return id ? id.slice(0, 8) : "";
}

export function ago(iso: string | null, now: Date): string {
  if (!iso) return "";
  const m = Math.max(0, Math.round((now.getTime() - new Date(iso).getTime()) / 60000));
  if (m < 60) return `${m}m`;
  if (m < 1440) return `${Math.round(m / 60)}h`;
  return `${Math.round(m / 1440)}d`;
}

/** Depth-first tree order (parents before children), preserving list order. */
export function workRows(repo: RepoSnapshot): WorkRow[] {
  const byParent = new Map<string | null, Work[]>();
  const ids = new Set(repo.works.map((w) => w.id));
  for (const w of repo.works) {
    const parent = w.parentId && ids.has(w.parentId) ? w.parentId : null;
    const list = byParent.get(parent) ?? [];
    list.push(w);
    byParent.set(parent, list);
  }
  const rows: WorkRow[] = [];
  const visit = (parent: string | null, depth: number) => {
    for (const work of byParent.get(parent) ?? []) {
      const blockers = work.blockedBy.map((b) => b.id);
      const status: RowStatus = work.state === "open" && (blockers.length || work.gate) ? "gated" : work.state;
      rows.push({ work, status, depth, blockers });
      visit(work.id, depth + 1);
    }
  };
  visit(null, 0);
  return rows;
}

export function progress(repo: RepoSnapshot): { done: number; total: number; active: number; pct: number } {
  const { open, active, done, cancelled } = repo.counts;
  const total = open + active + done + cancelled;
  return { done, total, active, pct: total ? Math.round((done / total) * 100) : 0 };
}

export function cards(repos: RepoSnapshot[], now: Date): Card[] {
  const out: Card[] = [];
  for (const r of repos) {
    for (const f of r.findings) {
      const subject = f.subjectWork ?? shortId(f.subjectSession);
      out.push({
        key: `${r.repo}:attention:${f.kind}:${subject}`,
        variant: "attention",
        repo: r.repo,
        title: `${f.kind} ${subject}`,
        sub: f.subjectSession ? `holder ${shortId(f.subjectSession)}` : "",
        body: f.packet,
        command: f.subjectWork ? `maestro work show ${f.subjectWork}` : "maestro attention",
        at: f.raisedAt,
        hot: true,
      });
    }
    for (const d of r.decisions.filter((d) => d.state === "draft")) {
      const hours = (now.getTime() - new Date(d.updatedAt).getTime()) / 3600000;
      out.push({
        key: `${r.repo}:decision:${d.id}`,
        variant: "decision",
        repo: r.repo,
        title: `${d.id} chờ lock`,
        sub: d.workId ? `draft · ${d.workId}` : "draft",
        body: d.rationale ? `${d.text}\n${d.rationale}` : d.text,
        command: `maestro decision lock ${d.id}`,
        at: d.updatedAt,
        hot: hours >= DECISION_HOT_HOURS,
      });
    }
    for (const w of r.works) {
      if (!w.gate) continue;
      out.push({
        key: `${r.repo}:gated:${w.id}`,
        variant: "gated",
        repo: r.repo,
        title: `${w.id} bị gate`,
        sub: w.gate.origin,
        body: w.gate.reason,
        command: w.gate.command || "maestro work list",
        at: null,
        hot: false,
      });
    }
  }
  return out;
}

export interface Counts {
  active: number;
  ready: number;
  attention: number;
}

export function counts(repos: RepoSnapshot[], cardList: Card[]): Counts {
  return {
    active: repos.reduce((n, r) => n + r.counts.active, 0),
    ready: repos.reduce((n, r) => n + r.works.filter((w) => w.state === "open" && !w.blockedBy.length && !w.gate).length, 0),
    attention: cardList.length,
  };
}

export type PillState = "idle" | "working" | "attention";

export function pillState(c: Counts): PillState {
  return c.attention ? "attention" : c.active ? "working" : "idle";
}
