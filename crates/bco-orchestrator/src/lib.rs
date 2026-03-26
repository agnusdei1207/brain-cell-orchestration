use bco_core::{
    ObjectiveId, Objective, ObjectiveState, RiskProfile, TaskIntent, ProgressStatus,
    ActiveModelState, ModelRef, ModelSwitchReason, ModelSwitchEvent,
    ModelFallbackPolicy,
};
use bco_session::{SessionRuntime, TokenUsage};
use bco_harness::{HarnessRegistry, HarnessKind, PlanPolicy, ReviewPolicy, CapabilityPolicy};
use bco_session::SessionBootstrap;
use bco_tui::TuiBlueprint;
use uuid::Uuid;
use std::sync::RwLock;
use std::collections::HashMap;

// =============================================================================
// A4: Operation and Control-Plane Contract
// =============================================================================

/// Unique identifier for a cell
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
// D1: Blackboard and Event Flow
// =============================================================================

/// Shared blackboard state for cell communication
#[derive(Debug)]
pub struct Blackboard {
    state: RwLock<BlackboardState>,
}

#[derive(Debug, Default, Clone)]
pub struct BlackboardState {
    /// Current objective
    pub objective: Option<bco_core::Objective>,
    /// Active cells
    pub cells: HashMap<CellId, CellState>,
    /// Pending approvals
    pub pending_approvals: HashMap<Uuid, ApprovalRequest>,
    /// Next actions queue
    pub next_actions: Vec<bco_core::NextAction>,
    /// Cell lineage (parent -> children)
    pub lineage: HashMap<CellId, Vec<CellId>>,
}

#[derive(Debug, Clone)]
pub struct CellState {
    pub identity: CellIdentity,
    pub status: CellStatus,
    pub last_interaction: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellStatus {
    Idle,
    Planning,
    Coordinating,
    Executing,
    Reviewing,
    WaitingApproval,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone)]
pub struct ApprovalRequest {
    pub id: Uuid,
    pub cell_id: CellId,
    pub action: String,
    pub risk: RiskProfile,
    pub requested_at: chrono::DateTime<chrono::Utc>,
}

impl Blackboard {
    pub fn new() -> Self {
        Self {
            state: RwLock::new(BlackboardState::default()),
        }
    }

    pub fn set_objective(&self, objective: bco_core::Objective) {
        let mut state = self.state.write().unwrap();
        state.objective = Some(objective);
    }

    pub fn get_objective(&self) -> Option<bco_core::Objective> {
        let state = self.state.read().unwrap();
        state.objective.clone()
    }

    pub fn add_cell(&self, cell: CellIdentity) {
        let mut state = self.state.write().unwrap();
        let cell_state = CellState {
            identity: cell.clone(),
            status: CellStatus::Idle,
            last_interaction: None,
        };
        state.cells.insert(cell.id, cell_state);

        // Update lineage
        if let Some(parent) = cell.parent {
            state.lineage.entry(parent).or_default().push(cell.id);
        }
    }

    pub fn update_cell_status(&self, cell_id: CellId, status: CellStatus) {
        let mut state = self.state.write().unwrap();
        if let Some(cell_state) = state.cells.get_mut(&cell_id) {
            cell_state.status = status;
            cell_state.last_interaction = Some(chrono::Utc::now());
        }
    }

    pub fn get_cell_status(&self, cell_id: CellId) -> Option<CellStatus> {
        let state = self.state.read().unwrap();
        state.cells.get(&cell_id).map(|c| c.status)
    }

    pub fn add_approval_request(&self, request: ApprovalRequest) {
        let mut state = self.state.write().unwrap();
        state.pending_approvals.insert(request.id, request);
    }

    pub fn get_pending_approvals(&self) -> Vec<ApprovalRequest> {
        let state = self.state.read().unwrap();
        state.pending_approvals.values().cloned().collect()
    }

    pub fn resolve_approval(&self, id: Uuid, _approved: bool) -> Option<ApprovalRequest> {
        let mut state = self.state.write().unwrap();
        state.pending_approvals.remove(&id)
    }

    pub fn push_next_action(&self, action: bco_core::NextAction) {
        let mut state = self.state.write().unwrap();
        state.next_actions.push(action);
    }

    pub fn pop_next_action(&self) -> Option<bco_core::NextAction> {
        let mut state = self.state.write().unwrap();
        state.next_actions.pop()
    }

    pub fn get_active_cells(&self) -> Vec<CellId> {
        let state = self.state.read().unwrap();
        state.cells
            .iter()
            .filter(|(_, s)| !matches!(s.status, CellStatus::Completed | CellStatus::Failed | CellStatus::Cancelled))
            .map(|(id, _)| *id)
            .collect()
    }

