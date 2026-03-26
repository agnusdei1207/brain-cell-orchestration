use bco_core::{IntentDomain, ModelRef, RiskProfile, TaskIntent};
use uuid::Uuid;

// =============================================================================
// A3: Harness Contract
// =============================================================================

/// Unique identifier for a harness
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HarnessId(pub Uuid);

impl HarnessId {
    pub fn new(name: &'static str) -> Self {
        // Create a deterministic UUID from the name for consistent IDs
        // Using v5 requires the "v5" feature which we don't have enabled
        // Instead, we'll use a simple hash approach
        let mut bytes = [0u8; 16];
        let hash = simple_hash(name);
        bytes.copy_from_slice(&hash[..16]);
        Self(uuid::Uuid::from_bytes(bytes))
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
    // Simple expansion
    for i in 16..32 {
        bytes[i] = bytes[i - 16] ^ bytes[i - 8];
    }
    bytes
}

impl std::fmt::Display for HarnessId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "harness-{}", self.0)
    }
}

/// Policy for how the planner cell should behave
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanPolicy {
    /// Decompose into small, verifiable steps
    Incremental,
    /// Create broad overview first, then detail
    TopDown,
    /// Plan as we go, minimal upfront planning
    Opportunistic,
    /// Fixed sequence of phases
    Sequential,
}

/// Policy for how the reviewer cell should behave
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewPolicy {
    /// Review after each action
    Continuous,
    /// Review after milestone completion
    Milestone,
    /// Review only on explicit request
    OnDemand,
    /// Automatic review with human override
    Advisory,
}

/// Policy for what capabilities this harness supports
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityPolicy {
    pub can_read: bool,
    pub can_write: bool,
    pub can_execute: bool,
    pub can_network: bool,
    pub max_risk_profile: RiskProfile,
    pub requires_approval_above: RiskProfile,
}

impl Default for CapabilityPolicy {
    fn default() -> Self {
        Self {
            can_read: true,
            can_write: true,
            can_execute: false,
            can_network: false,
            max_risk_profile: RiskProfile::Moderate,
            requires_approval_above: RiskProfile::High,
        }
    }
}

/// Model preference hints from harness
#[derive(Debug, Clone)]
pub struct ModelPreference {
    pub preferred: Vec<ModelRef>,
    pub acceptable: Vec<ModelRef>,
    pub forbidden: Vec<ModelRef>,
}

impl ModelPreference {
    pub fn any() -> Self {
        Self {
            preferred: Vec::new(),
            acceptable: Vec::new(),
            forbidden: Vec::new(),
        }
    }

    pub fn prefer(&mut self, model: ModelRef) {
        self.preferred.push(model);
    }

    pub fn allow(&mut self, model: ModelRef) {
        self.acceptable.push(model);
    }

    pub fn forbid(&mut self, model: ModelRef) {
        self.forbidden.push(model);
    }
}

/// Core harness trait - all domain harnesses must implement this
pub trait Harness: Send + Sync {
    /// Unique identifier for this harness
    fn id(&self) -> HarnessId;

    /// Human-readable name
    fn name(&self) -> &'static str;

    /// Check if this harness accepts the given intent
    fn accepts(&self, intent: &TaskIntent) -> bool;

    /// Get planning policy
    fn plan_policy(&self) -> PlanPolicy;

    /// Get review policy
    fn review_policy(&self) -> ReviewPolicy;

    /// Get capability policy
    fn capability_policy(&self) -> CapabilityPolicy;

    /// Get model preferences
    fn model_preference(&self) -> ModelPreference;
}

// =============================================================================
// Built-in Harness Implementations
// =============================================================================

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

    pub fn id(&self) -> HarnessId {
        match self {
            Self::Ctf => HarnessId::new("ctf"),
            Self::Pentest => HarnessId::new("pentest"),
            Self::Coding => HarnessId::new("coding"),
            Self::Generalist => HarnessId::new("generalist"),
        }
    }
}

/// CTF Harness - optimized for capture the flag competitions
pub struct CtfHarness;

impl CtfHarness {
    pub fn new() -> Self {
        Self
    }
}

impl Default for CtfHarness {
    fn default() -> Self {
        Self::new()
    }
}

impl Harness for CtfHarness {
    fn id(&self) -> HarnessId {
        HarnessKind::Ctf.id()
    }

    fn name(&self) -> &'static str {
        "CTF Harness"
    }

    fn accepts(&self, intent: &TaskIntent) -> bool {
        intent.domain() == IntentDomain::Ctf
    }

    fn plan_policy(&self) -> PlanPolicy {
        PlanPolicy::Incremental
    }

    fn review_policy(&self) -> ReviewPolicy {
        ReviewPolicy::Milestone
    }

    fn capability_policy(&self) -> CapabilityPolicy {
        CapabilityPolicy {
            can_read: true,
            can_write: true,
            can_execute: true,
            can_network: true,
            max_risk_profile: RiskProfile::High,
            requires_approval_above: RiskProfile::Critical,
        }
    }

    fn model_preference(&self) -> ModelPreference {
        ModelPreference::any()
    }
}

