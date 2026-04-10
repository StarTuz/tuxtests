use crate::engine;
use crate::models::TuxPayload;
use crossterm::event::{self, Event, KeyCode};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState, Wrap};
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
}

impl App {
    fn new() -> Self {
        Self {
            config: engine::load_config(),
            payload: None,
            analysis: None,
            status: "Starting TuxTests terminal dashboard...".to_string(),
            diagnostics: vec![
                "Keys: [r] refresh  [b] full-bench refresh  [a] analyze  [j/k] select drive  [q] quit"
                    .to_string(),
                "The dashboard renders the shared backend payload; no UI-side hardware logic is used."
                    .to_string(),
            ],
            is_loading: true,
            is_analyzing: false,
            full_bench_enabled: false,
            selected_drive: 0,
            last_refresh: None,
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
        self.payload = Some(payload);
        self.is_loading = false;
    }

    fn set_analysis(&mut self, result: Result<String, String>) {
        self.is_analyzing = false;
        match result {
            Ok(markdown) => {
                self.status = "AI analysis completed.".to_string();
                self.analysis = Some(markdown);
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
            }
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
        let result = engine::analyze_payload(&payload).await;
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
            "provider={} model={} mode={} {}",
            app.config.provider, app.config.ollama_model, bench_mode, refresh_text
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
}

fn render_summary(frame: &mut ratatui::Frame, app: &App, area: ratatui::layout::Rect) {
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

fn render_middle(frame: &mut ratatui::Frame, app: &App, area: ratatui::layout::Rect) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(56), Constraint::Percentage(44)])
        .split(area);

    render_drives(frame, app, columns[0]);
    render_drive_details(frame, app, columns[1]);
}

fn render_drives(frame: &mut ratatui::Frame, app: &App, area: ratatui::layout::Rect) {
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

fn render_drive_details(frame: &mut ratatui::Frame, app: &App, area: ratatui::layout::Rect) {
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
        ];

        if let Some(serial) = &drive.serial {
            lines.push(Line::from(vec![
                Span::styled("Serial: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(serial.clone()),
            ]));
        }

        if !drive.pcie_path.is_empty() {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "PCIe Path",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )));
            for path in drive.pcie_path.iter().take(3) {
                lines.push(Line::from(format!(
                    "- {} {} {}",
                    path.bdf,
                    path.current_link_speed.as_deref().unwrap_or("?"),
                    path.aspm.as_deref().unwrap_or("ASPM unknown")
                )));
            }
        }

        lines
    } else {
        vec![Line::from("No drive selected yet.")]
    };

    let panel = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Drive Details"),
        )
        .wrap(Wrap { trim: true });
    frame.render_widget(panel, area);
}

fn render_bottom(frame: &mut ratatui::Frame, app: &App, area: ratatui::layout::Rect) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(area);

    render_analysis(frame, app, columns[0]);
    render_diagnostics(frame, app, columns[1]);
}

fn render_analysis(frame: &mut ratatui::Frame, app: &App, area: ratatui::layout::Rect) {
    let analysis_text = if app.is_analyzing {
        "AI analysis in progress..."
    } else if let Some(analysis) = &app.analysis {
        analysis
    } else {
        "No analysis yet. Press [a] to analyze."
    };

    let panel = Paragraph::new(vec![Line::from(analysis_text.to_string())])
        .block(Block::default().borders(Borders::ALL).title("Analysis"))
        .wrap(Wrap { trim: true });
    frame.render_widget(panel, area);
}

fn render_diagnostics(frame: &mut ratatui::Frame, app: &App, area: ratatui::layout::Rect) {
    let mut lines = Vec::new();

    for entry in app.diagnostics.iter().rev().take(3).rev() {
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
            for anomaly in payload.kernel_anomalies.iter().take(3) {
                lines.push(Line::from(format!("- {}", anomaly)));
            }
        }
    }

    if lines.is_empty() {
        lines.push(Line::from("No diagnostics yet."));
    }

    let panel = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title("Diagnostics"))
        .wrap(Wrap { trim: true });
    frame.render_widget(panel, area);
}