    pub fn get_children(&self, parent_id: CellId) -> Vec<CellId> {
        let state = self.state.read().unwrap();
        state.lineage.get(&parent_id).cloned().unwrap_or_default()
    }

    pub fn shutdown_subtree(&self, root_id: CellId) {
        let mut state = self.state.write().unwrap();

        // Recursively find all descendants
        fn get_all_descendants(
            lineage: &HashMap<CellId, Vec<CellId>>,
            cell_id: CellId,
        ) -> Vec<CellId> {
            let mut result = vec![cell_id];
            if let Some(children) = lineage.get(&cell_id) {
                for child in children {
                    result.extend(get_all_descendants(lineage, *child));
                }
            }
            result
        }

        let all_cells = get_all_descendants(&state.lineage, root_id);

        for cell_id in all_cells {
            if let Some(cell_state) = state.cells.get_mut(&cell_id) {
                cell_state.status = CellStatus::Cancelled;
            }
        }
    }
}

impl Default for Blackboard {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// D2: Core Cells
// =============================================================================

/// Trait for all cells
pub trait Cell: Send + Sync {
    fn id(&self) -> CellId;
    fn cell_type(&self) -> CellType;
    fn process(&self, ctx: &ExecutionContext, blackboard: &Blackboard) -> CellResult;
}

/// Result of cell processing
#[derive(Debug)]
pub enum CellResult {
    /// Cell completed successfully
    Completed,
    /// Cell is waiting for something
    Waiting,
    /// Cell is blocked
    Blocked,
    /// Cell failed with error
    Failed(String),
}

/// Planner cell - decomposes objectives into subgoals
pub struct PlannerCell {
    identity: CellIdentity,
}

impl PlannerCell {
    pub fn new(parent: Option<CellId>) -> Self {
        Self {
            identity: CellIdentity::new(CellType::Planner, parent),
        }
    }
}

impl Cell for PlannerCell {
    fn id(&self) -> CellId {
        self.identity.id
    }

    fn cell_type(&self) -> CellType {
        CellType::Planner
    }

    fn process(&self, _ctx: &ExecutionContext, blackboard: &Blackboard) -> CellResult {
        let objective = match blackboard.get_objective() {
            Some(o) => o,
            None => return CellResult::Failed("No objective set".to_string()),
        };

        // If no subgoals yet, create initial subgoals
        if objective.subgoals.is_empty() {
            blackboard.push_next_action(bco_core::NextAction::new("Initial planning"));
            return CellResult::Waiting;
        }

        // Check if all subgoals are complete
        let all_complete = objective.subgoals.iter().all(|s| {
            s.state == bco_core::ObjectiveState::Completed
        });

        if all_complete {
            return CellResult::Completed;
        }

        CellResult::Waiting
    }
}

/// Coordinator cell - assigns work to executor cells
pub struct CoordinatorCell {
    identity: CellIdentity,
}

impl CoordinatorCell {
    pub fn new(parent: Option<CellId>) -> Self {
        Self {
            identity: CellIdentity::new(CellType::Coordinator, parent),
        }
    }
}

impl Cell for CoordinatorCell {
    fn id(&self) -> CellId {
        self.identity.id
    }

    fn cell_type(&self) -> CellType {
        CellType::Coordinator
    }

    fn process(&self, _ctx: &ExecutionContext, blackboard: &Blackboard) -> CellResult {
        // Get next pending action and assign to executor
        if let Some(mut action) = blackboard.pop_next_action() {
            action = action.with_cell("executor");
            blackboard.push_next_action(action);
            blackboard.update_cell_status(self.identity.id, CellStatus::Coordinating);
            return CellResult::Waiting;
        }

        CellResult::Waiting
    }
}

/// Executor cell - executes assigned work
pub struct ExecutorCell {
    identity: CellIdentity,
}

impl ExecutorCell {
    pub fn new(parent: Option<CellId>) -> Self {
        Self {
            identity: CellIdentity::new(CellType::Executor, parent),
        }
    }
}

impl Cell for ExecutorCell {
    fn id(&self) -> CellId {
        self.identity.id
    }

    fn cell_type(&self) -> CellType {
        CellType::Executor
    }

    fn process(&self, _ctx: &ExecutionContext, blackboard: &Blackboard) -> CellResult {
        blackboard.update_cell_status(self.identity.id, CellStatus::Executing);
        // Executor would call tools and produce evidence
        CellResult::Waiting
    }
}

/// Reviewer cell - reviews completed work and triggers replan if needed
pub struct ReviewerCell {
    identity: CellIdentity,
}

impl ReviewerCell {
    pub fn new(parent: Option<CellId>) -> Self {
        Self {
            identity: CellIdentity::new(CellType::Reviewer, parent),
        }
    }
}

impl Cell for ReviewerCell {
    fn id(&self) -> CellId {
        self.identity.id
    }