/// Pentest Harness - optimized for penetration testing
pub struct PentestHarness;

impl PentestHarness {
    pub fn new() -> Self {
        Self
    }
}

impl Default for PentestHarness {
    fn default() -> Self {
        Self::new()
    }
}

impl Harness for PentestHarness {
    fn id(&self) -> HarnessId {
        HarnessKind::Pentest.id()
    }

    fn name(&self) -> &'static str {
        "Pentest Harness"
    }

    fn accepts(&self, intent: &TaskIntent) -> bool {
        intent.domain() == IntentDomain::Pentesting
    }

    fn plan_policy(&self) -> PlanPolicy {
        PlanPolicy::Sequential
    }

    fn review_policy(&self) -> ReviewPolicy {
        ReviewPolicy::Advisory
    }

    fn capability_policy(&self) -> CapabilityPolicy {
        CapabilityPolicy {
            can_read: true,
            can_write: true,
            can_execute: true,
            can_network: true,
            max_risk_profile: RiskProfile::Critical,
            requires_approval_above: RiskProfile::High,
        }
    }

    fn model_preference(&self) -> ModelPreference {
        ModelPreference::any()
    }
}

/// Coding Harness - optimized for software development
pub struct CodingHarness;

impl CodingHarness {
    pub fn new() -> Self {
        Self
    }
}

impl Default for CodingHarness {
    fn default() -> Self {
        Self::new()
    }
}

impl Harness for CodingHarness {
    fn id(&self) -> HarnessId {
        HarnessKind::Coding.id()
    }

    fn name(&self) -> &'static str {
        "Coding Harness"
    }

    fn accepts(&self, intent: &TaskIntent) -> bool {
        intent.domain() == IntentDomain::Coding
    }

    fn plan_policy(&self) -> PlanPolicy {
        PlanPolicy::TopDown
    }

    fn review_policy(&self) -> ReviewPolicy {
        ReviewPolicy::Continuous
    }

    fn capability_policy(&self) -> CapabilityPolicy {
        CapabilityPolicy::default()
    }

    fn model_preference(&self) -> ModelPreference {
        ModelPreference::any()
    }
}

/// Generalist Harness - fallback for general engineering tasks
pub struct GeneralistHarness;

impl GeneralistHarness {
    pub fn new() -> Self {
        Self
    }
}

impl Default for GeneralistHarness {
    fn default() -> Self {
        Self::new()
    }
}

impl Harness for GeneralistHarness {
    fn id(&self) -> HarnessId {
        HarnessKind::Generalist.id()
    }

    fn name(&self) -> &'static str {
        "Generalist Harness"
    }

    fn accepts(&self, intent: &TaskIntent) -> bool {
        intent.domain() == IntentDomain::GeneralEngineering
    }

    fn plan_policy(&self) -> PlanPolicy {
        PlanPolicy::Opportunistic
    }

    fn review_policy(&self) -> ReviewPolicy {
        ReviewPolicy::OnDemand
    }

    fn capability_policy(&self) -> CapabilityPolicy {
        CapabilityPolicy::default()
    }

    fn model_preference(&self) -> ModelPreference {
        ModelPreference::any()
    }
}

// =============================================================================
// Harness Registry
// =============================================================================

#[derive(Default)]
pub struct HarnessRegistry {
    harnesses: Vec<Box<dyn Harness>>,
}

impl std::fmt::Debug for HarnessRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HarnessRegistry")
            .field("harnesses", &self.harnesses.len())
            .finish()
    }
}

impl HarnessRegistry {
    pub fn with_defaults() -> Self {
        let mut registry = Self::new();
        registry.register(Box::new(CtfHarness::new()));
        registry.register(Box::new(PentestHarness::new()));
        registry.register(Box::new(CodingHarness::new()));
        registry.register(Box::new(GeneralistHarness::new()));
        registry
    }

    pub fn new() -> Self {
        Self {
            harnesses: Vec::new(),
        }
    }

    pub fn register(&mut self, harness: Box<dyn Harness>) {
        self.harnesses.push(harness);
    }

    pub fn resolve(&self, intent: &TaskIntent) -> HarnessKind {
        // Try to find a matching harness
        for harness in &self.harnesses {
            if harness.accepts(intent) {
                return match harness.id().0 {
                    id if id == HarnessKind::Ctf.id().0 => HarnessKind::Ctf,
                    id if id == HarnessKind::Pentest.id().0 => HarnessKind::Pentest,
                    id if id == HarnessKind::Coding.id().0 => HarnessKind::Coding,
                    id if id == HarnessKind::Generalist.id().0 => HarnessKind::Generalist,
                    _ => HarnessKind::Generalist,
                };
            }
        }
        // Fallback to generalist
        HarnessKind::Generalist
    }

    pub fn get_harness(&self, kind: HarnessKind) -> Option<&dyn Harness> {
        for harness in &self.harnesses {
            if harness.id() == kind.id() {
                return Some(harness.as_ref());
            }
        }
        None
    }
}
