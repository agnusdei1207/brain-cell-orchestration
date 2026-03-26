use std::sync::RwLock;

// =============================================================================
// E1: Layout Shell
// =============================================================================

/// Layout regions for the TUI
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutRegion {
    Transcript,
    StatusBar,
    CurrentPlan,
    ActiveCells,
    PendingApprovals,
    Composer,
    Footer,
}

/// TUI layout shell
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TuiLayout {
    /// Minimum width for the terminal
    pub min_width: u16,
    /// Minimum height for the terminal
    pub min_height: u16,
    /// Transcript region height (percentage)
    pub transcript_height_pct: u8,
    /// Status bar height
    pub status_bar_height: u16,
    /// Composer height
    pub composer_height: u16,
    /// Footer height
    pub footer_height: u16,
}

impl TuiLayout {
    pub fn new() -> Self {
        Self {
            min_width: 80,
            min_height: 24,
            transcript_height_pct: 60,
            status_bar_height: 2,
            composer_height: 3,
            footer_height: 1,
        }
    }

    /// Calculate actual heights given terminal size
    pub fn calculate_heights(&self, terminal_height: u16) -> LayoutHeights {
        let transcript_height = (terminal_height as u32 * self.transcript_height_pct as u32 / 100) as u16;
        let remaining = terminal_height.saturating_sub(
            self.status_bar_height + self.composer_height + self.footer_height
        );

        LayoutHeights {
            transcript: transcript_height.min(remaining.saturating_sub(2)),
            status_bar: self.status_bar_height,
            current_plan: remaining / 3,
            active_cells: remaining / 3,
            pending_approvals: remaining / 3,
            composer: self.composer_height,
            footer: self.footer_height,
        }
    }
}

impl Default for TuiLayout {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayoutHeights {
    pub transcript: u16,
    pub status_bar: u16,
    pub current_plan: u16,
    pub active_cells: u16,
    pub pending_approvals: u16,
    pub composer: u16,
    pub footer: u16,
}

/// Terminal size check
pub fn check_terminal_size(width: u16, height: u16) -> Result<(), TuiError> {
    if width < 80 || height < 24 {
        return Err(TuiError::TerminalTooSmall { width, height });
    }
    Ok(())
}

#[derive(Debug)]
pub enum TuiError {
    TerminalTooSmall { width: u16, height: u16 },
    RenderError(String),
}

// =============================================================================
// E2: Operator Overlays
// =============================================================================

/// Overlay types for operator interaction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Overlay {
    Cells,
    Memory,
    Approvals,
    Resume,
    Model,
    Connect,
}

/// Overlay state
#[derive(Debug)]
pub struct OverlayState {
    pub active: Option<Overlay>,
    pub title: &'static str,
    pub content: Vec<String>,
    pub selected_index: usize,
}

impl OverlayState {
    pub fn new() -> Self {
        Self {
            active: None,
            title: "",
            content: Vec::new(),
            selected_index: 0,
        }
    }

    pub fn show(&mut self, overlay: Overlay) {
        self.active = Some(overlay);
        self.selected_index = 0;
    }

    pub fn hide(&mut self) {
        self.active = None;
    }

    pub fn is_visible(&self) -> bool {
        self.active.is_some()
    }

    pub fn move_up(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
        }
    }

    pub fn move_down(&mut self) {
        if self.selected_index < self.content.len().saturating_sub(1) {
            self.selected_index += 1;
        }
    }
}

