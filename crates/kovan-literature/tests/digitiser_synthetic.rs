//! # Graph-digitiser synthetic self-consistency tests
//!
//! ## Methodology (V&V rule: methodology + measured results, both required)
//!
//! Known analytic curves are rendered to in-memory plot images at known pixel
//! positions by [`kovan_literature::digitiser::synthetic`] (white background,
//! 1-px frame, 3-px-thick curve — typical of published figures), then pushed
//! through the **full automatic pipeline** ([`auto_digitise`]: frame
//! detection → calibration from frame-edge values → column-scan trace →
//! provenance-carrying dataset). Every recovered point `(x, y)` is compared
//! against the analytic `curve(x)`; the error metric is
//!
//! - linear y axis: `|y - curve(x)|` as a fraction of the y-axis span;
//! - logarithmic y axis: `|log10(y) - log10(curve(x))|` in decades
//!   (equivalently, relative value error ≈ `ln(10) ×` decades).
//!
//! One case per axis combination: linear-linear (curved quadratic),
//! log-linear (x log), and **log-log** (a decay-heat-like power law — the
//! case that matters for the Tobias figures and where naive linear-pixel
//! interpolation goes wrong). Pass criteria are set at 2× the pixel
//! quantisation scale of the rendered fixtures (≈ 1 px on a ~380-px-tall
//! frame).
//!
//! ## Measured results
//!
//! Measured 2026-08-11 by running `cargo test -p kovan-literature --release
//! --test digitiser_synthetic -- --nocapture` (kovan-literature graph
//! digitiser 0.0.0, synthetic fixtures 500×400 px, frame 461×361 px, 3-px
//! curve, default `TraceConfig`); numbers below are copied from that run's
//! output, not predicted:
//!
//! - linear-linear (`y = 0.2 x² + 1`, x ∈ [0, 10], y-span 25): 455 points,
//!   max |Δy| = 0.034604 y-units = **0.138 %** of the y span (limit 0.5 %).
//! - log-linear (`y = 3 log10 x + 2`, x ∈ [1, 10⁴], y-span 15): 455 points,
//!   max |Δy| = 0.020652 y-units = **0.138 %** of span (limit 0.5 %).
//! - log-log (`y = 8 x^-0.28`, x ∈ [1, 10⁶], y ∈ [0.1, 10]): 455 points,
//!   max log-error = **0.002765 decades** (0.639 % relative; limit 0.011
//!   decades ≈ 2 px). Per-point asymmetric uncertainty bands bracketed the
//!   analytic value at 455/455 points (100 %, criterion ≥ 99 %).
//!
//! Interpretation: on clean synthetic input the full automatic pipeline
//! recovers curves to about half a pixel, and the log-space calibration is
//! doing its job (errors are flat in log space, not value-proportional).
//!
//! ## Honest limits
//!
//! These are **self-consistency** results: the digitiser recovering curves
//! this crate itself rendered. They verify the pixel→data mapping (including
//! log-space calibration) and the tracer, but they say **nothing about
//! accuracy on real scanned figures** (skew, JPEG noise, gridlines, tick
//! geometry). That claim must wait for the maintainer-supplied hand-digitised
//! golden oracle (bead `op-amfh`); `oracle_comparison_shape` below shows the
//! intended comparison path so wiring it up is mechanical when it lands.

use kovan_literature::digitiser::auto::{
    auto_digitise, AutoDigitiseConfig, AxisPixelRefs, AxisValueSpec,
};
use kovan_literature::digitiser::calibration::AxisScale;
use kovan_literature::digitiser::dataset::{DigitisedDataset, FigureSource, ReviewStatus};
use kovan_literature::digitiser::detect::{DetectConfig, PixelRect};
use kovan_literature::digitiser::raster::PlotRaster;
use kovan_literature::digitiser::synthetic::{render_synthetic_plot, SyntheticPlotSpec};
use kovan_literature::digitiser::trace::TraceConfig;

/// Standard fixture geometry: 500×400 image, frame inset 20 px.
const FRAME: PixelRect = PixelRect {
    left: 20,
    right: 480,
    top: 20,
    bottom: 380,
};

fn spec(
    x_scale: AxisScale,
    x_min: f64,
    x_max: f64,
    y_scale: AxisScale,
    y_min: f64,
    y_max: f64,
    curve: fn(f64) -> f64,
) -> SyntheticPlotSpec {
    SyntheticPlotSpec {
        width: 500,
        height: 400,
        frame: FRAME,
        x_scale,
        x_min,
        x_max,
        y_scale,
        y_min,
        y_max,
        curve,
        curve_half_thickness: 1,
    }
}

