//! `kovan-digitise-tui` — hybrid graph digitiser (ratatui terminal review).
//!
//! **Automatic pass first, then human verification — and the verification is
//! recorded, never assumed.** `auto` mode runs exactly the same pipeline as
//! the `kovan-digitise` CLI (same flags, via
//! [`kovan::digitiser::frontend::AutoArgs`]), then drops into a
//! terminal review screen; `review` mode reopens a previously saved dataset
//! JSON. In the review screen the operator steps through the traced points
//! overlaid on a half-block rendering of the plot image, nudges or deletes
//! wrong ones (each edit is recorded per point as `HandCorrected` /
//! `HandPlaced` with the operator's name), and finally marks the dataset
//! reviewed — which stamps `ReviewStatus::Reviewed { by, at, interface: Tui }`
//! into the saved record.
//!
//! Keys: `Tab`/`Left`/`Right` select point · arrows after `e` (edit mode
//! toggles nudge) — see the on-screen help line for the full set · `d` delete
//! · `a` duplicate-as-hand-placed · `v` mark reviewed · `S` save · `q` quit.
//!
//! Terminal cells are coarse (each cell is 1×2 image pixels at best, usually
//! far less), so this reviewer is for *sanity checking and coarse fixes* on a
//! headless box or over SSH — Termux included; fine-grained editing belongs
//! to `kovan-digitise-gui`. Point-adding is limited to duplicating an
//! existing point and nudging it; free placement needs the GUI's mouse.
//!
//! This binary is Android/Termux-buildable by design (plain ratatui, no GUI
//! stack) — a terminal app is in scope for the workspace Android rule.

use clap::Parser;
use kovan::digitiser::dataset::{
    uncertainty_interval, utc_now_iso8601, DigitisedDataset, PointOrigin, ReviewInterface,
    ReviewStatus,
};
use kovan::digitiser::frontend::AutoArgs;
use kovan::digitiser::raster::PlotRaster;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Paragraph};

/// Hybrid digitiser: automatic pass, then recorded human review in the
/// terminal.
#[derive(Debug, Parser)]
#[command(
    name = "kovan-digitise-tui",
    version,
    about = "Hybrid graph digitiser: automatic pass, then recorded human review in the terminal"
)]
enum Cli {
    /// Run the automatic pipeline (same flags as `kovan-digitise`), then
    /// review the result interactively.
    Auto {
        #[command(flatten)]
        auto: AutoArgs,
        /// Where to save the reviewed dataset JSON (default:
        /// `<image>.digitised.json`).
        #[arg(long)]
        json: Option<String>,
        /// Also save CSV here on `S`.
        #[arg(long)]
        csv: Option<String>,
    },
    /// Reopen a previously saved dataset for (re-)review.
    Review {
        /// Dataset JSON written by `kovan-digitise` or a previous session.
        #[arg(long)]
        dataset: String,
        /// Image path override (defaults to the dataset's recorded
        /// `image_path`).
        #[arg(long)]
        image: Option<String>,
        /// Reviewer name recorded on edits and on `v`.
        #[arg(long, default_value = "unnamed reviewer")]
        operator: String,
        /// Save path (default: overwrite `--dataset`).
        #[arg(long)]
        json: Option<String>,
        /// Also save CSV here on `S`.
        #[arg(long)]
        csv: Option<String>,
    },
}

