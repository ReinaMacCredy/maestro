import type { RepoSnapshot, Session } from "../model";
import { progress, shortId, workRows } from "../model";
import { Icons } from "./icons";

// aicss task-list (TodoList) rendered from a repo snapshot; no demo timers.
export function Who({ id, sessions }: { id: string; sessions: Map<string, Session> }) {
  const s = sessions.get(id);
  return <span className={`who ${s?.harness ?? ""}`}>{s ? `${s.harness} ${shortId(id)}` : shortId(id)}</span>;
}

export function TaskList({
  repo,
  sessions,
  collapsed,
  onToggle,
}: {
  repo: RepoSnapshot;
  sessions: Map<string, Session>;
  collapsed: boolean;
  onToggle: () => void;
}) {
  const rows = workRows(repo);
  const p = progress(repo);
  const allDone = p.total > 0 && p.done === p.total;
  const headIcon =
    p.total === 0 ? Icons.list : allDone ? Icons.headCheck : p.active ? (
      <span className="todoHeadPie" style={{ "--todo-pie": `${p.pct}%` } as React.CSSProperties} aria-hidden="true">
        <svg className="todoHeadPieRing" viewBox="0 0 24 24">
          <circle cx="12" cy="12" r="10.5" fill="none" stroke="currentColor" strokeWidth="2.2" strokeDasharray="2.2 4.4" strokeLinecap="round" />
        </svg>
      </span>
    ) : Icons.list;

  return (
    <div className="todo" data-repo={repo.repo}>
      <button type="button" className="todoHead" aria-expanded={!collapsed} aria-label={`Toggle ${repo.repo}`} onClick={onToggle}>
        <span className="todoHeadIcon">
          {headIcon}
          {Icons.chevron}
        </span>
        <span className="todoTitle">{repo.repo}</span>
        {repo.error ? <span className="todoRepo">lỗi</span> : null}
        <span className="todoCount">{p.total ? `${p.done}/${p.total}` : "idle"}</span>
      </button>
      <div className={`todoCollapsible ${collapsed ? "isCollapsed" : ""}`}>
        <div className="todoInner">
          {repo.error ? (
            <div className="todoIdle">{repo.error}</div>
          ) : p.total ? (
            <ul className="todoList">
              {rows.map((r, i) => {
                const icon =
                  r.status === "done" ? Icons.check("on")
                  : r.status === "active" ? Icons.arrow("on")
                  : r.status === "cancelled" ? Icons.x("on")
                  : r.status === "gated" ? Icons.lock("on")
                  : Icons.dashed("on");
                return (
                  <li key={r.work.id} className={`todoItem ${r.status} ${r.depth ? "child" : ""}`} style={{ "--i": i } as React.CSSProperties}>
                    <span className="todoIconWrap">{icon}</span>
                    <span className="todoLabel" data-label={r.work.title}>{r.work.title}</span>
                    <span className="todoMeta">
                      <span className="id">{r.work.id}</span>
                      {r.status === "active" && r.work.heldBy ? <Who id={r.work.heldBy} sessions={sessions} /> : null}
                      {r.status === "gated" ? <span className="lock">{`chờ ${r.blockers.join(", ")}`}</span> : null}
                    </span>
                  </li>
                );
              })}
            </ul>
          ) : (
            <div className="todoIdle">Không có work đang theo dõi.</div>
          )}
        </div>
      </div>
    </div>
  );
}
