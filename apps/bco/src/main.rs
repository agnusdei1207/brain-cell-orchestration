use clap::{Parser, Subcommand};
use bco_core::{
    IntentDomain, RiskProfile, TaskIntent, ModelRef, ProviderRef, ConnectionProfile,
    ConnectionState, ProviderRegistry, ObjectiveId,
};
use bco_harness::HarnessRegistry;
use bco_orchestrator::{
    BrainCellOrchestrator, OrchestratorRuntime, RuntimeServices, OperatorInput,
    ExecutionContext, CellIdentity, CellType,
};
use bco_session::{SessionBootstrap, SessionId, SessionMeta, SessionState, SessionRuntime};
use bco_tui::{TuiBlueprint, TuiState, ConnectionHealth};
use serde::{Serialize, Deserialize};
use std::path::PathBuf;
use std::fs;

/// Provider config file path
const PROVIDER_CONFIG_PATH: &str = ".bco/providers.json";

/// Model config file path
const MODEL_CONFIG_PATH: &str = ".bco/model_state.json";

#[derive(Parser)]
#[command(name = "bco")]
#[command(version = "0.1.0")]
#[command(about = "brain-cell-orchestration - Dynamic task orchestration runtime")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Execute a task with the orchestration runtime
    Exec {
        /// The objective/task description
        #[arg(trailing_var_arg = true)]
        objective: Vec<String>,
    },
    /// Review the current session or a specific objective
    Review {
        /// Optional objective ID to review
        objective_id: Option<String>,
    },
    /// Resume an interrupted or paused session
    Resume {
        /// Session ID to resume
        session_id: Option<String>,
    },
    /// Fork a session to create a parallel branch
    Fork {
        /// Session ID to fork
        session_id: Option<String>,
    },
    /// Continue the current active step for a session
    Continue {
        /// Session ID to continue, defaults to the most recent session
        session_id: Option<String>,
    },
    /// Grant a pending approval request for a session
    Approve {
        /// Approval request ID
        request_id: String,
        /// Session ID to operate on, defaults to the most recent session
        session_id: Option<String>,
    },
    /// Deny a pending approval request for a session
    Deny {
        /// Approval request ID
        request_id: String,
        /// Session ID to operate on, defaults to the most recent session
        session_id: Option<String>,
        /// Denial reason
        #[arg(trailing_var_arg = true)]
        reason: Vec<String>,
    },
    /// List and manage provider connections
    Providers {
        #[command(subcommand)]
        action: Option<ProviderAction>,
    },
    /// List and manage models
    Models {
        #[command(subcommand)]
        action: Option<ModelAction>,
    },
}

#[derive(Subcommand)]
enum ProviderAction {
    /// List all configured providers
    List,
    /// Add a new provider connection
    Add {
        /// Provider name (e.g., anthropic, openai, ollama)
        name: String,
        /// Provider endpoint (optional)
        endpoint: Option<String>,
    },
    /// Remove a provider
    Remove {
        /// Provider name
        name: String,
    },
}

#[derive(Subcommand)]
enum ModelAction {
    /// List all available models
    List {
        /// Optional provider to filter by
        provider: Option<String>,
    },
    /// Show current active model
    Current,
    /// Switch to a different model
    Switch {
        /// Model in provider/model format
        model: String,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Exec { objective } => {
            exec_command(&objective);
        }
        Commands::Review { objective_id } => {
            review_command(objective_id.as_deref());
        }
        Commands::Resume { session_id } => {
            resume_command(session_id.as_deref());
        }
        Commands::Fork { session_id } => {
            fork_command(session_id.as_deref());
        }
        Commands::Continue { session_id } => {
            continue_command(session_id.as_deref());
        }
        Commands::Approve { request_id, session_id } => {
            approval_command(session_id.as_deref(), &request_id, true, None);
        }
        Commands::Deny { request_id, session_id, reason } => {
            let reason = if reason.is_empty() {
                "operator denied".to_string()
            } else {
                reason.join(" ")
            };
            approval_command(session_id.as_deref(), &request_id, false, Some(reason));
        }
        Commands::Providers { action } => {
            providers_command(action.as_ref());
        }
        Commands::Models { action } => {
            models_command(action.as_ref());
        }
    }
}

fn exec_command(objective: &[String]) {
    let objective_text = if objective.is_empty() {
        "bootstrap a dynamic orchestration runtime".to_string()
    } else {
        objective.join(" ")
    };
    let intent = TaskIntent::new(
        objective_text.clone(),
        infer_domain(&objective_text),
        RiskProfile::Moderate,
    );
    let session = SessionBootstrap::new(session_profile_for(&intent));

    // Bootstrap session
    if let Err(e) = session.bootstrap() {
        eprintln!("Failed to bootstrap session: {}", e);
        std::process::exit(1);
    }

    let registry = HarnessRegistry::with_defaults();
    let harness_kind = registry.resolve(&intent);
    let harness_name = harness_kind.as_str().to_string();
    let harness = registry
        .get_harness(harness_kind)
        .expect("default harness registry should always resolve");
    let capability_policy = harness.capability_policy();
    let plan_policy = harness.plan_policy();
    let review_policy = harness.review_policy();
    let mut services = RuntimeServices::new(capability_policy);
    services.provider_registry = load_provider_registry();
    if let Ok(model) = load_current_model() {
        if let Ok(model_ref) = ModelRef::parse(&model) {
            services.model_manager.set_active(model_ref);
        }
    }
    let runtime = OrchestratorRuntime::new(registry, services)
        .with_session_layout(session.layout().clone());
    let orchestrator = BrainCellOrchestrator::new(HarnessRegistry::with_defaults());
    let blueprint = TuiBlueprint::claude_code_inspired();

    // Print bootstrap info to stderr (TUI will show this in transcript)
    eprintln!("{}", orchestrator.describe_bootstrap(&intent, &session, &blueprint));
    eprintln!("Session ID: {}", session.id());

    // Update session state to Active
    update_session_state(&session, SessionState::Active);

    // Seed the runtime so the local session gets an objective and event log immediately.
    if let Err(error) = runtime.submit(OperatorInput::Execute {
        intent: intent.clone(),
    }) {
        eprintln!("Failed to submit initial turn: {}", error);
    } else {
        let ctx = ExecutionContext::new(
            ObjectiveId::new(),
            CellIdentity::new(CellType::Planner, None),
            harness_kind,
            plan_policy,
            review_policy,
        );
        if let Err(error) = runtime.process_turn(&ctx) {
            eprintln!("Failed to process initial turn: {}", error);
        }
    }

    let _ = sync_initial_pending_work(&session.layout().session_dir());

    let mut state = runtime.build_tui_state(intent.objective());
    state.status.harness = Some(harness_name);
    hydrate_model_status(&mut state);
    state.footer_hint = "Enter: send │ /help: commands │ Ctrl+C: interrupt";

    // Run TUI
    if let Err(e) = bco_tui::run_tui(state) {
        eprintln!("TUI error: {}", e);
    }
}

