use crate::engine;
use crate::models::TuxPayload;
use crossterm::event::{self, Event, KeyCode};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table, TableState, Wrap};
use ratatui::Terminal;
use std::error::Error;
use std::io;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

#[derive(Debug)]
enum UiEvent {
    Payload(Box<TuxPayload>),
    Analysis(Result<String, String>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ScrollFocus {
    Details,
    Analysis,
    Diagnostics,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ConfigField {
    Provider,
    OllamaModel,
    OllamaUrl,
}

impl ConfigField {
    fn next(self) -> Self {
        match self {
            Self::Provider => Self::OllamaModel,
            Self::OllamaModel => Self::OllamaUrl,
            Self::OllamaUrl => Self::Provider,
        }
    }

    fn previous(self) -> Self {
        match self {
            Self::Provider => Self::OllamaUrl,
            Self::OllamaModel => Self::Provider,
            Self::OllamaUrl => Self::OllamaModel,
        }
    }
}

#[derive(Clone, Debug)]
struct ConfigEditor {
    provider: String,
    ollama_model: String,
    ollama_url: String,
    active_field: ConfigField,
}

impl ConfigEditor {
    fn from_config(config: &crate::ai::config::AppConfig) -> Self {
        Self {
            provider: config.provider.clone(),
            ollama_model: config.ollama_model.clone(),
            ollama_url: config.ollama_url.clone(),
            active_field: ConfigField::Provider,
        }
    }

    fn active_label(&self) -> &'static str {
        match self.active_field {
            ConfigField::Provider => "Provider",
            ConfigField::OllamaModel => "Ollama Model",
            ConfigField::OllamaUrl => "Ollama URL",
        }
    }

    fn active_value_mut(&mut self) -> &mut String {
        match self.active_field {
            ConfigField::Provider => &mut self.provider,
            ConfigField::OllamaModel => &mut self.ollama_model,
            ConfigField::OllamaUrl => &mut self.ollama_url,
        }
    }

    fn cycle_provider(&mut self) {
        self.provider = if self.provider == "ollama" {
            "gemini".to_string()
        } else {
            "ollama".to_string()
        };
    }
}

struct App {
    config: crate::ai::config::AppConfig,
    payload: Option<TuxPayload>,
    analysis: Option<String>,
    status: String,
    diagnostics: Vec<String>,
    is_loading: bool,
    is_analyzing: bool,
    full_bench_enabled: bool,
    selected_drive: usize,
    last_refresh: Option<Instant>,
    details_scroll: u16,
    analysis_scroll: u16,
    diagnostics_scroll: u16,
    scroll_focus: ScrollFocus,
    config_editor: Option<ConfigEditor>,
}

impl App {
    fn new() -> Self {
        Self {
            config: engine::load_config(),
            payload: None,
            analysis: None,
            status: "Starting TuxTests terminal dashboard...".to_string(),
            diagnostics: vec![
                "Keys: [r] refresh  [b] full-bench  [a] analyze  [c] config  [tab] focus panel  [PgUp/PgDn] scroll  [j/k] select drive  [q] quit".to_string(),
                "The dashboard renders the shared backend payload; no UI-side hardware logic is used."
                    .to_string(),
            ],
            is_loading: true,
            is_analyzing: false,
            full_bench_enabled: false,
            selected_drive: 0,
            last_refresh: None,
            details_scroll: 0,
            analysis_scroll: 0,
            diagnostics_scroll: 0,
            scroll_focus: ScrollFocus::Details,
            config_editor: None,
        }
    }

    fn set_payload(&mut self, payload: TuxPayload) {
        self.status = format!(
            "Loaded payload with {} drives. Press [a] to run AI analysis.",
            payload.drives.len()
        );
        self.last_refresh = Some(Instant::now());
        if self.selected_drive >= payload.drives.len() {
            self.selected_drive = payload.drives.len().saturating_sub(1);
        }
        self.details_scroll = 0;
        self.diagnostics_scroll = 0;
        self.payload = Some(payload);
        self.is_loading = false;
    }

    fn set_analysis(&mut self, result: Result<String, String>) {
        self.is_analyzing = false;
        match result {
            Ok(markdown) => {
                self.status = "AI analysis completed.".to_string();
                self.analysis = Some(markdown);
                self.analysis_scroll = 0;
            }
            Err(err) => {
                self.status = "AI analysis failed.".to_string();
                self.diagnostics.push(err);
            }
        }
    }

    fn selected_drive(&self) -> Option<&crate::models::DriveInfo> {
        self.payload
            .as_ref()
            .and_then(|payload| payload.drives.get(self.selected_drive))
    }

    fn select_next(&mut self) {
        if let Some(payload) = &self.payload {
            if !payload.drives.is_empty() {
                self.selected_drive = (self.selected_drive + 1) % payload.drives.len();
                self.details_scroll = 0;
            }
        }
    }

    fn select_previous(&mut self) {
        if let Some(payload) = &self.payload {
            if !payload.drives.is_empty() {
                self.selected_drive = if self.selected_drive == 0 {
                    payload.drives.len() - 1
                } else {
                    self.selected_drive - 1
                };
                self.details_scroll = 0;
            }
        }
    }

    fn cycle_focus(&mut self) {
        self.scroll_focus = match self.scroll_focus {
            ScrollFocus::Details => ScrollFocus::Analysis,
            ScrollFocus::Analysis => ScrollFocus::Diagnostics,
            ScrollFocus::Diagnostics => ScrollFocus::Details,
        };
        self.status = format!("Scroll focus set to {}.", self.scroll_focus.label());
    }

    fn scroll_active_down(&mut self) {
        match self.scroll_focus {
            ScrollFocus::Details => self.details_scroll = self.details_scroll.saturating_add(1),
            ScrollFocus::Analysis => self.analysis_scroll = self.analysis_scroll.saturating_add(1),
            ScrollFocus::Diagnostics => {
                self.diagnostics_scroll = self.diagnostics_scroll.saturating_add(1)
            }
        }
    }

    fn scroll_active_up(&mut self) {
        match self.scroll_focus {
            ScrollFocus::Details => self.details_scroll = self.details_scroll.saturating_sub(1),
            ScrollFocus::Analysis => self.analysis_scroll = self.analysis_scroll.saturating_sub(1),
            ScrollFocus::Diagnostics => {
                self.diagnostics_scroll = self.diagnostics_scroll.saturating_sub(1)
            }
        }
    }

    fn open_config_editor(&mut self) {
        self.config_editor = Some(ConfigEditor::from_config(&self.config));
        self.status = "Editing config. [tab] next field, [enter] save, [esc] cancel.".to_string();
    }

    fn close_config_editor(&mut self) {
        self.config_editor = None;
        self.status = "Closed config editor.".to_string();
    }

    fn save_config_editor(&mut self) {
        let Some(editor) = self.config_editor.clone() else {
            return;
        };

        match engine::apply_config_update(engine::ConfigUpdate {
            provider: Some(editor.provider),
            ollama_model: Some(editor.ollama_model),
            ollama_url: Some(editor.ollama_url),
        }) {
            Ok(config) => {
                self.config = config;
                self.config_editor = None;
                self.status = "Saved TuxTests AI configuration.".to_string();
                self.diagnostics
                    .push("Config updated from the TUI via the shared backend.".to_string());
            }
            Err(err) => {
                self.status = "Failed to save config.".to_string();
                self.diagnostics.push(err);
            }
        }
    }
}

impl ScrollFocus {
    fn label(self) -> &'static str {
        match self {
            Self::Details => "details",
            Self::Analysis => "analysis",
            Self::Diagnostics => "diagnostics",
        }
    }
}

pub async fn run() -> Result<(), Box<dyn Error>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let (tx, mut rx) = mpsc::channel::<UiEvent>(4);
    let mut app = App::new();

