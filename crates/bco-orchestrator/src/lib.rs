use bco_core::TaskIntent;
use bco_harness::HarnessRegistry;
use bco_session::SessionBootstrap;
use bco_tui::TuiBlueprint;

#[derive(Debug, Clone)]
pub struct BrainCellOrchestrator {
    registry: HarnessRegistry,
}

impl BrainCellOrchestrator {
    pub fn new(registry: HarnessRegistry) -> Self {
        Self { registry }
    }

    pub fn describe_bootstrap(
        &self,
        intent: &TaskIntent,
        session: &SessionBootstrap,
        blueprint: &TuiBlueprint,
    ) -> String {
        let harness = self.registry.resolve(intent);
        format!(
            concat!(
                "brain-cell-orchestration bootstrap\n",
                "- objective: {}\n",
                "- domain: {}\n",
                "- selected harness: {}\n",
                "- session profile: {}\n",
                "- tui profile: {}\n",
                "- next milestone: implement planner/coordinator/executor/reviewer cells\n"
            ),
            intent.objective(),
            intent.domain().as_str(),
            harness.as_str(),
            session.profile(),
            blueprint.profile_name(),
        )
    }
}