fn review_command(objective_id: Option<&str>) {
    if let Some(id) = objective_id {
        println!("Reviewing objective: {}", id);
        let session_dir = PathBuf::from(format!(".bco/sessions/{}", id));
        match load_session_snapshot(&session_dir) {
            Ok(snapshot) => print_session_snapshot(&snapshot),
            Err(error) => println!("  Error loading session review: {}", error),
        }
    } else {
        println!("Reviewing current session...");
        match list_sessions() {
            Ok(sessions) => {
                if sessions.is_empty() {
                    println!("  No sessions found.");
                } else {
                    println!("Available sessions:");
                    for session_dir in sessions {
                        if let Ok(snapshot) = load_session_snapshot(&session_dir) {
                            println!(
                                "  [{}] {:?} - {} ({})",
                                snapshot.meta.id,
                                snapshot.meta.state,
                                snapshot.meta.profile,
                                snapshot.meta.created_at.format("%Y-%m-%d %H:%M")
                            );
                            if let Some(ref next_action) = snapshot.next_action {
                                println!("    next: {}", next_action);
                            }
                            if let Some(ref model) = snapshot.runtime.active_model {
                                println!("    model: {}", model);
                            }
                            if !snapshot.pending_work.is_empty() {
                                println!("    pending-work: {}", snapshot.pending_work.len());
                            }
                            if !snapshot.pending_approvals.is_empty() {
                                println!("    approvals: {}", snapshot.pending_approvals.len());
                            }
                            if snapshot.meta.state == SessionState::Paused {
                                println!("    paused: operator action required before autonomous progress");
                            }
                        }
                    }
                }
            }
            Err(e) => println!("  Error listing sessions: {}", e),
        }
    }
}

fn resume_command(session_id: Option<&str>) {
    let session_dir = if let Some(id) = session_id {
        PathBuf::from(format!(".bco/sessions/{}", id))
    } else {
        // Find most recent session
        match find_most_recent_session() {
            Some(dir) => dir,
            None => {
                println!("No sessions found to resume.");
                return;
            }
        }
    };

    println!("Resuming session from: {:?}", session_dir);

    match load_session_snapshot(&session_dir) {
        Ok(snapshot) => {
            println!("  Session ID: {}", snapshot.meta.id);
            println!("  State: {:?}", snapshot.meta.state);
            println!("  Profile: {}", snapshot.meta.profile);
            println!("  Created: {}", snapshot.meta.created_at);

            if let Some(session) = bootstrap_from_existing(&snapshot.meta) {
                let resumed_state = snapshot.meta.state;
                update_session_state(&session, resumed_state);
                println!("  Session resumed successfully.");

                let mut state = snapshot.into_tui_state();
                state.status.resumed = true;
                state.footer_hint = match resumed_state {
                    SessionState::Paused => {
                        "Session paused │ review pending work or operator decision before continue"
                    }
                    SessionState::Completed => {
                        "Session completed │ review artifacts or fork to branch new work"
                    }
                    _ => "Enter: send │ /help: commands │ Ctrl+C: interrupt",
                };
                if let Err(error) = bco_tui::run_tui(state) {
                    eprintln!("TUI error: {}", error);
                }
            }
        }
        Err(e) => println!("  Error loading session: {}", e),
    }
}

fn fork_command(session_id: Option<&str>) {
    let source_dir = if let Some(id) = session_id {
        PathBuf::from(format!(".bco/sessions/{}", id))
    } else {
        match find_most_recent_session() {
            Some(dir) => dir,
            None => {
                println!("No sessions found to fork.");
                return;
            }
        }
    };

    println!("Forking session from: {:?}", source_dir);

    match load_session_snapshot(&source_dir) {
        Ok(source) => {
            let fork_profile = format!("{}-forked", source.meta.profile);
            let new_session = SessionBootstrap::new(fork_profile);
            if let Err(error) = new_session.bootstrap() {
                println!("  Error bootstrapping fork: {}", error);
                return;
            }

            if let Err(error) = copy_session_artifacts(&source_dir, &new_session.layout().session_dir()) {
                println!("  Error copying session artifacts: {}", error);
                return;
            }

            let forked_runtime = SessionRuntime {
                session_id: new_session.id(),
                active_model: source.runtime.active_model.clone(),
                token_usage: source.runtime.token_usage,
                abort_count: source.runtime.abort_count,
                compaction_count: source.runtime.compaction_count,
                last_updated: chrono::Utc::now(),
            };
            if let Ok(json) = serde_json::to_string_pretty(&forked_runtime) {
                let _ = fs::write(new_session.layout().session_runtime_json(), json);
            }

            update_session_state(&new_session, SessionState::Active);
            println!("  Source: {} -> New: {}", source.meta.id, new_session.id());
            println!("  Fork created successfully.");
        }
        Err(e) => println!("  Error forking session: {}", e),
    }
}

