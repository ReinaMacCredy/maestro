use anyhow::{Result, anyhow};

use crate::domain::card;
use crate::domain::memory::{MemoryLifecycle, MemoryScope, ScopeKind, SignalType, TargetSurface};
use crate::foundation::core::paths::{MaestroPaths, discover_repo_root};
use crate::foundation::core::time::utc_now_timestamp;
use crate::operations::memory::{
    ApplyPromotionRequest, ApprovedMemory, CreateMemoryRequest, CreateSuggestionRequest,
    MaintenanceBudget, MaintenanceLevel, MaintenanceRequest, MemoryReadScope, MemoryReadSurface,
    MemorySuggestion, PlanPromotionRequest, SuggestionStatus, apply_promotion, approved_memory,
    attach_scorer_contract, create_maintenance_contract, create_memory, create_suggestion,
    dismiss_suggestion, list_suggestions, parse_source_ref, plan_promotion,
};

use super::{
    MemoryArgs, MemoryCommand, MemoryMaintainArgs, MemoryScorerCommand, MemorySuggestCommand,
};

pub fn run(args: MemoryArgs) -> Result<()> {
    let paths = MaestroPaths::new(discover_repo_root()?);
    match args.command {
        MemoryCommand::Create {
            from,
            summary,
            lesson,
            signal_type,
            scope_kind,
            scope_ref,
            target_surface,
            id_only,
        } => {
            let signal_type = signal_type.as_deref().map(parse_signal_type).transpose()?;
            let scope = MemoryScope {
                kind: parse_scope_kind(&scope_kind)?,
                refs: scope_ref,
            };
            let target_surface = parse_target_surface(&target_surface)?;
            let outcome = create_memory(
                &paths,
                CreateMemoryRequest {
                    from,
                    summary,
                    lesson,
                    signal_type,
                    scope: Some(scope),
                    target_surface: Some(target_surface),
                },
                &utc_now_timestamp(),
            )?;
            if id_only {
                println!("{}", outcome.id);
            } else if let Some(suggestion) = outcome.from_suggestion {
                println!("created {} (memory) from {}", outcome.id, suggestion);
            } else {
                println!("created {} (memory)", outcome.id);
            }
            Ok(())
        }
        MemoryCommand::List { all } => {
            let cards = card::query::scan(&paths)?;
            let mut shown = 0usize;
            for card in cards
                .iter()
                .filter(|card| card.card_type == card::schema::CardType::Memory)
            {
                let candidate = crate::domain::memory::validate_card(&paths, card)?;
                if !all && candidate.memory.lifecycle != MemoryLifecycle::Promoted {
                    continue;
                }
                println!(
                    "{} {} lifecycle={} scope={} summary={}",
                    card.id,
                    card.status,
                    candidate.memory.lifecycle.as_str(),
                    candidate.memory.scope.kind.as_str(),
                    card.title
                );
                shown += 1;
            }
            if shown == 0 {
                println!("no memory cards");
            }
            Ok(())
        }
        MemoryCommand::Show { id } => {
            let resolved = card::store::resolve(&paths, &id)?
                .ok_or_else(|| anyhow!("memory card {id} not found"))?;
            if resolved.card.card_type != card::schema::CardType::Memory {
                return Err(anyhow!("card {id} is not a memory card"));
            }
            let candidate = crate::domain::memory::validate_card(&paths, &resolved.card)?;
            let lesson_path = crate::domain::memory::memory_dir(&paths, &id)
                .join(crate::domain::memory::LESSON_FILE);
            let lesson = std::fs::read_to_string(&lesson_path)?;
            println!(
                "{} memory {} lifecycle={} target={} scope={}",
                resolved.card.id,
                resolved.card.title,
                candidate.memory.lifecycle.as_str(),
                candidate.memory.target_surface.as_str(),
                candidate.memory.scope.kind.as_str()
            );
            println!("lesson: {}", lesson_path.display());
            println!();
            print!("{lesson}");
            if !lesson.ends_with('\n') {
                println!();
            }
            Ok(())
        }
        MemoryCommand::Search { query } => {
            let set = approved_memory(
                &paths,
                MemoryReadSurface::Search,
                MemoryReadScope {
                    query: Some(query.join(" ")),
                    ..MemoryReadScope::default()
                },
            )?;
            render_approved_memory(&set.memories, set.omitted);
            Ok(())
        }
        MemoryCommand::Promote {
            id,
            plan,
            apply,
            scorer_receipt,
            review_evidence,
        } => {
            if plan == apply {
                return Err(anyhow!(
                    "memory promote requires exactly one of --plan or --apply"
                ));
            }
            if plan {
                let outcome = plan_promotion(
                    &paths,
                    PlanPromotionRequest {
                        memory_id: id,
                        scorer_receipt,
                        review_evidence,
                    },
                    &utc_now_timestamp(),
                )?;
                let mode = if outcome.review_only {
                    "review-only"
                } else {
                    "gated"
                };
                println!(
                    "planned {} ({mode}) path={}",
                    outcome.id,
                    outcome.path.display()
                );
            } else {
                if scorer_receipt.is_some() || review_evidence.is_some() {
                    return Err(anyhow!(
                        "--scorer-receipt and --review-evidence are accepted only with --plan"
                    ));
                }
                let outcome = apply_promotion(
                    &paths,
                    ApplyPromotionRequest { promotion_id: id },
                    &utc_now_timestamp(),
                )?;
                if let Some(backup) = outcome.backup_path {
                    println!(
                        "applied {} target={} backup={}",
                        outcome.id,
                        outcome.target_path.display(),
                        backup.display()
                    );
                } else {
                    println!(
                        "applied {} target={} backup=<none>",
                        outcome.id,
                        outcome.target_path.display()
                    );
                }
            }
            Ok(())
        }
        MemoryCommand::Maintain(args) => {
            run_maintenance(&paths, args, MaintenanceLevel::L1LocalTidy)
        }
        MemoryCommand::Dream(args) => {
            run_maintenance(&paths, args, MaintenanceLevel::L2FocusedRepair)
        }
        MemoryCommand::Scorer(args) => match args.command {
            MemoryScorerCommand::Attach { id, contract_file } => {
                let contract = std::fs::read_to_string(&contract_file)?;
                let outcome = attach_scorer_contract(&paths, &id, &contract)?;
                println!(
                    "attached scorer {} to {}",
                    outcome.scorer_type.as_str(),
                    outcome.id
                );
                Ok(())
            }
        },
        MemoryCommand::Suggest(args) => match args.command {
            MemorySuggestCommand::List { all } => {
                let suggestions = list_suggestions(&paths, all)?;
                render_suggestions(&suggestions);
                Ok(())
            }
            MemorySuggestCommand::Create {
                source_ref,
                signal_type,
                summary,
                scope_kind,
                scope_ref,
                target_surface,
                dedupe_key,
                expires_at,
            } => {
                if source_ref.is_empty() {
                    return Err(anyhow!("memory suggest create requires --source-ref"));
                }
                let now = utc_now_timestamp();
                let outcome = create_suggestion(
                    &paths,
                    CreateSuggestionRequest {
                        source_refs: source_ref.iter().map(|raw| parse_source_ref(raw)).collect(),
                        signal_type: parse_signal_type(&signal_type)?,
                        summary,
                        scope: MemoryScope {
                            kind: parse_scope_kind(&scope_kind)?,
                            refs: scope_ref,
                        },
                        target_surface: parse_target_surface(&target_surface)?,
                        dedupe_key,
                        expires_at,
                    },
                    &now,
                )?;
                let action = if outcome.created {
                    "created"
                } else {
                    "updated"
                };
                println!(
                    "{action} {} ({}) sources={} summary={}",
                    outcome.suggestion.id,
                    outcome.suggestion.status.as_str(),
                    outcome.suggestion.source_refs.len(),
                    outcome.suggestion.summary
                );
                println!(
                    "create: maestro memory create --from {}",
                    outcome.suggestion.id
                );
                println!(
                    "dismiss: maestro memory suggest dismiss {} --reason \"<reason>\"",
                    outcome.suggestion.id
                );
                Ok(())
            }
            MemorySuggestCommand::Dismiss { id, reason } => {
                let outcome = dismiss_suggestion(
                    &paths,
                    &id,
                    &reason,
                    &super::actor(),
                    &utc_now_timestamp(),
                )?;
                println!(
                    "dismissed {}: {}",
                    outcome.suggestion.id,
                    outcome
                        .suggestion
                        .dismissal_reason
                        .as_deref()
                        .unwrap_or("<no reason>")
                );
                Ok(())
            }
        },
    }
}

