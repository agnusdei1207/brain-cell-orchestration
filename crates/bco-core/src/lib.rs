use uuid::Uuid;
use serde::{Deserialize, Serialize};

// =============================================================================
// A1: Objective Model
// =============================================================================

/// Unique identifier for an objective
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ObjectiveId(pub Uuid);

impl ObjectiveId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for ObjectiveId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for ObjectiveId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Risk profile classification for tasks
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RiskProfile {
    /// Minimal risk, read-only operations
    Safe,
    /// Moderate risk, may modify local files
    Moderate,
    /// Elevated risk, may execute code or modify system
    Elevated,
    /// High risk, requires explicit approval
    High,
    /// Critical risk, requires multi-party approval
    Critical,
}

impl RiskProfile {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Safe => "safe",
            Self::Moderate => "moderate",
            Self::Elevated => "elevated",
            Self::High => "high",
            Self::Critical => "critical",
        }
    }
}

/// Current state of an objective
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ObjectiveState {
    /// Objective created but not yet started
    Pending,
    /// Objective is actively being worked on
    Active,
    /// Objective is blocked waiting on external input
    Blocked,
    /// Objective completed successfully
    Completed,
    /// Objective failed
    Failed,
    /// Objective was cancelled
    Cancelled,
}

impl ObjectiveState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Active => "active",
            Self::Blocked => "blocked",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }
}

/// Progress status of an objective or subgoal
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProgressStatus {
    /// No progress yet
    NotStarted,
    /// In progress, percentage unknown
    InProgress,
    /// In progress, estimated percentage complete
    Progress(u8), // 0-100
    /// Waiting on external event
    WaitingExternal,
    /// Waiting on approval
    WaitingApproval,
    /// Completed
    Done,
}

impl ProgressStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotStarted => "not_started",
            Self::InProgress => "in_progress",
            Self::Progress(_) => {
                // This is a simplification - in real impl would return dynamic string
                "progress"
            }
            Self::WaitingExternal => "waiting_external",
            Self::WaitingApproval => "waiting_approval",
            Self::Done => "done",
        }
    }
}

/// A subgoal that breaks down an objective
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Subgoal {
    pub id: Uuid,
    pub description: String,
    pub state: ObjectiveState,
    pub progress: ProgressStatus,
    pub parent_id: Option<Uuid>,
}

impl Subgoal {
    pub fn new(description: impl Into<String>, parent_id: Option<Uuid>) -> Self {
        Self {
            id: Uuid::new_v4(),
            description: description.into(),
            state: ObjectiveState::Pending,
            progress: ProgressStatus::NotStarted,
            parent_id,
        }
    }
}

/// The next action to be taken
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NextAction {
    pub id: Uuid,
    pub description: String,
    pub assigned_cell: Option<String>,
    pub estimated_risk: RiskProfile,
    pub requires_approval: bool,
}

impl NextAction {
    pub fn new(description: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            description: description.into(),
            assigned_cell: None,
            estimated_risk: RiskProfile::Moderate,
            requires_approval: false,
        }
    }

    pub fn with_risk(mut self, risk: RiskProfile) -> Self {
        self.estimated_risk = risk;
        self
    }

    pub fn with_cell(mut self, cell: impl Into<String>) -> Self {
        self.assigned_cell = Some(cell.into());
        self
    }

    pub fn with_approval_required(mut self) -> Self {
        self.requires_approval = true;
        self
    }
}

/// The overall objective tracked by the orchestrator
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Objective {
    pub id: ObjectiveId,
    pub intent: TaskIntent,
    pub state: ObjectiveState,
    pub progress: ProgressStatus,
    pub subgoals: Vec<Subgoal>,
    pub next_action: Option<NextAction>,
    pub risk_profile: RiskProfile,
}

impl Objective {
    pub fn new(intent: TaskIntent, risk_profile: RiskProfile) -> Self {
        Self {
            id: ObjectiveId::new(),
            intent,
            state: ObjectiveState::Pending,
            progress: ProgressStatus::NotStarted,
            subgoals: Vec::new(),
            next_action: None,
            risk_profile,
        }
    }

    pub fn current_goal(&self) -> String {
        self.intent.objective.clone()
    }

