#!/bin/bash
set -euo pipefail
LOG="$HOME/.collaborator/hook-debug.log"
INPUT=$(cat)
echo "[$(date -Iseconds)] hook fired" >> "$LOG"
echo "  raw input: $INPUT" >> "$LOG"
EVENT=$(echo "$INPUT" | jq -r '.hook_event_name')

# Discover the socket path from the breadcrumb file written by the
# JSON-RPC server. This works for both dev (~/.collaborator/dev/)
# and prod (~/.collaborator/) instances.
SOCKET_PATH_FILE="$HOME/.collaborator/socket-path"
if [ -f "$SOCKET_PATH_FILE" ]; then
  SOCKET=$(cat "$SOCKET_PATH_FILE")
else
  SOCKET="$HOME/.collaborator/ipc.sock"
fi

if [ ! -S "$SOCKET" ]; then
  echo "  socket not found at $SOCKET" >> "$LOG"
  exit 0
fi

case "$EVENT" in
  SessionStart)
    METHOD="agent.sessionStart"
    PAYLOAD=$(echo "$INPUT" | jq -c --arg pty "${COLLAB_PTY_SESSION_ID:-}" '{session_id: .session_id, cwd: .cwd, pty_session_id: $pty}')
    ;;
  PostToolUse)
    METHOD="agent.fileTouched"
    PAYLOAD=$(echo "$INPUT" | jq -c '{session_id: .session_id, tool_name: .tool_name, file_path: (.tool_input.file_path // .tool_input.path // null)}')
    ;;
  SessionEnd)
    METHOD="agent.sessionEnd"
    PAYLOAD=$(echo "$INPUT" | jq -c '{session_id: .session_id}')
    ;;
  *)
    echo "  unknown event: $EVENT" >> "$LOG"
    exit 0
    ;;
esac

echo "  method=$METHOD payload=$PAYLOAD" >> "$LOG"
RESULT=$(printf '{"jsonrpc":"2.0","id":1,"method":"%s","params":%s}\n' "$METHOD" "$PAYLOAD" \
  | nc -U -w1 "$SOCKET" 2>&1) || true
echo "  rpc result: $RESULT" >> "$LOG"

exit 0
