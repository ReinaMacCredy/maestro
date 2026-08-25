import type { Counts, PillState } from "../model";
import { Icons } from "./icons";

export function Pill({ counts, state, pinned, expanded, onClick }: { counts: Counts; state: PillState; pinned: boolean; expanded: boolean; onClick: () => void }) {
  return (
    <button type="button" className={`pill ${pinned ? "pinned" : ""}`} data-state={state} aria-expanded={expanded} aria-controls="panel" onClick={onClick}>
      <span className="dot" />
      <span>
        <span className="n">{counts.active}</span> active
      </span>
      <span className="sep">·</span>
      <span>
        <span className="n">{counts.ready}</span> ready
      </span>
      {counts.attention ? (
        <>
          <span className="sep">·</span>
          <span className="bang">
            <span className="n">{counts.attention}</span> !
          </span>
        </>
      ) : null}
      {pinned ? Icons.pin : null}
    </button>
  );
}
