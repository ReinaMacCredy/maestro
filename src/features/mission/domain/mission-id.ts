/**
 * Generate a date-sequential mission ID.
 * Format: YYYY-MM-DD-NNN (e.g. 2026-03-28-001)
 */
export function generateMissionId(
  existingIds: readonly string[],
  now: Date = new Date(),
): string {
  const datePrefix = formatDatePrefix(now);
  const todayIds = existingIds.filter((id) => id.startsWith(datePrefix));

  let maxSeq = 0;
  for (const id of todayIds) {
    const seqStr = id.slice(datePrefix.length + 1);
    const seq = parseInt(seqStr, 10);
    if (!Number.isNaN(seq) && seq > maxSeq) {
      maxSeq = seq;
    }
  }

  const nextSeq = String(maxSeq + 1).padStart(3, "0");
  return `${datePrefix}-${nextSeq}`;
}

function formatDatePrefix(date: Date): string {
  const y = date.getFullYear();
  const m = String(date.getMonth() + 1).padStart(2, "0");
  const d = String(date.getDate()).padStart(2, "0");
  return `${y}-${m}-${d}`;
}