    fn cell_type(&self) -> CellType {
        CellType::Reviewer
    }

    fn process(&self, _ctx: &ExecutionContext, blackboard: &Blackboard) -> CellResult {
        blackboard.update_cell_status(self.identity.id, CellStatus::Reviewing);
        CellResult::Waiting
    }
}

// =============================================================================
// D3: Capability and Approval Enforcement
// =============================================================================

/// Policy evaluator for capability and approval checks
#[derive(Debug)]
pub struct PolicyEvaluator {
    harness_capabilities: CapabilityPolicy,
}

impl PolicyEvaluator {
    pub fn new(capabilities: CapabilityPolicy) -> Self {
        Self {
            harness_capabilities: capabilities,
        }
    }

    /// Check if an action with given risk profile is allowed
    pub fn can_execute(&self, risk: RiskProfile) -> bool {
        risk <= self.harness_capabilities.max_risk_profile
    }

    /// Check if an action requires approval
    pub fn requires_approval(&self, risk: RiskProfile) -> bool {
        risk > self.harness_capabilities.requires_approval_above
    }

    /// Evaluate an action and return decision
    pub fn evaluate(&self, risk: RiskProfile) -> PolicyDecision {
        if !self.can_execute(risk) {
            return PolicyDecision::Denied("Exceeds max risk profile".to_string());
        }
        if self.requires_approval(risk) {
            return PolicyDecision::RequiresApproval;
        }
        PolicyDecision::Approved
    }
}

#[derive(Debug)]
pub enum PolicyDecision {
    Approved,
    RequiresApproval,
    Denied(String),
}

// =============================================================================
// D4: Runtime Services
// =============================================================================

/// Session state for persistence
#[derive(Debug, Clone)]
pub struct SessionState {
    pub objective_id: Option<ObjectiveId>,
    pub status: String,
}

/// Runtime service container
#[derive(Debug)]
pub struct RuntimeServices {
    pub provider_registry: bco_core::ProviderRegistry,
    pub model_manager: ModelManager,
    pub tool_registry: ToolRegistry,
    pub session_store: SessionStore,
    pub policy_evaluator: PolicyEvaluator,
}

impl RuntimeServices {
    pub fn new(harness_capabilities: CapabilityPolicy) -> Self {
        Self {
            provider_registry: bco_core::ProviderRegistry::new(),
            model_manager: ModelManager::new(),
            tool_registry: ToolRegistry::new(),
            session_store: SessionStore::new(),
            policy_evaluator: PolicyEvaluator::new(harness_capabilities),
        }
    }
}

/// Model manager for handling model connections
#[derive(Debug)]
pub struct ModelManager {
    active_model: RwLock<Option<ActiveModelState>>,
    fallback_policy: ModelFallbackPolicy,
}

impl ModelManager {
    pub fn new() -> Self {
        Self {
            active_model: RwLock::new(None),
            fallback_policy: ModelFallbackPolicy::default(),
        }
    }

    pub fn set_active(&self, model: ModelRef) {
        let mut state = self.active_model.write().unwrap();
        *state = Some(ActiveModelState::new(model));
    }

    pub fn get_active(&self) -> Option<ActiveModelState> {
        self.active_model.read().unwrap().clone()
    }

    pub fn switch_model(&self, new_model: ModelRef, reason: ModelSwitchReason) -> Option<ModelSwitchEvent> {
        let mut state = self.active_model.write().unwrap();
        if let Some(ref mut active) = *state {
            let old_model = active.current.clone();
            active.switch_to(new_model);
            return Some(ModelSwitchEvent {
                timestamp: chrono::Utc::now(),
                from: old_model,
                to: active.current.clone(),
                reason,
            });
        }
        None
    }
}

impl Default for ModelManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Tool registry for managing available tools
pub struct ToolRegistry {
    tools: RwLock<HashMap<String, Box<dyn Tool>>>,
}

impl std::fmt::Debug for ToolRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolRegistry")
            .field("tool_count", &self.tools.read().unwrap().len())
            .finish()
    }
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: RwLock::new(HashMap::new()),
        }
    }

    pub fn register(&self, name: String, tool: Box<dyn Tool>) {
        self.tools.write().unwrap().insert(name, tool);
    }

    pub fn get(&self, name: &str) -> Option<Box<dyn Tool>> {
        self.tools.read().unwrap().get(name).map(|t| t.box_clone())
    }

    pub fn list(&self) -> Vec<String> {
        self.tools.read().unwrap().keys().cloned().collect()
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Tool trait
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn execute(&self, input: &str) -> Result<String, String>;
    fn box_clone(&self) -> Box<dyn Tool>;
}

