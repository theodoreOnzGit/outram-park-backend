//! Rendering for the Ingest tab — one `draw_*` function per
//! [`IngestPhase`] variant.
//!
//! Kept separate from the state machine in `mod.rs` so the reducer stays
//! testable without a terminal (the workspace's existing `kovan-tui` testing
//! approach) and neither file grows past the workspace file-size cap.

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Wrap};
use ratatui::Frame;

use super::review::{ReviewField, ReviewState};
use super::{FailureReport, IngestPhase, IngestState, PickerField, RunningJob};

/// Rows of the review form, in display order. Mirrors the navigation order in
/// [`ReviewField::step`].
const REVIEW_ROWS: [ReviewField; 8] = [
    ReviewField::Title,
    ReviewField::Authors,
    ReviewField::Year,
    ReviewField::DocType,
    ReviewField::Institution,
    ReviewField::MarkdownOut,
    ReviewField::JsonOut,
    ReviewField::BibtexOut,
];

/// Render the Ingest tab into `area`.
///
/// `editing` is [`super::super::App`]'s shared edit-mode flag; it only affects
/// how the focused field is highlighted.
pub fn draw(frame: &mut Frame, area: Rect, state: &mut IngestState, editing: bool) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(area);

    frame.render_widget(Paragraph::new(state.status.as_str()), chunks[0]);

    // Split borrows: the status line above is already rendered, so the phase can
    // be borrowed on its own here.
    match &mut state.phase {
        IngestPhase::Picking => draw_picker(
            frame,
            chunks[1],
            state.root.value(),
            state.filter.value(),
            state.picker_field,
            editing,
            &state.candidates,
            &mut state.list_state,
        ),
        IngestPhase::Running(job) => draw_running(frame, chunks[1], job),
        IngestPhase::Review(review) => draw_review(frame, chunks[1], review, editing),
        IngestPhase::Failed(failure) => draw_failed(frame, chunks[1], failure),
    }
}

/// The PDF picker: root directory, substring filter, and the discovered files.
#[allow(clippy::too_many_arguments)] // flat scalars beat a struct that exists only to satisfy a lint
fn draw_picker(
    frame: &mut Frame,
    area: Rect,
    root: &str,
    filter: &str,
    field: PickerField,
    editing: bool,
    candidates: &[std::path::PathBuf],
    list_state: &mut ratatui::widgets::ListState,
) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(0),
        ])
        .split(area);

    let focus_style = |focused: bool| {
        if focused && editing {
            Style::default().add_modifier(Modifier::REVERSED)
        } else {
            Style::default()
        }
    };

    frame.render_widget(
        Paragraph::new(root)
            .style(focus_style(field == PickerField::Root))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Directory to search ('e' to edit)"),
            ),
        rows[0],
    );
    frame.render_widget(
        Paragraph::new(filter)
            .style(focus_style(field == PickerField::Filter))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Filename filter ('f' to edit, blank = all)"),
            ),
        rows[1],
    );

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(rows[2]);

    let items: Vec<ListItem> = candidates
        .iter()
        .map(|p| ListItem::new(p.display().to_string()))
        .collect();
    frame.render_stateful_widget(
        List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("PDFs ('r' to rescan, Enter to import)"),
            )
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED)),
        cols[0],
        list_state,
    );

    frame.render_widget(
        Paragraph::new(
            "Import runs kovan-literature's PDF → Markdown → KovanDocument \
             pipeline — the same call `kovan lit import` makes.\n\n\
             Extraction runs on a worker thread, so the UI stays responsive; a \
             large scanned report can take tens of seconds.\n\n\
             Afterwards you review and correct the extracted metadata (title, \
             authors, year, type) before anything is written to disk.",
        )
        .wrap(Wrap { trim: false })
        .block(Block::default().borders(Borders::ALL).title("What happens")),
        cols[1],
    );
}

