//! Skill catalog and the global `~/.maestro/skills` cache.

pub mod catalog;

mod global;

pub use global::{
    GlobalSkillDrift, GlobalSkillsOutcome, GlobalSkillsStatus, PreparedGlobalSkills,
    SkillVersionChange, global_skills_status, global_skills_status_at, prepare_global_skills,
    prepare_global_skills_at, prepare_global_skills_if_locked, render_global_skills_dry_run,
    render_global_skills_outcome, render_global_skills_resync_notice,
    resync_global_skills_if_drifted, resync_global_skills_if_drifted_at, sync_global_skills,
    sync_global_skills_at, sync_global_skills_if_locked, write_prepared_global_skills,
};