fn run_maintenance(
    paths: &MaestroPaths,
    args: MemoryMaintainArgs,
    default_level: MaintenanceLevel,
) -> Result<()> {
    let level = args
        .level
        .as_deref()
        .map(parse_maintenance_level)
        .transpose()?
        .unwrap_or(default_level);
    let explicit_budget = parse_explicit_budget(&args)?;
    let outcome = create_maintenance_contract(
        paths,
        MaintenanceRequest {
            level,
            scope: MemoryScope {
                kind: parse_scope_kind(&args.scope_kind)?,
                refs: args.scope_ref,
            },
            source_refs: args
                .source_ref
                .iter()
                .map(|raw| parse_source_ref(raw))
                .collect(),
            reason: args.reason,
            proof_links: args.proof_link,
            run_links: args.run_link,
            human_approved: args.human_approved,
            explicit_budget,
        },
        &utc_now_timestamp(),
    )?;
    if args.id_only {
        println!("{}", outcome.id);
    } else if let Some(path) = outcome.contract_path {
        println!(
            "wrote {} level={} contract={}",
            outcome.id,
            outcome.level.as_str(),
            path.display()
        );
    } else {
        println!(
            "recorded {} level={} contract=<none>",
            outcome.id,
            outcome.level.as_str()
        );
    }
    Ok(())
}