/// Session store for persisting session state
pub struct SessionStore {
    sessions: RwLock<HashMap<bco_session::SessionId, SessionState>>,
}

impl std::fmt::Debug for SessionStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionStore")
            .field("session_count", &self.sessions.read().unwrap().len())
            .finish()
    }
}

impl SessionStore {
    pub fn new() -> Self {
        Self {
            sessions: RwLock::new(HashMap::new()),
        }
    }

    pub fn save(&self, id: bco_session::SessionId, state: SessionState) {
        self.sessions.write().unwrap().insert(id, state);
    }

    pub fn load(&self, id: bco_session::SessionId) -> Option<SessionState> {
        self.sessions.read().unwrap().get(&id).cloned()
    }

    pub fn list(&self) -> Vec<bco_session::SessionId> {
        self.sessions.read().unwrap().keys().cloned().collect()
    }
}

impl Default for SessionStore {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// G1: Wake and Retry
// =============================================================================

use std::sync::RwLock as StdRwLock;

/// Wake reason for resuming
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WakeReason {
    Manual,
    Scheduled,
    Retry,
    External,
}

/// Retry classification
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetryClass {
    Transient,  // Can retry immediately
    RateLimit,  // Should wait before retry
    AuthFailure, // Needs auth refresh
    Permanent,  // Should not retry
}

/// Pending work item
#[derive(Debug, Clone)]
pub struct PendingWork {
    pub id: Uuid,
    pub objective_id: ObjectiveId,
    pub action: String,
    pub retry_count: u8,
    pub max_retries: u8,
    pub last_error: Option<String>,
    pub retry_class: RetryClass,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl PendingWork {
    pub fn new(objective_id: ObjectiveId, action: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            objective_id,
            action,
            retry_count: 0,
            max_retries: 3,
            last_error: None,
            retry_class: RetryClass::Transient,
            created_at: chrono::Utc::now(),
        }
    }

    pub fn can_retry(&self) -> bool {
        self.retry_count < self.max_retries
    }

    pub fn record_failure(&mut self, error: String, retry_class: RetryClass) {
        self.last_error = Some(error);
        self.retry_class = retry_class;
        self.retry_count += 1;
    }
}

/// Autonomy scheduler for wake and retry
#[derive(Debug)]
pub struct AutonomyScheduler {
    pending_work: StdRwLock<Vec<PendingWork>>,
    retry_delay_ms: u64,
}

impl AutonomyScheduler {
    pub fn new() -> Self {
        Self {
            pending_work: StdRwLock::new(Vec::new()),
            retry_delay_ms: 1000,
        }
    }

    pub fn add_pending_work(&self, work: PendingWork) {
        self.pending_work.write().unwrap().push(work);
    }

    pub fn get_pending_work(&self) -> Vec<PendingWork> {
        self.pending_work.read().unwrap().clone()
    }

    pub fn drain_ready(&self) -> Vec<PendingWork> {
        let mut pending = self.pending_work.write().unwrap();
        let ready: Vec<PendingWork> = pending.iter()
            .filter(|w| w.can_retry())
            .cloned()
            .collect();
        // Remove ready items
        pending.retain(|w| !w.can_retry());
        ready
    }

    pub fn remove_completed(&self, id: Uuid) {
        let mut pending = self.pending_work.write().unwrap();
        pending.retain(|w| w.id != id);
    }

    pub fn get_retry_delay(&self, retry_class: RetryClass) -> u64 {
        match retry_class {
            RetryClass::Transient => self.retry_delay_ms,
            RetryClass::RateLimit => self.retry_delay_ms * 5,
            RetryClass::AuthFailure => self.retry_delay_ms * 10,
            RetryClass::Permanent => 0,
        }
    }
}

impl Default for AutonomyScheduler {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// G2: Memory and Checkpoints
// =============================================================================

/// Checkpoint data
#[derive(Debug, Clone)]
pub struct Checkpoint {
    pub id: Uuid,
    pub objective_id: ObjectiveId,
    pub state: CheckpointState,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub cell_states: Vec<CellState>,
}

#[derive(Debug, Clone)]
pub struct CheckpointState {
    pub blackboard: BlackboardState,
    pub event_queue: Vec<OrchestrationEvent>,
    pub active_model: Option<String>,
}

/// Memory flush policy
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryFlushPolicy {
    OnCheckpoint,
    OnCompletion,
    OnDemand,
    Rollover(u32), // Flush after N events
}

/// Memory summary
#[derive(Debug, Clone)]
pub struct MemorySummary {
    pub objective_id: ObjectiveId,
    pub summary: String,
    pub key_findings: Vec<String>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Checkpoint manager
#[derive(Debug)]
pub struct CheckpointManager {
    checkpoints: StdRwLock<Vec<Checkpoint>>,
    memory_summaries: StdRwLock<Vec<MemorySummary>>,
    flush_policy: MemoryFlushPolicy,
}

impl CheckpointManager {
    pub fn new() -> Self {
        Self {
            checkpoints: StdRwLock::new(Vec::new()),
            memory_summaries: StdRwLock::new(Vec::new()),
            flush_policy: MemoryFlushPolicy::OnCheckpoint,
        }
    }