/// The progress screen. Shows elapsed time and a spinner — deliberately **not**
/// a percentage: `kovan-literature` exposes no progress callback, so any
/// percentage would be invented.
fn draw_running(frame: &mut Frame, area: Rect, job: &RunningJob) {
    let elapsed = job.started.elapsed().as_secs_f64();
    let size = if job.bytes == 0 {
        "unknown size".to_string()
    } else {
        format!("{:.1} MB", job.bytes as f64 / (1024.0 * 1024.0))
    };
    let text = format!(
        "{}  extracting…\n\n\
         file:     {}\n\
         size:     {}\n\
         elapsed:  {:.1} s\n\n\
         Text extraction and metadata parsing run in one library call \
         (kovan_literature::extract_metadata) on a worker thread. It reports no \
         intermediate progress, so this screen shows elapsed time rather than a \
         made-up percentage.\n\n\
         'x' abandons the wait (the worker cannot be interrupted; it finishes in \
         the background and its result is discarded).",
        job.spinner(),
        job.pdf.display(),
        size,
        elapsed,
    );
    frame.render_widget(
        Paragraph::new(text).wrap(Wrap { trim: false }).block(
            Block::default()
                .borders(Borders::ALL)
                .title("Importing (running)"),
        ),
        area,
    );
}

/// The review form (left) beside the derived record and save report (right).
fn draw_review(frame: &mut Frame, area: Rect, review: &ReviewState, editing: bool) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(area);

    let left = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(10), Constraint::Min(0)])
        .split(cols[0]);

    let lines: Vec<Line> = REVIEW_ROWS
        .iter()
        .map(|field| {
            let focused = *field == review.field;
            let marker = if review.is_edited(*field) { "*" } else { " " };
            let style = if focused {
                if editing {
                    Style::default().add_modifier(Modifier::REVERSED)
                } else {
                    Style::default().add_modifier(Modifier::BOLD)
                }
            } else {
                Style::default()
            };
            // Non-text rows are chosen, not typed — say so on the row itself.
            let hint = if field.is_text() {
                ""
            } else {
                "  (Left/Right)"
            };
            Line::from(vec![Span::styled(
                format!(
                    "{marker}{:<14} {}{hint}",
                    field.label(),
                    review.field_value(*field)
                ),
                style,
            )])
        })
        .collect();
    frame.render_widget(
        Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .title("Review metadata ('e' edit, Up/Down move, '*' = changed)"),
        ),
        left[0],
    );

    let advisories = review.advisories();
    let advisory_text = if advisories.is_empty() {
        "No advisories — extraction looks self-consistent. Check it against the \
         source anyway: extraction is best-effort."
            .to_string()
    } else {
        advisories
            .iter()
            .map(|a| format!("- {a}"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    frame.render_widget(
        Paragraph::new(advisory_text)
            .wrap(Wrap { trim: false })
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Check these (advisory only — nothing is auto-changed)"),
            ),
        left[1],
    );

    let right = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(8)])
        .split(cols[1]);

    frame.render_widget(
        Paragraph::new(derived_record_text(review))
            .wrap(Wrap { trim: false })
            .scroll((review.preview_scroll, 0))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Record to be saved (PgUp/PgDn)"),
            ),
        right[0],
    );

    let report = if review.save_report.is_empty() {
        "Press 's' to write the paths above. 'x' discards this import without \
         writing anything."
            .to_string()
    } else {
        review.save_report.join("\n")
    };
    frame.render_widget(
        Paragraph::new(report)
            .wrap(Wrap { trim: false })
            .block(Block::default().borders(Borders::ALL).title("Save report")),
        right[1],
    );
}

/// The text of the "record to be saved" pane: provenance, the derived
/// identifiers, and the BibTeX that would be generated.
fn derived_record_text(review: &ReviewState) -> String {
    let mut s = String::new();
    s.push_str(&format!("source:     {}\n", review.source_pdf.display()));
    s.push_str(&format!(
        "extracted:  {:.1} s\n",
        review.elapsed.as_secs_f64()
    ));
    s.push_str(&format!("visibility: {:?}\n", review.visibility()));
    s.push_str(&format!(
        "pages:      {}\n",
        review
            .extracted
            .page_count
            .map(|p| p.to_string())
            .unwrap_or_else(|| "unknown".to_string())
    ));
    s.push_str(&format!(
        "markdown:   {} chars / {} lines\n",
        review.extracted.markdown_body.chars().count(),
        review.extracted.markdown_body.lines().count()
    ));
    if let Some(doi) = &review.extracted.doi {
        s.push_str(&format!("doi:        {doi}\n"));
    }
    s.push('\n');

    match review.corrected_document() {
        Ok(doc) => {
            s.push_str(&format!("slug:       {}\n", doc.slug));
            s.push_str(&format!("id:         {}\n", doc.id));
            if doc.slug != review.extracted.slug {
                s.push_str(&format!(
                    "            (extractor said '{}')\n",
                    review.extracted.slug
                ));
            }
            s.push_str("\n--- BibTeX ---\n");
            s.push_str(&kovan_literature::to_bibtex(&doc));
        }
        Err(problems) => {
            s.push_str("cannot build the record yet:\n");
            for p in problems {
                s.push_str(&format!("- {p}\n"));
            }
        }
    }
    s
}