fn approval_command(
    session_id: Option<&str>,
    request_id: &str,
    approved: bool,
    reason: Option<String>,
) {
    let session_dir = if let Some(id) = session_id {
        PathBuf::from(format!(".bco/sessions/{}", id))
    } else {
        match find_most_recent_session() {
            Some(dir) => dir,
            None => {
                println!("No sessions found.");
                return;
            }
        }
    };

    match load_session_snapshot(&session_dir) {
        Ok(snapshot) => {
            let pending = snapshot
                .pending_approvals
                .iter()
                .find(|(_, _, _, id)| id == request_id);
            let Some((risk, action, _requested_at, _id)) = pending else {
                println!("Approval request '{}' not found in session {}.", request_id, snapshot.meta.id);
                return;
            };

            let now = chrono::Utc::now();
            let kind = if approved { "granted" } else { "denied" };
            let approvals_entry = serde_json::json!({
                "timestamp": now,
                "kind": kind,
                "request_id": request_id,
                "action": serde_json::Value::Null,
                "risk": serde_json::Value::Null,
                "reason": reason.clone(),
            });
            let approvals_path = session_dir.join("approvals.jsonl");
            let _ = append_jsonl_line(&approvals_path, &approvals_entry);

            let event = if approved {
                serde_json::json!({
                    "timestamp": now,
                    "event": { "ApprovalGranted": { "request_id": request_id } }
                })
            } else {
                serde_json::json!({
                    "timestamp": now,
                    "event": { "ApprovalDenied": { "request_id": request_id, "reason": reason.clone().unwrap_or_else(|| "operator denied".to_string()) } }
                })
            };
            let _ = append_jsonl_line(&session_dir.join("orchestrator_events.jsonl"), &event);

            let transcript_line = if approved {
                format!("[approval] granted {}", action)
            } else {
                format!(
                    "[approval] denied {} ({})",
                    action,
                    reason.clone().unwrap_or_else(|| "operator denied".to_string())
                )
            };
            let transcript = serde_json::json!({
                "timestamp": now,
                "line": transcript_line,
            });
            let _ = append_jsonl_line(&session_dir.join("transcript.jsonl"), &transcript);

            if approved {
                if let Ok(plan) = load_latest_plan_entry(&session_dir) {
                    let next_index = usize::min(
                        plan.active_index.unwrap_or(0).saturating_add(1),
                        plan.steps.len(),
                    );
                    let _ = append_plan_snapshot(&session_dir, &plan.objective_id, &plan.steps, next_index);
                    if next_index >= plan.steps.len() {
                        let completion_event = serde_json::json!({
                            "timestamp": now,
                            "event": {
                                "ObjectiveCompleted": {
                                    "id": plan.objective_id
                                }
                            }
                        });
                        let _ = append_jsonl_line(&session_dir.join("orchestrator_events.jsonl"), &completion_event);
                        let completion_transcript = serde_json::json!({
                            "timestamp": now,
                            "line": "[objective] completed".to_string(),
                        });
                        let _ = append_jsonl_line(&session_dir.join("transcript.jsonl"), &completion_transcript);
                        let _ = update_session_state_in_dir(
                            &session_dir,
                            &snapshot.meta,
                            SessionState::Completed,
                        );
                        let _ = rewrite_pending_work(&session_dir, &plan.objective_id, &[]);
                    } else {
                        let next_step = plan.steps.get(next_index).cloned().unwrap_or_default();
                        let progress_event = serde_json::json!({
                            "timestamp": now,
                            "event": {
                                "ObjectiveProgress": {
                                    "id": plan.objective_id,
                                    "status": "InProgress"
                                }
                            }
                        });
                        let _ = append_jsonl_line(&session_dir.join("orchestrator_events.jsonl"), &progress_event);
                        let progress_transcript = serde_json::json!({
                            "timestamp": now,
                            "line": format!("[turn] advanced to {}", next_step),
                        });
                        let _ = append_jsonl_line(&session_dir.join("transcript.jsonl"), &progress_transcript);
                        let _ = update_session_state_in_dir(
                            &session_dir,
                            &snapshot.meta,
                            SessionState::Active,
                        );
                        let _ = rewrite_pending_work(
                            &session_dir,
                            &plan.objective_id,
                            std::slice::from_ref(&next_step),
                        );
                    }
                }
            } else {
                let paused_transcript = serde_json::json!({
                    "timestamp": now,
                    "line": format!(
                        "[turn] paused after denial {}",
                        reason.clone().unwrap_or_else(|| "operator denied".to_string())
                    ),
                });
                let _ = append_jsonl_line(&session_dir.join("transcript.jsonl"), &paused_transcript);
                let _ = update_session_state_in_dir(&session_dir, &snapshot.meta, SessionState::Paused);
                let pending_objective_id = load_latest_plan_entry(&session_dir)
                    .map(|plan| plan.objective_id)
                    .unwrap_or_else(|_| snapshot.meta.id.to_string());
                let _ = rewrite_pending_work(
                    &session_dir,
                    &pending_objective_id,
                    std::slice::from_ref(action),
                );
            }

            println!(
                "Approval {} for session {}: [{}] {}",
                kind, snapshot.meta.id, risk, action
            );
        }
        Err(error) => println!("  Error loading session: {}", error),
    }
}