fn render_suggestions(suggestions: &[MemorySuggestion]) {
    if suggestions.is_empty() {
        println!("no memory suggestions");
        return;
    }
    for suggestion in suggestions {
        println!(
            "{} {} scope={} sources={} summary={}",
            suggestion.id,
            suggestion.status.as_str(),
            suggestion.scope.kind.as_str(),
            suggestion.source_refs.len(),
            suggestion.summary
        );
        if suggestion.status == SuggestionStatus::Open {
            println!("  create: maestro memory create --from {}", suggestion.id);
            println!(
                "  dismiss: maestro memory suggest dismiss {} --reason \"<reason>\"",
                suggestion.id
            );
        }
    }
}

pub(crate) fn render_approved_memory(memories: &[ApprovedMemory], omitted: usize) {
    if memories.is_empty() {
        println!("no approved memory");
        return;
    }
    println!("APPROVED MEMORY");
    for memory in memories {
        println!(
            "{}. {} scope={} risk={} {}",
            memory.rank,
            memory.id,
            memory.scope_kind.as_str(),
            memory.risk.as_str(),
            memory.summary
        );
        println!("   show: {}", memory.show_command);
    }
    if omitted > 0 {
        println!("... {omitted} omitted; search with `maestro memory search <query>`");
    }
}

fn parse_signal_type(word: &str) -> Result<SignalType> {
    SignalType::parse(word).ok_or_else(|| {
        anyhow!(
            "unknown signal type {word:?}; expected failure, user_correction, verified_success, repeated_block, loop_hard_stop, good_run, manual_final_decision, approval, rejection, or health_signal"
        )
    })
}

fn parse_scope_kind(word: &str) -> Result<ScopeKind> {
    ScopeKind::parse(word).ok_or_else(|| {
        anyhow!("unknown scope kind {word:?}; expected task, card, feature, project, repo, global, or team")
    })
}

fn parse_target_surface(word: &str) -> Result<TargetSurface> {
    TargetSurface::parse(word).ok_or_else(|| {
        anyhow!(
            "unknown target surface {word:?}; expected memory_note, local_skill, shipped_skill, recurrence_guard, harness_policy, hook, cli_behavior, or external_action"
        )
    })
}

fn parse_maintenance_level(word: &str) -> Result<MaintenanceLevel> {
    MaintenanceLevel::parse(word).ok_or_else(|| {
        anyhow!(
            "unknown maintenance level {word:?}; expected L0, L1, L2, L3, l0_detect, l1_local_tidy, l2_focused_repair, or l3_deep_rebuild"
        )
    })
}

fn parse_explicit_budget(args: &MemoryMaintainArgs) -> Result<Option<MaintenanceBudget>> {
    let values = [
        args.tokens,
        args.wall_minutes,
        args.max_source_refs,
        args.max_files,
        args.subagents,
    ];
    if values.iter().all(Option::is_none) {
        return Ok(None);
    }
    let Some(tokens) = args.tokens else {
        return Err(anyhow!("explicit maintenance budget requires --tokens"));
    };
    let Some(wall_minutes) = args.wall_minutes else {
        return Err(anyhow!(
            "explicit maintenance budget requires --wall-minutes"
        ));
    };
    let Some(max_source_refs) = args.max_source_refs else {
        return Err(anyhow!(
            "explicit maintenance budget requires --max-source-refs"
        ));
    };
    let Some(max_files) = args.max_files else {
        return Err(anyhow!("explicit maintenance budget requires --max-files"));
    };
    let Some(subagents) = args.subagents else {
        return Err(anyhow!("explicit maintenance budget requires --subagents"));
    };
    Ok(Some(MaintenanceBudget {
        tokens,
        wall_minutes,
        max_source_refs,
        max_files,
        subagents,
    }))
}
