//! Headless smoke tests using ratatui's [`TestBackend`].
//!
//! There is no way to drive this app's touchscreen/mouse loop interactively
//! from an automated test, so these instead render a real frame — through
//! the exact same `draw` function `main`'s event loop calls — into an
//! in-memory buffer and assert the expected screens actually put content on
//! it, at terminal sizes representative of a phone (narrow width) and a
//! desktop terminal. This is the harness-mandated smoke test for a TUI that
//! cannot otherwise be exercised end to end.

use ratatui::backend::TestBackend;
use ratatui::Terminal;

use crate::app::{App, Screen};

/// Join a rendered buffer's cells back into plain text, one line per
/// terminal row, so assertions can do a simple substring check instead of an
/// exact (and therefore brittle, given dynamic list/plot content) line-by-line
/// match against `assert_buffer_lines`.
fn buffer_text(buf: &ratatui::buffer::Buffer) -> String {
    let width = buf.area.width as usize;
    buf.content
        .chunks(width.max(1))
        .map(|row| row.iter().map(|c| c.symbol()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn finder_screen_renders_a_title_and_at_least_one_nuclide_row() {
    // 32x24 stands in for a narrow phone-terminal font/size, per op-omf.
    let backend = TestBackend::new(32, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    let app = App::new();

    let completed = terminal.draw(|f| crate::draw(f, &app)).unwrap();
    let text = buffer_text(completed.buffer);

    assert!(text.contains("njoy-tui"), "finder title missing:\n{text}");
    // The unfiltered list starts at H-1 (Z=1, A=1) — the catalog's first
    // entry in ascending (Z, A) order (see `NuclideCatalog::build`).
    assert!(
        text.contains("H-1"),
        "expected the first catalog row (H-1) visible:\n{text}"
    );
}

#[test]
fn plot_screen_renders_nuclide_name_back_button_and_a_chart() {
    let backend = TestBackend::new(40, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = App::new();
    app.open_nuclide("U238");
    if let Screen::Plot(state) = &mut app.screen {
        state.refresh();
    }
    assert!(
        matches!(app.screen, Screen::Plot(_)),
        "open_nuclide(\"U238\") did not navigate"
    );

    let completed = terminal.draw(|f| crate::draw(f, &app)).unwrap();
    let text = buffer_text(completed.buffer);

    assert!(text.contains("back"), "back button missing:\n{text}");
    assert!(
        text.contains("U238"),
        "nuclide name missing from plot screen:\n{text}"
    );
    assert!(
        text.contains("total"),
        "MT selector (default: total) missing:\n{text}"
    );
    // A braille-marker line dataset draws using Unicode Braille Pattern
    // codepoints (U+2800-U+28FF); at least one should appear in the chart
    // area for a nuclide/channel with nonzero cross section over the range.
    assert!(
        text.chars().any(|c| ('\u{2800}'..='\u{28ff}').contains(&c)),
        "expected at least one braille dot in the rendered chart:\n{text}"
    );
}

#[test]
fn plot_screen_still_renders_without_panicking_on_a_very_narrow_terminal() {
    // Narrow enough to force the MT-selector 2-row wrap (op-omf's
    // narrow-screen layout adaptation, see `ui::mt_button_rects`).
    let backend = TestBackend::new(20, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = App::new();
    app.open_nuclide("H1");
    if let Screen::Plot(state) = &mut app.screen {
        state.refresh();
    }

    let completed = terminal.draw(|f| crate::draw(f, &app)).unwrap();
    let text = buffer_text(completed.buffer);
    assert!(
        text.chars().any(|c| c != ' ' && c != '\n'),
        "narrow plot screen rendered nothing:\n{text}"
    );
}

#[test]
fn finder_search_narrows_the_rendered_list() {
    let backend = TestBackend::new(32, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = App::new();
    if let Screen::Finder(state) = &mut app.screen {
        state.query = "u235".to_string();
    }

    let completed = terminal.draw(|f| crate::draw(f, &app)).unwrap();
    let text = buffer_text(completed.buffer);
    assert!(
        text.contains("U-235"),
        "typed query \"u235\" should surface U-235:\n{text}"
    );
}