fn continue_command(session_id: Option<&str>) {
    let session_dir = if let Some(id) = session_id {
        PathBuf::from(format!(".bco/sessions/{}", id))
    } else {
        match find_most_recent_session() {
            Some(dir) => dir,
            None => {
                println!("No sessions found.");
                return;
            }
        }
    };

    match load_session_snapshot(&session_dir) {
        Ok(snapshot) => {
            if !snapshot.pending_approvals.is_empty() {
                println!("Session {} is waiting on approval.", snapshot.meta.id);
                return;
            }

            let Ok(plan) = load_latest_plan_entry(&session_dir) else {
                println!("No plan found for session {}.", snapshot.meta.id);
                return;
            };

            let active_index = plan.active_index.unwrap_or(0);
            let active_step = snapshot
                .pending_work
                .first()
                .cloned()
                .or_else(|| plan.steps.get(active_index).cloned());

            let Some(active_step) = active_step else {
                let _ = update_session_state_in_dir(&session_dir, &snapshot.meta, SessionState::Completed);
                let _ = rewrite_pending_work(&session_dir, &plan.objective_id, &[]);
                println!("Session {} is already complete.", snapshot.meta.id);
                return;
            };
            let now = chrono::Utc::now();
            let reopening_paused = snapshot.meta.state == SessionState::Paused;

            if reopening_paused {
                let _ = append_jsonl_line(
                    &session_dir.join("transcript.jsonl"),
                    &serde_json::json!({
                        "timestamp": now,
                        "line": format!("[resume] reopening paused work {}", active_step),
                    }),
                );
                let _ = append_jsonl_line(
                    &session_dir.join("orchestrator_events.jsonl"),
                    &serde_json::json!({
                        "timestamp": now,
                        "event": {
                            "ObjectiveProgress": {
                                "id": plan.objective_id,
                                "status": "InProgress"
                            }
                        }
                    }),
                );
            }

            let interaction_begin = serde_json::json!({
                "timestamp": now,
                "event": {
                    "InteractionBegin": {
                        "from": "6bc66dad-ecf8-6fef-ef6f-f8ecad6dc66b",
                        "to": "736aa25d-3044-f9c2-c2f9-44305da26a73"
                    }
                }
            });
            let _ = append_jsonl_line(&session_dir.join("orchestrator_events.jsonl"), &interaction_begin);
            let _ = append_jsonl_line(
                &session_dir.join("transcript.jsonl"),
                &serde_json::json!({
                    "timestamp": now,
                    "line": format!("[coord] delegated {}", active_step),
                }),
            );

            let objective_text = snapshot.meta.profile.replace("offensive-", "").replace('-', " ");
            let risk = classify_action_risk_local(&active_step, &objective_text);
            if matches!(risk, RiskProfile::High | RiskProfile::Critical) {
                let request_id = deterministic_request_id(&active_step);
                let risk_label = title_case_ascii(risk.as_str());
                let _ = append_jsonl_line(
                    &session_dir.join("approvals.jsonl"),
                    &serde_json::json!({
                        "timestamp": now,
                        "kind": "requested",
                        "request_id": request_id,
                        "cell": "736aa25d-3044-f9c2-c2f9-44305da26a73",
                        "action": active_step,
                        "risk": risk_label,
                        "reason": serde_json::Value::Null,
                    }),
                );
                let _ = append_jsonl_line(
                    &session_dir.join("orchestrator_events.jsonl"),
                    &serde_json::json!({
                        "timestamp": now,
                        "event": {
                            "ApprovalRequested": {
                                "cell": "736aa25d-3044-f9c2-c2f9-44305da26a73",
                                "action": active_step,
                                "risk": risk_label
                            }
                        }
                    }),
                );
                let _ = append_jsonl_line(
                    &session_dir.join("transcript.jsonl"),
                    &serde_json::json!({
                        "timestamp": now,
                        "line": format!("[approval] requested {}", active_step),
                    }),
                );
                let _ = update_session_state_in_dir(&session_dir, &snapshot.meta, SessionState::Active);
                let _ = rewrite_pending_work(
                    &session_dir,
                    &plan.objective_id,
                    std::slice::from_ref(&active_step),
                );
                println!("Session {} continued into approval-gated step.", snapshot.meta.id);
                return;
            }

            let _ = append_jsonl_line(
                &session_dir.join("orchestrator_events.jsonl"),
                &serde_json::json!({
                    "timestamp": now,
                    "event": {
                        "InteractionEnd": {
                            "from": "6bc66dad-ecf8-6fef-ef6f-f8ecad6dc66b",
                            "to": "736aa25d-3044-f9c2-c2f9-44305da26a73"
                        }
                    }
                }),
            );
            let _ = append_jsonl_line(
                &session_dir.join("transcript.jsonl"),
                &serde_json::json!({
                    "timestamp": now,
                    "line": format!("[coord] executor returned {}", active_step),
                }),
            );

            let next_index = usize::min(active_index.saturating_add(1), plan.steps.len());
            let _ = append_plan_snapshot(&session_dir, &plan.objective_id, &plan.steps, next_index);
            if next_index >= plan.steps.len() {
                let _ = append_jsonl_line(
                    &session_dir.join("orchestrator_events.jsonl"),
                    &serde_json::json!({
                        "timestamp": now,
                        "event": { "ObjectiveCompleted": { "id": plan.objective_id } }
                    }),
                );
                let _ = append_jsonl_line(
                    &session_dir.join("transcript.jsonl"),
                    &serde_json::json!({
                        "timestamp": now,
                        "line": "[objective] completed".to_string(),
                    }),
                );
                let _ = update_session_state_in_dir(&session_dir, &snapshot.meta, SessionState::Completed);
                let _ = rewrite_pending_work(&session_dir, &plan.objective_id, &[]);
            } else {
                let next_step = plan.steps.get(next_index).cloned().unwrap_or_default();
                let _ = append_jsonl_line(
                    &session_dir.join("transcript.jsonl"),
                    &serde_json::json!({
                        "timestamp": now,
                        "line": format!("[turn] advanced to {}", next_step),
                    }),
                );
                let _ = update_session_state_in_dir(&session_dir, &snapshot.meta, SessionState::Active);
                let _ = rewrite_pending_work(
                    &session_dir,
                    &plan.objective_id,
                    std::slice::from_ref(&next_step),
                );
            }

            println!("Session {} continued successfully.", snapshot.meta.id);
        }
        Err(error) => println!("  Error loading session: {}", error),
    }
}

fn providers_command(action: Option<&ProviderAction>) {
    let mut registry = load_provider_registry();

    match action {
        Some(ProviderAction::List) => {
            println!("Configured providers:");
            let names = registry.list_names();
            if names.is_empty() {
                println!("  (No providers configured)");
            } else {
                for name in names {
                    if let Some(profile) = registry.get(name) {
                        println!("  {}: {:?} - {}",
                            name,
                            profile.provider,
                            match profile.state {
                                ConnectionState::Connected => "connected",
                                ConnectionState::Disconnected => "disconnected",
                                ConnectionState::Connecting => "connecting",
                                ConnectionState::Error => "error",
                            }
                        );
                        if let Some(ref endpoint) = profile.endpoint {
                            println!("    endpoint: {}", endpoint);
                        }
                    }
                }
            }
        }
        Some(ProviderAction::Add { name, endpoint }) => {
            let profile = ConnectionProfile {
                provider: ProviderRef::new(name.clone()),
                endpoint: endpoint.clone(),
                state: ConnectionState::Disconnected,
            };
            registry.upsert(profile);
            if save_provider_registry(&registry) {
                println!("Provider '{}' added successfully.", name);
            } else {
                println!("Failed to save provider configuration.");
            }
        }
        Some(ProviderAction::Remove { name }) => {
            if registry.remove(name).is_some() {
                if save_provider_registry(&registry) {
                    println!("Provider '{}' removed.", name);
                } else {
                    println!("Failed to save provider configuration.");
                }
            } else {
                println!("Provider '{}' not found.", name);
            }
        }
        None => {
            println!("Provider management");
            println!("  Use 'bco providers list' to list providers");
            println!("  Use 'bco providers add <name>' to add a provider");
            println!("  Use 'bco providers remove <name>' to remove a provider");
        }
    }
}

