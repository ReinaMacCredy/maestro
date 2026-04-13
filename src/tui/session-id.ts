export function shortenSessionId(sessionId: string): string {
  return sessionId.length > 10 ? `${sessionId.slice(0, 8)}…` : sessionId;
}
