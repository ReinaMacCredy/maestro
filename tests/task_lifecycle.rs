use maestro::domain::task::{
    BlockerKind, TaskRecord, TaskState, TransitionDetails, has_unresolved_blockers,
};
use maestro::task::blockers::{add_blocker, resolve_blocker};
use maestro::task::lifecycle::transition;

#[test]
fn forward_transitions_append_state_history_and_set_claimant() {
    let mut task = TaskRecord::draft("task-001", "Add CSV export", "t0");
    task.acceptance_locked = true;

    transition(
        &mut task,
        TaskState::Exploring,
        "human",
        "t1",
        TransitionDetails::default(),
    )
    .expect("invariant: draft should explore");
    transition(
        &mut task,
        TaskState::Ready,
        "human",
        "t2",
        TransitionDetails::default(),
    )
    .expect("invariant: exploring should become ready with locked acceptance");
    transition(
        &mut task,
        TaskState::InProgress,
        "codex",
        "t3",
        TransitionDetails::default(),
    )
    .expect("invariant: ready should claim");

    assert_eq!(task.state, TaskState::InProgress);
    assert_eq!(task.claimed_by.as_deref(), Some("codex"));
    assert_eq!(task.claimed_at.as_deref(), Some("t3"));
    assert_eq!(task.state_history.len(), 4);
}

#[test]
fn ready_to_in_progress_requires_locked_acceptance_and_no_blockers() {
    let mut task = TaskRecord::draft("task-001", "Add CSV export", "t0");
    task.state = TaskState::Ready;

    let error = transition(
        &mut task,
        TaskState::InProgress,
        "codex",
        "t1",
        TransitionDetails::default(),
    )
    .expect_err("invariant: unlocked acceptance should block claim");
    assert!(error.to_string().contains("acceptance must be locked"));

    task.acceptance_locked = true;
    add_blocker(
        &mut task,
        "blk-001".to_string(),
        BlockerKind::Human,
        None,
        "Needs answer".to_string(),
        "Question open".to_string(),
        "t2".to_string(),
    );

    let error = transition(
        &mut task,
        TaskState::InProgress,
        "codex",
        "t3",
        TransitionDetails::default(),
    )
    .expect_err("invariant: unresolved blocker should block claim");
    assert!(error.to_string().contains("unresolved blockers"));
}

#[test]
fn blockers_are_overlay_metadata_not_states() {
    let mut task = TaskRecord::draft("task-001", "Add CSV export", "t0");
    task.state = TaskState::Ready;

    add_blocker(
        &mut task,
        "blk-001".to_string(),
        BlockerKind::Human,
        None,
        "Needs answer".to_string(),
        "Question open".to_string(),
        "t1".to_string(),
    );

    assert_eq!(task.state, TaskState::Ready);
    assert!(has_unresolved_blockers(&task));

    resolve_blocker(&mut task, "blk-001", "t2".to_string())
        .expect("invariant: blocker should resolve");
    assert_eq!(task.state, TaskState::Ready);
    assert!(!has_unresolved_blockers(&task));
}

#[test]
fn terminal_transitions_are_allowed_from_non_terminal_states() {
    let mut task = TaskRecord::draft("task-001", "Add CSV export", "t0");

    transition(
        &mut task,
        TaskState::Rejected,
        "human",
        "t1",
        TransitionDetails::default(),
    )
    .expect("invariant: draft can be rejected");

    assert_eq!(task.state, TaskState::Rejected);
}

#[test]
fn invalid_transition_names_the_current_and_target_states() {
    let mut task = TaskRecord::draft("task-001", "Add CSV export", "t0");
    task.state = TaskState::Exploring;

    let error = transition(
        &mut task,
        TaskState::InProgress,
        "codex",
        "t1",
        TransitionDetails::default(),
    )
    .expect_err("invariant: exploring cannot jump straight to in_progress");
    let message = error.to_string();
    assert!(
        message.contains("exploring"),
        "error should name the current state: {message}"
    );
    assert!(
        message.contains("in_progress"),
        "error should name the target state: {message}"
    );
}

#[test]
fn generic_lifecycle_cannot_mark_verified() {
    let mut task = TaskRecord::draft("task-001", "Add CSV export", "t0");
    task.state = TaskState::NeedsVerification;

    let error = transition(
        &mut task,
        TaskState::Verified,
        "codex",
        "t1",
        TransitionDetails::default(),
    )
    .expect_err("invariant: verification subsystem owns verified transition");

    assert!(error.to_string().contains("verification subsystem"));
}