fn models_command(action: Option<&ModelAction>) {
    let registry = load_provider_registry();

    match action {
        Some(ModelAction::List { provider }) => {
            println!("Available models:");
            let names = registry.list_names();
            if names.is_empty() {
                println!("  (No providers configured - add a provider first)");
            } else {
                for name in names {
                    if let Some(ref p) = provider {
                        if p != name {
                            continue;
                        }
                    }
                    if let Some(profile) = registry.get(name) {
                        println!("  {} (endpoint: {:?})",
                            profile.provider,
                            profile.endpoint
                        );
                    }
                }
            }
            println!("\nNote: Full model listing requires provider API.");
        }
        Some(ModelAction::Current) => {
            match load_current_model() {
                Ok(model) => println!("Current model: {}", model),
                Err(_) => println!("Current model: none selected"),
            }
        }
        Some(ModelAction::Switch { model }) => {
            // Parse and validate model
            match ModelRef::parse(model) {
                Ok(model_ref) => {
                    if save_current_model(&model_ref.to_string()) {
                        println!("Switched to model: {}", model_ref);
                    } else {
                        println!("Failed to save model configuration.");
                    }
                }
                Err(e) => println!("Invalid model format: {}. Expected provider/model (e.g., anthropic/claude-sonnet-4-6)", e),
            }
        }
        None => {
            println!("Model management");
            println!("  Use 'bco models list' to list available models");
            println!("  Use 'bco models current' to show current model");
            println!("  Use 'bco models switch <provider/model>' to switch model");
        }
    }
}

// =============================================================================
// Helper functions
// =============================================================================

fn get_session_base_path() -> PathBuf {
    std::env::current_dir().unwrap_or_default()
}

fn infer_domain(objective: &str) -> IntentDomain {
    let lower = objective.to_lowercase();

    if lower.contains("ctf")
        || lower.contains("pwn")
        || lower.contains("reversing")
        || lower.contains("forensics")
        || lower.contains("web challenge")
        || lower.contains("crypto challenge")
        || lower.contains("flag")
    {
        return IntentDomain::Ctf;
    }

    if lower.contains("pentest")
        || lower.contains("penetration test")
        || lower.contains("red team")
        || lower.contains("exploit chain")
        || lower.contains("target scope")
    {
        return IntentDomain::Pentesting;
    }

    if lower.contains("rust")
        || lower.contains("refactor")
        || lower.contains("bug")
        || lower.contains("test")
        || lower.contains("code")
        || lower.contains("implement")
    {
        return IntentDomain::Coding;
    }

    IntentDomain::GeneralEngineering
}

fn list_sessions() -> Result<Vec<PathBuf>, std::io::Error> {
    let sessions_dir = get_session_base_path().join(".bco/sessions");
    if !sessions_dir.exists() {
        return Ok(Vec::new());
    }

    let mut sessions = Vec::new();
    for entry in fs::read_dir(sessions_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            sessions.push(path);
        }
    }

    // Sort by modification time (most recent first)
    sessions.sort_by(|a, b| {
        let a_time = a.metadata().and_then(|m| m.modified()).ok();
        let b_time = b.metadata().and_then(|m| m.modified()).ok();
        b_time.cmp(&a_time)
    });

    Ok(sessions)
}

fn find_most_recent_session() -> Option<PathBuf> {
    list_sessions().ok()?.into_iter().next()
}

fn load_session_meta(session_dir: &PathBuf) -> Result<SessionMeta, String> {
    let session_json_path = session_dir.join("session.json");
    let content = fs::read_to_string(&session_json_path)
        .map_err(|e| format!("Failed to read session.json: {}", e))?;
    serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse session.json: {}", e))
}

fn load_session_runtime(session_dir: &PathBuf) -> Result<SessionRuntime, String> {
    let runtime_json_path = session_dir.join("session_runtime.json");
    let content = fs::read_to_string(&runtime_json_path)
        .map_err(|e| format!("Failed to read session_runtime.json: {}", e))?;
    serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse session_runtime.json: {}", e))
}

fn bootstrap_from_existing(meta: &SessionMeta) -> Option<SessionBootstrap> {
    Some(SessionBootstrap::with_id(meta.id, meta.profile.clone()))
}

#[derive(Debug, Clone)]
struct SessionSnapshot {
    meta: SessionMeta,
    runtime: SessionRuntime,
    plan_steps: Vec<String>,
    transcript: Vec<String>,
    next_action: Option<String>,
    active_cells: Vec<(String, String)>,
    pending_approvals: Vec<(String, String, String, String)>,
    pending_work: Vec<String>,
}

impl SessionSnapshot {
    fn into_tui_state(self) -> TuiState {
        let mut state = TuiState::with_objective(
            self.meta.profile.as_str()
        );
        state.status.objective = Some(self.meta.profile.clone());
        state.current_plan = self.plan_steps.clone();
        state.transcript.extend(self.transcript);
        state.active_cells = self
            .active_cells
            .into_iter()
            .map(|(name, status)| bco_tui::CellDisplay { name, status })
            .collect();
        state.pending_approvals = self
            .pending_approvals
            .into_iter()
            .map(|(risk, action, requested_at, _request_id)| bco_tui::ApprovalDisplay {
                risk,
                action,
                requested_at,
            })
            .collect();
        state.status.approval_state = match state.pending_approvals.len() {
            0 => bco_tui::ApprovalState::None,
            count => bco_tui::ApprovalState::Pending(count as u32),
        };
        state.status.model = self.runtime.active_model.clone();
        if let Some(ref model) = self.runtime.active_model {
            if let Ok(model_ref) = ModelRef::parse(model) {
                state.status.provider = Some(model_ref.provider().as_str().to_string());
            }
        }
        if let Some(next_action) = self.next_action {
            state.status.subgoal = Some(next_action);
        }
        state
    }
}

fn load_session_snapshot(session_dir: &PathBuf) -> Result<SessionSnapshot, String> {
    let meta = load_session_meta(session_dir)?;
    let runtime = load_session_runtime(session_dir)?;
    let (plan_steps, transcript, next_action, active_cells, pending_approvals) =
        parse_session_artifacts(session_dir)?;
    let pending_work = parse_pending_work(session_dir)?;

    Ok(SessionSnapshot {
        meta,
        runtime,
        plan_steps,
        transcript,
        next_action,
        active_cells,
        pending_approvals,
        pending_work,
    })
}

fn parse_session_artifacts(
    session_dir: &PathBuf,
) -> Result<
    (
        Vec<String>,
        Vec<String>,
        Option<String>,
        Vec<(String, String)>,
        Vec<(String, String, String, String)>
    ),
    String,
