use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Result, bail};
use serde::Serialize;

use crate::domain::card;
use crate::domain::proof;
use crate::domain::run::{self, Presence};
use crate::domain::task;
use crate::foundation::core::git;
use crate::foundation::core::paths::MaestroPaths;
use crate::foundation::core::time::utc_now_timestamp;

const SNAPSHOT_SCHEMA: &str = "maestro.mission_control.snapshot.v1";
const DEFAULT_WIDTH: usize = 120;
const DEFAULT_HEIGHT: usize = 40;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PreviewScreen {
    Dashboard,
    Cards,
    Tasks,
    Activity,
    Proof,
    Config,
    Help,
}

impl PreviewScreen {
    pub fn parse(value: &str) -> Result<Self> {
        match value.to_ascii_lowercase().as_str() {
            "dashboard" | "dash" | "home" => Ok(Self::Dashboard),
            "cards" | "card" | "features" | "feature" | "feat" => Ok(Self::Cards),
            "tasks" | "task" => Ok(Self::Tasks),
            "activity" | "events" | "event" | "sessions" | "session" => Ok(Self::Activity),
            "proof" | "verify" | "verification" => Ok(Self::Proof),
            "config" | "cfg" | "environment" | "env" => Ok(Self::Config),
            "help" => Ok(Self::Help),
            other => bail!("unknown preview screen '{other}'"),
        }
    }

    fn all() -> &'static [Self] {
        &[
            Self::Dashboard,
            Self::Cards,
            Self::Tasks,
            Self::Activity,
            Self::Proof,
            Self::Config,
            Self::Help,
        ]
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Dashboard => "dashboard",
            Self::Cards => "cards",
            Self::Tasks => "tasks",
            Self::Activity => "activity",
            Self::Proof => "proof",
            Self::Config => "config",
            Self::Help => "help",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PreviewFormat {
    Plain,
    Ansi,
}

#[derive(Clone, Copy, Debug)]
pub struct RenderOptions<'a> {
    pub screen: Option<PreviewScreen>,
    pub feature: Option<&'a str>,
    pub width: Option<usize>,
    pub height: Option<usize>,
    pub format: PreviewFormat,
}

#[derive(Debug, Serialize)]
pub struct MissionControlSnapshot {
    pub schema: &'static str,
    pub mode: &'static str,
    pub repo: RepoSnapshot,
    pub summary: SummarySnapshot,
    pub features: Vec<FeatureSnapshot>,
    pub tasks: Vec<TaskSnapshot>,
    pub sessions: Vec<SessionSnapshot>,
    pub proof: ProofSnapshot,
    pub config: ConfigSnapshot,
}

#[derive(Debug, Serialize)]
pub struct RepoSnapshot {
    pub root: String,
    pub branch: Option<String>,
    pub dirty: bool,
    pub code_other_dirty: usize,
    pub maestro_dirty: usize,
}

#[derive(Debug, Serialize)]
pub struct SummarySnapshot {
    pub features: usize,
    pub workable_cards: usize,
    pub ready: usize,
    pub active: usize,
    pub needs_verification: usize,
    pub blocked: usize,
    pub done: usize,
    pub live_sessions: usize,
}

#[derive(Debug, Serialize)]
pub struct FeatureSnapshot {
    pub id: String,
    pub title: String,
    pub status: String,
    pub total: usize,
    pub ready: usize,
    pub active: usize,
    pub needs_verification: usize,
    pub blocked: usize,
    pub done: usize,
}

