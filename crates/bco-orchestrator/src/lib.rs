use bco_core::{
    ObjectiveId, RiskProfile, TaskIntent, ProgressStatus,
};
use bco_harness::{HarnessRegistry, HarnessKind, PlanPolicy, ReviewPolicy};
use bco_session::SessionBootstrap;
use bco_tui::TuiBlueprint;
use uuid::Uuid;

// =============================================================================
// A4: Operation and Control-Plane Contract
// =============================================================================

/// Unique identifier for a cell
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellId(pub Uuid);

impl CellId {
    pub fn new(cell_type: CellType) -> Self {
        let mut bytes = [0u8; 16];
        let hash = simple_hash(&format!("{:?}", cell_type));
        bytes.copy_from_slice(&hash[..16]);
        Self(uuid::Uuid::from_bytes(bytes))
    }
}

impl std::fmt::Display for CellId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "cell-{}", self.0)
    }
}

fn simple_hash(input: &str) -> [u8; 32] {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    input.hash(&mut hasher);
    let hash = hasher.finish();
    let mut bytes = [0u8; 32];
    bytes[..8].copy_from_slice(&hash.to_le_bytes());
    bytes[8..16].copy_from_slice(&hash.to_be_bytes());
    for i in 16..32 {
        bytes[i] = bytes[i - 16] ^ bytes[i - 8];
    }
    bytes
}

/// Types of cells in the orchestration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellType {
    Planner,
    Coordinator,
    Executor,
    Reviewer,
    /// Specialist cells (recon, exploit, coding, report, etc.)
    Specialist(&'static str),
}

/// Cell identity with parent-child relationship
#[derive(Debug, Clone)]
pub struct CellIdentity {
    pub id: CellId,
    pub cell_type: CellType,
    pub path: CellPath,
    pub parent: Option<CellId>,
}

impl CellIdentity {
    pub fn new(cell_type: CellType, parent: Option<CellId>) -> Self {
        let id = CellId::new(cell_type);
        let path = CellPath::new(&id, parent.as_ref());
        Self {
            id,
            cell_type,
            path,
            parent,
        }
    }
}

/// Hierarchical path of a cell (e.g., "root/planner/coordinator/executor")
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CellPath(Vec<CellId>);

impl CellPath {
    pub fn new(root: &CellId, parent: Option<&CellId>) -> Self {
        let mut path = Vec::new();
        if let Some(p) = parent {
            path.push(*p);
        }
        path.push(*root);
        Self(path)
    }

    pub fn depth(&self) -> i32 {
        self.0.len() as i32
    }

    pub fn exceeds_max_depth(&self, max_depth: i32) -> bool {
        self.depth() > max_depth
    }

    pub fn as_str(&self) -> String {
        self.0.iter()
            .map(|id| id.to_string())
            .collect::<Vec<_>>()
            .join("/")
    }
}

/// Pluggable operation/task trait
pub trait Operation: Send + Sync {
    fn id(&self) -> &str;
    fn kind(&self) -> OperationKind;
    fn risk(&self) -> RiskProfile;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationKind {
    Planning,
    Coordinating,
    Executing,
    Reviewing,
    ToolCall,
}

/// Orchestrator control plane - handles spawn, interrupt, shutdown
#[derive(Debug)]
pub struct ControlPlane {
    max_cell_depth: i32,
}

impl ControlPlane {
    pub fn new(max_cell_depth: i32) -> Self {
        Self { max_cell_depth }
    }

    pub fn can_spawn(&self, parent_path: &CellPath) -> bool {
        !parent_path.exceeds_max_depth(self.max_cell_depth - 1)
    }

