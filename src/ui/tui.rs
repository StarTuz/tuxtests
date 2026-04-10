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
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table, Wrap};
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
    payload: Option<TuxPayload>,
    analysis: Option<String>,
    status: String,
    diagnostics: Vec<String>,
    is_loading: bool,
    is_analyzing: bool,
    last_refresh: Option<Instant>,
}

impl App {
    fn new() -> Self {
        Self {
            payload: None,
            analysis: None,
            status: "Starting TuxTests terminal dashboard...".to_string(),
            diagnostics: vec![
                "Keys: [r] refresh  [a] analyze  [q] quit".to_string(),
                "The dashboard renders the shared backend payload; no UI-side hardware logic is used."
                    .to_string(),
            ],
            is_loading: true,
            is_analyzing: false,
            last_refresh: None,
        }
    }

    fn set_payload(&mut self, payload: TuxPayload) {
        self.status = format!(
            "Loaded payload with {} drives. Press [a] to run AI analysis.",
            payload.drives.len()
        );
        self.last_refresh = Some(Instant::now());
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
}

pub async fn run() -> Result<(), Box<dyn Error>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let (tx, mut rx) = mpsc::channel::<UiEvent>(4);
    let mut app = App::new();

    spawn_refresh(tx.clone());

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
                            app.status = "Refreshing hardware payload...".to_string();
                            app.is_loading = true;
                            spawn_refresh(tx.clone());
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
                    _ => {}
                }
            }
        }
    }
}

fn spawn_refresh(tx: mpsc::Sender<UiEvent>) {
    tokio::spawn(async move {
        let payload = match tokio::task::spawn_blocking(|| engine::collect_payload(false)).await {
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
            Constraint::Min(10),
            Constraint::Length(6),
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

    let header = Paragraph::new(app.status.clone())
        .block(Block::default().borders(Borders::ALL).title(header_title))
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );
    frame.render_widget(header, chunks[0]);

    render_summary(frame, app, chunks[1]);
    render_drives(frame, app, chunks[2]);
    render_analysis(frame, app, chunks[3]);
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
    .block(Block::default().borders(Borders::ALL).title("Drives"));

    frame.render_widget(table, area);
}

fn render_analysis(frame: &mut ratatui::Frame, app: &App, area: ratatui::layout::Rect) {
    let analysis_text = if app.is_analyzing {
        "AI analysis in progress..."
    } else if let Some(analysis) = &app.analysis {
        analysis
    } else {
        "No analysis yet. Press [a] to analyze or [r] to refresh."
    };

    let mut lines = vec![Line::from(analysis_text.to_string())];
    if !app.diagnostics.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Diagnostics",
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        )));
        for entry in app.diagnostics.iter().rev().take(3).rev() {
            lines.push(Line::from(format!("- {}", entry)));
        }
    }

    let panel = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title("Analysis"))
        .wrap(Wrap { trim: true });
    frame.render_widget(panel, area);
}
