//! `l2r-tools msg-color` — a terminal editor for system-message colours.
//!
//! Opens the decrypted `SystemMsg*` table, lets you find messages by id or by
//! text, recolour as many as you like in one session, and writes the result
//! back through pack + encrypt so the client can read it.
//!
//! Saving is explicit and all-at-once: edits live in memory until `Ctrl-S`, so
//! a session can be abandoned with `Esc`/`q` without touching either
//! directory. See [`tools::system_msg`] for the model.

use ratatui::Frame;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};
use std::path::PathBuf;
use tools::client_files;
use tools::dat_schema::SchemaSet;
use tools::system_msg::{MsgFile, PRESETS};

#[derive(clap::Args)]
pub struct Args {
    /// System-message table to edit, as named in the client.
    #[arg(long, default_value = "SystemMsg_Classic-eu.dat")]
    file: String,

    /// Where the client lives.
    #[arg(long, default_value = "dist/client")]
    client_dir: PathBuf,

    /// The client's `system` directory. Defaults to `<client-dir>/system`.
    #[arg(long)]
    system_dir: Option<PathBuf>,

    /// Decrypted files. Defaults to `<client-dir>/system_decrypted`.
    #[arg(long)]
    decrypted_dir: Option<PathBuf>,

    /// The vendored schema set.
    #[arg(long, default_value = "dist/client/structure")]
    structure_dir: PathBuf,
}

/// Which pane has the keyboard.
enum Mode {
    /// Typing into the search box.
    Search,
    /// Moving through the results.
    Browse,
    /// Picking a colour for the selected message.
    Colour { input: String, preset: usize },
}

struct App {
    file: MsgFile,
    query: String,
    /// Indices into `file.messages` matching `query`, in order.
    matches: Vec<usize>,
    list: ListState,
    mode: Mode,
    status: String,
    quit: bool,
}

impl App {
    fn new(file: MsgFile) -> Self {
        let mut app = App {
            file,
            query: String::new(),
            matches: Vec::new(),
            list: ListState::default(),
            mode: Mode::Search,
            status: "Type to search, Tab to browse, Enter to recolour, Ctrl-S to save".into(),
            quit: false,
        };
        app.refilter();
        app
    }

    /// Match on id or on message text, so `2810` and `dead` both work.
    fn refilter(&mut self) {
        let needle = self.query.trim().to_lowercase();
        self.matches = self
            .file
            .messages
            .iter()
            .enumerate()
            .filter(|(_, m)| {
                needle.is_empty()
                    || m.id.to_string().contains(&needle)
                    || m.text.to_lowercase().contains(&needle)
            })
            .map(|(i, _)| i)
            .collect();
        let selected = (!self.matches.is_empty()).then_some(0);
        self.list.select(selected);
    }

    fn selected(&self) -> Option<usize> {
        self.list
            .selected()
            .and_then(|i| self.matches.get(i))
            .copied()
    }

    fn step(&mut self, delta: isize) {
        if self.matches.is_empty() {
            return;
        }
        let current = self.list.selected().unwrap_or(0) as isize;
        let last = self.matches.len() as isize - 1;
        self.list
            .select(Some(current.saturating_add(delta).clamp(0, last) as usize));
    }
}

pub fn run(args: &Args) {
    let system = args
        .system_dir
        .clone()
        .unwrap_or_else(|| args.client_dir.join("system"));
    let decrypted = args
        .decrypted_dir
        .clone()
        .unwrap_or_else(|| args.client_dir.join("system_decrypted"));

    let mut set = SchemaSet::load(&args.structure_dir).unwrap_or_else(|e| fail(&e));
    let file = MsgFile::open(&decrypted, &args.file).unwrap_or_else(|e| fail(&e));
    let mut app = App::new(file);

    let mut terminal = ratatui::init();
    let result = event_loop(&mut terminal, &mut app, &mut set, &system, &decrypted);
    ratatui::restore();

    if let Err(e) = result {
        fail(&e.to_string());
    }
    println!("{}", app.status);
}

fn event_loop(
    terminal: &mut ratatui::DefaultTerminal,
    app: &mut App,
    set: &mut SchemaSet,
    system: &std::path::Path,
    decrypted: &std::path::Path,
) -> std::io::Result<()> {
    while !app.quit {
        terminal.draw(|frame| draw(frame, app))?;
        let Event::Key(key) = event::read()? else {
            continue;
        };
        // Windows sends both press and release; act on press only.
        if key.kind != KeyEventKind::Press {
            continue;
        }
        if save_requested(&key) {
            let cfg = client_files::Config {
                system_dir: system,
                decrypted_dir: decrypted,
                chronicle: None,
            };
            let edited = app.file.edited_count();
            app.status = match app.file.save(set, &cfg) {
                Ok(()) => format!("saved {edited} change(s) to {}", app.file.name),
                Err(e) => format!("SAVE FAILED, nothing written: {e}"),
            };
            continue;
        }
        handle_key(app, key);
    }
    Ok(())
}

fn save_requested(key: &KeyEvent) -> bool {
    key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('s'))
}

