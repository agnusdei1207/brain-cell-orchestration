use bco_core::{
    ObjectiveId, Objective, RiskProfile, TaskIntent, ProgressStatus,
    ActiveModelState, ModelRef, ModelSwitchReason, ModelSwitchEvent,
    ModelFallbackPolicy,
};
use bco_session::{SessionRuntime, TokenUsage, SessionLayout};
use bco_harness::{HarnessRegistry, HarnessKind, PlanPolicy, ReviewPolicy, CapabilityPolicy};
use bco_session::SessionBootstrap;
use bco_tui::TuiBlueprint;
use uuid::Uuid;
use serde::Serialize;
use std::io::Write;
use std::sync::RwLock;
use std::collections::HashMap;

// =============================================================================
// A4: Operation and Control-Plane Contract
// =============================================================================

/// Unique identifier for a cell
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
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

    pub fn update_objective<F>(&self, mutator: F)
    where
        F: FnOnce(&mut bco_core::Objective),
    {
        let mut state = self.state.write().unwrap();
        if let Some(objective) = state.objective.as_mut() {
            mutator(objective);
        }
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

    pub fn next_actions(&self) -> Vec<bco_core::NextAction> {
        let state = self.state.read().unwrap();
        state.next_actions.clone()
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

    pub fn cell_states(&self) -> Vec<CellState> {
        let state = self.state.read().unwrap();
        state.cells.values().cloned().collect()
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
    /// Retry state per model (reason -> retry count)
    retry_state: RwLock<HashMap<String, u8>>,
}

/// Recommendation for model switch after failure
#[derive(Debug)]
pub struct ModelSwitchRecommendation {
    pub should_switch: bool,
    pub reason: ModelSwitchReason,
    pub fallback_model: Option<ModelRef>,
    pub retry_delay_ms: u64,
}

impl ModelManager {
    pub fn new() -> Self {
        Self {
            active_model: RwLock::new(None),
            fallback_policy: ModelFallbackPolicy::default(),
            retry_state: RwLock::new(HashMap::new()),
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
            active.switch_to(new_model.clone());

            // Reset retry count on successful switch
            if reason == ModelSwitchReason::Manual {
                let mut retries = self.retry_state.write().unwrap();
                retries.remove(&new_model.to_string());
            }

            return Some(ModelSwitchEvent {
                timestamp: chrono::Utc::now(),
                from: old_model,
                to: active.current.clone(),
                reason,
            });
        }
        None
    }

    /// Switch to the fallback model if one is configured
    pub fn switch_to_fallback(&self, fallback: ModelRef) {
        let mut state = self.active_model.write().unwrap();
        if let Some(ref mut active) = *state {
            let old_model = active.current.clone();
            // Move current to fallback, set new fallback
            active.fallback = Some(old_model);
            active.current = fallback;
            active.switch_count += 1;

            // Reset retry count on switch
            let mut retries = self.retry_state.write().unwrap();
            retries.remove(&active.current.to_string());
        }
    }

    /// Handle a model failure and determine if fallback should occur
    pub fn handle_model_failure(&self, model: &ModelRef, error: &str) -> ModelSwitchRecommendation {
        let reason = self.classify_failure_reason(error);
        let model_str = model.to_string();

        // Increment retry count
        {
            let mut retries = self.retry_state.write().unwrap();
            let count = retries.entry(model_str.clone()).or_insert(0);
            *count += 1;
        }

        // Check if we've exceeded retries for this reason
        let retries = self.retry_state.read().unwrap();
        let current_retries = *retries.get(&model_str).unwrap_or(&0);
        let max_retries = self.fallback_policy.max_retries;

        if current_retries >= max_retries {
            return ModelSwitchRecommendation {
                should_switch: true,
                reason,
                fallback_model: None,
                retry_delay_ms: self.get_retry_delay(reason),
            };
        }

        // Rate limit errors should wait longer
        if reason == ModelSwitchReason::RateLimit {
            return ModelSwitchRecommendation {
                should_switch: false,
                reason,
                fallback_model: None,
                retry_delay_ms: self.get_retry_delay(reason),
            };
        }

        // Configuration failures need intervention
        if reason == ModelSwitchReason::ConfigurationError {
            return ModelSwitchRecommendation {
                should_switch: false,
                reason,
                fallback_model: None,
                retry_delay_ms: self.get_retry_delay(reason),
            };
        }

        ModelSwitchRecommendation {
            should_switch: false,
            reason,
            fallback_model: None,
            retry_delay_ms: self.get_retry_delay(reason),
        }
    }

    /// Classify failure reason from error message
    fn classify_failure_reason(&self, error: &str) -> ModelSwitchReason {
        let error_lower = error.to_lowercase();

        if error_lower.contains("rate limit") || error_lower.contains("429") ||
           error_lower.contains("too many requests") || error_lower.contains("quota") {
            return ModelSwitchReason::RateLimit;
        }

        if error_lower.contains("invalid configuration") || error_lower.contains("bad request") ||
           error_lower.contains("permission denied") || error_lower.contains("unsupported model") ||
           error_lower.contains("invalid provider") {
            return ModelSwitchReason::ConfigurationError;
        }

        if error_lower.contains("not found") || error_lower.contains("404") ||
           error_lower.contains("model not found") || error_lower.contains("unknown model") {
            return ModelSwitchReason::ModelNotFound;
        }

        if error_lower.contains("provider") || error_lower.contains("connection") ||
           error_lower.contains("timeout") || error_lower.contains("network") ||
           error_lower.contains("503") || error_lower.contains("502") {
            return ModelSwitchReason::ProviderError;
        }

        ModelSwitchReason::ProviderError
    }

    /// Get retry delay based on failure reason
    fn get_retry_delay(&self, reason: ModelSwitchReason) -> u64 {
        match reason {
            ModelSwitchReason::RateLimit => self.fallback_policy.retry_delay_ms * 10,
            ModelSwitchReason::ConfigurationError => self.fallback_policy.retry_delay_ms * 5,
            ModelSwitchReason::ProviderError => self.fallback_policy.retry_delay_ms * 2,
            _ => self.fallback_policy.retry_delay_ms,
        }
    }

    /// Check if fallback is enabled
    pub fn is_fallback_enabled(&self) -> bool {
        self.fallback_policy.enabled
    }

    /// Update fallback policy
    pub fn set_fallback_policy(&mut self, policy: ModelFallbackPolicy) {
        self.fallback_policy = policy;
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
    Configuration, // Needs operator correction
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
            RetryClass::Configuration => self.retry_delay_ms * 10,
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

// =============================================================================
// Hook System - event-driven automation
// =============================================================================

/// Hook event types that can trigger automation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HookEvent {
    /// On checkpoint creation
    OnCheckpoint,
    /// On objective completion
    OnComplete,
    /// On objective failure
    OnFail,
    /// On approval requested
    OnApprovalRequested,
    /// On approval granted
    OnApprovalGranted,
    /// On model switch
    OnModelSwitch,
    /// On cell spawn
    OnCellSpawn,
    /// On cell complete
    OnCellComplete,
    /// On session start
    OnSessionStart,
    /// On session end
    OnSessionEnd,
    /// Rollover event (after N events)
    OnRollover,
}

/// Hook action to perform
#[derive(Debug, Clone)]
pub enum HookAction {
    /// Flush memory summary
    FlushMemory,
    /// Create checkpoint
    CreateCheckpoint,
}

/// A registered hook
#[derive(Debug, Clone)]
pub struct Hook {
    pub event: HookEvent,
    pub action: HookAction,
    pub enabled: bool,
}

impl Hook {
    pub fn new(event: HookEvent, action: HookAction) -> Self {
        Self { event, action, enabled: true }
    }
}

/// Hook registry for automation
#[derive(Debug)]
pub struct HookRegistry {
    hooks: StdRwLock<Vec<Hook>>,
    event_counts: StdRwLock<HashMap<HookEvent, u32>>,
}

impl HookRegistry {
    pub fn new() -> Self {
        Self {
            hooks: StdRwLock::new(Vec::new()),
            event_counts: StdRwLock::new(HashMap::new()),
        }
    }

    /// Register a hook
    pub fn register(&self, hook: Hook) {
        let mut hooks = self.hooks.write().unwrap();
        hooks.push(hook);
    }

    /// Register a hook for an event with action
    pub fn on(&self, event: HookEvent, action: HookAction) {
        self.register(Hook::new(event, action));
    }

    /// Unregister all hooks for an event
    pub fn unregister(&self, event: HookEvent) {
        let mut hooks = self.hooks.write().unwrap();
        hooks.retain(|h| h.event != event);
    }

    /// Trigger hooks for an event, returns actions to execute
    pub fn trigger(&self, event: HookEvent) -> Vec<HookAction> {
        // Increment event count
        {
            let mut counts = self.event_counts.write().unwrap();
            *counts.entry(event).or_insert(0) += 1;
        }

        let hooks = self.hooks.read().unwrap();
        hooks.iter()
            .filter(|h| h.enabled && h.event == event)
            .map(|h| h.action.clone())
            .collect()
    }

    /// Check if any hooks are registered for an event
    pub fn has_hooks(&self, event: HookEvent) -> bool {
        let hooks = self.hooks.read().unwrap();
        hooks.iter().any(|h| h.enabled && h.event == event)
    }

    /// Get event count for rollover tracking
    pub fn event_count(&self, event: HookEvent) -> u32 {
        let counts = self.event_counts.read().unwrap();
        *counts.get(&event).unwrap_or(&0)
    }

    /// Reset event count
    pub fn reset_count(&self, event: HookEvent) {
        let mut counts = self.event_counts.write().unwrap();
        counts.remove(&event);
    }
}

impl Default for HookRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Checkpoint manager
#[derive(Debug)]
pub struct CheckpointManager {
    checkpoints: StdRwLock<Vec<Checkpoint>>,
    memory_summaries: StdRwLock<Vec<MemorySummary>>,
    flush_policy: MemoryFlushPolicy,
    hook_registry: HookRegistry,
    event_counter: StdRwLock<u32>,
}

impl CheckpointManager {
    pub fn new() -> Self {
        Self {
            checkpoints: StdRwLock::new(Vec::new()),
            memory_summaries: StdRwLock::new(Vec::new()),
            flush_policy: MemoryFlushPolicy::OnCheckpoint,
            hook_registry: HookRegistry::new(),
            event_counter: StdRwLock::new(0),
        }
    }

    /// Get the hook registry for registering hooks
    pub fn hooks(&self) -> &HookRegistry {
        &self.hook_registry
    }

    /// Set memory flush policy
    pub fn set_flush_policy(&mut self, policy: MemoryFlushPolicy) {
        self.flush_policy = policy;
    }

    /// Trigger hooks and execute actions
    pub fn trigger_hooks(&self, event: HookEvent) -> Vec<HookAction> {
        // Increment counter
        {
            let mut counter = self.event_counter.write().unwrap();
            *counter += 1;
        }

        // Check rollover policy
        if let MemoryFlushPolicy::Rollover(n) = self.flush_policy {
            let counter = *self.event_counter.read().unwrap();
            if counter >= n {
                // Reset and trigger rollover
                *self.event_counter.write().unwrap() = 0;
                return vec![HookAction::FlushMemory];
            }
        }

        self.hook_registry.trigger(event)
    }

    /// Check if memory should be flushed based on policy
    pub fn should_flush(&self, event: HookEvent) -> bool {
        match self.flush_policy {
            MemoryFlushPolicy::OnCheckpoint => matches!(event, HookEvent::OnCheckpoint),
            MemoryFlushPolicy::OnCompletion => matches!(event, HookEvent::OnComplete),
            MemoryFlushPolicy::OnDemand => false,
            MemoryFlushPolicy::Rollover(_) => {
                let counter = *self.event_counter.read().unwrap();
                if let MemoryFlushPolicy::Rollover(n) = self.flush_policy {
                    counter >= n
                } else {
                    false
                }
            }
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
#[derive(Debug, Clone)]
pub struct SubmittedInput {
    pub session_id: bco_session::SessionId,
    pub input: OperatorInput,
}

#[derive(Debug)]
pub struct SubmissionQueue {
    messages: std::collections::VecDeque<SubmittedInput>,
}

impl SubmissionQueue {
    pub fn new() -> Self {
        Self {
            messages: std::collections::VecDeque::new(),
        }
    }

    pub fn enqueue(&mut self, session_id: bco_session::SessionId, input: OperatorInput) {
        self.messages.push_back(SubmittedInput { session_id, input });
    }

    pub fn dequeue(&mut self) -> Option<SubmittedInput> {
        self.messages.pop_front()
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

    pub fn len(&self) -> usize {
        self.events.len()
    }
}

impl Default for EventQueue {
    fn default() -> Self {
        Self::new()
    }
}

/// Core orchestration event categories
#[derive(Debug, Clone, Serialize)]
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
    ObjectivePlanReady { id: ObjectiveId, steps: Vec<String> },

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

// =============================================================================
// Inter-Cell Message Bus - real message routing between cells
// =============================================================================

/// Inter-cell message bus - routes messages between cells
#[derive(Debug)]
pub struct MessageBus {
    /// Pending messages indexed by recipient
    pending: RwLock<HashMap<CellId, Vec<InterCellMessage>>>,
    /// Delivery mode preference
    delivery_mode: DeliveryMode,
}

impl MessageBus {
    pub fn new() -> Self {
        Self {
            pending: RwLock::new(HashMap::new()),
            delivery_mode: DeliveryMode::QueueOnly,
        }
    }

    /// Send a message to a cell
    pub fn send(&self, message: InterCellMessage) {
        let mut pending = self.pending.write().unwrap();
        pending
            .entry(message.recipient)
            .or_default()
            .push(message);
    }

    /// Send message to multiple recipients (broadcast)
    pub fn broadcast(&self, authors: &[CellId], recipient: CellId, content: CellMessageContent) {
        for author in authors {
            let msg = InterCellMessage::new(*author, recipient, content.clone(), self.delivery_mode);
            self.send(msg);
        }
    }

    /// Receive messages for a specific cell
    pub fn receive(&self, recipient: CellId) -> Vec<InterCellMessage> {
        let mut pending = self.pending.write().unwrap();
        pending.remove(&recipient).unwrap_or_default()
    }

    /// Check if a cell has pending messages
    pub fn has_messages(&self, recipient: CellId) -> bool {
        let pending = self.pending.read().unwrap();
        pending.get(&recipient).map(|msgs| !msgs.is_empty()).unwrap_or(false)
    }

    /// Get pending message count for a cell
    pub fn pending_count(&self, recipient: CellId) -> usize {
        let pending = self.pending.read().unwrap();
        pending.get(&recipient).map(|msgs| msgs.len()).unwrap_or(0)
    }

    /// Clear all pending messages
    pub fn clear(&self) {
        let mut pending = self.pending.write().unwrap();
        pending.clear();
    }
}

impl Default for MessageBus {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Session Actor Queue - serializes session operations
// =============================================================================

/// Session actor queue - ensures serialized session mutations
#[derive(Debug)]
pub struct SessionActorQueue {
    /// Per-session state tracking (replaces global gate)
    session_states: StdRwLock<HashMap<bco_session::SessionId, SessionActorState>>,
    pending_count: StdRwLock<HashMap<bco_session::SessionId, u32>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SessionActorState {
    Idle,
    Processing,
    Busy,
}

impl SessionActorQueue {
    pub fn new() -> Self {
        Self {
            session_states: StdRwLock::new(HashMap::new()),
            pending_count: StdRwLock::new(HashMap::new()),
        }
    }

    /// Check if a session is currently processing
    pub fn is_processing(&self, session_id: bco_session::SessionId) -> bool {
        let states = self.session_states.read().unwrap();
        match states.get(&session_id) {
            Some(state) => *state != SessionActorState::Idle,
            None => false,
        }
    }

    /// Enqueue an operation for a session
    /// Returns true if session was idle and is now processing, false if already busy
    pub fn enqueue(&self, session_id: bco_session::SessionId) -> bool {
        let mut states = self.session_states.write().unwrap();

        // Check this session's current state
        match states.get(&session_id) {
            Some(state) if *state != SessionActorState::Idle => {
                // Session is already processing or busy, just increment pending
                drop(states);
                let mut count = self.pending_count.write().unwrap();
                *count.entry(session_id).or_insert(0) += 1;
                return false;
            }
            _ => {
                // Session is idle (or new), set to processing
                states.insert(session_id, SessionActorState::Processing);
            }
        }

        // Initialize pending count if needed
        let mut count = self.pending_count.write().unwrap();
        *count.entry(session_id).or_insert(0) += 1;
        true
    }

    /// Dequeue (complete) an operation for a session
    pub fn dequeue(&self, session_id: bco_session::SessionId) {
        let mut count = self.pending_count.write().unwrap();
        if let Some(c) = count.get_mut(&session_id) {
            *c = c.saturating_sub(1);
            if *c == 0 {
                count.remove(&session_id);
                // Clear session state to idle
                drop(count);
                let mut states = self.session_states.write().unwrap();
                states.remove(&session_id);
            }
        }
    }

    /// Get total pending operations across all sessions
    pub fn total_pending(&self) -> usize {
        let count = self.pending_count.read().unwrap();
        count.values().sum::<u32>() as usize
    }

    /// Force state to busy (when session is handling multiple operations)
    pub fn set_busy(&self, session_id: bco_session::SessionId) {
        let mut states = self.session_states.write().unwrap();
        states.insert(session_id, SessionActorState::Busy);
        drop(states);
        let mut count = self.pending_count.write().unwrap();
        *count.entry(session_id).or_insert(0) += 1;
    }
}

impl Default for SessionActorQueue {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Orchestrator Runtime - actual turn loop wiring
// =============================================================================

use std::sync::Arc;

/// Event log wrapper that implements the timestamp trait for persistence
#[derive(Debug, Clone, Serialize)]
struct EventLogEntry {
    timestamp: chrono::DateTime<chrono::Utc>,
    event: OrchestrationEvent,
}

#[derive(Debug, Clone, Serialize)]
struct CellTopologyEntry {
    timestamp: chrono::DateTime<chrono::Utc>,
    cell: String,
    parent: Option<String>,
    cell_type: &'static str,
}

#[derive(Debug, Clone, Serialize)]
struct ModelEventEntry {
    timestamp: chrono::DateTime<chrono::Utc>,
    from: String,
    to: String,
    reason: String,
}

#[derive(Debug, Clone, Serialize)]
struct PendingWorkLogEntry {
    timestamp: chrono::DateTime<chrono::Utc>,
    id: Uuid,
    objective_id: ObjectiveId,
    action: String,
    retry_count: u8,
    max_retries: u8,
    last_error: Option<String>,
    retry_class: &'static str,
}

#[derive(Debug, Clone, Serialize)]
struct ApprovalLogEntry {
    timestamp: chrono::DateTime<chrono::Utc>,
    kind: &'static str,
    request_id: Uuid,
    cell: Option<CellId>,
    action: Option<String>,
    risk: Option<RiskProfile>,
    reason: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct TranscriptEntry {
    timestamp: chrono::DateTime<chrono::Utc>,
    line: String,
}

#[derive(Debug, Clone, Serialize)]
struct PlanEntry {
    timestamp: chrono::DateTime<chrono::Utc>,
    objective_id: ObjectiveId,
    steps: Vec<String>,
    active_index: usize,
}

/// Orchestrator runtime - wired execution loop
#[derive(Debug)]
pub struct OrchestratorRuntime {
    orchestrator: Arc<std::sync::RwLock<BrainCellOrchestrator>>,
    blackboard: Arc<Blackboard>,
    message_bus: Arc<MessageBus>,
    submission_queue: Arc<std::sync::RwLock<SubmissionQueue>>,
    session_queue: Arc<SessionActorQueue>,
    autonomy_scheduler: Arc<AutonomyScheduler>,
    services: RuntimeServices,
    /// Optional session layout for event persistence
    session_layout: Option<SessionLayout>,
}

impl OrchestratorRuntime {
    pub fn new(registry: HarnessRegistry, services: RuntimeServices) -> Self {
        Self {
            orchestrator: Arc::new(std::sync::RwLock::new(BrainCellOrchestrator::new(registry))),
            blackboard: Arc::new(Blackboard::new()),
            message_bus: Arc::new(MessageBus::new()),
            submission_queue: Arc::new(std::sync::RwLock::new(SubmissionQueue::new())),
            session_queue: Arc::new(SessionActorQueue::new()),
            autonomy_scheduler: Arc::new(AutonomyScheduler::new()),
            services,
            session_layout: None,
        }
    }

    /// Set the session layout for event persistence
    pub fn with_session_layout(mut self, layout: SessionLayout) -> Self {
        self.session_layout = Some(layout);
        self
    }

    /// Persist any queued events to the event log file
    pub fn flush_events(&self) -> std::io::Result<()> {
        let Some(layout) = &self.session_layout else {
            return Ok(()); // No layout configured, skip persistence
        };

        let events = {
            let mut orch = self.orchestrator.write().unwrap();
            orch.event_queue.drain()
        };

        if events.is_empty() {
            return Ok(());
        }

        let event_path = layout.orchestrator_events_jsonl();
        let transcript_path = layout.transcript_jsonl();
        let plan_path = layout.plan_jsonl();
        let topology_path = layout.cell_topology_jsonl();
        let model_path = layout.model_events_jsonl();
        let approvals_path = layout.approvals_jsonl();
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&event_path)?;
        let transcript_file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&transcript_path)?;
        let plan_file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&plan_path)?;
        let topology_file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&topology_path)?;
        let model_file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&model_path)?;
        let approvals_file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&approvals_path)?;

        let mut file = std::io::BufWriter::new(file);
        let mut transcript_file = std::io::BufWriter::new(transcript_file);
        let mut plan_file = std::io::BufWriter::new(plan_file);
        let mut topology_file = std::io::BufWriter::new(topology_file);
        let mut model_file = std::io::BufWriter::new(model_file);
        let mut approvals_file = std::io::BufWriter::new(approvals_file);
        for event in events {
            let now = chrono::Utc::now();
            let entry = EventLogEntry {
                timestamp: now,
                event: event.clone(),
            };
            let json = serde_json::to_string(&entry)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
            writeln!(file, "{}", json)?;
            let transcript_entry = TranscriptEntry {
                timestamp: now,
                line: event_to_transcript_line(&entry.event),
            };
            let json = serde_json::to_string(&transcript_entry)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
            writeln!(transcript_file, "{}", json)?;
            match event {
                OrchestrationEvent::CellSpawned { cell, parent, cell_type } => {
                    let topology = CellTopologyEntry {
                        timestamp: now,
                        cell: cell.to_string(),
                        parent: parent.map(|id| id.to_string()),
                        cell_type,
                    };
                    let json = serde_json::to_string(&topology)
                        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
                    writeln!(topology_file, "{}", json)?;
                }
                OrchestrationEvent::ModelSwitch { from, to, reason } => {
                    let model_entry = ModelEventEntry {
                        timestamp: now,
                        from,
                        to,
                        reason,
                    };
                    let json = serde_json::to_string(&model_entry)
                        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
                    writeln!(model_file, "{}", json)?;
                }
                OrchestrationEvent::ObjectivePlanReady { id, steps } => {
                    let plan_entry = PlanEntry {
                        timestamp: now,
                        objective_id: id,
                        steps,
                        active_index: 0,
                    };
                    let json = serde_json::to_string(&plan_entry)
                        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
                    writeln!(plan_file, "{}", json)?;
                }
                OrchestrationEvent::ApprovalRequested { cell, action, risk } => {
                    let approval_entry = ApprovalLogEntry {
                        timestamp: now,
                        kind: "requested",
                        request_id: approval_request_id(cell, &action, risk),
                        cell: Some(cell),
                        action: Some(action),
                        risk: Some(risk),
                        reason: None,
                    };
                    let json = serde_json::to_string(&approval_entry)
                        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
                    writeln!(approvals_file, "{}", json)?;
                }
                OrchestrationEvent::ApprovalGranted { request_id } => {
                    let approval_entry = ApprovalLogEntry {
                        timestamp: now,
                        kind: "granted",
                        request_id,
                        cell: None,
                        action: None,
                        risk: None,
                        reason: None,
                    };
                    let json = serde_json::to_string(&approval_entry)
                        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
                    writeln!(approvals_file, "{}", json)?;
                }
                OrchestrationEvent::ApprovalDenied { request_id, reason } => {
                    let approval_entry = ApprovalLogEntry {
                        timestamp: now,
                        kind: "denied",
                        request_id,
                        cell: None,
                        action: None,
                        risk: None,
                        reason: Some(reason),
                    };
                    let json = serde_json::to_string(&approval_entry)
                        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
                    writeln!(approvals_file, "{}", json)?;
                }
                _ => {}
            }
        }
        file.flush()?;
        transcript_file.flush()?;
        plan_file.flush()?;
        topology_file.flush()?;
        model_file.flush()?;
        approvals_file.flush()?;
        Ok(())
    }

    /// Persist runtime state to session_runtime.json
    pub fn flush_runtime_state(&self) -> std::io::Result<()> {
        let Some(layout) = &self.session_layout else {
            return Ok(()); // No layout configured, skip persistence
        };

        // Capture current runtime state
        let writeback = RuntimeWriteback::capture_from(&self.services);
        let runtime = writeback.to_session_runtime(layout.id());

        let path = layout.session_runtime_json();
        let json = serde_json::to_string_pretty(&runtime)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(&path, json)?;

        Ok(())
    }

    pub fn flush_pending_work(&self) -> std::io::Result<()> {
        let Some(layout) = &self.session_layout else {
            return Ok(());
        };

        let pending = self.autonomy_scheduler.get_pending_work();
        let lines = pending
            .into_iter()
            .map(|work| {
                let entry = PendingWorkLogEntry {
                    timestamp: chrono::Utc::now(),
                    id: work.id,
                    objective_id: work.objective_id,
                    action: work.action,
                    retry_count: work.retry_count,
                    max_retries: work.max_retries,
                    last_error: work.last_error,
                    retry_class: match work.retry_class {
                        RetryClass::Transient => "transient",
                        RetryClass::RateLimit => "rate-limit",
                        RetryClass::Configuration => "configuration",
                        RetryClass::Permanent => "permanent",
                    },
                };
                serde_json::to_string(&entry)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
            })
            .collect::<Result<Vec<_>, _>>()?;

        let content = if lines.is_empty() {
            String::new()
        } else {
            format!("{}\n", lines.join("\n"))
        };
        std::fs::write(layout.pending_work_jsonl(), content)?;
        Ok(())
    }

    /// Submit operator input to the orchestrator
    pub fn submit(&self, input: OperatorInput) -> Result<(), RuntimeError> {
        let session_id = self
            .session_layout
            .as_ref()
            .map(|layout| layout.id())
            .unwrap_or_default();

        self.session_queue.enqueue(session_id);

        {
            let mut queue = self.submission_queue.write().unwrap();
            queue.enqueue(session_id, input);
        }
        Ok(())
    }

    /// Process one turn - main runtime loop step
    pub fn process_turn(&self, ctx: &ExecutionContext) -> Result<TurnResult, RuntimeError> {
        let submitted = {
            let mut queue = self.submission_queue.write().unwrap();
            queue.dequeue()
        };

        let Some(submitted) = submitted else {
            return Ok(TurnResult::WaitingForInput);
        };

        let result = {
            let mut orch = self.orchestrator.write().unwrap();
            self.handle_operator_input(&mut orch, submitted.input)?;
            let objective_id = self
                .blackboard
                .get_objective()
                .map(|objective| objective.id)
                .unwrap_or(ctx.objective_id);

            orch.emit_event(OrchestrationEvent::TurnSubmitted {
                objective_id,
            });
            Ok::<(), RuntimeError>(())
        };

        self.session_queue.dequeue(submitted.session_id);
        result?;

        if let Err(e) = self.flush_events() {
            eprintln!("Warning: failed to persist events: {}", e);
        }

        // Persist runtime state
        if let Err(e) = self.flush_runtime_state() {
            eprintln!("Warning: failed to persist runtime state: {}", e);
        }

        if let Err(e) = self.flush_pending_work() {
            eprintln!("Warning: failed to persist pending work: {}", e);
        }

        Ok(TurnResult::Processed)
    }

    fn handle_operator_input(&self, orch: &mut BrainCellOrchestrator, input: OperatorInput) -> Result<(), RuntimeError> {
        match input {
            OperatorInput::Execute { intent } => {
                let harness_kind = orch.registry.resolve(&intent);
                let objective = Objective::new(intent.clone(), RiskProfile::Moderate);
                let objective_id = objective.id;
                self.blackboard.set_objective(objective);
                self.seed_core_cells(orch);
                self.plan_offensive_workflow(orch, objective_id, harness_kind, &intent);
                if matches!(
                    self.coordinate_execution(orch, objective_id),
                    ExecutionDisposition::Completed
                ) {
                    self.review_objective(orch, objective_id);
                }
            }
            OperatorInput::Interrupt => {
                // Request cancellation on all active cells
                for cell_id in self.blackboard.get_active_cells() {
                    self.blackboard.update_cell_status(cell_id, CellStatus::Cancelled);
                }
            }
            OperatorInput::SwitchModel { model } => {
                let model_ref = ModelRef::parse(&model)
                    .map_err(|e| RuntimeError::InvalidModel(e.to_string()))?;
                self.services.model_manager.set_active(model_ref.clone());
                orch.emit_event(OrchestrationEvent::ModelSwitch {
                    from: "unknown".to_string(),
                    to: model,
                    reason: "manual".to_string(),
                });
            }
            OperatorInput::Approve { request_id } => {
                self.blackboard.resolve_approval(request_id, true);
                orch.emit_event(OrchestrationEvent::ApprovalGranted { request_id });
            }
            OperatorInput::Deny { request_id, reason } => {
                self.blackboard.resolve_approval(request_id, false);
                orch.emit_event(OrchestrationEvent::ApprovalDenied { request_id, reason });
            }
            OperatorInput::Resume { objective_id: _ } => {
                // Would restore from checkpoint
            }
        }
        Ok(())
    }

    fn seed_core_cells(&self, orch: &mut BrainCellOrchestrator) {
        let cells = [
            PlannerCell::new(None).identity,
            CoordinatorCell::new(None).identity,
            ExecutorCell::new(None).identity,
            ReviewerCell::new(None).identity,
        ];

        for cell in cells {
            self.blackboard.add_cell(cell.clone());
            let cell_type = match cell.cell_type {
                CellType::Planner => "planner",
                CellType::Coordinator => "coordinator",
                CellType::Executor => "executor",
                CellType::Reviewer => "reviewer",
                CellType::Specialist(name) => name,
            };
            orch.emit_event(OrchestrationEvent::CellSpawned {
                cell: cell.id,
                parent: cell.parent,
                cell_type,
            });
        }
    }

    fn plan_offensive_workflow(
        &self,
        orch: &mut BrainCellOrchestrator,
        objective_id: ObjectiveId,
        harness_kind: HarnessKind,
        intent: &TaskIntent,
    ) {
        let plan_steps = offensive_plan_steps(harness_kind, intent);
        let active_subgoal = plan_steps.first().cloned();
        let mut subgoals = Vec::new();

        for (index, step) in plan_steps.iter().enumerate() {
            let mut subgoal = bco_core::Subgoal::new(step.clone(), None);
            if index == 0 {
                subgoal.state = bco_core::ObjectiveState::Active;
                subgoal.progress = ProgressStatus::InProgress;
            }
            subgoals.push(subgoal);
            self.blackboard.push_next_action(
                bco_core::NextAction::new(step.clone()).with_cell("coordinator")
            );
        }

        self.blackboard.update_objective(|objective| {
            objective.state = bco_core::ObjectiveState::Active;
            objective.progress = ProgressStatus::InProgress;
            objective.subgoals = subgoals;
            objective.next_action = active_subgoal
                .clone()
                .map(|step| bco_core::NextAction::new(step).with_cell("coordinator"));
        });

        if let Some(planner) = self.find_cell(CellType::Planner) {
            self.blackboard.update_cell_status(planner, CellStatus::Planning);
            self.blackboard.update_cell_status(planner, CellStatus::Completed);
        }

        orch.emit_event(OrchestrationEvent::ObjectiveCreated { id: objective_id });
        orch.emit_event(OrchestrationEvent::ObjectivePlanReady {
            id: objective_id,
            steps: plan_steps,
        });
        orch.emit_event(OrchestrationEvent::ObjectiveProgress {
            id: objective_id,
            status: ProgressStatus::InProgress,
        });
    }

    fn coordinate_execution(
        &self,
        orch: &mut BrainCellOrchestrator,
        objective_id: ObjectiveId,
    ) -> ExecutionDisposition {
        let next_action = self.blackboard.next_actions().into_iter().next();

        if let Some(coordinator) = self.find_cell(CellType::Coordinator) {
            self.blackboard.update_cell_status(coordinator, CellStatus::Coordinating);
        }

        if let Some(executor) = self.find_cell(CellType::Executor) {
            self.blackboard.update_cell_status(executor, CellStatus::Executing);
        }

        if let Some(action) = next_action {
            let objective_summary = self
                .blackboard
                .get_objective()
                .map(|objective| objective.intent.objective().to_string());
            let risk = classify_action_risk(&action.description, objective_summary.as_deref());
            match self.services.policy_evaluator.evaluate(risk) {
                PolicyDecision::Denied(reason) => {
                    if let Some(coordinator) = self.find_cell(CellType::Coordinator) {
                        self.blackboard.update_cell_status(coordinator, CellStatus::Failed);
                    }
                    if let Some(executor) = self.find_cell(CellType::Executor) {
                        self.blackboard.update_cell_status(executor, CellStatus::Failed);
                    }
                    orch.emit_event(OrchestrationEvent::ObjectiveFailed {
                        id: objective_id,
                        error: reason,
                    });
                    return ExecutionDisposition::Failed;
                }
                PolicyDecision::RequiresApproval => {
                    if let (Some(coordinator), Some(executor)) =
                        (self.find_cell(CellType::Coordinator), self.find_cell(CellType::Executor))
                    {
                        let request = ApprovalRequest {
                            id: approval_request_id(executor, &action.description, risk),
                            cell_id: executor,
                            action: action.description.clone(),
                            risk,
                            requested_at: chrono::Utc::now(),
                        };
                        self.blackboard.add_approval_request(request.clone());
                        self.blackboard.update_cell_status(coordinator, CellStatus::WaitingApproval);
                        self.blackboard.update_cell_status(executor, CellStatus::WaitingApproval);
                        orch.emit_event(OrchestrationEvent::ApprovalRequested {
                            cell: request.cell_id,
                            action: request.action,
                            risk: request.risk,
                        });
                    }
                    return ExecutionDisposition::WaitingApproval;
                }
                PolicyDecision::Approved => {}
            }
            if let (Some(coordinator), Some(executor)) =
                (self.find_cell(CellType::Coordinator), self.find_cell(CellType::Executor))
            {
                self.message_bus.send(InterCellMessage::new(
                    coordinator,
                    executor,
                    CellMessageContent::Request {
                        action: action.description.clone(),
                        payload: objective_id.to_string(),
                    },
                    DeliveryMode::TriggerNow,
                ));
                orch.emit_event(OrchestrationEvent::InteractionBegin {
                    from: coordinator,
                    to: executor,
                });
                orch.emit_event(OrchestrationEvent::InteractionEnd {
                    from: coordinator,
                    to: executor,
                });
            }
        }

        if let Some(coordinator) = self.find_cell(CellType::Coordinator) {
            self.blackboard.update_cell_status(coordinator, CellStatus::Completed);
        }
        ExecutionDisposition::Completed
    }

    fn review_objective(&self, orch: &mut BrainCellOrchestrator, objective_id: ObjectiveId) {
        if let Some(reviewer) = self.find_cell(CellType::Reviewer) {
            self.blackboard.update_cell_status(reviewer, CellStatus::Reviewing);
        }

        let next_action = self
            .blackboard
            .next_actions()
            .into_iter()
            .next()
            .map(|action| action.with_cell("executor"));
        self.blackboard.update_objective(|objective| {
            objective.next_action = next_action;
        });

        if let Some(executor) = self.find_cell(CellType::Executor) {
            self.blackboard.update_cell_status(executor, CellStatus::Completed);
        }
        if let Some(reviewer) = self.find_cell(CellType::Reviewer) {
            self.blackboard.update_cell_status(reviewer, CellStatus::Completed);
        }

        orch.emit_event(OrchestrationEvent::TurnCompleted { objective_id });
    }

    fn find_cell(&self, cell_type: CellType) -> Option<CellId> {
        self.blackboard
            .cell_states()
            .into_iter()
            .find(|state| state.identity.cell_type == cell_type)
            .map(|state| state.identity.id)
    }

    /// Get the blackboard for UI display
    pub fn blackboard(&self) -> &Blackboard {
        &self.blackboard
    }

    /// Get the message bus
    pub fn message_bus(&self) -> &MessageBus {
        &self.message_bus
    }

    /// Handle a model failure and potentially switch to fallback
    /// Returns the retry delay in milliseconds if retry is recommended
    pub fn handle_model_failure(&self, error: &str) -> Option<u64> {
        let active_model = self.services.model_manager.get_active()?;
        let model_ref = active_model.current.clone();

        let recommendation = self.services.model_manager.handle_model_failure(&model_ref, error);

        // Emit model switch event if we should switch
        if recommendation.should_switch {
            let mut orch = self.orchestrator.write().unwrap();
            orch.emit_event(OrchestrationEvent::ModelSwitch {
                from: model_ref.to_string(),
                to: recommendation.fallback_model
                    .as_ref()
                    .map(|m| m.to_string())
                    .unwrap_or_else(|| "fallback".to_string()),
                reason: format!("{:?}", recommendation.reason),
            });
        }

        // If there's a fallback model, switch to it
        if let Some(fallback) = recommendation.fallback_model {
            self.services.model_manager.switch_to_fallback(fallback);
        }

        // Return retry delay if we should wait before retry
        if recommendation.retry_delay_ms > 0 {
            return Some(recommendation.retry_delay_ms);
        }

        None
    }

    /// Get event queue for UI consumption
    pub fn drain_events(&self) -> Vec<OrchestrationEvent> {
        let mut orch = self.orchestrator.write().unwrap();
        orch.event_queue.drain()
    }

    pub fn schedule_pending_work(&self, work: PendingWork) {
        self.autonomy_scheduler.add_pending_work(work);
    }

    pub fn build_tui_state(&self, objective: &str) -> bco_tui::TuiState {
        let mut state = bco_tui::TuiState::with_objective(objective);

        if let Some(objective) = self.blackboard.get_objective() {
            if let Some(active_subgoal) = objective.active_subgoal() {
                state.status.subgoal = Some(active_subgoal.description.clone());
            }
            state.current_plan = objective
                .subgoals
                .iter()
                .map(|subgoal| {
                    format!(
                        "[{}] {}",
                        match subgoal.state {
                            bco_core::ObjectiveState::Active => "active",
                            bco_core::ObjectiveState::Completed => "done",
                            bco_core::ObjectiveState::Blocked => "blocked",
                            bco_core::ObjectiveState::Failed => "failed",
                            _ => "pending",
                        },
                        subgoal.description
                    )
                })
                .collect();
        }

        state.active_cells = self
            .blackboard
            .cell_states()
            .into_iter()
            .map(|cell| bco_tui::CellDisplay {
                name: match cell.identity.cell_type {
                    CellType::Planner => "planner".to_string(),
                    CellType::Coordinator => "coordinator".to_string(),
                    CellType::Executor => "executor".to_string(),
                    CellType::Reviewer => "reviewer".to_string(),
                    CellType::Specialist(name) => name.to_string(),
                },
                status: match cell.status {
                    CellStatus::Idle => "idle".to_string(),
                    CellStatus::Planning => "planning".to_string(),
                    CellStatus::Coordinating => "coordinating".to_string(),
                    CellStatus::Executing => "executing".to_string(),
                    CellStatus::Reviewing => "reviewing".to_string(),
                    CellStatus::WaitingApproval => "waiting".to_string(),
                    CellStatus::Completed => "completed".to_string(),
                    CellStatus::Failed => "failed".to_string(),
                    CellStatus::Cancelled => "cancelled".to_string(),
                },
            })
            .collect();

        state.pending_approvals = self
            .blackboard
            .get_pending_approvals()
            .into_iter()
            .map(|approval| bco_tui::ApprovalDisplay {
                risk: approval.risk.as_str().to_string(),
                action: approval.action,
                requested_at: approval.requested_at.format("%H:%M:%S").to_string(),
            })
            .collect();
        state.status.approval_state = match state.pending_approvals.len() {
            0 => bco_tui::ApprovalState::None,
            count => bco_tui::ApprovalState::Pending(count as u32),
        };

        state.transcript.extend(self.render_transcript_events());
        state
    }

    fn render_transcript_events(&self) -> Vec<String> {
        self.orchestrator
            .read()
            .unwrap()
            .event_queue
            .events
            .iter()
            .map(event_to_transcript_line)
            .collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExecutionDisposition {
    Completed,
    WaitingApproval,
    Failed,
}

fn offensive_plan_steps(harness_kind: HarnessKind, intent: &TaskIntent) -> Vec<String> {
    match harness_kind {
        HarnessKind::Ctf => vec![
            "Classify the challenge and constraints".to_string(),
            "Run targeted recon and artifact discovery".to_string(),
            "Develop the shortest viable solve path".to_string(),
            "Capture the flag and preserve evidence".to_string(),
        ],
        HarnessKind::Pentest => vec![
            "Confirm target scope, assumptions, and guardrails".to_string(),
            "Enumerate attack surface and high-signal findings".to_string(),
            "Chain access, escalation, or objective paths".to_string(),
            "Document evidence, impact, and next operator actions".to_string(),
        ],
        HarnessKind::Coding => vec![
            "Inspect the codebase and isolate the objective".to_string(),
            "Design the smallest safe change set".to_string(),
            "Implement and verify the change".to_string(),
            "Review regressions and summarize outcome".to_string(),
        ],
        HarnessKind::Generalist => {
            let objective = intent.objective().to_lowercase();
            if objective.contains("red team")
                || objective.contains("adversary")
                || objective.contains("exploit")
                || objective.contains("recon")
                || objective.contains("phishing")
            {
                vec![
                    "Frame the offensive objective and constraints".to_string(),
                    "Enumerate access paths, tooling, and signals".to_string(),
                    "Execute the highest-confidence path".to_string(),
                    "Review evidence, persistence, and follow-up actions".to_string(),
                ]
            } else {
                vec![
                    "Understand the task".to_string(),
                    "Plan the next steps".to_string(),
                    "Execute the highest-signal action".to_string(),
                    "Review and summarize".to_string(),
                ]
            }
        }
    }
}

fn event_to_transcript_line(event: &OrchestrationEvent) -> String {
    match event {
        OrchestrationEvent::ObjectiveCreated { id } => format!("[objective] created {}", id),
        OrchestrationEvent::ObjectivePlanReady { steps, .. } => {
            format!("[planner] {}", steps.join(" -> "))
        }
        OrchestrationEvent::ObjectiveProgress { status, .. } => {
            format!("[objective] progress {}", status.as_str())
        }
        OrchestrationEvent::CellSpawned { cell_type, .. } => format!("[cell] spawned {}", cell_type),
        OrchestrationEvent::InteractionBegin { .. } => "[coord] delegated work to executor".to_string(),
        OrchestrationEvent::TurnSubmitted { objective_id } => format!("[turn] submitted {}", objective_id),
        OrchestrationEvent::TurnCompleted { objective_id } => format!("[turn] completed {}", objective_id),
        OrchestrationEvent::ModelSwitch { to, .. } => format!("[model] switched to {}", to),
        OrchestrationEvent::ApprovalRequested { action, .. } => format!("[approval] requested {}", action),
        OrchestrationEvent::ApprovalGranted { .. } => "[approval] granted".to_string(),
        OrchestrationEvent::ApprovalDenied { reason, .. } => format!("[approval] denied {}", reason),
        OrchestrationEvent::ObjectiveCompleted { id } => format!("[objective] completed {}", id),
        OrchestrationEvent::ObjectiveFailed { error, .. } => format!("[objective] failed {}", error),
        OrchestrationEvent::CellCompleted { cell } => format!("[cell] completed {}", cell),
        OrchestrationEvent::CellFailed { error, .. } => format!("[cell] failed {}", error),
        OrchestrationEvent::CellCancelled { cell } => format!("[cell] cancelled {}", cell),
        OrchestrationEvent::CellInterrupted { cell } => format!("[cell] interrupted {}", cell),
        OrchestrationEvent::InteractionEnd { .. } => "[coord] executor returned".to_string(),
        OrchestrationEvent::TurnAborted { reason, .. } => format!("[turn] aborted {}", reason),
    }
}

fn classify_action_risk(action: &str, objective: Option<&str>) -> RiskProfile {
    let action = action.to_lowercase();
    let objective = objective.unwrap_or_default().to_lowercase();
    let combined = format!("{} {}", action, objective);
    if combined.contains("lateral movement")
        || combined.contains("initial access")
        || combined.contains("exploit")
        || combined.contains("persistence")
        || combined.contains("exfiltration")
    {
        RiskProfile::Critical
    } else if combined.contains("enumerate")
        || combined.contains("attack surface")
        || combined.contains("vulnerability")
        || combined.contains("recon")
    {
        RiskProfile::High
    } else {
        RiskProfile::Moderate
    }
}

fn approval_request_id(cell: CellId, action: &str, risk: RiskProfile) -> Uuid {
    let mut bytes = [0u8; 16];
    let hash = simple_hash(&format!("{}:{}:{:?}", cell.0, action, risk));
    bytes.copy_from_slice(&hash[..16]);
    Uuid::from_bytes(bytes)
}

#[derive(Debug)]
pub enum RuntimeError {
    SessionBusy { session_id: bco_session::SessionId },
    InvalidModel(String),
    CellNotFound { cell_id: CellId },
}

impl std::fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SessionBusy { session_id } => write!(f, "session {} is busy", session_id),
            Self::InvalidModel(msg) => write!(f, "invalid model: {}", msg),
            Self::CellNotFound { cell_id } => write!(f, "cell not found: {}", cell_id),
        }
    }
}

impl std::error::Error for RuntimeError {}

#[derive(Debug)]
pub enum TurnResult {
    Processed,
    WaitingForInput,
    ObjectiveComplete,
}