fn main() -> std::process::ExitCode {
    let (raster, dataset, operator, json_path, csv_path) = match Cli::parse() {
        Cli::Auto { auto, json, csv } => match auto.run() {
            Ok((raster, dataset)) => {
                let json = json.unwrap_or_else(|| format!("{}.digitised.json", auto.image));
                (raster, dataset, auto.operator.clone(), json, csv)
            }
            Err(e) => {
                eprintln!("kovan-digitise-tui: {e}");
                return std::process::ExitCode::FAILURE;
            }
        },
        Cli::Review {
            dataset,
            image,
            operator,
            json,
            csv,
        } => {
            let loaded = match DigitisedDataset::read_json(std::path::Path::new(&dataset)) {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("kovan-digitise-tui: {e}");
                    return std::process::ExitCode::FAILURE;
                }
            };
            let image_path = image
                .or_else(|| loaded.source.image_path.clone())
                .unwrap_or_default();
            if image_path.is_empty() {
                eprintln!(
                    "kovan-digitise-tui: dataset records no image_path; pass --image explicitly"
                );
                return std::process::ExitCode::FAILURE;
            }
            let raster = match PlotRaster::from_path(std::path::Path::new(&image_path)) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("kovan-digitise-tui: {e}");
                    return std::process::ExitCode::FAILURE;
                }
            };
            let json = json.unwrap_or(dataset);
            (raster, loaded, operator, json, csv)
        }
    };

    let mut app = App {
        raster,
        dataset,
        operator,
        json_path,
        csv_path,
        selected: 0,
        dirty: false,
        message: "automatic pass loaded — verify the overlay, then `v` + `S`".to_string(),
    };

    let mut terminal = match ratatui::try_init() {
        Ok(t) => t,
        Err(e) => {
            eprintln!("kovan-digitise-tui: cannot initialise terminal: {e}");
            return std::process::ExitCode::FAILURE;
        }
    };
    let result = app.run(&mut terminal);
    ratatui::restore();
    match result {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("kovan-digitise-tui: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}

/// All mutable review-session state. Owned by value — no shared state, no
/// lifetimes, per the workspace design rules.
struct App {
    raster: PlotRaster,
    dataset: DigitisedDataset,
    operator: String,
    json_path: String,
    csv_path: Option<String>,
    selected: usize,
    dirty: bool,
    message: String,
}

impl App {
    fn run(&mut self, terminal: &mut ratatui::DefaultTerminal) -> Result<(), String> {
        loop {
            terminal
                .draw(|f| self.draw(f))
                .map_err(|e| format!("draw failed: {e}"))?;
            if !event::poll(std::time::Duration::from_millis(200))
                .map_err(|e| format!("event poll failed: {e}"))?
            {
                continue;
            }
            let Event::Key(key) = event::read().map_err(|e| format!("event read failed: {e}"))?
            else {
                continue;
            };
            if key.kind != KeyEventKind::Press {
                continue;
            }
            let step = if key.modifiers.contains(KeyModifiers::SHIFT) {
                5.0
            } else {
                1.0
            };
            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                KeyCode::Right | KeyCode::Tab => self.select(1),
                KeyCode::Left | KeyCode::BackTab => self.select(-1),
                KeyCode::Up => self.nudge(0.0, -step),
                KeyCode::Down => self.nudge(0.0, step),
                KeyCode::Char('h') => self.nudge(-step, 0.0),
                KeyCode::Char('l') => self.nudge(step, 0.0),
                KeyCode::Char('d') => self.delete_selected(),
                KeyCode::Char('a') => self.duplicate_selected(),
                KeyCode::Char('v') => {
                    self.dataset.record_review(
                        self.operator.clone(),
                        utc_now_iso8601(),
                        ReviewInterface::Tui,
                    );
                    self.dirty = true;
                    self.message = format!("marked reviewed by {} — save with S", self.operator);
                }
                KeyCode::Char('S') | KeyCode::Char('s') => self.save(),
                _ => {}
            }
        }
    }

    fn select(&mut self, delta: i64) {
        let n = self.dataset.points.len();
        if n == 0 {
            return;
        }
        self.selected = (self.selected as i64 + delta).rem_euclid(n as i64) as usize;
    }

    /// Move the selected point by whole image pixels and recompute its data
    /// coordinates + reading uncertainty through the dataset's calibration.
    /// Records the edit as `HandCorrected` by the operator (or keeps
    /// `HandPlaced` if it was placed by hand in the first place).
    fn nudge(&mut self, dx: f64, dy: f64) {
        let cal = self.dataset.calibration;
        let Some(p) = self.dataset.points.get_mut(self.selected) else {
            return;
        };
        let x_px = p.x_px.unwrap_or_else(|| cal.x.pixel_at(p.x).unwrap_or(0.0)) + dx;
        let y_px = p.y_px.unwrap_or_else(|| cal.y.pixel_at(p.y).unwrap_or(0.0)) + dy;
        p.x_px = Some(x_px);
        p.y_px = Some(y_px);
        (p.x, p.y) = cal.point_at(x_px, y_px);
        // Hand precision: half-pixel reading error on both axes.
        (p.x_minus, p.x_plus) = uncertainty_interval(&cal.x, x_px, 0.5);
        (p.y_minus, p.y_plus) = uncertainty_interval(&cal.y, y_px, 0.5);
        if !matches!(p.origin, PointOrigin::HandPlaced { .. }) {
            p.origin = PointOrigin::HandCorrected {
                by: self.operator.clone(),
            };
        }
        self.mark_edited();
    }

    fn delete_selected(&mut self) {
        if self.dataset.points.is_empty() {
            return;
        }
        self.dataset.points.remove(self.selected);
        if self.selected >= self.dataset.points.len() && self.selected > 0 {
            self.selected -= 1;
        }
        self.mark_edited();
        self.message = "point deleted".to_string();
    }

    /// Insert a `HandPlaced` copy of the selected point right after it (then
    /// nudge it into place). The TUI's substitute for mouse placement.
    fn duplicate_selected(&mut self) {
        let Some(p) = self.dataset.points.get(self.selected) else {
            return;
        };
        let mut q = p.clone();
        q.origin = PointOrigin::HandPlaced {
            by: self.operator.clone(),
        };
        self.dataset.points.insert(self.selected + 1, q);
        self.selected += 1;
        self.mark_edited();
        self.message = "hand-placed copy inserted — nudge it into position".to_string();
    }

    /// Any edit invalidates a previously recorded review: reviewing covers
    /// the points as they were, so the status drops back to Unreviewed.
    fn mark_edited(&mut self) {
        if matches!(self.dataset.review, ReviewStatus::Reviewed { .. }) {
            self.dataset.review = ReviewStatus::Unreviewed;
            self.message = "edited after review — status reset to UNREVIEWED".to_string();
        }
        self.dirty = true;
    }

    fn save(&mut self) {
        if let Err(e) = self
            .dataset
            .write_json(std::path::Path::new(&self.json_path))
        {
            self.message = format!("save failed: {e}");
            return;
        }
        if let Some(csv) = &self.csv_path {
            if let Err(e) = self.dataset.write_csv(std::path::Path::new(csv)) {
                self.message = format!("json saved, csv failed: {e}");
                return;
            }
        }
        self.dirty = false;
        self.message = format!("saved to {}", self.json_path);
    }

    fn draw(&self, f: &mut ratatui::Frame<'_>) {
        let [image_area, status_area] =
            Layout::vertical([Constraint::Min(5), Constraint::Length(5)]).areas(f.area());

        // --- image as half-blocks, nearest-neighbour fit ---
        let block = Block::default()
            .borders(Borders::ALL)
            .title(format!(" {} ", self.dataset.source.figure));
        let inner = block.inner(image_area);
        f.render_widget(block, image_area);
        let (iw, ih) = (self.raster.width(), self.raster.height());
        if inner.width > 0 && inner.height > 0 && iw > 0 && ih > 0 {
            let buf = f.buffer_mut();
            for cy in 0..inner.height {
                for cx in 0..inner.width {
                    let sample = |sub: u32| -> Color {
                        let px = (cx as u64 * iw as u64 / inner.width as u64) as u32;
                        let py = (((cy as u64 * 2 + sub as u64) * ih as u64)
                            / (inner.height as u64 * 2)) as u32;
                        let [r, g, b] = self.raster.rgb(px.min(iw - 1), py.min(ih - 1));
                        Color::Rgb(r, g, b)
                    };
                    let cell = &mut buf[(inner.x + cx, inner.y + cy)];
                    cell.set_symbol("▀");
                    cell.set_fg(sample(0));
                    cell.set_bg(sample(1));
                }
            }
            // --- point overlay ---
            for (i, p) in self.dataset.points.iter().enumerate() {
                let (Some(x_px), Some(y_px)) = (p.x_px, p.y_px) else {
                    continue;
                };
                let cx = (x_px * inner.width as f64 / iw as f64) as u16;
                let cy = (y_px * inner.height as f64 / ih as f64) as u16;
                if cx < inner.width && cy < inner.height {
                    let cell = &mut buf[(inner.x + cx, inner.y + cy)];
                    if i == self.selected {
                        cell.set_symbol("X");
                        cell.set_fg(Color::Yellow);
                        cell.set_bg(Color::Black);
                    } else {
                        cell.set_symbol("o");
                        cell.set_fg(Color::Red);
                    }
                }
            }
        }

        // --- status + help ---
        let review = match &self.dataset.review {
            ReviewStatus::Unreviewed => "UNREVIEWED".to_string(),
            ReviewStatus::Reviewed { by, at, .. } => format!("reviewed by {by} at {at}"),
        };
        let point_line = match self.dataset.points.get(self.selected) {
            Some(p) => format!(
                "point {}/{}: x = {:.6e} (-{:.1e}/+{:.1e})  y = {:.6e} (-{:.1e}/+{:.1e})  [{}]",
                self.selected + 1,
                self.dataset.points.len(),
                p.x,
                p.x_minus,
                p.x_plus,
                p.y,
                p.y_minus,
                p.y_plus,
                match &p.origin {
                    PointOrigin::AutoTraced => "auto".to_string(),
                    PointOrigin::HandPlaced { by } => format!("hand-placed by {by}"),
                    PointOrigin::HandCorrected { by } => format!("hand-corrected by {by}"),
                },
            ),
            None => "no points".to_string(),
        };
        let status = Paragraph::new(vec![
            Line::from(point_line),
            Line::from(format!(
                "{review}{}   save: {}",
                if self.dirty { "  [unsaved]" } else { "" },
                self.json_path
            )),
            Line::from(self.message.clone()),
            Line::from(
                "Tab/←→ select · ↑↓ nudge y · h/l nudge x (Shift=5px) · d delete · \
                 a duplicate · v mark reviewed · S save · q quit",
            ),
        ])
        .style(Style::default())
        .block(Block::default().borders(Borders::ALL).title(" review "));
        f.render_widget(status, status_area);
    }
}