> {
    let plan_steps = parse_plan_log(session_dir)?;
    let transcript = parse_transcript_log(session_dir)?;
    let active_cells = parse_cell_states(session_dir)?;
    let pending_approvals = parse_pending_approvals(session_dir)?;
    let pending_work = parse_pending_work(session_dir).unwrap_or_default();
    let next_action = pending_work
        .first()
        .cloned()
        .or_else(|| {
            plan_steps
                .iter()
                .find_map(|step| step.strip_prefix("[active] ").map(str::to_string))
        })
        .or_else(|| {
            plan_steps
                .iter()
                .find_map(|step| step.strip_prefix("[pending] ").map(str::to_string))
        });
    Ok((plan_steps, transcript, next_action, active_cells, pending_approvals))
}

fn parse_plan_log(session_dir: &PathBuf) -> Result<Vec<String>, String> {
    let entry = load_latest_plan_entry(session_dir)?;
    let active_index = entry.active_index.unwrap_or(0);
    let mut plan_steps = Vec::new();

    for (index, step) in entry.steps.iter().enumerate() {
        let rendered = if index < active_index {
            format!("[done] {}", step)
        } else if index == active_index {
            format!("[active] {}", step)
        } else {
            format!("[pending] {}", step)
        };
        plan_steps.push(rendered);
    }

    Ok(plan_steps)
}

fn load_latest_plan_entry(session_dir: &PathBuf) -> Result<PlanLogEntry, String> {
    let path = session_dir.join("plan.jsonl");
    let content = fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read plan log: {}", e))?;
    let mut latest = None;

    for line in content.lines() {
        if line.trim().is_empty() {
            continue;
        }

        let entry: PlanLogEntry = serde_json::from_str(line)
            .map_err(|e| format!("Failed to parse plan log line: {}", e))?;
        latest = Some(entry);
    }

    latest.ok_or_else(|| "No plan entries found".to_string())
}

fn parse_transcript_log(session_dir: &PathBuf) -> Result<Vec<String>, String> {
    let path = session_dir.join("transcript.jsonl");
    let content = fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read transcript log: {}", e))?;
    let mut transcript = Vec::new();

    for line in content.lines() {
        if line.trim().is_empty() {
            continue;
        }

        let entry: TranscriptLogEntry = serde_json::from_str(line)
            .map_err(|e| format!("Failed to parse transcript log line: {}", e))?;
        transcript.push(entry.line);
    }

    Ok(transcript)
}

fn parse_cell_states(session_dir: &PathBuf) -> Result<Vec<(String, String)>, String> {
    let topology_path = session_dir.join("cell_topology.jsonl");
    let topology_content = fs::read_to_string(&topology_path)
        .map_err(|e| format!("Failed to read cell topology log: {}", e))?;
    let events_path = session_dir.join("orchestrator_events.jsonl");
    let events_content = fs::read_to_string(&events_path)
        .map_err(|e| format!("Failed to read orchestrator events: {}", e))?;
    let meta = load_session_meta(session_dir)?;
    let pending_approvals = parse_pending_approvals(session_dir).unwrap_or_default();
    let pending_work = parse_pending_work(session_dir).unwrap_or_default();

    let turn_completed = events_content.lines().any(|line| line.contains("\"TurnCompleted\""));
    let has_pending_approval = !pending_approvals.is_empty();
    let has_pending_work = !pending_work.is_empty();
    let mut active_cells = Vec::new();

    for line in topology_content.lines() {
        if line.trim().is_empty() {
            continue;
        }

        let entry: CellTopologyLogEntry = serde_json::from_str(line)
            .map_err(|e| format!("Failed to parse cell topology line: {}", e))?;
        let status = if meta.state == SessionState::Paused && has_pending_work {
            match entry.cell_type.as_str() {
                "planner" => "completed",
                "coordinator" | "executor" => "paused",
                "reviewer" => "idle",
                _ => "idle",
            }
        } else if has_pending_approval {
            match entry.cell_type.as_str() {
                "planner" => "completed",
                "coordinator" | "executor" => "waiting",
                "reviewer" => "idle",
                _ => "waiting",
            }
        } else if has_pending_work {
            match entry.cell_type.as_str() {
                "planner" => "completed",
                "coordinator" => "coordinating",
                "executor" => "executing",
                "reviewer" => "idle",
                _ => "idle",
            }
        } else if turn_completed {
            "completed"
        } else {
            "idle"
        };
        active_cells.push((entry.cell_type, status.to_string()));
    }

    Ok(active_cells)
}

fn parse_pending_approvals(session_dir: &PathBuf) -> Result<Vec<(String, String, String, String)>, String> {
    let path = session_dir.join("approvals.jsonl");
    let content = fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read approval log: {}", e))?;
    let mut pending = std::collections::BTreeMap::<String, (String, String, String, String)>::new();

    for line in content.lines() {
        if line.trim().is_empty() {
            continue;
        }

        let entry: ApprovalLogEntry = serde_json::from_str(line)
            .map_err(|e| format!("Failed to parse approval log line: {}", e))?;

        match entry.kind.as_str() {
            "requested" => {
                if let (Some(action), Some(risk)) = (entry.action, entry.risk) {
                    let request_id = entry.request_id.clone();
                    pending.insert(
                        request_id.clone(),
                        (
                            risk,
                            action,
                            entry.timestamp.format("%H:%M:%S").to_string(),
                            request_id,
                        ),
                    );
                }
            }
            "granted" | "denied" => {
                pending.remove(&entry.request_id);
            }
            _ => {}
        }
    }

    Ok(pending.into_values().collect())
}

fn parse_pending_work(session_dir: &PathBuf) -> Result<Vec<String>, String> {
    let path = session_dir.join("pending_work.jsonl");
    let content = fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read pending work log: {}", e))?;
    let mut items = Vec::new();

    for line in content.lines() {
        if line.trim().is_empty() {
            continue;
        }

        let entry: PendingWorkLogEntry = serde_json::from_str(line)
            .map_err(|e| format!("Failed to parse pending work line: {}", e))?;
        items.push(entry.action);
    }

    Ok(items)
}