    pub fn validate_spawn(&self, parent_path: &CellPath) -> Result<(), ControlPlaneError> {
        if !self.can_spawn(parent_path) {
            return Err(ControlPlaneError::DepthLimitExceeded {
                path: parent_path.as_str(),
                max: self.max_cell_depth,
            });
        }
        Ok(())
    }
}

#[derive(Debug)]
pub enum ControlPlaneError {
    DepthLimitExceeded { path: String, max: i32 },
    CellNotFound { id: CellId },
    InvalidState { reason: &'static str },
}

/// Typed execution context passed to cells
#[derive(Debug, Clone)]
pub struct ExecutionContext {
    pub objective_id: ObjectiveId,
    pub cell: CellIdentity,
    pub harness: HarnessKind,
    pub plan_policy: PlanPolicy,
    pub review_policy: ReviewPolicy,
    pub risk_profile: RiskProfile,
    pub cancellation_requested: bool,
}

impl ExecutionContext {
    pub fn new(
        objective_id: ObjectiveId,
        cell: CellIdentity,
        harness: HarnessKind,
        plan_policy: PlanPolicy,
        review_policy: ReviewPolicy,
    ) -> Self {
        Self {
            objective_id,
            cell,
            harness,
            plan_policy,
            review_policy,
            risk_profile: RiskProfile::Moderate,
            cancellation_requested: false,
        }
    }

