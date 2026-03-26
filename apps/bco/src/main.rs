use clap::{Parser, Subcommand};
use bco_core::{IntentDomain, RiskProfile, TaskIntent};
use bco_harness::HarnessRegistry;
use bco_orchestrator::BrainCellOrchestrator;
use bco_session::SessionBootstrap;
use bco_tui::{TuiBlueprint, TuiState};

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
        /// Provider name (e.g., anthropic, openai)
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
    let intent = TaskIntent::new(
        objective.join(" "),
        IntentDomain::GeneralEngineering,
        RiskProfile::Moderate,
    );
    let session = SessionBootstrap::new("local-bootstrap");

    // Bootstrap session
    if let Err(e) = session.bootstrap() {
        eprintln!("Failed to bootstrap session: {}", e);
        std::process::exit(1);
    }

    let registry = HarnessRegistry::with_defaults();
    let harness_name = registry.resolve(&intent).as_str().to_string();
    let orchestrator = BrainCellOrchestrator::new(registry);
    let blueprint = TuiBlueprint::claude_code_inspired();

    // Print bootstrap info to stderr (TUI will show this in transcript)
    eprintln!("{}", orchestrator.describe_bootstrap(&intent, &session, &blueprint));
    eprintln!("Session ID: {}", session.id());

    // Create TUI state and run
    let mut state = TuiState::with_objective(&intent.objective());
    state.status.harness = Some(harness_name);
    state.footer_hint = "Enter: send │ /help: commands │ Ctrl+C: interrupt";

    // Run TUI
    if let Err(e) = bco_tui::run_tui(state) {
        eprintln!("TUI error: {}", e);
    }
}

fn review_command(objective_id: Option<&str>) {
    if let Some(id) = objective_id {
        println!("Reviewing objective: {}", id);
    } else {
        println!("Reviewing current session...");
    }
    println!("(Review functionality not yet implemented)");
}

fn resume_command(session_id: Option<&str>) {
    if let Some(id) = session_id {
        println!("Resuming session: {}", id);
    } else {
        println!("Resuming most recent session...");
    }
    println!("(Resume functionality not yet implemented)");
}

fn fork_command(session_id: Option<&str>) {
    if let Some(id) = session_id {
        println!("Forking session: {}", id);
    } else {
        println!("Forking current session...");
    }
    println!("(Fork functionality not yet implemented)");
}

fn providers_command(action: Option<&ProviderAction>) {
    match action {
        Some(ProviderAction::List) => {
            println!("Configured providers:");
            println!("  (No providers configured)");
        }
        Some(ProviderAction::Add { name, endpoint }) => {
            println!("Adding provider: {}", name);
            if let Some(ep) = endpoint {
                println!("  endpoint: {}", ep);
            }
            println!("(Provider add not yet implemented)");
        }
        Some(ProviderAction::Remove { name }) => {
            println!("Removing provider: {}", name);
            println!("(Provider remove not yet implemented)");
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
    match action {
        Some(ModelAction::List { provider }) => {
            println!("Available models:");
            if let Some(p) = provider {
                println!("  (Filtering by provider: {})", p);
            }
            println!("  (No models available)");
        }
        Some(ModelAction::Current) => {
            println!("Current model: none selected");
        }
        Some(ModelAction::Switch { model }) => {
            println!("Switching to model: {}", model);
            println!("(Model switch not yet implemented)");
        }
        None => {
            println!("Model management");
            println!("  Use 'bco models list' to list available models");
            println!("  Use 'bco models current' to show current model");
            println!("  Use 'bco models switch <provider/model>' to switch model");
        }
    }
}
