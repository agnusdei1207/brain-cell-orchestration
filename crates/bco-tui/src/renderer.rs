//! TUI Renderer - Terminal UI rendering with ratatui

use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Terminal,
};
use std::io::Stdout;

/// TUI Renderer - handles all terminal UI rendering
pub struct TuiRenderer {
    terminal: Terminal<CrosstermBackend<Stdout>>,
}

impl TuiRenderer {
    /// Create a new TUI renderer
    pub fn new() -> Result<Self, std::io::Error> {
        let stdout = std::io::stdout();
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;
        Ok(Self { terminal })
    }

    /// Render the main layout
    pub fn render(&mut self, state: &TuiState) -> Result<(), std::io::Error> {
        self.terminal.draw(|f| {
            let chunks = compute_layout(f.size(), &state.layout);

            // Render status bar
            render_status_bar(f, chunks[0], &state.status);

            // Render transcript
            render_transcript(f, chunks[1], &state.transcript);

            // Render current plan
            render_current_plan(f, chunks[2], &state.current_plan);

            // Render active cells
            render_active_cells(f, chunks[3], &state.active_cells);

            // Render pending approvals
            render_pending_approvals(f, chunks[4], &state.pending_approvals);

            // Render composer
            render_composer(f, chunks[5], &state.composer);

            // Render footer
            render_footer(f, chunks[6], state.footer_hint);

            // Render overlay if active
            if let Some(overlay) = &state.overlay {
                render_overlay(f, overlay);
            }
        })?;
        Ok(())
    }

    /// Hide the cursor
    pub fn hide_cursor(&mut self) {
        crossterm::execute!(std::io::stdout(), crossterm::cursor::Hide).ok();
    }

    /// Show the cursor
    pub fn show_cursor(&mut self) {
        crossterm::execute!(std::io::stdout(), crossterm::cursor::Show).ok();
    }

    /// Clear the screen
    pub fn clear(&mut self) {
        self.terminal.clear().ok();
    }
}

fn compute_layout(area: Rect, layout: &super::TuiLayout) -> Vec<Rect> {
    let heights = layout.calculate_heights(area.height);

    Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(heights.status_bar),
            Constraint::Percentage(60),
            Constraint::Length(heights.current_plan),
            Constraint::Length(heights.active_cells),
            Constraint::Length(heights.pending_approvals),
            Constraint::Length(heights.composer),
            Constraint::Length(heights.footer),
        ])
        .split(area).to_vec()
}

fn render_status_bar(f: &mut ratatui::Frame, area: Rect, status: &super::StatusInfo) {
    let status_text = status.render_status_line();
    let style = Style::default()
        .bg(Color::Blue)
        .fg(Color::White)
        .add_modifier(Modifier::BOLD);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Status ");

    let paragraph = Paragraph::new(status_text)
        .style(style)
        .block(block);

    f.render_widget(paragraph, area);
}

fn render_transcript(f: &mut ratatui::Frame, area: Rect, transcript: &[String]) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Transcript ")
        .style(Style::default().bg(Color::Black).fg(Color::Gray));

    let items: Vec<ListItem> = transcript
        .iter()
        .map(|line| {
            let style = if line.starts_with('[') {
                Style::default().fg(Color::Yellow)
            } else if line.starts_with('>') {
                Style::default().fg(Color::Green)
            } else {
                Style::default().fg(Color::White)
            };
            ListItem::new(Line::from(Span::styled(line, style)))
        })
        .collect();

    let list = List::new(items).block(block);
    f.render_widget(list, area);
}

fn render_current_plan(f: &mut ratatui::Frame, area: Rect, current_plan: &[String]) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Current Plan ");

    let text = if current_plan.is_empty() {
        vec![Line::from(Span::styled("(no active plan)", Style::default().fg(Color::DarkGray)))]
    } else {
        current_plan
            .iter()
            .map(|line| Line::from(Span::raw(line)))
            .collect()
    };

    let paragraph = Paragraph::new(text)
        .block(block)
        .wrap(ratatui::widgets::Wrap { trim: true });
    f.render_widget(paragraph, area);
}

fn render_active_cells(f: &mut ratatui::Frame, area: Rect, active_cells: &[CellDisplay]) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Active Cells ");

    let items: Vec<ListItem> = active_cells
        .iter()
        .map(|cell| {
            let style = match cell.status.as_str() {
                "executing" => Style::default().fg(Color::Green),
                "waiting" => Style::default().fg(Color::Yellow),
                "failed" => Style::default().fg(Color::Red),
                _ => Style::default().fg(Color::White),
            };
            ListItem::new(Line::from(Span::styled(
                format!("{} ({})", cell.name, cell.status),
                style,
            )))
        })
        .collect();

    let list = List::new(items).block(block);
    f.render_widget(list, area);
}