    spawn_refresh(tx.clone(), false);

    let result = run_loop(&mut terminal, &mut app, tx, &mut rx).await;

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result.map_err(|err| -> Box<dyn Error> { Box::new(err) })
}

async fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    tx: mpsc::Sender<UiEvent>,
    rx: &mut mpsc::Receiver<UiEvent>,
) -> io::Result<()> {
    loop {
        while let Ok(event) = rx.try_recv() {
            match event {
                UiEvent::Payload(payload) => app.set_payload(*payload),
                UiEvent::Analysis(result) => app.set_analysis(result),
            }
        }

        terminal.draw(|frame| render(frame, app))?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if let Some(editor) = &mut app.config_editor {
                    match key.code {
                        KeyCode::Esc => app.close_config_editor(),
                        KeyCode::Enter => app.save_config_editor(),
                        KeyCode::Tab | KeyCode::Down => {
                            editor.active_field = editor.active_field.next()
                        }
                        KeyCode::BackTab | KeyCode::Up => {
                            editor.active_field = editor.active_field.previous()
                        }
                        KeyCode::Backspace => {
                            editor.active_value_mut().pop();
                        }
                        KeyCode::Char(' ') if editor.active_field == ConfigField::Provider => {
                            editor.cycle_provider();
                        }
                        KeyCode::Char(c) => {
                            if editor.active_field == ConfigField::Provider {
                                if c == 'g' || c == 'o' {
                                    editor.cycle_provider();
                                }
                            } else {
                                editor.active_value_mut().push(c);
                            }
                        }
                        _ => {}
                    }
                    continue;
                }

                match key.code {
                    KeyCode::Char('q') => return Ok(()),
                    KeyCode::Char('r') => {
                        if !app.is_loading {
                            app.full_bench_enabled = false;
                            app.status = "Refreshing hardware payload...".to_string();
                            app.is_loading = true;
                            spawn_refresh(tx.clone(), false);
                        }
                    }
                    KeyCode::Char('b') => {
                        if !app.is_loading {
                            app.full_bench_enabled = true;
                            app.status = "Refreshing hardware payload with SMART and benchmarks..."
                                .to_string();
                            app.is_loading = true;
                            spawn_refresh(tx.clone(), true);
                        }
                    }
                    KeyCode::Char('a') => {
                        if !app.is_analyzing {
                            if let Some(payload) = app.payload.clone() {
                                app.status = "Running AI analysis...".to_string();
                                app.is_analyzing = true;
                                spawn_analysis(tx.clone(), payload);
                            } else {
                                app.diagnostics.push(
                                    "Cannot analyze yet: no payload has been collected."
                                        .to_string(),
                                );
                            }
                        }
                    }
                    KeyCode::Char('c') => app.open_config_editor(),
                    KeyCode::Tab => app.cycle_focus(),
                    KeyCode::PageDown => app.scroll_active_down(),
                    KeyCode::PageUp => app.scroll_active_up(),
                    KeyCode::Down | KeyCode::Char('j') => app.select_next(),
                    KeyCode::Up | KeyCode::Char('k') => app.select_previous(),
                    _ => {}
                }
            }
        }
    }
}

