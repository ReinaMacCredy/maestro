import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

// Spike build: pill + panel, hover driven by the native window (see lib.rs),
// pin by clicking the pill, one copy button. UI port lands in w25.
export function App() {
  const [open, setOpen] = useState(false);
  const [pinned, setPinned] = useState(false);
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    const un = listen<boolean>("hover", (e) => setOpen(e.payload));
    return () => {
      un.then((f) => f());
    };
  }, []);

  const togglePin = () => {
    const next = !pinned;
    setPinned(next);
    invoke("set_pinned", { value: next }).catch(console.error);
  };

  const copy = async () => {
    await navigator.clipboard.writeText("maestro ready");
    setCopied(true);
    window.setTimeout(() => setCopied(false), 1200);
  };

  return (
    <div className="widget">
      <div className={"panelWrap" + (open ? " open" : "")}>
        <div className="panel">
          <div className="section">Cần bạn</div>
          <div className="card">
            <div className="title">w15 gated by w21</div>
            <button className="copy" onClick={copy} data-done={copied ? "1" : undefined}>
              {copied ? "Đã copy" : "Copy"}
            </button>
          </div>
        </div>
      </div>
      <button className={"pill" + (pinned ? " pinned" : "")} onClick={togglePin}>
        <span className="dot working" />
        <span>2 active · 1 ready · 3 !</span>
      </button>
    </div>
  );
}
