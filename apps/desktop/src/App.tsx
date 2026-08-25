import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Panel } from "./components/Panel";
import { Pill } from "./components/Pill";
import { FIXTURE } from "./fixture";
import { cards, counts, pillState, sessionIndex, type RepoSnapshot } from "./model";

const inTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

// Hover open/close is owned by the native window (src-tauri/src/lib.rs) and
// arrives as the "hover" event; outside Tauri (vite dev) the panel stays open.
export function App() {
  const [repos, setRepos] = useState<RepoSnapshot[]>(inTauri ? [] : FIXTURE);
  const [open, setOpen] = useState(!inTauri);
  const [pinned, setPinned] = useState(false);
  const [collapsed, setCollapsed] = useState<Set<string>>(new Set());
  const [toast, setToast] = useState(false);
  const [now, setNow] = useState(() => new Date());

  useEffect(() => {
    if (!inTauri) return;
    const unHover = listen<boolean>("hover", (e) => setOpen(e.payload));
    const unSnap = listen<RepoSnapshot[]>("snapshot", (e) => setRepos(e.payload));
    return () => {
      unHover.then((f) => f());
      unSnap.then((f) => f());
    };
  }, []);

  useEffect(() => {
    const t = window.setInterval(() => setNow(new Date()), 1000);
    return () => window.clearInterval(t);
  }, []);

  const cardList = useMemo(() => cards(repos, now), [repos, now]);
  const c = useMemo(() => counts(repos, cardList), [repos, cardList]);
  const sessions = useMemo(() => sessionIndex(repos), [repos]);

  const togglePin = () => {
    const next = !pinned;
    setPinned(next);
    if (inTauri) invoke("set_pinned", { value: next }).catch(console.error);
  };
  const toggleRepo = (repo: string) =>
    setCollapsed((s) => {
      const n = new Set(s);
      n.has(repo) ? n.delete(repo) : n.add(repo);
      return n;
    });
  const copied = () => {
    setToast(true);
    window.setTimeout(() => setToast(false), 1200);
  };

  return (
    <div className={`widget ${open ? "open" : ""} ${pinned ? "pinned" : ""}`}>
      <div className="panelWrap">
        <div className="panelInner">
          <Panel repos={repos} cards={cardList} counts={c} sessions={sessions} now={now} collapsed={collapsed} onToggleRepo={toggleRepo} onCopied={copied} />
        </div>
      </div>
      <Pill counts={c} state={pillState(c)} pinned={pinned} expanded={open} onClick={togglePin} />
      <div className={`toast ${toast ? "show" : ""}`}>Đã copy</div>
    </div>
  );
}