fn print_session_snapshot(snapshot: &SessionSnapshot) {
    println!("  Session: {}", snapshot.meta.id);
    println!("  State: {:?}", snapshot.meta.state);
    println!("  Profile: {}", snapshot.meta.profile);
    if let Some(ref model) = snapshot.runtime.active_model {
        println!("  Active model: {}", model);
    }
    if let Some(ref next_action) = snapshot.next_action {
        println!("  Next action: {}", next_action);
    }
    if snapshot.meta.state == SessionState::Paused && !snapshot.pending_work.is_empty() {
        println!("  Pause reason: operator denial or manual stop; pending work is preserved locally");
    }
    if !snapshot.plan_steps.is_empty() {
        println!("  Plan:");
        for step in &snapshot.plan_steps {
            println!("    {}", step);
        }
    }
    if !snapshot.active_cells.is_empty() {
        println!("  Cells:");
        for (name, status) in &snapshot.active_cells {
            println!("    {} ({})", name, status);
        }
    }
    if !snapshot.pending_work.is_empty() {
        println!("  Pending work:");
        for action in &snapshot.pending_work {
            println!("    {}", action);
        }
    }
    if !snapshot.pending_approvals.is_empty() {
        println!("  Pending approvals:");
        for (risk, action, requested_at, request_id) in &snapshot.pending_approvals {
            println!("    [{}] {} at {} ({})", risk, action, requested_at, request_id);
        }
    }
}

fn copy_session_artifacts(source_dir: &PathBuf, target_dir: &PathBuf) -> Result<(), String> {
    for entry in fs::read_dir(source_dir).map_err(|e| format!("Failed to read source session dir: {}", e))? {
        let entry = entry.map_err(|e| format!("Failed to read source session entry: {}", e))?;
        let source_path = entry.path();
        let target_path = target_dir.join(entry.file_name());

        if source_path.is_dir() {
            fs::create_dir_all(&target_path)
                .map_err(|e| format!("Failed to create target dir: {}", e))?;
            copy_session_artifacts(&source_path, &target_path)?;
        } else if source_path.file_name().and_then(|name| name.to_str()) != Some("session.json") {
            fs::copy(&source_path, &target_path)
                .map_err(|e| format!("Failed to copy session artifact: {}", e))?;
        }
    }

    Ok(())
}

fn update_session_state(session: &SessionBootstrap, state: SessionState) {
    let existing_created_at = fs::read_to_string(session.layout().session_json())
        .ok()
        .and_then(|content| serde_json::from_str::<SessionMeta>(&content).ok())
        .map(|meta| meta.created_at)
        .unwrap_or_else(chrono::Utc::now);

    let meta = SessionMeta {
        id: session.id(),
        created_at: existing_created_at,
        profile: session.profile().to_string(),
        state,
    };

    if let Ok(json) = serde_json::to_string_pretty(&meta) {
        let path = session.layout().session_json();
        let _ = fs::write(path, json);
    }

    let _ = touch_session_runtime(
        &session.layout().session_dir(),
        session.id(),
        None,
    );
}

fn update_session_state_in_dir(
    session_dir: &PathBuf,
    existing: &SessionMeta,
    state: SessionState,
) -> Result<(), String> {
    let meta = SessionMeta {
        id: existing.id,
        created_at: existing.created_at,
        profile: existing.profile.clone(),
        state,
    };
    let json = serde_json::to_string_pretty(&meta)
        .map_err(|e| format!("Failed to serialize session meta: {}", e))?;
    fs::write(session_dir.join("session.json"), json)
        .map_err(|e| format!("Failed to write session.json: {}", e))?;

    touch_session_runtime(session_dir, existing.id, None)
}

fn rewrite_pending_work(
    session_dir: &PathBuf,
    objective_id: &str,
    actions: &[String],
) -> Result<(), String> {
    let path = session_dir.join("pending_work.jsonl");
    if actions.is_empty() {
        return fs::write(path, "").map_err(|e| format!("Failed to clear pending_work.jsonl: {}", e));
    }

    let now = chrono::Utc::now();
    let mut lines = Vec::new();
    for action in actions {
        let entry = PendingWorkLogEntry {
            timestamp: now,
            id: deterministic_request_id(action),
            objective_id: objective_id.to_string(),
            action: action.clone(),
            retry_count: 0,
            max_retries: 3,
            last_error: None,
            retry_class: "transient".to_string(),
        };
        let line = serde_json::to_string(&entry)
            .map_err(|e| format!("Failed to serialize pending work line: {}", e))?;
        lines.push(line);
    }

    fs::write(path, format!("{}\n", lines.join("\n")))
        .map_err(|e| format!("Failed to write pending_work.jsonl: {}", e))
}

fn sync_initial_pending_work(session_dir: &PathBuf) -> Result<(), String> {
    let plan = load_latest_plan_entry(session_dir)?;
    let pending_approvals = parse_pending_approvals(session_dir).unwrap_or_default();

    if !pending_approvals.is_empty() {
        let actions = pending_approvals
            .iter()
            .map(|(_, action, _, _)| action.clone())
            .collect::<Vec<_>>();
        return rewrite_pending_work(session_dir, &plan.objective_id, &actions);
    }

    let active_index = plan.active_index.unwrap_or(0);
    if let Some(active_step) = plan.steps.get(active_index) {
        return rewrite_pending_work(
            session_dir,
            &plan.objective_id,
            std::slice::from_ref(active_step),
        );
    }

    rewrite_pending_work(session_dir, &plan.objective_id, &[])
}

fn touch_session_runtime(
    session_dir: &PathBuf,
    session_id: SessionId,
    active_model: Option<String>,
) -> Result<(), String> {
    let runtime_path = session_dir.join("session_runtime.json");
    let mut runtime = if runtime_path.exists() {
        load_session_runtime(session_dir)?
    } else {
        SessionRuntime {
            session_id,
            active_model: None,
            token_usage: None,
            abort_count: 0,
            compaction_count: 0,
            last_updated: chrono::Utc::now(),
        }
    };

    runtime.session_id = session_id;
    if let Some(model) = active_model {
        runtime.active_model = Some(model);
    }
    runtime.last_updated = chrono::Utc::now();

    let json = serde_json::to_string_pretty(&runtime)
        .map_err(|e| format!("Failed to serialize session_runtime.json: {}", e))?;
    fs::write(runtime_path, json)
        .map_err(|e| format!("Failed to write session_runtime.json: {}", e))
}

fn load_provider_registry() -> ProviderRegistry {
    let config_path = get_session_base_path().join(PROVIDER_CONFIG_PATH);
    if let Ok(content) = fs::read_to_string(&config_path) {
        if let Ok(registry) = serde_json::from_str(&content) {
            return registry;
        }
    }
    ProviderRegistry::new()
}

fn save_provider_registry(registry: &ProviderRegistry) -> bool {
    let config_path = get_session_base_path().join(PROVIDER_CONFIG_PATH);

    // Ensure .bco directory exists
    if let Some(parent) = config_path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    if let Ok(json) = serde_json::to_string_pretty(registry) {
        fs::write(&config_path, json).is_ok()
    } else {
        false
    }
}

