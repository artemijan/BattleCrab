//! `l2r-tools msg-color` — a terminal editor for system-message colours.
//!
//! Opens the `SystemMsg*` table straight out of the client, lets you find
//! messages by id or by text, and recolour as many as you like.
//!
//! The whole session is in memory — nothing on disk changes until you say so.
//! Closing with unsaved edits asks first, and answering yes packs and
//! re-encrypts the file back into `system/` so the client reads it. See
//! [`tools::system_msg`] for the model.

use ratatui::Frame;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};
use std::path::PathBuf;
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
    /// Closing with unsaved edits: save, discard, or keep editing.
    ConfirmQuit,
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
            status: "Type to search, Tab to browse, Enter to recolour, Esc to close".into(),
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
    let mut set = SchemaSet::load(&args.structure_dir).unwrap_or_else(|e| fail(&e));
    let file = MsgFile::open(&mut set, &system, &args.file).unwrap_or_else(|e| fail(&e));
    let mut app = App::new(file);

    let mut terminal = ratatui::init();
    let result = event_loop(&mut terminal, &mut app, &mut set);
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
            save(app, set);
            continue;
        }
        if matches!(app.mode, Mode::ConfirmQuit) {
            match key.code {
                KeyCode::Char('y') => {
                    save(app, set);
                    // Only leave if it actually landed; otherwise the prompt
                    // stays up with the reason.
                    app.quit = app.file.edited_count() == 0;
                    if !app.quit {
                        app.mode = Mode::Browse;
                    }
                }
                KeyCode::Char('n') => app.quit = true,
                KeyCode::Esc | KeyCode::Char('c') => app.mode = Mode::Browse,
                _ => {}
            }
            continue;
        }
        handle_key(app, key);
    }
    Ok(())
}

fn save(app: &mut App, set: &mut SchemaSet) {
    let edited = app.file.edited_count();
    if edited == 0 {
        app.status = "no changes to save".into();
        return;
    }
    app.status = match app.file.save(set) {
        Ok(()) => format!("saved {edited} change(s) to {}", app.file.name),
        Err(e) => format!("SAVE FAILED, nothing written: {e}"),
    };
}

fn save_requested(key: &KeyEvent) -> bool {
    key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('s'))
}

