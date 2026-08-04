//! A one-line progress bar for the batch conversions.
//!
//! `client-dat` spends most of a run inside RSA `modpow` over multi-megabyte
//! files, which is slow enough that a silent terminal looks like a hang. This
//! draws a Ratatui [`Gauge`] labelled with the file currently being converted.
//!
//! # Why it paints the line itself
//!
//! The obvious build is a [`ratatui::Terminal`] with an inline viewport, and
//! it is the wrong one here. Placing an inline viewport means asking the
//! terminal where the cursor is (`ESC[6n`) and reading the answer back off
//! stdin, which needs raw mode — and raw mode costs the user Ctrl-C for the
//! length of a multi-minute conversion. Worse, anywhere nothing answers that
//! query (a CI job, a `script` capture, a background task) the bar draws
//! nothing at all and leaves the bare escape sequence in the log.
//!
//! So the widget is rendered into an off-screen [`Buffer`] one row tall and
//! the row is printed over itself with a carriage return. No cursor query, no
//! raw mode, no signal handling to get wrong, and it degrades to silence the
//! moment stdout is not a terminal.
//!
//! Redraws are throttled, and every write may fail silently: a progress bar
//! must never be the reason a conversion does not finish.

use ratatui::backend::IntoCrossterm;
use ratatui::buffer::Buffer;
use ratatui::crossterm::style::{ContentStyle, PrintStyledContent, ResetColor, StyledContent};
use ratatui::crossterm::terminal::{Clear, ClearType};
use ratatui::crossterm::{cursor, queue, terminal};
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::widgets::{Gauge, Widget};
use std::io::{IsTerminal, Write};
use std::time::{Duration, Instant};
use tools::client_files::Observer;

/// Fast enough to look live, rare enough to cost nothing.
const FRAME: Duration = Duration::from_millis(50);

/// Used when the terminal will not say how wide it is.
const FALLBACK_WIDTH: u16 = 80;

pub struct Bar {
    /// `false` when there is no terminal to draw on, which makes every method
    /// on this type a no-op.
    attached: bool,
    title: &'static str,
    total: usize,
    done: usize,
    current: String,
    started: Instant,
    last_drawn: Option<Instant>,
}

impl Bar {
    /// A bar titled `title`, or a silent stand-in when stdout is not a tty.
    pub fn new(title: &'static str) -> Self {
        Self {
            attached: std::io::stdout().is_terminal(),
            title,
            total: 0,
            done: 0,
            current: String::new(),
            started: Instant::now(),
            last_drawn: None,
        }
    }

    /// The whole status as one string, which the gauge centres over its bar.
    fn label(&self) -> String {
        let elapsed = self.started.elapsed().as_secs_f64();
        let mut label = format!(
            "{}  {}/{}  {elapsed:.0}s",
            self.title, self.done, self.total
        );
        if !self.current.is_empty() {
            label.push_str("  ");
            label.push_str(&self.current);
        }
        label
    }

    /// Render one row and print it over whatever is on the current line.
    fn draw(&mut self, force: bool) {
        if !self.attached {
            return;
        }
        if !force && self.last_drawn.is_some_and(|t| t.elapsed() < FRAME) {
            return;
        }
        let width = terminal::size().map_or(FALLBACK_WIDTH, |(w, _)| w).max(20);
        let area = Rect::new(0, 0, width, 1);
        let mut buf = Buffer::empty(area);
        let ratio = if self.total == 0 {
            0.0
        } else {
            (self.done as f64 / self.total as f64).clamp(0.0, 1.0)
        };
        Gauge::default()
            .gauge_style(Style::default().fg(Color::Cyan).bg(Color::DarkGray))
            .ratio(ratio)
            .label(self.label())
            .render(area, &mut buf);

        let mut out = std::io::stdout().lock();
        let _ = queue!(out, cursor::MoveToColumn(0), Clear(ClearType::CurrentLine));
        for x in 0..width {
            let Some(cell) = buf.cell((x, 0)) else {
                continue;
            };
            let style: ContentStyle = cell.style().into_crossterm();
            let _ = queue!(
                out,
                PrintStyledContent(StyledContent::new(style, cell.symbol()))
            );
        }
        let _ = queue!(out, ResetColor);
        let _ = out.flush();
        self.last_drawn = Some(Instant::now());
    }

    /// Clear the line so whatever prints next starts clean.
    fn erase(&mut self) {
        if !self.attached {
            return;
        }
        let mut out = std::io::stdout().lock();
        let _ = queue!(out, cursor::MoveToColumn(0), Clear(ClearType::CurrentLine));
        let _ = out.flush();
    }
}

impl Drop for Bar {
    fn drop(&mut self) {
        self.erase();
    }
}

impl Observer for Bar {
    fn begin(&mut self, total: usize) {
        self.total = total;
        self.done = 0;
        self.started = Instant::now();
        self.draw(true);
    }

    fn step(&mut self, done: usize, total: usize, file: &str) {
        self.done = done;
        self.total = total;
        self.current = file.to_string();
        self.draw(false);
    }

    fn finish(&mut self) {
        self.done = self.total;
        self.current.clear();
        self.draw(true);
        self.erase();
    }
}
