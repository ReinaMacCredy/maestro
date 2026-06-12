//! Skill catalog and extraction helpers.

pub mod catalog;
pub mod extract;
pub mod symlink;

mod global;

pub use global::{
    GlobalSkillDrift, GlobalSkillsOutcome, GlobalSkillsStatus, PreparedGlobalSkills,
    SkillVersionChange, global_skills_status, global_skills_status_at, prepare_global_skills,
    prepare_global_skills_at, prepare_global_skills_if_locked, render_global_skills_dry_run,
    render_global_skills_outcome, sync_global_skills, sync_global_skills_at,
    sync_global_skills_if_locked, write_prepared_global_skills,
};
