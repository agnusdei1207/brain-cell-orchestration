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
use bco_session::{SessionBootstrap, SessionMeta, SessionState, SessionRuntime};
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
    let session = SessionBootstrap::new("local-bootstrap");

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
                update_session_state(&session, SessionState::Active);
                println!("  Session resumed successfully.");

                let mut state = snapshot.into_tui_state();
                state.status.resumed = true;
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
}

impl SessionSnapshot {
    fn into_tui_state(self) -> TuiState {
        let mut state = TuiState::with_objective(
            self.meta.profile.as_str()
        );
        state.status.objective = Some(self.meta.profile.clone());
        state.current_plan = self.plan_steps.clone();
        state.transcript.extend(self.transcript);
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
    let (plan_steps, transcript, next_action) = parse_orchestrator_events(session_dir)?;

    Ok(SessionSnapshot {
        meta,
        runtime,
        plan_steps,
        transcript,
        next_action,
    })
}

fn parse_orchestrator_events(session_dir: &PathBuf) -> Result<(Vec<String>, Vec<String>, Option<String>), String> {
    let path = session_dir.join("orchestrator_events.jsonl");
    let content = fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read orchestrator events: {}", e))?;

    let mut plan_steps = Vec::new();
    let mut transcript = Vec::new();
    let mut next_action = None;

    for line in content.lines() {
        if line.trim().is_empty() {
            continue;
        }

        let value: serde_json::Value = serde_json::from_str(line)
            .map_err(|e| format!("Failed to parse orchestrator event line: {}", e))?;

        if let Some(event) = value.get("event") {
            if let Some(plan) = event.get("ObjectivePlanReady") {
                if let Some(steps) = plan.get("steps").and_then(|steps| steps.as_array()) {
                    for (index, step) in steps.iter().filter_map(|step| step.as_str()).enumerate() {
                        let rendered = if index == 0 {
                            format!("[active] {}", step)
                        } else {
                            format!("[pending] {}", step)
                        };
                        if next_action.is_none() {
                            next_action = Some(step.to_string());
                        }
                        plan_steps.push(rendered);
                    }
                }
            }

            if let Some(cell) = event.get("CellSpawned").and_then(|cell| cell.get("cell_type")).and_then(|cell| cell.as_str()) {
                transcript.push(format!("[cell] spawned {}", cell));
            } else if event.get("InteractionBegin").is_some() {
                transcript.push("[coord] delegated work to executor".to_string());
            } else if event.get("TurnCompleted").is_some() {
                transcript.push("[turn] completed".to_string());
            } else if event.get("TurnSubmitted").is_some() {
                transcript.push("[turn] submitted".to_string());
            }
        }
    }

    Ok((plan_steps, transcript, next_action))
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
    if !snapshot.plan_steps.is_empty() {
        println!("  Plan:");
        for step in &snapshot.plan_steps {
            println!("    {}", step);
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