fn render_pending_approvals(f: &mut ratatui::Frame, area: Rect, approvals: &[ApprovalDisplay]) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Pending Approvals ");

    let items: Vec<ListItem> = approvals
        .iter()
        .map(|approval| {
            ListItem::new(Line::from(Span::raw(format!(
                "[{}] {} - {}",
                approval.risk, approval.action, approval.requested_at
            ))))
        })
        .collect();

    let list = List::new(items).block(block);
    f.render_widget(list, area);
}

fn render_composer(f: &mut ratatui::Frame, area: Rect, composer: &str) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Composer ")
        .style(Style::default().bg(Color::Blue).fg(Color::White));

    let cursor_len = composer.len();
    let text = vec![
        Line::from(Span::raw("> ")),
        Line::from(Span::raw(composer.to_string())),
    ];

    let paragraph = Paragraph::new(text).block(block);
    f.render_widget(paragraph, area);

    // Calculate and set cursor position
    let cursor_x = 2 + cursor_len as u16;
    let cursor_y = area.y + 1;
    f.set_cursor(cursor_x, cursor_y);
}

fn render_footer(f: &mut ratatui::Frame, area: Rect, hint: &str) {
    let style = Style::default().bg(Color::DarkGray).fg(Color::White);

    let text = Line::from(Span::raw(hint));
    let paragraph = Paragraph::new(text)
        .style(style)
        .block(Block::default().borders(Borders::TOP));

    f.render_widget(paragraph, area);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_height = r.height * percent_y / 100;
    let popup_width = r.width * percent_x / 100;

    Rect {
        x: (r.width - popup_width) / 2,
        y: (r.height - popup_height) / 2,
        width: popup_width,
        height: popup_height,
    }
}

fn render_overlay(f: &mut ratatui::Frame, overlay: &OverlayState) {
    let area = centered_rect(60, 40, f.size());

    let block = Block::default()
        .borders(Borders::ALL)
        .title(overlay.title)
        .style(Style::default().bg(Color::Blue).fg(Color::White));

    let items: Vec<ListItem> = overlay
        .content
        .iter()
        .enumerate()
        .map(|(idx, line)| {
            let style = if idx == overlay.selected_index {
                Style::default()
                    .bg(Color::White)
                    .fg(Color::Blue)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            ListItem::new(Line::from(Span::styled(line, style)))
        })
        .collect();

    let list = List::new(items).block(block);
    f.render_widget(list, area);
}

/// TUI application state
#[derive(Debug, Clone)]
pub struct TuiState {
    pub layout: super::TuiLayout,
    pub status: super::StatusInfo,
    pub transcript: Vec<String>,
    pub current_plan: Vec<String>,
    pub active_cells: Vec<CellDisplay>,
    pub pending_approvals: Vec<ApprovalDisplay>,
    pub composer: String,
    pub footer_hint: &'static str,
    pub overlay: Option<OverlayState>,
}

#[derive(Debug, Clone)]
pub struct CellDisplay {
    pub name: String,
    pub status: String,
}

#[derive(Debug, Clone)]
pub struct ApprovalDisplay {
    pub risk: String,
    pub action: String,
    pub requested_at: String,
}

/// Overlay state for modal dialogs
#[derive(Debug, Clone)]
pub struct OverlayState {
    pub title: &'static str,
    pub content: Vec<String>,
    pub selected_index: usize,
}

impl Default for TuiState {
    fn default() -> Self {
        Self {
            layout: super::TuiLayout::new(),
            status: super::StatusInfo::new(),
            transcript: vec![
                "[system] Welcome to brain-cell-orchestration".to_string(),
                "[system] Session initialized".to_string(),
                "[system] Ready for input".to_string(),
            ],
            current_plan: vec![],
            active_cells: vec![
                CellDisplay { name: "planner".to_string(), status: "idle".to_string() },
                CellDisplay { name: "coordinator".to_string(), status: "idle".to_string() },
                CellDisplay { name: "executor".to_string(), status: "idle".to_string() },
                CellDisplay { name: "reviewer".to_string(), status: "idle".to_string() },
            ],
            pending_approvals: vec![],
            composer: String::new(),
            footer_hint: "Enter: send | /help: commands | Ctrl+C: interrupt",
            overlay: None,
        }
    }
}

impl TuiState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_objective(objective: &str) -> Self {
        let mut state = Self::default();
        state.status.objective = Some(objective.to_string());
        state.transcript.push(format!("> objective: {}", objective));
        state
    }
}