#[derive(Debug, Serialize)]
pub struct TaskSnapshot {
    pub id: String,
    pub title: String,
    pub status: String,
    pub state: &'static str,
    pub parent: Option<String>,
    pub claimed_by: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SessionSnapshot {
    pub session_id: String,
    pub agent_runtime: Option<String>,
    pub mode: Option<String>,
    pub bound_card: Option<String>,
    pub last_action: String,
    pub age_minutes: u64,
    pub presence: &'static str,
}

#[derive(Debug, Serialize)]
pub struct ProofSnapshot {
    pub needs_verification: usize,
    pub verified_or_done: usize,
    pub blocked: usize,
    pub proof_missing: usize,
    pub proof_failed: usize,
    pub proof_accepted: usize,
    pub proof_stale: usize,
}

#[derive(Debug, Serialize)]
pub struct ConfigSnapshot {
    pub preview_screens: Vec<&'static str>,
    pub read_only: bool,
    pub source: &'static str,
}

#[derive(Debug, Serialize)]
pub struct RenderCheckResult {
    pub schema: &'static str,
    pub ok: bool,
    pub width: usize,
    pub height: usize,
    pub screens: Vec<RenderCheckScreen>,
}

#[derive(Debug, Serialize)]
pub struct RenderCheckScreen {
    pub screen: &'static str,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Clone, Debug)]
struct CardView {
    id: String,
    title: String,
    status: String,
    parent: Option<String>,
    claimed_by: Option<String>,
    state: Option<card::query::RowState>,
    is_feature: bool,
}

pub fn snapshot(paths: &MaestroPaths) -> Result<MissionControlSnapshot> {
    let mut cards: Vec<_> = card::query::scan_with_failures(paths)?
        .cards
        .into_iter()
        .map(|(card, _)| card)
        .collect();
    cards.sort_by(|left, right| left.id.cmp(&right.id));
    let blocked_ids: BTreeSet<String> = card::query::blocked(&cards)
        .into_iter()
        .map(|card| card.id.clone())
        .collect();
    let sessions = active_sessions(paths);
    let git = git::snapshot(paths.repo_root()).ok();
    let views: Vec<CardView> = cards
        .iter()
        .map(|card| CardView {
            id: card.id.clone(),
            title: card.title.clone(),
            status: card.status.clone(),
            parent: card.parent.clone(),
            claimed_by: card.claimed_by.clone(),
            state: card::query::classify(card, &blocked_ids),
            is_feature: card::query::feature_of(card).is_some_and(|feature| feature == card.id),
        })
        .collect();
    let features = feature_snapshots(&views);
    let tasks = task_snapshots(&views);
    let counts = counts_from_views(views.iter());
    let proof = proof_snapshot(
        paths,
        git.as_ref().and_then(|git| git.head.clone()),
        &counts,
    )?;
    let feature_count = views.iter().filter(|card| card.is_feature).count();
    Ok(MissionControlSnapshot {
        schema: SNAPSHOT_SCHEMA,
        mode: "home",
        repo: RepoSnapshot {
            root: paths.repo_root().display().to_string(),
            branch: git.as_ref().and_then(|git| git.branch.clone()),
            dirty: git.as_ref().is_some_and(|git| git.dirty),
            code_other_dirty: git.as_ref().map_or(0, |git| git.code_other_dirty),
            maestro_dirty: git.as_ref().map_or(0, |git| git.maestro_dirty),
        },
        summary: SummarySnapshot {
            features: feature_count,
            workable_cards: counts.total(),
            ready: counts.ready,
            active: counts.active,
            needs_verification: counts.needs_verification,
            blocked: counts.blocked,
            done: counts.done,
            live_sessions: sessions.len(),
        },
        features,
        tasks,
        sessions,
        proof,
        config: ConfigSnapshot {
            preview_screens: PreviewScreen::all()
                .iter()
                .map(|screen| screen.as_str())
                .collect(),
            read_only: true,
            source: "current Maestro card/task/run/proof read models",
        },
    })
}

pub fn render_preview(paths: &MaestroPaths, options: RenderOptions<'_>) -> Result<String> {
    let snapshot = snapshot(paths)?;
    Ok(render_snapshot(&snapshot, options))
}

pub fn render_check(
    paths: &MaestroPaths,
    size: Option<(usize, usize)>,
) -> Result<RenderCheckResult> {
    let (width, height) = size.unwrap_or((DEFAULT_WIDTH, DEFAULT_HEIGHT));
    let snapshot = snapshot(paths)?;
    let mut screens = Vec::new();
    for screen in PreviewScreen::all() {
        let result = std::panic::catch_unwind(|| {
            render_snapshot(
                &snapshot,
                RenderOptions {
                    screen: Some(*screen),
                    feature: None,
                    width: Some(width),
                    height: Some(height),
                    format: PreviewFormat::Plain,
                },
            )
        });
        match result {
            Ok(frame) if !frame.trim().is_empty() => screens.push(RenderCheckScreen {
                screen: screen.as_str(),
                ok: true,
                error: None,
            }),
            Ok(_) => screens.push(RenderCheckScreen {
                screen: screen.as_str(),
                ok: false,
                error: Some("empty frame".to_string()),
            }),
            Err(_) => screens.push(RenderCheckScreen {
                screen: screen.as_str(),
                ok: false,
                error: Some("render panic".to_string()),
            }),
        }
    }
    Ok(RenderCheckResult {
        schema: "maestro.mission_control.render_check.v1",
        ok: screens.iter().all(|screen| screen.ok),
        width,
        height,
        screens,
    })
}

fn active_sessions(paths: &MaestroPaths) -> Vec<SessionSnapshot> {
    let roots = crate::interfaces::cli::worktree_roots(paths);
    let now = utc_now_timestamp();
    run::active_sessions_union(&roots, &now)
        .unwrap_or_default()
        .into_iter()
        .filter(|session| session.presence != Presence::Stale)
        .map(|session| SessionSnapshot {
            session_id: session.session_id,
            agent_runtime: session.agent_runtime,
            mode: session.mode,
            bound_card: session.bound_card,
            last_action: session.last_action,
            age_minutes: session.age_minutes,
            presence: presence_label(session.presence),
        })
        .collect()
}

fn feature_snapshots(cards: &[CardView]) -> Vec<FeatureSnapshot> {
    let mut children: BTreeMap<&str, Vec<&CardView>> = BTreeMap::new();
    for card in cards {
        if card.state.is_some()
            && let Some(parent) = card.parent.as_deref()
        {
            children.entry(parent).or_default().push(card);
        }
    }
    cards
        .iter()
        .filter(|card| card.is_feature)
        .map(|feature| {
            let counts = counts_from_views(
                children
                    .get(feature.id.as_str())
                    .into_iter()
                    .flat_map(|children| children.iter().copied()),
            );
            FeatureSnapshot {
                id: feature.id.clone(),
                title: feature.title.clone(),
                status: feature.status.clone(),
                total: counts.total(),
                ready: counts.ready,
                active: counts.active,
                needs_verification: counts.needs_verification,
                blocked: counts.blocked,
                done: counts.done,
            }
        })
        .collect()
}

fn task_snapshots(cards: &[CardView]) -> Vec<TaskSnapshot> {
    cards
        .iter()
        .filter_map(|card| {
            card.state.map(|state| TaskSnapshot {
                id: card.id.clone(),
                title: card.title.clone(),
                status: card.status.clone(),
                state: row_state_label(state),
                parent: card.parent.clone(),
                claimed_by: card.claimed_by.clone(),
            })
        })
        .collect()
}

fn counts_from_views<'a>(
    cards: impl IntoIterator<Item = &'a CardView>,
) -> card::query::RowStateCounts {
    let mut counts = card::query::RowStateCounts::default();
    for card in cards {
        match card.state {
            Some(card::query::RowState::Done) => counts.done += 1,
            Some(card::query::RowState::Blocked) => counts.blocked += 1,
            Some(card::query::RowState::NeedsVerification) => counts.needs_verification += 1,
            Some(card::query::RowState::Active) => counts.active += 1,
            Some(card::query::RowState::Ready) => counts.ready += 1,
            None => {}
        }
    }
    counts
}

fn proof_snapshot(
    paths: &MaestroPaths,
    current_commit: Option<String>,
    counts: &card::query::RowStateCounts,
) -> Result<ProofSnapshot> {
    let mut proof_missing = 0;
    let mut proof_failed = 0;
    let mut proof_accepted = 0;
    let mut proof_stale = 0;

    for (task, _) in task::cards::scan(paths)? {
        match proof::proof_status_kind_for_task(&task, current_commit.clone())? {
            proof::ProofStatusKind::Missing => proof_missing += 1,
            proof::ProofStatusKind::Failed => proof_failed += 1,
            proof::ProofStatusKind::Accepted => proof_accepted += 1,
            proof::ProofStatusKind::Stale => proof_stale += 1,
        }
    }

    Ok(ProofSnapshot {
        needs_verification: counts.needs_verification,
        verified_or_done: counts.done,
        blocked: counts.blocked,
        proof_missing,
        proof_failed,
        proof_accepted,
        proof_stale,
    })
}

fn render_snapshot(snapshot: &MissionControlSnapshot, options: RenderOptions<'_>) -> String {
    let width = options.width.unwrap_or(DEFAULT_WIDTH).max(40);
    let height = options.height.unwrap_or(DEFAULT_HEIGHT).max(20);
    let screen = options.screen.unwrap_or(PreviewScreen::Dashboard);
    let mut frame = String::new();
    push_header(&mut frame, snapshot, width, screen);
    match screen {
        PreviewScreen::Dashboard => push_dashboard(&mut frame, snapshot, options.feature, width),
        PreviewScreen::Cards => push_cards(&mut frame, snapshot, options.feature, width),
        PreviewScreen::Tasks => push_tasks(&mut frame, snapshot, width),
        PreviewScreen::Activity => push_activity(&mut frame, snapshot, width),
        PreviewScreen::Proof => push_proof(&mut frame, snapshot, width),
        PreviewScreen::Config => push_config(&mut frame, snapshot, width),
        PreviewScreen::Help => push_help(&mut frame, width),
    }
    push_footer(&mut frame, width, height, options.format);
    frame
}

fn push_header(
    frame: &mut String,
    snapshot: &MissionControlSnapshot,
    width: usize,
    screen: PreviewScreen,
) {
    let branch = snapshot.repo.branch.as_deref().unwrap_or("detached");
    frame.push_str(&format!(
        "Mission Control | {} | branch {branch} | {screen}\n",
        snapshot.repo.root,
        screen = screen.as_str()
    ));
    frame.push_str(&format!(
        "features {} | work {} | ready {} | active {} | verify {} | blocked {} | sessions {}\n",
        snapshot.summary.features,
        snapshot.summary.workable_cards,
        snapshot.summary.ready,
        snapshot.summary.active,
        snapshot.summary.needs_verification,
        snapshot.summary.blocked,
        snapshot.summary.live_sessions
    ));
    frame.push_str(&rule(width));
}

fn push_dashboard(
    frame: &mut String,
    snapshot: &MissionControlSnapshot,
    feature: Option<&str>,
    width: usize,
) {
    push_panel(frame, "Overview", width, |out| {
        out.push(format!(
            "Repo dirty: {} (code/other {}, maestro {})",
            snapshot.repo.dirty, snapshot.repo.code_other_dirty, snapshot.repo.maestro_dirty
        ));
        out.push(format!("Current source: {}", snapshot.config.source));
        out.push("Read-only restore slice: preview/json/render-check only".to_string());
    });
    push_cards(frame, snapshot, feature, width);
    push_activity(frame, snapshot, width);
    push_proof(frame, snapshot, width);
}

fn push_cards(
    frame: &mut String,
    snapshot: &MissionControlSnapshot,
    feature: Option<&str>,
    width: usize,
) {
    push_panel(frame, "Cards / Features", width, |out| {
        let rows = snapshot
            .features
            .iter()
            .filter(|row| feature.is_none_or(|feature| row.id == feature));
        for feature in rows.take(8) {
            out.push(format!(
                "{} | {} | total {} ready {} active {} verify {} blocked {} done {}",
                feature.id,
                feature.title,
                feature.total,
                feature.ready,
                feature.active,
                feature.needs_verification,
                feature.blocked,
                feature.done
            ));
        }
        if out.is_empty() {
            out.push("No feature cards found for this selection.".to_string());
        }
    });
}

fn push_tasks(frame: &mut String, snapshot: &MissionControlSnapshot, width: usize) {
    push_panel(frame, "Tasks", width, |out| {
        for task in snapshot.tasks.iter().take(12) {
            let owner = task.claimed_by.as_deref().unwrap_or("unclaimed");
            out.push(format!(
                "{} | {} | {} | {}",
                task.state, task.id, task.title, owner
            ));
        }
        if out.is_empty() {
            out.push("No task/bug/chore cards found.".to_string());
        }
    });
}

fn push_activity(frame: &mut String, snapshot: &MissionControlSnapshot, width: usize) {
    push_panel(frame, "Activity / Events", width, |out| {
        for session in snapshot.sessions.iter().take(8) {
            let agent = session.agent_runtime.as_deref().unwrap_or("-");
            let mode = session.mode.as_deref().unwrap_or("-");
            let card = session.bound_card.as_deref().unwrap_or("-");
            out.push(format!(
                "{agent} | {mode} | {card} | {} | {}m | {}",
                session.presence, session.age_minutes, session.last_action
            ));
        }
        if out.is_empty() {
            out.push("No live sessions.".to_string());
        }
    });
}

fn push_proof(frame: &mut String, snapshot: &MissionControlSnapshot, width: usize) {
    push_panel(frame, "Proof / Verify", width, |out| {
        out.push(format!(
            "needs_verification {} | verified/done {} | blocked {}",
            snapshot.proof.needs_verification,
            snapshot.proof.verified_or_done,
            snapshot.proof.blocked
        ));
        out.push(format!(
            "proof missing {} | failed {} | accepted {} | stale {}",
            snapshot.proof.proof_missing,
            snapshot.proof.proof_failed,
            snapshot.proof.proof_accepted,
            snapshot.proof.proof_stale
        ));
        out.push(
            "Verification details stay one command away: `maestro task proof <id>`.".to_string(),
        );
    });
}

fn push_config(frame: &mut String, snapshot: &MissionControlSnapshot, width: usize) {
    push_panel(frame, "Config / Environment", width, |out| {
        out.push(format!("root: {}", snapshot.repo.root));
        out.push(format!(
            "branch: {}",
            snapshot.repo.branch.as_deref().unwrap_or("detached")
        ));
        out.push(format!("read_only: {}", snapshot.config.read_only));
        out.push(format!(
            "screens: {}",
            snapshot.config.preview_screens.join(", ")
        ));
    });
}

fn push_help(frame: &mut String, width: usize) {
    push_panel(frame, "Help", width, |out| {
        out.push(
            "maestro mission-control --preview [dashboard|cards|tasks|activity|proof|config|help]"
                .to_string(),
        );
        out.push("maestro mission-control --json".to_string());
        out.push("maestro mission-control --render-check --size 120x40".to_string());
        out.push(
            "This slice is read-only; interactive writes are intentionally deferred.".to_string(),
        );
    });
}

fn push_footer(frame: &mut String, width: usize, height: usize, format: PreviewFormat) {
    frame.push_str(&rule(width));
    frame.push_str(&format!(
        "preview {width}x{height} | format {} | restored shell, current Maestro data\n",
        match format {
            PreviewFormat::Plain => "plain",
            PreviewFormat::Ansi => "ansi",
        }
    ));
}

fn push_panel<F>(frame: &mut String, title: &str, width: usize, build: F)
where
    F: FnOnce(&mut Vec<String>),
{
    let mut lines = Vec::new();
    build(&mut lines);
    frame.push_str(&format!("[ {title} ]\n"));
    for line in lines {
        frame.push_str("  ");
        frame.push_str(&truncate(&line, width.saturating_sub(2)));
        frame.push('\n');
    }
    frame.push('\n');
}

fn rule(width: usize) -> String {
    format!("{}\n", "-".repeat(width.min(120)))
}

fn truncate(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        return value.to_string();
    }
    let head: String = value.chars().take(max.saturating_sub(1)).collect();
    format!("{head}.")
}