    pub fn save_checkpoint(&self, checkpoint: Checkpoint) {
        self.checkpoints.write().unwrap().push(checkpoint);
    }

    pub fn get_latest_checkpoint(&self, objective_id: ObjectiveId) -> Option<Checkpoint> {
        let checkpoints = self.checkpoints.read().unwrap();
        checkpoints.iter()
            .filter(|c| c.objective_id == objective_id)
            .max_by_key(|c| c.timestamp)
            .cloned()
    }

    pub fn save_memory_summary(&self, summary: MemorySummary) {
        self.memory_summaries.write().unwrap().push(summary);
    }

    pub fn get_memory_summary(&self, objective_id: ObjectiveId) -> Option<MemorySummary> {
        let summaries = self.memory_summaries.read().unwrap();
        summaries.iter()
            .filter(|s| s.objective_id == objective_id)
            .max_by_key(|s| s.timestamp)
            .cloned()
    }
}

impl Default for CheckpointManager {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// G3: Failure Handling
// =============================================================================

/// Failure type
#[derive(Debug, Clone)]
pub enum FailureType {
    Crash,
    ModelFailover,
    ProviderReconnect,
    StaleSession,
}

/// Recovery action
#[derive(Debug, Clone)]
pub enum RecoveryAction {
    Restart,
    SwitchModel,
    ReconnectProvider,
    RestoreCheckpoint(Uuid),
    Abort,
}

/// Failure handler
#[derive(Debug)]
pub struct FailureHandler {
    checkpoint_manager: CheckpointManager,
}

impl FailureHandler {
    pub fn new() -> Self {
        Self {
            checkpoint_manager: CheckpointManager::new(),
        }
    }

    pub fn handle_failure(&self, failure: FailureType, context: &FailureContext) -> RecoveryAction {
        match failure {
            FailureType::Crash => {
                // Try to restore from latest checkpoint
                if let Some(checkpoint) = self.checkpoint_manager
                    .get_latest_checkpoint(context.objective_id)
                {
                    RecoveryAction::RestoreCheckpoint(checkpoint.id)
                } else {
                    RecoveryAction::Abort
                }
            }
            FailureType::ModelFailover => RecoveryAction::SwitchModel,
            FailureType::ProviderReconnect => RecoveryAction::ReconnectProvider,
            FailureType::StaleSession => RecoveryAction::Restart,
        }
    }

    pub fn checkpoint_manager(&self) -> &CheckpointManager {
        &self.checkpoint_manager
    }
}

impl Default for FailureHandler {
    fn default() -> Self {
        Self::new()
    }
}

/// Context for failure handling
#[derive(Debug)]
pub struct FailureContext {
    pub objective_id: ObjectiveId,
    pub error: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Runtime state that gets persisted
#[derive(Debug)]
pub struct RuntimeWriteback {
    pub active_model: Option<String>,
    pub token_usage: Option<TokenUsage>,
    pub abort_count: u32,
    pub compaction_count: u32,
    pub last_updated: chrono::DateTime<chrono::Utc>,
}

impl RuntimeWriteback {
    pub fn capture_from(services: &RuntimeServices) -> Self {
        Self {
            active_model: services.model_manager.get_active().map(|m| m.current.to_string()),
            token_usage: None, // Would be populated from actual model usage
            abort_count: 0,
            compaction_count: 0,
            last_updated: chrono::Utc::now(),
        }
    }

    pub fn to_session_runtime(&self, session_id: bco_session::SessionId) -> SessionRuntime {
        SessionRuntime {
            session_id,
            active_model: self.active_model.clone(),
            token_usage: self.token_usage,
            abort_count: self.abort_count,
            compaction_count: self.compaction_count,
            last_updated: self.last_updated,
        }
    }
}

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