/// Render `spec`, digitise it with frame-edge calibration, and return the
/// dataset (checking basic dataset invariants on the way).
fn digitise(spec: &SyntheticPlotSpec) -> DigitisedDataset {
    let (raster, _cal) = render_synthetic_plot(spec).expect("fixture renders");
    let cfg = AutoDigitiseConfig {
        x: AxisValueSpec {
            scale: spec.x_scale,
            refs: AxisPixelRefs::FrameEdges {
                min_value: spec.x_min,
                max_value: spec.x_max,
            },
        },
        y: AxisValueSpec {
            scale: spec.y_scale,
            refs: AxisPixelRefs::FrameEdges {
                min_value: spec.y_min,
                max_value: spec.y_max,
            },
        },
        detect: DetectConfig::default(),
        trace: TraceConfig::default(),
    };
    let d = auto_digitise(
        &raster,
        &cfg,
        FigureSource::new("synthetic fixture").unwrap(),
        "x (arbitrary)",
        "y (arbitrary)",
        "digitiser_synthetic test",
        "2026-08-11T00:00:00Z",
    )
    .expect("auto pipeline succeeds");
    assert_eq!(d.review, ReviewStatus::Unreviewed);
    let t = d.trace.as_ref().expect("trace record present");
    assert!(t.frame_auto_detected, "frame should be auto-detected");
    assert_eq!(t.frame, FRAME, "detected frame must match the drawn frame");
    // Interior is 461 columns minus 2×inset(3) plus endpoint ⇒ expect ≳ 440
    // samples for a continuous curve spanning the frame.
    assert!(
        d.points.len() > 400,
        "only {} points traced",
        d.points.len()
    );
    d
}

fn quadratic(x: f64) -> f64 {
    0.2 * x * x + 1.0
}

fn log_line(x: f64) -> f64 {
    3.0 * x.log10() + 2.0
}

fn power_law(x: f64) -> f64 {
    8.0 * x.powf(-0.28)
}

#[test]
fn linear_linear_quadratic_recovered_within_half_percent_of_span() {
    let s = spec(
        AxisScale::Linear,
        0.0,
        10.0,
        AxisScale::Linear,
        0.0,
        25.0,
        quadratic,
    );
    let d = digitise(&s);
    let span = 25.0;
    let mut max_err = 0.0f64;
    for p in &d.points {
        let err = (p.y - quadratic(p.x)).abs();
        max_err = max_err.max(err);
    }
    eprintln!(
        "lin-lin: {} points, max |dy| = {max_err:.6} y-units = {:.4} % of span",
        d.points.len(),
        100.0 * max_err / span
    );
    assert!(
        max_err / span < 0.005,
        "max error {max_err} exceeds 0.5 % of y span"
    );
}

#[test]
fn log_linear_recovered_within_half_percent_of_span() {
    let s = spec(
        AxisScale::Logarithmic,
        1.0,
        1e4,
        AxisScale::Linear,
        0.0,
        15.0,
        log_line,
    );
    let d = digitise(&s);
    let span = 15.0;
    let mut max_err = 0.0f64;
    for p in &d.points {
        max_err = max_err.max((p.y - log_line(p.x)).abs());
    }
    eprintln!(
        "log-lin: {} points, max |dy| = {max_err:.6} y-units = {:.4} % of span",
        d.points.len(),
        100.0 * max_err / span
    );
    assert!(
        max_err / span < 0.005,
        "max error {max_err} exceeds 0.5 % of y span"
    );
}

#[test]
fn log_log_power_law_recovered_within_pixel_scale_decades() {
    // The decay-heat-shaped case. y decade span is 2 over a 361-px frame
    // height ⇒ one pixel ≈ 0.00554 decades; pass limit 2 px ≈ 0.011 decades.
    let s = spec(
        AxisScale::Logarithmic,
        1.0,
        1e6,
        AxisScale::Logarithmic,
        0.1,
        10.0,
        power_law,
    );
    let d = digitise(&s);
    let mut max_dec = 0.0f64;
    for p in &d.points {
        let dec = (p.y.log10() - power_law(p.x).log10()).abs();
        max_dec = max_dec.max(dec);
    }
    eprintln!(
        "log-log: {} points, max log-error = {max_dec:.6} decades ({:.3} % relative)",
        d.points.len(),
        100.0 * (10f64.powf(max_dec) - 1.0)
    );
    assert!(
        max_dec < 0.011,
        "max log error {max_dec} decades exceeds the 2-pixel limit"
    );
}

#[test]
fn log_log_uncertainties_are_asymmetric_and_bracket_the_truth() {
    let s = spec(
        AxisScale::Logarithmic,
        1.0,
        1e6,
        AxisScale::Logarithmic,
        0.1,
        10.0,
        power_law,
    );
    let d = digitise(&s);
    let mut bracketed = 0usize;
    for p in &d.points {
        // Log axis ⇒ upward pixel error maps to a larger magnitude than
        // downward: strictly asymmetric bands.
        assert!(
            p.y_plus > p.y_minus,
            "log-axis uncertainty should be asymmetric at y={}",
            p.y
        );
        assert!(p.x_plus > p.x_minus);
        let truth = power_law(p.x);
        if truth >= p.y - p.y_minus && truth <= p.y + p.y_plus {
            bracketed += 1;
        }
    }
    let frac = bracketed as f64 / d.points.len() as f64;
    eprintln!(
        "log-log bracketing: {bracketed}/{} points ({:.1} %)",
        d.points.len(),
        100.0 * frac
    );
    // The band is the half-line-thickness reading uncertainty; the analytic
    // value must fall inside it essentially everywhere.
    assert!(
        frac >= 0.99,
        "only {:.1} % of points bracket the truth",
        100.0 * frac
    );
}