fn spawn_refresh(tx: mpsc::Sender<UiEvent>, full_bench: bool) {
    tokio::spawn(async move {
        let payload =
            match tokio::task::spawn_blocking(move || engine::collect_payload(full_bench)).await {
                Ok(payload) => payload,
                Err(err) => {
                    let _ = tx
                        .send(UiEvent::Analysis(Err(format!(
                            "Payload refresh task failed: {}",
                            err
                        ))))
                        .await;
                    return;
                }
            };
        let _ = tx.send(UiEvent::Payload(Box::new(payload))).await;
    });
}

fn spawn_analysis(tx: mpsc::Sender<UiEvent>, payload: TuxPayload) {
    tokio::spawn(async move {
        let result = engine::analyze_payload_quiet(&payload).await;
        let _ = tx.send(UiEvent::Analysis(result)).await;
    });
}

fn render(frame: &mut ratatui::Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(8),
            Constraint::Min(14),
            Constraint::Length(10),
        ])
        .split(frame.size());

    let header_title = app
        .payload
        .as_ref()
        .map(|payload| {
            format!(
                "TuxTests TUI | {} | {} drives",
                payload.system.hostname,
                payload.drives.len()
            )
        })
        .unwrap_or_else(|| "TuxTests TUI | loading...".to_string());

    let bench_mode = if app.full_bench_enabled {
        "full-bench"
    } else {
        "scan"
    };
    let refresh_text = app
        .last_refresh
        .map(|instant| format!("refreshed {}s ago", instant.elapsed().as_secs()))
        .unwrap_or_else(|| "not refreshed yet".to_string());

    let header = Paragraph::new(vec![
        Line::from(app.status.clone()),
        Line::from(format!(
            "provider={} model={} mode={} focus={} {}",
            app.config.provider,
            app.config.ollama_model,
            bench_mode,
            app.scroll_focus.label(),
            refresh_text
        )),
    ])
    .block(Block::default().borders(Borders::ALL).title(header_title))
    .style(
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    );
    frame.render_widget(header, chunks[0]);

    render_summary(frame, app, chunks[1]);
    render_middle(frame, app, chunks[2]);
    render_bottom(frame, app, chunks[3]);

    if app.config_editor.is_some() {
        render_config_modal(frame, app);
    }
}