fn handle_key(app: &mut App, key: KeyEvent) {
    match &mut app.mode {
        Mode::Search => match key.code {
            KeyCode::Char(c) => {
                app.query.push(c);
                app.refilter();
            }
            KeyCode::Backspace => {
                app.query.pop();
                app.refilter();
            }
            KeyCode::Tab | KeyCode::Down | KeyCode::Enter => app.mode = Mode::Browse,
            KeyCode::Esc => app.quit = true,
            _ => {}
        },

        Mode::Browse => match key.code {
            KeyCode::Char('q') | KeyCode::Esc => app.mode = Mode::Search,
            KeyCode::Char('/') | KeyCode::Tab => app.mode = Mode::Search,
            KeyCode::Down | KeyCode::Char('j') => app.step(1),
            KeyCode::Up | KeyCode::Char('k') => app.step(-1),
            KeyCode::PageDown => app.step(10),
            KeyCode::PageUp => app.step(-10),
            KeyCode::Char('r') => {
                if let Some(index) = app.selected() {
                    app.file.revert(index);
                    app.status = "reverted".into();
                }
            }
            KeyCode::Enter => {
                if let Some(index) = app.selected() {
                    app.mode = Mode::Colour {
                        input: app.file.messages[index].colour.clone(),
                        preset: 0,
                    };
                }
            }
            _ => {}
        },

        Mode::Colour { input, preset } => match key.code {
            KeyCode::Esc => app.mode = Mode::Browse,
            KeyCode::Char(c) if c.is_ascii_hexdigit() => {
                if input.len() < 8 {
                    input.push(c.to_ascii_uppercase());
                }
            }
            KeyCode::Backspace => {
                input.pop();
            }
            KeyCode::Down => *preset = (*preset + 1).min(PRESETS.len() - 1),
            KeyCode::Up => *preset = preset.saturating_sub(1),
            // Space applies the highlighted preset without leaving the picker.
            KeyCode::Char(' ') => *input = PRESETS[*preset].1.to_string(),
            KeyCode::Enter => {
                let value = input.clone();
                if let Some(index) = app.selected() {
                    match app.file.set_colour(index, &value) {
                        Ok(()) => {
                            app.status =
                                format!("message {} -> #{value}", app.file.messages[index].id);
                            app.mode = Mode::Browse;
                        }
                        Err(e) => app.status = e,
                    }
                }
            }
            _ => {}
        },
    }
}

fn swatch(rgb: (u8, u8, u8)) -> Style {
    Style::default().fg(Color::Rgb(rgb.0, rgb.1, rgb.2))
}

fn draw(frame: &mut Frame, app: &mut App) {
    let areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(6),
            Constraint::Length(1),
        ])
        .split(frame.area());

    let edited = app.file.edited_count();
    let title = format!(
        " {} — {} message(s), {edited} edited ",
        app.file.name,
        app.file.messages.len()
    );
    frame.render_widget(
        Paragraph::new(app.query.as_str()).block(
            Block::default()
                .borders(Borders::ALL)
                .title(" search by id or text ")
                .title_bottom(title),
        ),
        areas[0],
    );

    let items: Vec<ListItem> = app
        .matches
        .iter()
        .map(|&i| {
            let m = &app.file.messages[i];
            ListItem::new(Line::from(vec![
                Span::styled(
                    format!("{:>6} ", m.id),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled("███ ", swatch(m.rgb())),
                Span::styled(
                    format!("{:<9}", m.colour),
                    if m.edited() {
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::DarkGray)
                    },
                ),
                Span::raw(m.text.replace('\n', " ")),
            ]))
        })
        .collect();

    frame.render_stateful_widget(
        List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!(" {} match(es) ", app.matches.len())),
            )
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED)),
        areas[1],
        &mut app.list,
    );

    draw_detail(frame, app, areas[2]);

    let help = match app.mode {
        Mode::Search => "type to filter · Tab/↓ list · Esc quit",
        Mode::Browse => "↑↓ move · Enter recolour · r revert · / search · Ctrl-S save · Esc back",
        Mode::Colour { .. } => {
            "type hex · ↑↓ preset · Space apply preset · Enter confirm · Esc cancel"
        }
    };
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                format!(" {help} "),
                Style::default().fg(Color::Black).bg(Color::Gray),
            ),
            Span::raw("  "),
            Span::raw(app.status.clone()),
        ])),
        areas[3],
    );
}

fn draw_detail(frame: &mut Frame, app: &App, area: Rect) {
    let Some(index) = app.selected() else {
        frame.render_widget(
            Paragraph::new("no match").block(Block::default().borders(Borders::ALL)),
            area,
        );
        return;
    };
    let message = &app.file.messages[index];

    let body = match &app.mode {
        Mode::Colour { input, preset } => {
            let mut lines = vec![Line::from(vec![
                Span::raw("new colour: "),
                Span::styled(
                    format!("{input:<8}"),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::raw("   (RRGGBB or RRGGBBAA)"),
            ])];
            for (i, (name, value)) in PRESETS.iter().enumerate() {
                let rgb = (
                    u8::from_str_radix(&value[0..2], 16).unwrap_or(0),
                    u8::from_str_radix(&value[2..4], 16).unwrap_or(0),
                    u8::from_str_radix(&value[4..6], 16).unwrap_or(0),
                );
                lines.push(Line::from(vec![
                    Span::raw(if i == *preset { "> " } else { "  " }),
                    Span::styled("███ ", swatch(rgb)),
                    Span::raw(format!("{value}  {name}")),
                ]));
            }
            lines
        }
        _ => vec![
            Line::from(vec![
                Span::styled(
                    format!("id {} ", message.id),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled("███ ", swatch(message.rgb())),
                Span::raw(message.colour.clone()),
                Span::raw(if message.edited() {
                    format!("  (was {})", message.original_colour)
                } else {
                    String::new()
                }),
            ]),
            Line::raw(message.text.clone()),
        ],
    };

    frame.render_widget(
        Paragraph::new(body)
            .wrap(Wrap { trim: false })
            .block(Block::default().borders(Borders::ALL).title(" message ")),
        area,
    );
}

fn fail(message: &str) -> ! {
    eprintln!("{message}");
    std::process::exit(2)
}