#[test]
fn pipeline_is_deterministic_end_to_end() {
    let s = spec(
        AxisScale::Logarithmic,
        1.0,
        1e6,
        AxisScale::Logarithmic,
        0.1,
        10.0,
        power_law,
    );
    let a = digitise(&s);
    let b = digitise(&s);
    assert_eq!(a, b, "same image + config must give identical datasets");
}

#[test]
fn png_encode_decode_path_preserves_the_trace() {
    // Same pipeline but through actual PNG bytes (the CLI's input path),
    // proving decode introduces no drift and the sha256 lands in provenance.
    let s = spec(
        AxisScale::Linear,
        0.0,
        10.0,
        AxisScale::Linear,
        0.0,
        25.0,
        quadratic,
    );
    let (raster, _) = render_synthetic_plot(&s).unwrap();
    let png = raster.to_png_bytes().unwrap();
    let decoded = PlotRaster::from_bytes(&png).unwrap();
    let cfg = AutoDigitiseConfig {
        x: AxisValueSpec {
            scale: AxisScale::Linear,
            refs: AxisPixelRefs::FrameEdges {
                min_value: 0.0,
                max_value: 10.0,
            },
        },
        y: AxisValueSpec {
            scale: AxisScale::Linear,
            refs: AxisPixelRefs::FrameEdges {
                min_value: 0.0,
                max_value: 25.0,
            },
        },
        detect: DetectConfig::default(),
        trace: TraceConfig::default(),
    };
    let src = FigureSource::new("Fig. png-path").unwrap();
    let d = auto_digitise(
        &decoded,
        &cfg,
        src,
        "x",
        "y",
        "test",
        "2026-08-11T00:00:00Z",
    )
    .unwrap();
    assert_eq!(
        d.source.image_sha256.as_deref(),
        decoded.source_sha256(),
        "image sha256 must be recorded as provenance"
    );
    let direct = digitise(&s);
    assert_eq!(d.points.len(), direct.points.len());
    for (p, q) in d.points.iter().zip(&direct.points) {
        assert_eq!(p.x_px, q.x_px);
        assert_eq!(p.y_px, q.y_px);
    }
}

/// Shape of the future golden-oracle comparison (bead `op-amfh`): given a
/// hand-digitised reference set, every reference point must be matched by the
/// digitiser output within a stated tolerance. Runs here against synthetic
/// truth so the comparison code exists and is exercised; swap the reference
/// vector for the maintainer's Tobias points when they land.
#[test]
fn oracle_comparison_shape() {
    let s = spec(
        AxisScale::Logarithmic,
        1.0,
        1e6,
        AxisScale::Logarithmic,
        0.1,
        10.0,
        power_law,
    );
    let d = digitise(&s);
    // Stand-in "oracle": analytic samples at round decades.
    let oracle: Vec<(f64, f64)> = (0..=6)
        .map(|k| {
            let x = 10f64.powi(k);
            (x, power_law(x))
        })
        .collect();
    let tol_decades = 0.011; // same 2-px basis as above
    let mut compared = 0usize;
    for (ox, oy) in oracle {
        // Interpolate the digitised curve at the oracle's x (linear in
        // log-log space, as a real oracle comparison would). Oracle points
        // outside the traced x range are skipped — the tracer cannot see
        // inside the frame inset, and a real oracle would be cropped to the
        // digitised range too.
        let Some(y) = interp_loglog(&d, ox) else {
            continue;
        };
        compared += 1;
        let dec = (y.log10() - oy.log10()).abs();
        assert!(
            dec < tol_decades,
            "at x={ox}: digitised y={y} vs oracle {oy} ({dec} decades off)"
        );
    }
    assert!(compared >= 5, "only {compared} oracle points fell in range");
}

/// Linear interpolation of the digitised points in log-log space at `x`.
/// `None` outside the digitised x range. This is the helper the real
/// `op-amfh` comparison will reuse.
fn interp_loglog(d: &DigitisedDataset, x: f64) -> Option<f64> {
    let pts = &d.points;
    let lx = x.log10();
    let i = pts.iter().position(|p| p.x >= x)?;
    if i == 0 {
        return (pts[0].x == x).then_some(pts[0].y);
    }
    let (a, b) = (&pts[i - 1], &pts[i]);
    let t = (lx - a.x.log10()) / (b.x.log10() - a.x.log10());
    Some(10f64.powf(a.y.log10() + t * (b.y.log10() - a.y.log10())))
}
