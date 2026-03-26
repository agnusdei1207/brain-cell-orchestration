#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntentDomain {
    Ctf,
    Pentesting,
    Coding,
    GeneralEngineering,
}

impl IntentDomain {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ctf => "ctf",
            Self::Pentesting => "pentesting",
            Self::Coding => "coding",
            Self::GeneralEngineering => "general-engineering",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskIntent {
    objective: String,
    domain: IntentDomain,
}

impl TaskIntent {
    pub fn new(objective: impl Into<String>, domain: IntentDomain) -> Self {
        Self {
            objective: objective.into(),
            domain,
        }
    }

    pub fn objective(&self) -> &str {
        &self.objective
    }

    pub fn domain(&self) -> IntentDomain {
        self.domain
    }
}

