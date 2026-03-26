use bco_core::{IntentDomain, TaskIntent};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HarnessKind {
    Ctf,
    Pentest,
    Coding,
    Generalist,
}

impl HarnessKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ctf => "ctf-harness",
            Self::Pentest => "pentest-harness",
            Self::Coding => "coding-harness",
            Self::Generalist => "generalist-harness",
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct HarnessRegistry;

impl HarnessRegistry {
    pub fn with_defaults() -> Self {
        Self
    }

    pub fn resolve(&self, intent: &TaskIntent) -> HarnessKind {
        match intent.domain() {
            IntentDomain::Ctf => HarnessKind::Ctf,
            IntentDomain::Pentesting => HarnessKind::Pentest,
            IntentDomain::Coding => HarnessKind::Coding,
            IntentDomain::GeneralEngineering => HarnessKind::Generalist,
        }
    }
}

