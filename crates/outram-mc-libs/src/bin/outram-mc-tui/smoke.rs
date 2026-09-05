//! Headless smoke test (op-omf / op-z5q acceptance): drives the app state
//! machine and the `crate::ui` render path with ratatui's [`TestBackend`],
//! since there is no interactive terminal to click through in CI. Runs a
//! tiny, fast transport case (a handful of histories) synchronously — never
//! through the background-thread [`RunHandle`] — so the test asserts
//! immediately instead of polling for a spawned thread to finish.
//!
//! This is a smoke test, not a physics verification: it only asserts the
//! render path doesn't panic and produces the frames a user would expect to
//! see, and that the tiny run returns a finite, positive k-eigenvalue. It is
//! not a substitute for `outram-mc-libs`'s own V&V test suite
//! (`tests/openmc_notebooks/`), which this crate's presets deliberately
//! mirror the atom densities/geometry of.

use ratatui::backend::TestBackend;
use ratatui::Terminal;

use crate::app::{App, Screen};
use crate::presets::{GeometryPreset, SphereVariant};
use crate::settings::RunSettings;
use crate::transport::run_case;
use crate::ui::draw;

/// Settings small enough to run in a few seconds even for this crate's
/// slowest preset (the free-gas-hydrogen LWR cell — see
/// `RunSettings::default`'s doc comment for why that one is markedly slower
/// per history than the others). Deliberately spelled out rather than reused
/// from `RunSettings::default()`, so this stays "a few histories" regardless
/// of what a future change to that default picks.
fn tiny_settings() -> RunSettings {
    RunSettings {
        n_particles: 20,
        n_inactive: 1,
        n_active: 2,
        ..RunSettings::default()
    }
}

/// Renders the geometry-picker screen (the app's initial screen) and checks
/// the frame contains the expected title and at least one preset card title —
/// the baseline "does the touch UI actually draw something sane" check.
#[test]
fn geometry_picker_renders() {
    let backend = TestBackend::new(48, 30);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    let mut app = App::new();

    terminal
        .draw(|frame| draw(frame, &mut app))
        .expect("draw geometry picker");

    let text = buffer_text(terminal.backend().buffer());
    assert!(
        text.contains("OUTRAM-MC"),
        "header missing from geometry picker frame:\n{text}"
    );
    assert!(
        text.contains("Pebble bed") || text.contains("LWR cell"),
        "no preset card visible:\n{text}"
    );
    // At least one tap target was registered (the picker is genuinely touch-driven).
    assert!(
        !app.hit_regions.is_empty(),
        "geometry picker registered no tap targets"
    );
}

/// A tiny, real Godiva-preset transport run (synchronous, not through the
/// background thread), then renders the run screen with the finished result
/// and checks the k-eigenvalue readout and the convergence chart both appear.
#[test]
fn tiny_godiva_run_and_render() {
    let preset = GeometryPreset::BareSphere(SphereVariant::Godiva);
    let settings = tiny_settings();

    let outcome = run_case(preset, settings).expect("tiny Godiva run should succeed");
    assert!(
        outcome.k_mean.is_finite() && outcome.k_mean > 0.0,
        "k_mean not finite/positive: {}",
        outcome.k_mean
    );
    assert_eq!(
        outcome.k_by_generation.len(),
        settings.n_inactive + settings.n_active,
        "ran all generations"
    );
    assert!(
        outcome.spectrum.is_some(),
        "bare-sphere preset (CSG-backed) should carry a spectrum overlay"
    );

    let backend = TestBackend::new(48, 30);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    let mut app = App::new();
    app.selected_preset = preset;
    app.settings = settings;
    app.screen = Screen::Run;
    app.revealed = outcome.k_by_generation.len();
    app.last_outcome = Some(Ok(outcome));

    terminal
        .draw(|frame| draw(frame, &mut app))
        .expect("draw run screen");
    let text = buffer_text(terminal.backend().buffer());
    assert!(
        text.contains("k_mean"),
        "run screen missing k_mean readout:\n{text}"
    );
    assert!(
        text.contains("convergence"),
        "run screen missing the convergence chart block:\n{text}"
    );
}

/// Renders the spectrum screen after a tiny LWR-cell run (the other
/// CSG-backed, spectrum-capable preset) and checks the overlay caption shows.
#[test]
fn spectrum_screen_renders_after_lwr_run() {
    let preset = GeometryPreset::LwrCell;
    let settings = tiny_settings();
    let outcome = run_case(preset, settings).expect("tiny LWR-cell run should succeed");
    assert!(
        outcome.spectrum.is_some(),
        "LWR cell preset should carry a spectrum overlay"
    );

    let backend = TestBackend::new(48, 30);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    let mut app = App::new();
    app.selected_preset = preset;
    app.settings = settings;
    app.screen = Screen::Spectrum;
    app.last_outcome = Some(Ok(outcome));

    terminal
        .draw(|frame| draw(frame, &mut app))
        .expect("draw spectrum screen");
    let text = buffer_text(terminal.backend().buffer());
    assert!(
        text.contains("flux") && text.contains("Sigma_t"),
        "spectrum overlay caption missing:\n{text}"
    );
}

/// A pebble-bed preset (delta-tracking driver, no tally hook) should honestly
/// report no spectrum overlay rather than a fabricated one.
#[test]
fn pebble_bed_has_no_spectrum_overlay() {
    let preset = GeometryPreset::PebbleBed;
    assert!(!preset.supports_spectrum());
    let settings = tiny_settings();
    let outcome = run_case(preset, settings).expect("tiny pebble-bed run should succeed");
    assert!(
        outcome.spectrum.is_none(),
        "pebble bed should not carry a spectrum overlay in this crate version"
    );
}

fn buffer_text(buffer: &ratatui::buffer::Buffer) -> String {
    buffer.content().iter().map(|cell| cell.symbol()).collect()
}

/// Every preset (all 5 concrete cases, including both bare-sphere isotope
/// variants) builds and runs to a finite, positive k with a tiny history
/// count — a gap-check that a CORE-library nuclide-name typo in any preset
/// (e.g. the Jezebel Pu/Ga composition, or the FLiBe-salt TMSR matrix) would
/// have caught immediately. Not a benchmark accuracy check (see each preset's
/// `blurb()` for documented approximations) — just "does it build and run".
#[test]
fn every_preset_builds_and_runs() {
    let settings = tiny_settings();
    for preset in [
        GeometryPreset::PebbleBed,
        GeometryPreset::LwrCell,
        GeometryPreset::TmsrPebbleBed,
        GeometryPreset::BareSphere(SphereVariant::Godiva),
        GeometryPreset::BareSphere(SphereVariant::Jezebel),
    ] {
        let outcome = run_case(preset, settings)
            .unwrap_or_else(|e| panic!("{} failed to build/run: {e}", preset.title()));
        assert!(
            outcome.k_mean.is_finite() && outcome.k_mean > 0.0,
            "{}: k_mean not finite/positive: {}",
            preset.title(),
            outcome.k_mean
        );
        eprintln!(
            "[every_preset_builds_and_runs] {}: k = {:.4} +/- {:.4}",
            preset.title(),
            outcome.k_mean,
            outcome.k_std
        );
    }
}