fn row_state_label(state: card::query::RowState) -> &'static str {
    match state {
        card::query::RowState::Done => "done",
        card::query::RowState::Blocked => "blocked",
        card::query::RowState::NeedsVerification => "needs_verification",
        card::query::RowState::Active => "active",
        card::query::RowState::Ready => "ready",
    }
}

fn presence_label(presence: Presence) -> &'static str {
    match presence {
        Presence::Working => "working",
        Presence::QuietWorking => "quiet-working",
        Presence::Waiting => "waiting",
        Presence::Released => "released",
        Presence::Done => "done",
        Presence::Unconfirmed => "unconfirmed",
        Presence::Idle => "idle",
        Presence::Stale => "stale",
    }
}

impl std::fmt::Display for PreviewScreen {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::{PreviewScreen, RenderOptions, SNAPSHOT_SCHEMA};
    use crate::interfaces::tui::mission_control::{PreviewFormat, render_snapshot};

    #[test]
    fn preview_screen_accepts_old_aliases() {
        assert_eq!(
            PreviewScreen::parse("dashboard").unwrap(),
            PreviewScreen::Dashboard
        );
        assert_eq!(PreviewScreen::parse("feat").unwrap(), PreviewScreen::Cards);
        assert_eq!(
            PreviewScreen::parse("events").unwrap(),
            PreviewScreen::Activity
        );
        assert_eq!(
            PreviewScreen::parse("verify").unwrap(),
            PreviewScreen::Proof
        );
    }