fn render_summary(frame: &mut ratatui::Frame, app: &App, area: Rect) {
    let lines = if let Some(payload) = &app.payload {
        vec![
            Line::from(vec![
                Span::styled("OS: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(
                    payload
                        .system
                        .os_release
                        .get("PRETTY_NAME")
                        .cloned()
                        .unwrap_or_else(|| "Unknown".to_string()),
                ),
            ]),
            Line::from(vec![
                Span::styled("Kernel: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(payload.system.kernel_version.clone()),
            ]),
            Line::from(vec![
                Span::styled("CPU: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(payload.system.cpu.clone()),
            ]),
            Line::from(vec![
                Span::styled("RAM: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(format!("{} GB", payload.system.ram_gb)),
            ]),
            Line::from(vec![
                Span::styled(
                    "ASPM Policy: ",
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::raw(
                    payload
                        .system
                        .pcie_aspm_policy
                        .clone()
                        .unwrap_or_else(|| "Unknown".to_string()),
                ),
            ]),
        ]
    } else {
        vec![Line::from("Collecting system summary...")]
    };

    let summary = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title("System Summary"),
    );
    frame.render_widget(summary, area);
}

fn render_middle(frame: &mut ratatui::Frame, app: &App, area: Rect) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(56), Constraint::Percentage(44)])
        .split(area);

    render_drives(frame, app, columns[0]);
    render_drive_details(frame, app, columns[1]);
}

fn render_drives(frame: &mut ratatui::Frame, app: &App, area: Rect) {
    let rows: Vec<Row> = app
        .payload
        .as_ref()
        .map(|payload| {
            payload
                .drives
                .iter()
                .map(|drive| {
                    Row::new(vec![
                        Cell::from(drive.name.clone()),
                        Cell::from(drive.drive_type.clone()),
                        Cell::from(drive.connection.clone()),
                        Cell::from(format!("{} GB", drive.capacity_gb)),
                        Cell::from(format!("{}%", drive.usage_percent)),
                        Cell::from(if drive.health_ok { "OK" } else { "WARN" }),
                    ])
                })
                .collect()
        })
        .unwrap_or_default();

    let table = Table::new(
        rows,
        [
            Constraint::Length(10),
            Constraint::Length(10),
            Constraint::Percentage(40),
            Constraint::Length(10),
            Constraint::Length(8),
            Constraint::Length(8),
        ],
    )
    .header(
        Row::new(vec!["Drive", "Type", "Connection", "Size", "Use", "Health"]).style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
    )
    .highlight_style(Style::default().bg(Color::DarkGray))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title("Drives [j/k to select]"),
    );

    let mut state = TableState::default();
    if app.payload.as_ref().is_some_and(|p| !p.drives.is_empty()) {
        state.select(Some(app.selected_drive));
    }
    frame.render_stateful_widget(table, area, &mut state);
}

fn render_drive_details(frame: &mut ratatui::Frame, app: &App, area: Rect) {
    let lines = if let Some(drive) = app.selected_drive() {
        let mut lines = vec![
            Line::from(vec![
                Span::styled("Name: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(drive.name.clone()),
            ]),
            Line::from(vec![
                Span::styled("Type: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(drive.drive_type.clone()),
            ]),
            Line::from(vec![
                Span::styled(
                    "Connection: ",
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::raw(drive.connection.clone()),
            ]),
            Line::from(vec![
                Span::styled("Path: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(drive.physical_path.clone()),
            ]),
            Line::from(vec![
                Span::styled("Mounts: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(if drive.active_mountpoints.is_empty() {
                    "none".to_string()
                } else {
                    drive.active_mountpoints.join(", ")
                }),
            ]),
            Line::from(vec![
                Span::styled("Health: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(if drive.health_ok {
                    "OK".to_string()
                } else {
                    "Needs attention".to_string()
                }),
            ]),
        ];

        if let Some(serial) = &drive.serial {
            lines.push(Line::from(vec![
                Span::styled("Serial: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(serial.clone()),
            ]));
        }

        if !drive.topology.is_empty() {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Topology",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )));
            for topology in &drive.topology {
                lines.push(Line::from(format!(
                    "- L{} {} {}",
                    topology.level, topology.subsystem, topology.sysname
                )));
            }
        }

        if !drive.pcie_path.is_empty() {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "PCIe Path",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )));
            for path in &drive.pcie_path {
                lines.push(Line::from(format!(
                    "- {} {} {}",
                    path.bdf,
                    path.current_link_speed.as_deref().unwrap_or("?"),
                    path.aspm.as_deref().unwrap_or("ASPM unknown")
                )));
                if let Some(capability) = &path.aspm_capability {
                    lines.push(Line::from(format!("  capability: {}", capability)));
                }
                if let Some(error) = &path.aspm_probe_error {
                    lines.push(Line::from(format!("  probe: {}", error)));
                }
            }
        }

        lines
    } else {
        vec![Line::from("No drive selected yet.")]
    };

    let panel = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title(format!(
            "Drive Details{} [{}]",
            if app.scroll_focus == ScrollFocus::Details {
                " [focused]"
            } else {
                ""
            },
            app.details_scroll
        )))
        .scroll((app.details_scroll, 0))
        .wrap(Wrap { trim: true });
    frame.render_widget(panel, area);
}

