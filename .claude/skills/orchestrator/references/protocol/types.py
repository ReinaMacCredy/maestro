"""Message types for orchestrator protocol.

Defines message type enum, importance mappings, and payload schemas.
"""

from __future__ import annotations

from enum import Enum
from typing import TypedDict


class MessageType(Enum):
    """All message types in the orchestrator protocol."""

    ASSIGN = "ASSIGN"
    WAKE = "WAKE"
    PING = "PING"
    PONG = "PONG"
    PROGRESS = "PROGRESS"
    BLOCKED = "BLOCKED"
    COMPLETED = "COMPLETED"
    FAILED = "FAILED"
    STEAL = "STEAL"
    RELEASE = "RELEASE"
    ESCALATE = "ESCALATE"


IMPORTANCE_MAP: dict[MessageType, str] = {
    MessageType.ASSIGN: "high",
    MessageType.WAKE: "high",
    MessageType.PING: "normal",
    MessageType.PONG: "normal",
    MessageType.PROGRESS: "low",
    MessageType.BLOCKED: "high",
    MessageType.COMPLETED: "normal",
    MessageType.FAILED: "urgent",
    MessageType.STEAL: "high",
    MessageType.RELEASE: "normal",
    MessageType.ESCALATE: "urgent",
}


class AssignPayload(TypedDict):
    """Payload for ASSIGN messages."""

    track: str
    beads: list[str]
    file_scope: str
    thread_id: str


class WakePayload(TypedDict):
    """Payload for WAKE messages."""

    reason: str
    dependency_satisfied: str | None


class PingPayload(TypedDict):
    """Payload for PING messages."""

    request_id: str


class PongPayload(TypedDict):
    """Payload for PONG messages."""

    request_id: str
    status: str


class ProgressPayload(TypedDict):
    """Payload for PROGRESS messages."""

    bead_id: str
    percent: int
    notes: str | None


class BlockedPayload(TypedDict):
    """Payload for BLOCKED messages."""

    bead_id: str
    blocker: str
    needs: str


class CompletedPayload(TypedDict):
    """Payload for COMPLETED messages."""

    track: str
    beads_closed: list[str]
    files_changed: list[str]


class FailedPayload(TypedDict):
    """Payload for FAILED messages."""

    track: str
    bead_id: str
    error: str
    recoverable: bool


class StealPayload(TypedDict):
    """Payload for STEAL messages."""

    bead_id: str
    reason: str


class ReleasePayload(TypedDict):
    """Payload for RELEASE messages."""

    bead_id: str
    new_assignee: str | None


class EscalatePayload(TypedDict):
    """Payload for ESCALATE messages."""

    issue: str
    context: str
    suggested_action: str | None


REQUIRED_FIELDS: dict[MessageType, list[str]] = {
    MessageType.ASSIGN: ["track", "beads", "file_scope", "thread_id"],
    MessageType.WAKE: ["reason"],
    MessageType.PING: ["request_id"],
    MessageType.PONG: ["request_id", "status"],
    MessageType.PROGRESS: ["bead_id", "percent"],
    MessageType.BLOCKED: ["bead_id", "blocker", "needs"],
    MessageType.COMPLETED: ["track", "beads_closed", "files_changed"],
    MessageType.FAILED: ["track", "bead_id", "error", "recoverable"],
    MessageType.STEAL: ["bead_id", "reason"],
    MessageType.RELEASE: ["bead_id"],
    MessageType.ESCALATE: ["issue", "context"],
}


def validate_payload(msg_type: MessageType, payload: dict) -> list[str]:
    """Validate payload has required fields for message type.

    Args:
        msg_type: The message type to validate against.
        payload: The payload dictionary to validate.

    Returns:
        List of missing field names. Empty if valid.
    """
    required = REQUIRED_FIELDS.get(msg_type, [])
    return [field for field in required if field not in payload]


def get_importance(msg_type: MessageType) -> str:
    """Get importance level for a message type.

    Args:
        msg_type: The message type.

    Returns:
        Importance string: 'low', 'normal', 'high', or 'urgent'.
    """
    return IMPORTANCE_MAP.get(msg_type, "normal")