    #[test]
    fn render_snapshot_includes_mission_control_sections() {
        let snapshot = super::MissionControlSnapshot {
            schema: SNAPSHOT_SCHEMA,
            mode: "home",
            repo: super::RepoSnapshot {
                root: "/tmp/repo".to_string(),
                branch: Some("main".to_string()),
                dirty: false,
                code_other_dirty: 0,
                maestro_dirty: 0,
            },
            summary: super::SummarySnapshot {
                features: 1,
                workable_cards: 1,
                ready: 1,
                active: 0,
                needs_verification: 0,
                blocked: 0,
                done: 0,
                live_sessions: 0,
            },
            features: vec![super::FeatureSnapshot {
                id: "feature-x".to_string(),
                title: "Feature X".to_string(),
                status: "in_progress".to_string(),
                total: 1,
                ready: 1,
                active: 0,
                needs_verification: 0,
                blocked: 0,
                done: 0,
            }],
            tasks: vec![super::TaskSnapshot {
                id: "task-x".to_string(),
                title: "Task X".to_string(),
                status: "ready".to_string(),
                state: "ready",
                parent: Some("feature-x".to_string()),
                claimed_by: None,
            }],
            sessions: Vec::new(),
            proof: super::ProofSnapshot {
                needs_verification: 0,
                verified_or_done: 0,
                blocked: 0,
                proof_missing: 1,
                proof_failed: 0,
                proof_accepted: 0,
                proof_stale: 0,
            },
            config: super::ConfigSnapshot {
                preview_screens: vec!["dashboard", "cards"],
                read_only: true,
                source: "current Maestro card/task/run/proof read models",
            },
        };
        let frame = render_snapshot(
            &snapshot,
            RenderOptions {
                screen: Some(PreviewScreen::Dashboard),
                feature: None,
                width: Some(120),
                height: Some(40),
                format: PreviewFormat::Plain,
            },
        );
        assert!(frame.contains("Mission Control"));
        assert!(frame.contains("[ Overview ]"));
        assert!(frame.contains("[ Cards / Features ]"));
        assert!(frame.contains("[ Activity / Events ]"));
        assert!(frame.contains("[ Proof / Verify ]"));
    }
}