#[derive(Serialize, Deserialize)]
struct ModelState {
    current_model: Option<String>,
    last_updated: chrono::DateTime<chrono::Utc>,
}

fn load_current_model() -> Result<String, String> {
    let config_path = get_session_base_path().join(MODEL_CONFIG_PATH);
    let content = fs::read_to_string(&config_path)
        .map_err(|_| "No model selected".to_string())?;
    let state: ModelState = serde_json::from_str(&content)
        .map_err(|_| "Invalid model state".to_string())?;
    state.current_model.ok_or_else(|| "No model selected".to_string())
}

fn save_current_model(model: &str) -> bool {
    let config_path = get_session_base_path().join(MODEL_CONFIG_PATH);

    // Ensure .bco directory exists
    if let Some(parent) = config_path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    let state = ModelState {
        current_model: Some(model.to_string()),
        last_updated: chrono::Utc::now(),
    };

    if let Ok(json) = serde_json::to_string_pretty(&state) {
        fs::write(&config_path, json).is_ok()
    } else {
        false
    }
}

fn hydrate_model_status(state: &mut TuiState) {
    let registry = load_provider_registry();

    if let Ok(model) = load_current_model() {
        state.status.model = Some(model.clone());
        if let Ok(model_ref) = ModelRef::parse(&model) {
            let provider_name = model_ref.provider().as_str().to_string();
            state.status.provider = Some(provider_name.clone());
            state.status.connection_health = registry
                .get(&provider_name)
                .map(|profile| match profile.state {
                    ConnectionState::Connected => ConnectionHealth::Connected,
                    ConnectionState::Disconnected => ConnectionHealth::Disconnected,
                    ConnectionState::Connecting => ConnectionHealth::Unknown,
                    ConnectionState::Error => ConnectionHealth::Error,
                })
                .unwrap_or(ConnectionHealth::Unknown);
        }
    }
}

fn session_profile_for(intent: &TaskIntent) -> String {
    let domain = match intent.domain() {
        IntentDomain::Ctf => "ctf",
        IntentDomain::Pentesting => "offensive",
        IntentDomain::Coding => "coding",
        IntentDomain::GeneralEngineering => "general",
    };
    let slug = intent
        .objective()
        .split_whitespace()
        .take(6)
        .map(|part| {
            part.chars()
                .filter(|ch| ch.is_ascii_alphanumeric())
                .collect::<String>()
                .to_lowercase()
        })
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    if slug.is_empty() {
        format!("{}-session", domain)
    } else {
        format!("{}-{}", domain, slug)
    }
}

#[derive(Debug, Deserialize)]
struct TranscriptLogEntry {
    line: String,
}

#[derive(Debug, Deserialize)]
struct PlanLogEntry {
    objective_id: String,
    steps: Vec<String>,
    active_index: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct CellTopologyLogEntry {
    cell_type: String,
}

#[derive(Debug, Deserialize)]
struct ApprovalLogEntry {
    timestamp: chrono::DateTime<chrono::Utc>,
    kind: String,
    request_id: String,
    action: Option<String>,
    risk: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct PendingWorkLogEntry {
    timestamp: chrono::DateTime<chrono::Utc>,
    id: String,
    objective_id: String,
    action: String,
    retry_count: u8,
    max_retries: u8,
    last_error: Option<String>,
    retry_class: String,
}

fn append_jsonl_line(path: &PathBuf, value: &serde_json::Value) -> Result<(), String> {
    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| format!("Failed to open {}: {}", path.display(), e))?;
    let line = serde_json::to_string(value)
        .map_err(|e| format!("Failed to serialize jsonl line: {}", e))?;
    writeln!(file, "{}", line)
        .map_err(|e| format!("Failed to append {}: {}", path.display(), e))
}

fn append_plan_snapshot(
    session_dir: &PathBuf,
    objective_id: &str,
    steps: &[String],
    active_index: usize,
) -> Result<(), String> {
    let entry = serde_json::json!({
        "timestamp": chrono::Utc::now(),
        "objective_id": objective_id,
        "steps": steps,
        "active_index": active_index,
    });
    append_jsonl_line(&session_dir.join("plan.jsonl"), &entry)
}

fn classify_action_risk_local(action: &str, objective: &str) -> RiskProfile {
    let action = action.to_lowercase();
    let objective = objective.to_lowercase();

    if action.contains("document")
        || action.contains("evidence")
        || action.contains("impact")
        || action.contains("operator actions")
        || action.contains("report")
        || action.contains("summary")
    {
        return RiskProfile::Moderate;
    }

    if action.contains("confirm target scope")
        || action.contains("guardrails")
        || action.contains("scope")
    {
        return RiskProfile::High;
    }

    if action.contains("enumerate")
        || action.contains("attack surface")
        || action.contains("vulnerability")
        || action.contains("recon")
    {
        return RiskProfile::High;
    }

    let combined = format!("{} {}", action, objective);
    if combined.contains("lateral movement")
        || combined.contains("initial access")
        || combined.contains("exploit")
        || combined.contains("persistence")
        || combined.contains("exfiltration")
        || combined.contains("escalation")
        || combined.contains("chain access")
    {
        RiskProfile::Critical
    } else {
        RiskProfile::Moderate
    }
}

fn deterministic_request_id(step: &str) -> String {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    step.hash(&mut hasher);
    let hash = hasher.finish();
    let bytes = [
        (hash >> 56) as u8,
        (hash >> 48) as u8,
        (hash >> 40) as u8,
        (hash >> 32) as u8,
        (hash >> 24) as u8,
        (hash >> 16) as u8,
        (hash >> 8) as u8,
        hash as u8,
        (hash >> 56) as u8,
        (hash >> 48) as u8,
        (hash >> 40) as u8,
        (hash >> 32) as u8,
        (hash >> 24) as u8,
        (hash >> 16) as u8,
        (hash >> 8) as u8,
        hash as u8,
    ];
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0], bytes[1], bytes[2], bytes[3],
        bytes[4], bytes[5],
        bytes[6], bytes[7],
        bytes[8], bytes[9],
        bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15],
    )
}

fn title_case_ascii(value: &str) -> String {
    let mut chars = value.chars();
    match chars.next() {
        Some(first) => format!("{}{}", first.to_ascii_uppercase(), chars.as_str()),
        None => String::new(),
    }
}
