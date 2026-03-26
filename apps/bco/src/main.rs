use bco_core::{IntentDomain, TaskIntent};
use bco_harness::HarnessRegistry;
use bco_orchestrator::BrainCellOrchestrator;
use bco_session::SessionBootstrap;
use bco_tui::TuiBlueprint;

fn main() {
    let intent = TaskIntent::new(
        "bootstrap a dynamic orchestration runtime",
        IntentDomain::GeneralEngineering,
    );
    let session = SessionBootstrap::new("local-bootstrap");
    let registry = HarnessRegistry::with_defaults();
    let orchestrator = BrainCellOrchestrator::new(registry);
    let blueprint = TuiBlueprint::claude_code_inspired();

    println!("{}", orchestrator.describe_bootstrap(&intent, &session, &blueprint));
}

