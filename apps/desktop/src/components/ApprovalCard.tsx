import { useState } from "react";
import type { Card } from "../model";
import { ago } from "../model";
import { Icons } from "./icons";

// aicss approval-card, read-only: no approve/reject actions and no auto-approve.
// The only affordance is copying the maestro command for the terminal.
export function CopyButton({ command, onCopied }: { command: string; onCopied?: () => void }) {
  const [done, setDone] = useState(false);
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(command);
    } catch (err) {
      console.error(err);
      return;
    }
    setDone(true);
    onCopied?.();
    window.setTimeout(() => setDone(false), 1600);
  };
  return (
    <button type="button" className="copy" data-cmd={command} data-done={done ? "true" : "false"} onClick={copy}>
      {done ? Icons.tick : Icons.copy}
      {done ? "Đã copy" : "Copy"}
    </button>
  );
}

export function ApprovalCard({ card, now, index, onCopied }: { card: Card; now: Date; index: number; onCopied?: () => void }) {
  return (
    <div className="card" data-variant={card.variant} style={{ "--i": index } as React.CSSProperties}>
      <div className="head">
        <span className="icon" data-variant={card.variant}>{Icons[card.variant]}</span>
        <div className="headText">
          <div className="title">{card.title}</div>
          <div className="sub">
            <code>{card.repo}</code>
            {card.sub ? ` · ${card.sub}` : ""}
          </div>
        </div>
        <span className={`age ${card.hot ? "hot" : ""}`}>{ago(card.at, now)}</span>
      </div>
      {card.body ? <div className="planSummary">{card.body}</div> : null}
      <div className="cmdBlock">
        <pre className="cmd">
          <span className="p">$ </span>
          {card.command}
        </pre>
        <CopyButton command={card.command} onCopied={onCopied} />
      </div>
    </div>
  );
}