impl Default for OverlayState {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// E3: Status Density
// =============================================================================

/// Status information to display in TUI
#[derive(Debug, Clone)]
pub struct StatusInfo {
    pub objective: Option<String>,
    pub subgoal: Option<String>,
    pub harness: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub connection_health: ConnectionHealth,
    pub approval_state: ApprovalState,
    pub resumed: bool,
    pub scheduled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionHealth {
    Connected,
    Disconnected,
    Error,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalState {
    None,
    Pending(u32),
    Granted,
    Denied,
}

impl StatusInfo {
    pub fn new() -> Self {
        Self {
            objective: None,
            subgoal: None,
            harness: None,
            provider: None,
            model: None,
            connection_health: ConnectionHealth::Unknown,
            approval_state: ApprovalState::None,
            resumed: false,
            scheduled: false,
        }
    }

    #[allow(dead_code)]
    pub fn from_objective(objective_summary: &str, subgoal_summary: Option<&str>) -> Self {
        Self {
            objective: Some(objective_summary.to_string()),
            subgoal: subgoal_summary.map(String::from),
            harness: None, // Would be set from context
            provider: None,
            model: None,
            connection_health: ConnectionHealth::Unknown,
            approval_state: ApprovalState::None,
            resumed: false,
            scheduled: false,
        }
    }

    /// Render status line for status bar
    pub fn render_status_line(&self) -> String {
        let mut parts = Vec::new();

        if let Some(ref obj) = self.objective {
            parts.push(format!("obj:{}", truncate(obj, 20)));
        }

        if let Some(ref sub) = self.subgoal {
            parts.push(format!("sub:{}", truncate(sub, 15)));
        }

        if let Some(ref h) = self.harness {
            parts.push(format!("h:{}", h));
        }

        if let Some(ref m) = self.model {
            parts.push(format!("m:{}", m));
        }

        let health_str = match self.connection_health {
            ConnectionHealth::Connected => "✓",
            ConnectionHealth::Disconnected => "○",
            ConnectionHealth::Error => "✗",
            ConnectionHealth::Unknown => "?",
        };
        parts.push(format!("conn:{}", health_str));

        let approval_str: String = match self.approval_state {
            ApprovalState::None => return parts.join(" │ "),
            ApprovalState::Pending(n) => format!("appr:{}", n),
            ApprovalState::Granted => "appr:✓".to_string(),
            ApprovalState::Denied => "appr:✗".to_string(),
        };
        parts.push(approval_str);

        if self.resumed {
            parts.push("resumed".to_string());
        }

        if self.scheduled {
            parts.push("sched".to_string());
        }

        parts.join(" │ ")
    }
}

impl Default for StatusInfo {
    fn default() -> Self {
        Self::new()
    }
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}

/// Composer state for multiline input
#[derive(Debug)]
pub struct Composer {
    pub lines: Vec<String>,
    pub cursor_line: usize,
    pub cursor_col: usize,
}

impl Composer {
    pub fn new() -> Self {
        Self {
            lines: vec![String::new()],
            cursor_line: 0,
            cursor_col: 0,
        }
    }

    pub fn push_char(&mut self, c: char) {
        if self.cursor_line >= self.lines.len() {
            self.lines.push(String::new());
        }
        self.lines[self.cursor_line].push(c);
        self.cursor_col += 1;
    }

    pub fn backspace(&mut self) {
        if self.cursor_col > 0 {
            if self.cursor_line < self.lines.len() {
                self.lines[self.cursor_line].pop();
                self.cursor_col -= 1;
            }
        } else if self.cursor_line > 0 {
            self.cursor_line -= 1;
            self.cursor_col = self.lines[self.cursor_line].len();
        }
    }

    pub fn newline(&mut self) {
        if self.cursor_line >= self.lines.len() {
            self.lines.push(String::new());
        }
        // Split current line at cursor
        let remaining: String = self.lines[self.cursor_line].chars().skip(self.cursor_col).collect();
        self.lines[self.cursor_line] = self.lines[self.cursor_line].chars().take(self.cursor_col).collect();
        self.cursor_line += 1;
        self.lines.insert(self.cursor_line, remaining);
        self.cursor_col = 0;
    }

    pub fn get_content(&self) -> String {
        self.lines.join("\n")
    }

    pub fn is_empty(&self) -> bool {
        self.lines.iter().all(|l| l.is_empty())
    }
}

impl Default for Composer {
    fn default() -> Self {
        Self::new()
    }
}

/// Multiline composer for terminal
pub struct MultilineComposer {
    composer: RwLock<Composer>,
}

impl MultilineComposer {
    pub fn new() -> Self {
        Self {
            composer: RwLock::new(Composer::new()),
        }
    }

    pub fn handle_key(&self, key: &str) {
        let mut composer = self.composer.write().unwrap();
        match key {
            "\n" | "\r" => composer.newline(),
            "\x7f" | "\x08" => composer.backspace(), // Backspace
            _ => {
                for c in key.chars() {
                    composer.push_char(c);
                }
            }
        }
    }

    pub fn get_content(&self) -> String {
        self.composer.read().unwrap().get_content()
    }

    pub fn is_empty(&self) -> bool {
        self.composer.read().unwrap().is_empty()
    }

    pub fn clear(&self) {
        *self.composer.write().unwrap() = Composer::new();
    }
}

impl Default for MultilineComposer {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// TuiBlueprint (existing type, expanded)
// =============================================================================

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TuiBlueprint {
    profile_name: &'static str,
    layout: TuiLayout,
    show_transcript: bool,
    show_status: bool,
    compact_mode: bool,
}

impl TuiBlueprint {
    pub fn claude_code_inspired() -> Self {
        Self {
            profile_name: "claude-code-inspired-dense-terminal",
            layout: TuiLayout::new(),
            show_transcript: true,
            show_status: true,
            compact_mode: false,
        }
    }

    pub fn profile_name(&self) -> &'static str {
        self.profile_name
    }

    pub fn layout(&self) -> &TuiLayout {
        &self.layout
    }

    pub fn show_transcript(&self) -> bool {
        self.show_transcript
    }

    pub fn show_status(&self) -> bool {
        self.show_status
    }

    pub fn compact_mode(&self) -> bool {
        self.compact_mode
    }
}

// =============================================================================
// Compact Footer
// =============================================================================

/// Footer information
#[derive(Debug, Clone)]
pub struct FooterInfo {
    pub mode: &'static str,
    pub shortcut_hint: &'static str,
}

impl FooterInfo {
    pub fn new(mode: &'static str) -> Self {
        let shortcut_hint = match mode {
            "exec" => "Enter: send │ /help: commands",
            "overlay" => "↑↓: navigate │ Enter: select │ Esc: close",
            _ => "Ctrl+C: interrupt",
        };
        Self { mode, shortcut_hint }
    }
}

// =============================================================================
// TUI Renderer Module
// =============================================================================

pub mod renderer;
pub use renderer::{TuiRenderer, TuiState, CellDisplay, ApprovalDisplay};

/// Run the TUI with given state
pub fn run_tui(state: TuiState) -> Result<(), std::io::Error> {
    let mut renderer = TuiRenderer::new()?;
    renderer.hide_cursor();
    renderer.render(&state)?;
    renderer.show_cursor();
    Ok(())
}
