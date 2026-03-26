use uuid::Uuid;
use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};
use std::path::PathBuf;

// =============================================================================
// B1: Session Layout
// =============================================================================

/// Session root directory name
const SESSION_DIR: &str = ".bco/sessions";

/// Unique session identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionId(pub Uuid);

impl SessionId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for SessionId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Session metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMeta {
    pub id: SessionId,
    pub created_at: DateTime<Utc>,
    pub profile: String,
    pub state: SessionState,
}

/// Session state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionState {
    Initializing,
    Active,
    Paused,
    Busy,
    Interrupted,
    Completed,
    Failed,
}

/// Session runtime metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRuntime {
    pub session_id: SessionId,
    pub active_model: Option<String>,
    pub token_usage: Option<TokenUsage>,
    pub abort_count: u32,
    pub compaction_count: u32,
    pub last_updated: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_hits: u64,
}

/// Session layout structure
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionLayout {
    root: PathBuf,
    session_id: SessionId,
}

impl SessionLayout {
    /// Create a new session layout at the given base path
    pub fn new(base_path: impl Into<PathBuf>, session_id: SessionId) -> Self {
        Self {
            root: base_path.into(),
            session_id,
        }
    }

    /// Get the session root directory path
    pub fn session_dir(&self) -> PathBuf {
        self.root.join(SESSION_DIR).join(self.session_id.to_string())
    }

    /// Session metadata file
    pub fn session_json(&self) -> PathBuf {
        self.session_dir().join("session.json")
    }

    /// Append-only transcript
    pub fn transcript_jsonl(&self) -> PathBuf {
        self.session_dir().join("transcript.jsonl")
    }

    /// Plan log
    pub fn plan_jsonl(&self) -> PathBuf {
        self.session_dir().join("plan.jsonl")
    }

    /// Approval log
    pub fn approvals_jsonl(&self) -> PathBuf {
        self.session_dir().join("approvals.jsonl")
    }

    /// Evidence log
    pub fn evidence_jsonl(&self) -> PathBuf {
        self.session_dir().join("evidence.jsonl")
    }

    /// Tool execution log
    pub fn tool_runs_jsonl(&self) -> PathBuf {
        self.session_dir().join("tool_runs.jsonl")
    }

    /// Orchestration events log
    pub fn orchestrator_events_jsonl(&self) -> PathBuf {
        self.session_dir().join("orchestrator_events.jsonl")
    }

    /// Cell topology log
    pub fn cell_topology_jsonl(&self) -> PathBuf {
        self.session_dir().join("cell_topology.jsonl")
    }

    /// Model events log
    pub fn model_events_jsonl(&self) -> PathBuf {
        self.session_dir().join("model_events.jsonl")
    }

    /// Session runtime state
    pub fn session_runtime_json(&self) -> PathBuf {
        self.session_dir().join("session_runtime.json")
    }

    /// Pending work log
    pub fn pending_work_jsonl(&self) -> PathBuf {
        self.session_dir().join("pending_work.jsonl")
    }

    /// Checkpoints directory
    pub fn checkpoints_dir(&self) -> PathBuf {
        self.session_dir().join("checkpoints")
    }

    /// Memory directory
    pub fn memory_dir(&self) -> PathBuf {
        self.session_dir().join("memory")
    }

    /// Create all directories in the session layout
    pub fn create_dirs(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(self.session_dir())?;
        std::fs::create_dir_all(self.checkpoints_dir())?;
        std::fs::create_dir_all(self.memory_dir())?;
        Ok(())
    }

    /// Get session ID
    pub fn id(&self) -> SessionId {
        self.session_id
    }
}

// =============================================================================
// Session Bootstrap (existing type, updated)
// =============================================================================

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionBootstrap {
    pub session_id: SessionId,
    pub profile: String,
    pub layout: SessionLayout,
}

impl SessionBootstrap {
    pub fn new(profile: impl Into<String>) -> Self {
        let session_id = SessionId::new();
        let layout = SessionLayout::new(std::env::current_dir().unwrap_or_default(), session_id);
        Self {
            session_id,
            profile: profile.into(),
            layout,
        }
    }

    pub fn with_id(session_id: SessionId, profile: impl Into<String>) -> Self {
        let layout = SessionLayout::new(std::env::current_dir().unwrap_or_default(), session_id);
        Self {
            session_id,
            profile: profile.into(),
            layout,
        }
    }

    pub fn profile(&self) -> &str {
        &self.profile
    }

    pub fn id(&self) -> SessionId {
        self.session_id
    }

    pub fn layout(&self) -> &SessionLayout {
        &self.layout
    }

    /// Bootstrap the session - create directories and initial files
    pub fn bootstrap(&self) -> std::io::Result<()> {
        self.layout.create_dirs()?;

        // Create initial session.json
        let meta = SessionMeta {
            id: self.session_id,
            created_at: Utc::now(),
            profile: self.profile.clone(),
            state: SessionState::Initializing,
        };
        let json = serde_json::to_string_pretty(&meta).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(self.layout.session_json(), json)?;

        // Create empty initial files
        std::fs::write(self.layout.transcript_jsonl(), "")?;
        std::fs::write(self.layout.plan_jsonl(), "")?;
        std::fs::write(self.layout.approvals_jsonl(), "")?;
        std::fs::write(self.layout.evidence_jsonl(), "")?;
        std::fs::write(self.layout.tool_runs_jsonl(), "")?;
        std::fs::write(self.layout.orchestrator_events_jsonl(), "")?;
        std::fs::write(self.layout.cell_topology_jsonl(), "")?;
        std::fs::write(self.layout.model_events_jsonl(), "")?;
        std::fs::write(self.layout.pending_work_jsonl(), "")?;

        // Create initial runtime
        let runtime = SessionRuntime {
            session_id: self.session_id,
            active_model: None,
            token_usage: None,
            abort_count: 0,
            compaction_count: 0,
            last_updated: Utc::now(),
        };
        let json = serde_json::to_string_pretty(&runtime).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(self.layout.session_runtime_json(), json)?;

        Ok(())
    }
}

// =============================================================================
// Append-only log writers
// =============================================================================

/// Append-only log entry
pub trait AppendLogEntry: Serialize {
    fn timestamp(&self) -> DateTime<Utc>;
}

/// Append to a JSONL file
pub fn append_jsonl<T: AppendLogEntry>(path: &PathBuf, entry: &T) -> std::io::Result<()> {
    let json = serde_json::to_string(entry).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    let line = format!("{}\n", json);
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?
        .write_all(line.as_bytes())?;
    Ok(())
}

use std::io::Write;
