import type { Card, Counts, RepoSnapshot, Session } from "../model";
import { ago } from "../model";
import { ApprovalCard } from "./ApprovalCard";
import { Icons } from "./icons";
import { TaskList } from "./TaskList";

export function Panel({
  repos,
  cards,
  counts,
  sessions,
  now,
  collapsed,
  configPath,
  onToggleRepo,
  onCopied,
}: {
  repos: RepoSnapshot[];
  cards: Card[];
  counts: Counts;
  sessions: Map<string, Session>;
  now: Date;
  collapsed: Set<string>;
  configPath?: string | null;
  onToggleRepo: (repo: string) => void;
  onCopied?: () => void;
}) {
  const at = repos.map((r) => r.at).filter(Boolean).sort().at(-1) ?? null;
  return (
    <div className="panel" id="panel">
      <div className="sect">
        Cần bạn <span className="cnt">{counts.attention}</span>
      </div>
      {cards.length ? (
        cards.map((c, i) => <ApprovalCard key={c.key} card={c} now={now} index={i} onCopied={onCopied} />)
      ) : (
        <div className="allClear">{Icons.clear}Không có gì chờ bạn.</div>
      )}
      <div className="sect">
        Work <span className="cnt">{`${counts.active} active · ${counts.ready} ready`}</span>
      </div>
      {repos.length ? (
        repos.map((r) => (
          <TaskList key={r.repo} repo={r} sessions={sessions} collapsed={collapsed.has(r.repo)} onToggle={() => onToggleRepo(r.repo)} />
        ))
      ) : (
        <div className="allClear">
          {configPath ? `Thêm đường dẫn repo vào "repos" trong ${configPath}` : "Chưa có repo nào trong config."}
        </div>
      )}
      <div className="panelFoot">
        <span>{at ? `cập nhật ${ago(at, now)} trước` : "chưa có dữ liệu"}</span>
        <span>
          <code>{`${repos.length} repo`}</code> · read-only
        </span>
      </div>
    </div>
  );
}
