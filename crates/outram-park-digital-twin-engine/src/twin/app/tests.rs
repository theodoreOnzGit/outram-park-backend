use super::*;
use crate::twin::{CvRole, KeffResult};
use uom::si::f64::{Mass, MassRate, Power, ThermodynamicTemperature};
use uom::si::thermodynamic_temperature::kelvin;

/// The page text: CV and link rows (a missing value is "—", never a
/// number), the Monte Carlo lines in each state, and "not built".
#[test]
fn the_page_text_is_what_the_model_has() {
    let v = ControlVolume {
        id: "core".into(),
        name: "Core".into(),
        medium: "fuel".into(),
        role: CvRole::Balance,
        volume: None,
        temperature: Some(ThermodynamicTemperature::new::<kelvin>(1000.0)),
        mass: None,
        on_pressure_boundary: true,
    };
    assert_eq!(cv_row(&v), ["core", "Core", "balance", "726.9", "—", "yes"].map(String::from));
    let flow = CvLink { from: "a".into(), to: "b".into(), kind: CvLinkKind::Flow(MassRate::new::<kilogram_per_second>(5.0)) };
    assert_eq!(link_row(&flow), ["a", "b", "flow", "5.00 kg/s"].map(String::from));
    let heat = CvLink { from: "a".into(), to: "b".into(), kind: CvLinkKind::Heat(Some(Power::new::<kilowatt>(2.5))) };
    assert_eq!(link_row(&heat)[3], "2.5 kW");
    let unknown = CvLink { kind: CvLinkKind::Heat(None), ..heat };
    assert_eq!(link_row(&unknown)[3], "—");
    let with_mass = ControlVolume { mass: Some(Mass::new::<kilogram>(12.34)), on_pressure_boundary: false, ..v };
    assert_eq!(cv_row(&with_mass)[4..], ["12.3".to_string(), "no".to_string()]);

    let mut mc = McPageView {
        runs: vec![McRunView { label: "A".into(), result: Some(KeffResult { k: 1.0, sigma: 0.001 }) }, McRunView { label: "C".into(), result: None }],
        running: true,
        live: Vec::new(),
        in_use: "cited: x".into(),
        error: None,
    };
    assert_eq!(
        mc_lines(&mc),
        ["In use: cited: x", "A: k_eff = 1.00000 ± 0.00100", "C: running…", "No live set yet: run the cases, then send the result to the plant."]
    );
    mc.running = false;
    mc.live = vec!["fuel -1 /K".into()];
    mc.error = Some("boom".into());
    let lines = mc_lines(&mc);
    assert_eq!(lines[2], "C: not run");
    assert_eq!(lines[3..], ["Live set (not in use until sent):", "fuel -1 /K", "Failed: boom"]);
    assert!(not_built(TwinPage::MonteCarlo, 1313).starts_with("Monte Carlo, full core is not built for this twin yet. Tracked in gh:#1313."));
}

/// The trend window zooms by halves and doubles inside its bounds and
/// resets; the points are the last window of history.
#[test]
fn the_trend_window_zooms_and_slices() {
    let mut w = TrendWindow::default();
    w.apply(Zoom::In);
    assert_eq!(w.seconds, 300.0);
    for _ in 0..20 {
        w.apply(Zoom::In);
    }
    assert_eq!(w.seconds, TrendWindow::MIN_S);
    for _ in 0..40 {
        w.apply(Zoom::Out);
    }
    assert_eq!(w.seconds, TrendWindow::MAX_S);
    w.apply(Zoom::Reset);
    assert_eq!(w, TrendWindow::default());

    let h: Vec<Vec<f64>> = (0..100).map(|i| vec![i as f64, 2.0 * i as f64]).collect();
    let p = trend_points(&h, 1, 10.0);
    assert_eq!(p.first(), Some(&[89.0, 178.0]));
    assert_eq!(p.len(), 11);
    assert!(trend_points(&h, 5, 10.0).is_empty());
    assert!(trend_points(&[], 1, 10.0).is_empty());
}

/// A steady value is not magnified into its round-off; a real change is
/// shown as it is.
#[test]
fn the_trend_range_does_not_magnify_round_off() {
    let steady = [[0.0, 8.0], [1.0, 8.0 + 1.5e-12]];
    let (lo, hi) = y_range(&steady).expect("range");
    assert!((hi - lo - 8e-3).abs() < 1e-12 && lo < 8.0 && hi > 8.0, "{lo} {hi}");
    assert_eq!(y_range(&[[0.0, 5.0], [1.0, 9.0]]), Some((5.0, 9.0)));
    let zero = y_range(&[[0.0, 0.0]]).expect("range");
    assert!((zero.1 - zero.0 - 1e-9).abs() < 1e-18);
    assert_eq!(y_range(&[]), None);
}

/// **Gruvbox by default** (maintainer, 2026-11-25). The theme is applied
/// on the first frame and again only when the reader changes it; a frame
/// of the app on a headless context leaves the context in Gruvbox Dark.
#[test]
fn the_app_applies_gruvbox() {
    use bugis::Theme;
    assert_eq!(theme_to_apply(Theme::GruvboxDark, None), Some(Theme::GruvboxDark));
    assert_eq!(theme_to_apply(Theme::GruvboxDark, Some(Theme::GruvboxDark)), None);
    assert_eq!(theme_to_apply(Theme::GruvboxLight, Some(Theme::GruvboxDark)), Some(Theme::GruvboxLight));

    struct Quiet;
    impl TwinHost for Quiet {
        fn title(&self) -> String {
            "quiet".into()
        }
        fn page_status(&self, _page: TwinPage) -> Result<(), TwinError> {
            Ok(())
        }
        fn columns(&self) -> Vec<&'static str> {
            vec!["time_s", "x"]
        }
        fn history(&self) -> Vec<Vec<f64>> {
            vec![vec![0.0, 1.0], vec![1.0, 2.0]]
        }
        fn control_volumes(&self) -> ControlVolumes {
            ControlVolumes::default()
        }
        fn monte_carlo(&self) -> McPageView {
            McPageView::default()
        }
        fn multiphysics(&self, _r: MeshResolution) -> Vec<String> {
            Vec::new()
        }
        fn controls(&mut self, _ui: &mut Ui) {}
        fn act(&mut self, _a: TwinAction) {}
    }
    let ctx = egui::Context::default();
    let mut app = TwinApp::new(Quiet);
    assert_eq!(app.shell.theme, Theme::GruvboxDark);
    for page in TwinPage::ALL {
        app.shell.page = page;
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| app.ui(ui));
    }
    assert_eq!(ctx.theme(), egui::Theme::Dark);
    assert_eq!(ctx.style_of(egui::Theme::Dark).visuals.panel_fill, Theme::GruvboxDark.visuals().panel_fill);
    app.shell.theme = Theme::GruvboxLight;
    let _ = ctx.run_ui(egui::RawInput::default(), |ui| app.ui(ui));
    assert_eq!(ctx.theme(), egui::Theme::Light);
    assert_eq!(ctx.style_of(egui::Theme::Light).visuals.panel_fill, Theme::GruvboxLight.visuals().panel_fill);
}