/// The failure screen. Extraction errors are shown here rather than aborting the
/// program, so the terminal is never left in raw mode by an ingestion problem.
fn draw_failed(frame: &mut Frame, area: Rect, failure: &FailureReport) {
    let text = format!(
        "Import failed.\n\n\
         file:  {}\n\
         error: {}\n\n\
         Nothing was written. Press 'x' or Enter to go back to the picker.",
        failure.pdf.display(),
        failure.message
    );
    frame.render_widget(
        Paragraph::new(text).wrap(Wrap { trim: false }).block(
            Block::default()
                .borders(Borders::ALL)
                .title("Import failed"),
        ),
        area,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use kovan_common::{DocumentType, KovanDocument, Visibility};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use std::path::PathBuf;
    use std::time::Duration;

    fn rendered(state: &mut IngestState) -> String {
        let backend = TestBackend::new(120, 34);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|f| draw(f, f.area(), state, false))
            .expect("draw must not panic");
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect()
    }

    fn review_state() -> IngestState {
        let mut doc = KovanDocument::new(
            "kovan-1",
            "2004anl7416",
            Visibility::Open,
            DocumentType::Other,
            "ANL-7416 Supplement 2",
        );
        doc.year = Some(2004);
        doc.page_count = Some(447);
        doc.markdown_body = "Argonne Code Center. June 1977.".to_string();
        IngestState {
            phase: IngestPhase::Review(ReviewState::new(
                PathBuf::from("/tmp/anl-7416.pdf"),
                doc,
                Duration::from_secs(41),
            )),
            ..Default::default()
        }
    }

    #[test]
    fn picker_renders_its_fields_and_guidance() {
        let mut state = IngestState::default();
        let text = rendered(&mut state);
        assert!(text.contains("Directory to search"));
        assert!(text.contains("Filename filter"));
        assert!(text.contains("PDFs"));
    }

    #[test]
    fn running_screen_shows_elapsed_time_and_no_fake_percentage() {
        let dir = tempfile::tempdir().expect("tempdir");
        let pdf = dir.path().join("x.pdf");
        std::fs::write(&pdf, b"not a pdf").unwrap();
        let mut state = IngestState::default();
        state.start_extraction(pdf);
        let text = rendered(&mut state);
        assert!(text.contains("extracting"));
        assert!(text.contains("elapsed"));
        assert!(!text.contains('%'), "no invented percentage may be shown");
        // Let the worker finish so it does not outlive the temp directory.
        while !state.tick() {
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn review_screen_shows_the_form_advisories_and_bibtex() {
        let mut state = review_state();
        let text = rendered(&mut state);
        assert!(text.contains("Review metadata"));
        assert!(text.contains("ANL-7416"));
        assert!(text.contains("digitisation"), "advisory must be visible");
        assert!(text.contains("@misc"), "BibTeX preview must be visible");
        assert!(text.contains("447"), "page count must be visible");
    }

    #[test]
    fn review_screen_shows_a_validation_problem_instead_of_the_bibtex() {
        let mut state = review_state();
        if let IngestPhase::Review(review) = &mut state.phase {
            review.year.set("not-a-year");
        }
        let text = rendered(&mut state);
        assert!(text.contains("cannot build the record yet"));
    }

    #[test]
    fn failed_screen_shows_the_error_message() {
        let mut state = IngestState {
            phase: IngestPhase::Failed(FailureReport {
                pdf: PathBuf::from("/tmp/broken.pdf"),
                message: "io error: not a PDF".to_string(),
            }),
            ..Default::default()
        };
        let text = rendered(&mut state);
        assert!(text.contains("Import failed"));
        assert!(text.contains("not a PDF"));
    }
}
