// Inline SVG icons from the aicss task-list and approval-card sources.
const stroke = { fill: "none", stroke: "currentColor", strokeLinecap: "round", strokeLinejoin: "round" } as const;

export const Icons = {
  check: (cls = "") => (
    <svg className={`todoIcon ${cls}`} viewBox="0 0 24 24" aria-hidden="true">
      <path d="M9 12.75 11.25 15 15 9.75M21 12a9 9 0 1 1-18 0 9 9 0 0 1 18 0Z" {...stroke} strokeWidth="1.6" />
    </svg>
  ),
  arrow: (cls = "") => (
    <svg className={`todoIcon strong ${cls}`} viewBox="0 0 24 24" aria-hidden="true">
      <path d="m12.75 15 3-3m0 0-3-3m3 3h-7.5M21 12a9 9 0 1 1-18 0 9 9 0 0 1 18 0Z" {...stroke} strokeWidth="1.6" />
    </svg>
  ),
  dashed: (cls = "") => (
    <svg className={`todoIcon ${cls}`} viewBox="0 0 24 24" aria-hidden="true">
      <circle cx="12" cy="12" r="9" fill="none" stroke="currentColor" strokeWidth="1.8" strokeDasharray="1.8 3.6" strokeLinecap="round" />
    </svg>
  ),
  lock: (cls = "") => (
    <svg className={`todoIcon ${cls}`} viewBox="0 0 24 24" aria-hidden="true">
      <circle cx="12" cy="12" r="9" fill="none" stroke="currentColor" strokeWidth="1.6" />
      <path d="M9 12.5h6M9.5 12.5v-2a2.5 2.5 0 0 1 5 0v2" {...stroke} strokeWidth="1.5" />
    </svg>
  ),
  x: (cls = "") => (
    <svg className={`todoIcon ${cls}`} viewBox="0 0 24 24" aria-hidden="true">
      <path d="m9.5 9.5 5 5m0-5-5 5M21 12a9 9 0 1 1-18 0 9 9 0 0 1 18 0Z" {...stroke} strokeWidth="1.6" />
    </svg>
  ),
  list: (
    <svg className="todoListIcon" viewBox="0 0 24 24" {...stroke} strokeWidth="2" aria-hidden="true">
      <path d="M13 5h8M13 12h8M13 19h8m-18-2 2 2 4-4M3 7l2 2 4-4" />
    </svg>
  ),
  chevron: (
    <svg className="todoChevron" viewBox="0 0 24 24" aria-hidden="true">
      <path d="m19.5 8.25-7.5 7.5-7.5-7.5" {...stroke} strokeWidth="1.8" />
    </svg>
  ),
  headCheck: (
    <svg className="todoHeadCheck" viewBox="0 0 24 24" aria-hidden="true">
      <path
        fillRule="evenodd"
        clipRule="evenodd"
        d="M2.25 12c0-5.385 4.365-9.75 9.75-9.75s9.75 4.365 9.75 9.75-4.365 9.75-9.75 9.75S2.25 17.385 2.25 12Zm13.36-1.814a.75.75 0 1 0-1.22-.872l-3.236 4.53L9.53 12.22a.75.75 0 0 0-1.06 1.06l2.25 2.25a.75.75 0 0 0 1.14-.094l3.75-5.25Z"
        fill="currentColor"
      />
    </svg>
  ),
  copy: (
    <svg viewBox="0 0 24 24" {...stroke} strokeWidth="2.2" aria-hidden="true">
      <rect x="9" y="9" width="11" height="11" rx="2" />
      <path d="M5 15V5a2 2 0 0 1 2-2h10" />
    </svg>
  ),
  tick: (
    <svg viewBox="0 0 24 24" {...stroke} strokeWidth="2.4" aria-hidden="true">
      <path d="m5 12 5 5L20 7" />
    </svg>
  ),
  decision: (
    <svg viewBox="0 0 24 24" {...stroke} strokeWidth="2" aria-hidden="true">
      <path d="M9 11h6M9 15h4M7 3h10a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2z" />
    </svg>
  ),
  attention: (
    <svg viewBox="0 0 24 24" {...stroke} strokeWidth="2" aria-hidden="true">
      <path d="M12 9v4m0 4h.01M10.3 3.9 1.8 18a2 2 0 0 0 1.7 3h17a2 2 0 0 0 1.7-3L13.7 3.9a2 2 0 0 0-3.4 0z" />
    </svg>
  ),
  gated: (
    <svg viewBox="0 0 24 24" {...stroke} strokeWidth="2" aria-hidden="true">
      <rect x="4" y="11" width="16" height="10" rx="2" />
      <path d="M8 11V7a4 4 0 0 1 8 0v4" />
    </svg>
  ),
  clear: (
    <svg viewBox="0 0 24 24" {...stroke} strokeWidth="2" aria-hidden="true">
      <path d="M20 6 9 17l-5-5" />
    </svg>
  ),
  pin: (
    <svg className="pin" viewBox="0 0 24 24" {...stroke} strokeWidth="2" aria-hidden="true">
      <path d="M12 17v5M9 3h6l-1 7 3 3H7l3-3z" />
    </svg>
  ),
};