/// Close, or ask first when there is something to lose.
fn request_quit(app: &mut App) {
    if app.file.edited_count() == 0 {
        app.quit = true;
    } else {
        app.mode = Mode::ConfirmQuit;
    }
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
            KeyCode::Esc => request_quit(app),
            _ => {}
        },

        Mode::Browse => match key.code {
            KeyCode::Char('q') | KeyCode::Esc => request_quit(app),
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

        Mode::ConfirmQuit => {}

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
    // The picker needs a row per preset plus the input line and borders. A
    // fixed height silently cut the list off and took the cursor with it, so
    // the detail pane is sized to whatever it is currently showing.
    let detail_height = match app.mode {
        Mode::Colour { .. } => PRESETS.len() as u16 + 4,
        Mode::ConfirmQuit => 6,
        _ => 6,
    };
    let areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(3),
            Constraint::Length(detail_height),
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
        Mode::Search => "type to filter · Tab/↓ list · Esc close",
        Mode::Browse => "↑↓ move · Enter recolour · r revert · / search · Ctrl-S save · Esc close",
        Mode::Colour { .. } => {
            "type hex · ↑↓ preset · Space apply preset · Enter confirm · Esc cancel"
        }
        Mode::ConfirmQuit => "y save and close · n discard and close · Esc keep editing",
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
        Mode::ConfirmQuit => vec![
            Line::from(Span::styled(
                format!("{} unsaved change(s)", app.file.edited_count()),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::raw(format!(
                "Pack and re-encrypt them into {} before closing?",
                app.file.name
            )),
            Line::raw("y = save and close    n = discard and close    Esc = keep editing"),
        ],
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

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    /// Render into a buffer and flatten it to lines of text.
    fn render(app: &mut App, width: u16, height: u16) -> Vec<String> {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal.draw(|frame| draw(frame, app)).unwrap();
        let buffer = terminal.backend().buffer().clone();
        (0..buffer.area.height)
            .map(|y| {
                (0..buffer.area.width)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect()
    }

    fn app() -> App {
        let line = "msg_begin\t0\t1\t[Disconnected.]\t2\t799BB0FF\t1\t1\tmsg_end";
        App::new(
            MsgFile::from_text(
                "SystemMsg-t.dat",
                line,
                "413".into(),
                tools::dat_schema::Layout {
                    version: "Helios".into(),
                    safe_package: true,
                    nodes: Vec::new(),
                },
                std::path::PathBuf::from("/nonexistent/SystemMsg-t.dat"),
            )
            .unwrap(),
        )
    }

    /// The bug this guards: a fixed-height detail pane showed only the first
    /// few presets, and moving the cursor past them scrolled it out of sight.
    #[test]
    fn the_colour_picker_shows_every_preset() {
        let mut app = app();
        app.mode = Mode::Colour {
            input: "799BB0FF".into(),
            preset: 0,
        };
        let screen = render(&mut app, 100, 30).join("\n");
        for (name, value) in PRESETS {
            assert!(
                screen.contains(value),
                "preset {name} ({value}) not rendered"
            );
        }
    }

    #[test]
    fn the_cursor_stays_visible_on_the_last_preset() {
        let mut app = app();
        app.mode = Mode::Colour {
            input: "799BB0FF".into(),
            preset: PRESETS.len() - 1,
        };
        let lines = render(&mut app, 100, 30);
        let marked: Vec<&String> = lines.iter().filter(|l| l.contains("> ")).collect();
        assert_eq!(marked.len(), 1, "expected exactly one cursor row");
        assert!(
            marked[0].contains(PRESETS[PRESETS.len() - 1].1),
            "cursor is not on the selected preset: {:?}",
            marked[0]
        );
    }

    #[test]
    fn arrow_keys_move_the_preset_cursor_and_stop_at_the_ends() {
        let mut app = app();
        app.mode = Mode::Colour {
            input: String::new(),
            preset: 0,
        };
        for _ in 0..PRESETS.len() + 5 {
            handle_key(&mut app, KeyEvent::from(KeyCode::Down));
        }
        let Mode::Colour { preset, .. } = app.mode else {
            panic!("left the picker")
        };
        assert_eq!(preset, PRESETS.len() - 1, "should clamp at the last preset");

        for _ in 0..PRESETS.len() + 5 {
            handle_key(&mut app, KeyEvent::from(KeyCode::Up));
        }
        let Mode::Colour { preset, .. } = app.mode else {
            panic!("left the picker")
        };
        assert_eq!(preset, 0, "should clamp at the first preset");
    }

    #[test]
    fn space_applies_the_highlighted_preset_and_enter_commits_it() {
        let mut app = app();
        app.mode = Mode::Colour {
            input: String::new(),
            preset: 4,
        };
        handle_key(&mut app, KeyEvent::from(KeyCode::Char(' ')));
        handle_key(&mut app, KeyEvent::from(KeyCode::Enter));
        assert_eq!(app.file.messages[0].colour, PRESETS[4].1);
        assert_eq!(app.file.edited_count(), 1);
    }

    /// Closing with edits must not lose them silently.
    #[test]
    fn closing_with_edits_asks_first() {
        // Nothing changed yet: Esc just closes.
        let mut clean = app();
        handle_key(&mut clean, KeyEvent::from(KeyCode::Esc));
        assert!(clean.quit);

        let mut dirty = app();
        dirty.file.set_colour(0, "FF0000FF").unwrap();
        dirty.mode = Mode::Browse;
        handle_key(&mut dirty, KeyEvent::from(KeyCode::Esc));
        assert!(!dirty.quit, "must not close straight away");
        assert!(matches!(dirty.mode, Mode::ConfirmQuit));
    }

    #[test]
    fn the_close_prompt_names_the_file_and_the_count() {
        let mut app = app();
        app.file.set_colour(0, "FF0000FF").unwrap();
        app.mode = Mode::ConfirmQuit;
        let screen = render(&mut app, 100, 30).join("\n");
        assert!(screen.contains("1 unsaved change"), "{screen}");
        assert!(screen.contains("SystemMsg-t.dat"));
        assert!(screen.contains("y save and close"));
    }

    #[test]
    fn search_matches_id_and_text() {
        let mut app = app();
        app.query = "disconn".into();
        app.refilter();
        assert_eq!(app.matches.len(), 1);
        app.query = "0".into();
        app.refilter();
        assert_eq!(app.matches.len(), 1);
        app.query = "nothing here".into();
        app.refilter();
        assert!(app.matches.is_empty());
        assert_eq!(app.selected(), None);
    }
}