    pub fn active_subgoal(&self) -> Option<&Subgoal> {
        self.subgoals.iter().find(|s| s.state == ObjectiveState::Active)
    }
}

// =============================================================================
// A2: Intent Domain (already existed, moved here)
// =============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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

/// Task intent passed to the orchestrator
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskIntent {
    objective: String,
    domain: IntentDomain,
    risk_profile: RiskProfile,
}

impl TaskIntent {
    pub fn new(
        objective: impl Into<String>,
        domain: IntentDomain,
        risk_profile: RiskProfile,
    ) -> Self {
        Self {
            objective: objective.into(),
            domain,
            risk_profile,
        }
    }

    pub fn objective(&self) -> &str {
        &self.objective
    }

    pub fn domain(&self) -> IntentDomain {
        self.domain
    }

    pub fn risk_profile(&self) -> RiskProfile {
        self.risk_profile
    }
}

// =============================================================================
// A2: Model Identity and Connection Model
// =============================================================================

use std::fmt;

/// Reference to a model provider (e.g., "anthropic", "openai", "ollama")
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProviderRef {
    id: String,
}

impl ProviderRef {
    pub fn new(id: impl Into<String>) -> Self {
        Self { id: id.into() }
    }

    pub fn as_str(&self) -> &str {
        &self.id
    }
}

impl fmt::Display for ProviderRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.id)
    }
}

impl<'a> From<&'a str> for ProviderRef {
    fn from(s: &'a str) -> Self {
        Self::new(s)
    }
}

/// Reference to a specific model (e.g., "claude-sonnet-4-6")
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ModelRef {
    provider: ProviderRef,
    model_id: String,
}

impl ModelRef {
    pub fn new(provider: ProviderRef, model_id: impl Into<String>) -> Self {
        Self {
            provider,
            model_id: model_id.into(),
        }
    }

    /// Parse from canonical "provider/model" string format
    pub fn parse(s: &str) -> Result<Self, ModelParseError> {
        let parts: Vec<&str> = s.split('/').collect();
        if parts.len() != 2 {
            return Err(ModelParseError(s.to_string()));
        }
        Ok(Self::new(ProviderRef::new(parts[0]), parts[1]))
    }

    pub fn provider(&self) -> &ProviderRef {
        &self.provider
    }

    pub fn model_id(&self) -> &str {
        &self.model_id
    }
}

impl fmt::Display for ModelRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.provider, self.model_id)
    }
}

#[derive(Debug, Clone)]
pub struct ModelParseError(String);

impl fmt::Display for ModelParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid model reference: {} (expected 'provider/model')", self.0)
    }
}

impl std::error::Error for ModelParseError {}

/// Connection profile for a provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionProfile {
    pub provider: ProviderRef,
    pub endpoint: Option<String>,
    pub api_key: Option<String>,
    pub auth_type: AuthType,
    pub state: ConnectionState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthType {
    None,
    ApiKey,
    OAuth,
    Bearer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Error,
    Cooldown,
}

/// Active model state in the runtime
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveModelState {
    pub current: ModelRef,
    pub fallback: Option<ModelRef>,
    pub switch_count: u32,
}

impl ActiveModelState {
    pub fn new(current: ModelRef) -> Self {
        Self {
            current,
            fallback: None,
            switch_count: 0,
        }
    }

    pub fn with_fallback(mut self, fallback: ModelRef) -> Self {
        self.fallback = Some(fallback);
        self
    }

    pub fn switch_to(&mut self, new_model: ModelRef) {
        self.fallback = Some(std::mem::replace(&mut self.current, new_model));
        self.switch_count += 1;
    }
}

/// Event emitted when model is switched
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelSwitchEvent {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub from: ModelRef,
    pub to: ModelRef,
    pub reason: ModelSwitchReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModelSwitchReason {
    Manual,
    RateLimit,
    AuthFailure,
    ModelNotFound,
    ProviderError,
    AutoFallback,
}

/// Policy for automatic model fallback
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelFallbackPolicy {
    pub enabled: bool,
    pub max_retries: u8,
    pub retry_delay_ms: u64,
}

impl Default for ModelFallbackPolicy {
    fn default() -> Self {
        Self {
            enabled: true,
            max_retries: 3,
            retry_delay_ms: 1000,
        }
    }
}