    pub fn request_cancellation(&mut self) {
        self.cancellation_requested = true;
    }
}

/// Abort or interruption path
#[derive(Debug)]
#[allow(dead_code)]
pub struct AbortHandle {
    cell_id: CellId,
    reason: AbortReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbortReason {
    UserRequested,
    PolicyDenied,
    ResourceExhausted,
    ParentCancelled,
    Error,
}

// =============================================================================
// A5: Messaging and Observability Contract
// =============================================================================

/// Inter-cell message type
#[derive(Debug, Clone)]
pub struct InterCellMessage {
    pub id: Uuid,
    pub author: CellId,
    pub recipient: CellId,
    pub content: CellMessageContent,
    pub delivery: DeliveryMode,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl InterCellMessage {
    pub fn new(
        author: CellId,
        recipient: CellId,
        content: CellMessageContent,
        delivery: DeliveryMode,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            author,
            recipient,
            content,
            delivery,
            timestamp: chrono::Utc::now(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum CellMessageContent {
    /// Request an action from another cell
    Request { action: String, payload: String },
    /// Report progress
    Progress { status: ProgressStatus, message: String },
    /// Report completion
    Completed { result: String },
    /// Report failure
    Failed { error: String },
    /// Request approval
    ApprovalRequest { action: String, risk: RiskProfile },
    /// Approval granted
    ApprovalGranted { request_id: Uuid },
    /// Approval denied
    ApprovalDenied { request_id: Uuid, reason: String },
}

/// Delivery mode for inter-cell messages
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryMode {
    /// Message is queued, cell processes on next poll
    QueueOnly,
    /// Message triggers immediate turn processing
    TriggerNow,
}

/// Submission queue type (operator input -> orchestrator)
#[derive(Debug)]
pub struct SubmissionQueue {
    messages: Vec<OperatorInput>,
}

impl SubmissionQueue {
    pub fn new() -> Self {
        Self { messages: Vec::new() }
    }

    pub fn enqueue(&mut self, input: OperatorInput) {
        self.messages.push(input);
    }

    pub fn dequeue(&mut self) -> Option<OperatorInput> {
        self.messages.pop()
    }

    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }
}

impl Default for SubmissionQueue {
    fn default() -> Self {
        Self::new()
    }
}

/// Operator input to the orchestrator
#[derive(Debug, Clone)]
pub enum OperatorInput {
    /// Execute a task
    Execute { intent: TaskIntent },
    /// Request approval for an action
    Approve { request_id: Uuid },
    /// Deny approval request
    Deny { request_id: Uuid, reason: String },
    /// Switch model
    SwitchModel { model: String },
    /// Interrupt current operation
    Interrupt,
    /// Resume paused operation
    Resume { objective_id: ObjectiveId },
}

/// Event queue type (orchestrator -> UI/logs)
#[derive(Debug, Clone)]
pub struct EventQueue {
    events: Vec<OrchestrationEvent>,
}

impl EventQueue {
    pub fn new() -> Self {
        Self { events: Vec::new() }
    }

    pub fn emit(&mut self, event: OrchestrationEvent) {
        self.events.push(event);
    }

    pub fn drain(&mut self) -> Vec<OrchestrationEvent> {
        std::mem::take(&mut self.events)
    }
}

impl Default for EventQueue {
    fn default() -> Self {
        Self::new()
    }
}

/// Core orchestration event categories
#[derive(Debug, Clone)]
pub enum OrchestrationEvent {
    /// Cell lifecycle events
    CellSpawned { cell: CellId, parent: Option<CellId>, cell_type: &'static str },
    CellCompleted { cell: CellId },
    CellFailed { cell: CellId, error: String },
    CellCancelled { cell: CellId },
    CellInterrupted { cell: CellId },

    /// Interaction events
    InteractionBegin { from: CellId, to: CellId },
    InteractionEnd { from: CellId, to: CellId },

    /// Approval events
    ApprovalRequested { cell: CellId, action: String, risk: RiskProfile },
    ApprovalGranted { request_id: Uuid },
    ApprovalDenied { request_id: Uuid, reason: String },

    /// Model events
    ModelSwitch { from: String, to: String, reason: String },

    /// Objective events
    ObjectiveCreated { id: ObjectiveId },
    ObjectiveProgress { id: ObjectiveId, status: ProgressStatus },
    ObjectiveCompleted { id: ObjectiveId },
    ObjectiveFailed { id: ObjectiveId, error: String },

    /// Turn events
    TurnSubmitted { objective_id: ObjectiveId },
    TurnCompleted { objective_id: ObjectiveId },
    TurnAborted { objective_id: ObjectiveId, reason: String },
}

// =============================================================================
// Updated BrainCellOrchestrator
// =============================================================================

#[derive(Debug)]
pub struct BrainCellOrchestrator {
    registry: HarnessRegistry,
    control_plane: ControlPlane,
    event_queue: EventQueue,
}

impl BrainCellOrchestrator {
    pub fn new(registry: HarnessRegistry) -> Self {
        Self {
            registry,
            control_plane: ControlPlane::new(8), // max 8 levels deep
            event_queue: EventQueue::new(),
        }
    }

    pub fn describe_bootstrap(
        &self,
        intent: &TaskIntent,
        session: &SessionBootstrap,
        blueprint: &TuiBlueprint,
    ) -> String {
        let harness_kind = self.registry.resolve(intent);
        let harness = self.registry.get_harness(harness_kind);

        let harness_info = harness
            .map(|h| format!("{} ({:?})", h.name(), h.plan_policy()))
            .unwrap_or_else(|| "unknown".to_string());

        format!(
            concat!(
                "brain-cell-orchestration bootstrap\n",
                "- objective: {}\n",
                "- domain: {}\n",
                "- risk: {:?}\n",
                "- selected harness: {}\n",
                "- plan policy: {:?}\n",
                "- review policy: {:?}\n",
                "- session profile: {}\n",
                "- tui profile: {}\n",
                "- control plane depth limit: {}\n",
                "- next milestone: implement planner/coordinator/executor/reviewer cells\n"
            ),
            intent.objective(),
            intent.domain().as_str(),
            intent.risk_profile(),
            harness_info,
            harness.map(|h| h.plan_policy()).unwrap_or(PlanPolicy::Opportunistic),
            harness.map(|h| h.review_policy()).unwrap_or(ReviewPolicy::OnDemand),
            session.profile(),
            blueprint.profile_name(),
            self.control_plane.max_cell_depth,
        )
    }

    pub fn control_plane(&self) -> &ControlPlane {
        &self.control_plane
    }

    pub fn event_queue(&mut self) -> &mut EventQueue {
        &mut self.event_queue
    }

    pub fn emit_event(&mut self, event: OrchestrationEvent) {
        self.event_queue.emit(event);
    }
}