fn render_bottom(frame: &mut ratatui::Frame, app: &App, area: Rect) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(area);

    render_analysis(frame, app, columns[0]);
    render_diagnostics(frame, app, columns[1]);
}

fn render_analysis(frame: &mut ratatui::Frame, app: &App, area: Rect) {
    let analysis_text = if app.is_analyzing {
        "AI analysis in progress..."
    } else if let Some(analysis) = &app.analysis {
        analysis
    } else {
        "No analysis yet. Press [a] to analyze."
    };

    let panel = Paragraph::new(vec![Line::from(analysis_text.to_string())])
        .block(Block::default().borders(Borders::ALL).title(format!(
            "Analysis{} [{}]",
            if app.scroll_focus == ScrollFocus::Analysis {
                " [focused]"
            } else {
                ""
            },
            app.analysis_scroll
        )))
        .scroll((app.analysis_scroll, 0))
        .wrap(Wrap { trim: true });
    frame.render_widget(panel, area);
}

fn render_diagnostics(frame: &mut ratatui::Frame, app: &App, area: Rect) {
    let mut lines = Vec::new();

    for entry in &app.diagnostics {
        lines.push(Line::from(format!("- {}", entry)));
    }

    if let Some(payload) = &app.payload {
        if !payload.kernel_anomalies.is_empty() {
            if !lines.is_empty() {
                lines.push(Line::from(""));
            }
            lines.push(Line::from(Span::styled(
                "Kernel Anomalies",
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            )));
            for anomaly in &payload.kernel_anomalies {
                lines.push(Line::from(format!("- {}", anomaly)));
            }
        }
    }

    if lines.is_empty() {
        lines.push(Line::from("No diagnostics yet."));
    }

    let panel = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title(format!(
            "Diagnostics{} [{}]",
            if app.scroll_focus == ScrollFocus::Diagnostics {
                " [focused]"
            } else {
                ""
            },
            app.diagnostics_scroll
        )))
        .scroll((app.diagnostics_scroll, 0))
        .wrap(Wrap { trim: true });
    frame.render_widget(panel, area);
}

fn render_config_modal(frame: &mut ratatui::Frame, app: &App) {
    let Some(editor) = &app.config_editor else {
        return;
    };

    let area = centered_rect(70, 45, frame.size());
    frame.render_widget(Clear, area);

    let fields = [
        (
            ConfigField::Provider,
            "Provider",
            format!("{}  (space toggles gemini/ollama)", editor.provider),
        ),
        (
            ConfigField::OllamaModel,
            "Ollama Model",
            editor.ollama_model.clone(),
        ),
        (
            ConfigField::OllamaUrl,
            "Ollama URL",
            editor.ollama_url.clone(),
        ),
    ];

    let mut lines = vec![Line::from("Backend-driven config editor"), Line::from("")];

    for (field, label, value) in fields {
        let prefix = if editor.active_field == field {
            "> "
        } else {
            "  "
        };
        lines.push(Line::from(vec![
            Span::styled(
                format!("{}{}: ", prefix, label),
                Style::default()
                    .fg(if editor.active_field == field {
                        Color::Cyan
                    } else {
                        Color::White
                    })
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(value),
        ]));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(format!("Editing: {}", editor.active_label())));
    lines.push(Line::from(
        "Keys: [tab/shift-tab] move  [enter] save  [esc] cancel",
    ));

    let modal = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title("Config"))
        .alignment(Alignment::Left)
        .wrap(Wrap { trim: true });
    frame.render_widget(modal, area);
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
