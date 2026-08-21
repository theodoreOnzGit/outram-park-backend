//! End-to-end graph-digitiser demo on a synthetic log-log figure.
//!
//! Renders a known decay-heat-shaped power law (`y = 8 x^-0.28`) to a PNG,
//! digitises it back through the fully automatic pipeline, and prints a few
//! recovered points next to the analytic truth. Everything runs offline and
//! deterministically; the PNG is written next to your current directory so
//! you can also try the CLI on it:
//!
//! ```text
//! cargo run --release -p kovan --example digitiser_synthetic_demo
//! kovan-digitise --image digitiser_demo_loglog.png \
//!     --x-scale log --x-range 1,1e6 --y-scale log --y-range 0.1,10 \
//!     --figure "synthetic demo" --json demo.json --csv demo.csv
//! ```
//!
//! This is a *self-consistency* demonstration (the digitiser reading a curve
//! this crate itself drew) — see the `digitiser` module docs for the honest
//! limits versus real scanned figures.

use kovan::digitiser::auto::{auto_digitise, AutoDigitiseConfig, AxisPixelRefs, AxisValueSpec};
use kovan::digitiser::calibration::AxisScale;
use kovan::digitiser::dataset::FigureSource;
use kovan::digitiser::detect::{DetectConfig, PixelRect};
use kovan::digitiser::synthetic::{render_synthetic_plot, SyntheticPlotSpec};
use kovan::digitiser::trace::TraceConfig;

/// The known curve: a decay-heat-like power law.
fn power_law(x: f64) -> f64 {
    8.0 * x.powf(-0.28)
}

fn main() {
    // 1. Render the known curve to a plot image (500x400, log-log axes).
    let spec = SyntheticPlotSpec {
        width: 500,
        height: 400,
        frame: PixelRect {
            left: 20,
            right: 480,
            top: 20,
            bottom: 380,
        },
        x_scale: AxisScale::Logarithmic,
        x_min: 1.0,
        x_max: 1e6,
        y_scale: AxisScale::Logarithmic,
        y_min: 0.1,
        y_max: 10.0,
        curve: power_law,
        curve_half_thickness: 1,
    };
    let (raster, _truth_cal) = render_synthetic_plot(&spec).expect("fixture renders");
    let png = raster.to_png_bytes().expect("png encodes");
    let png_path = "digitiser_demo_loglog.png";
    std::fs::write(png_path, &png).expect("png written");
    println!("wrote {png_path}");

    // 2. Digitise it back: automatic frame detection, frame-edge calibration.
    let config = AutoDigitiseConfig {
        x: AxisValueSpec {
            scale: AxisScale::Logarithmic,
            refs: AxisPixelRefs::FrameEdges {
                min_value: 1.0,
                max_value: 1e6,
            },
        },
        y: AxisValueSpec {
            scale: AxisScale::Logarithmic,
            refs: AxisPixelRefs::FrameEdges {
                min_value: 0.1,
                max_value: 10.0,
            },
        },
        detect: DetectConfig::default(),
        trace: TraceConfig::default(),
    };
    let dataset = auto_digitise(
        &kovan::digitiser::raster::PlotRaster::from_bytes(&png).expect("decodes"),
        &config,
        FigureSource::new("synthetic demo (log-log power law)").expect("figure label"),
        "x (arbitrary, log)",
        "y (arbitrary, log)",
        "digitiser_synthetic_demo example",
        kovan::digitiser::dataset::utc_now_iso8601(),
    )
    .expect("pipeline runs");

    // 3. Show recovered points against the analytic truth.
    println!("{} points traced; sample:", dataset.points.len());
    println!(
        "{:>12} {:>12} {:>12} {:>10}",
        "x", "y (traced)", "y (true)", "err %"
    );
    for p in dataset.points.iter().step_by(90) {
        let truth = power_law(p.x);
        println!(
            "{:>12.4e} {:>12.4e} {:>12.4e} {:>10.3}",
            p.x,
            p.y,
            truth,
            100.0 * (p.y - truth).abs() / truth
        );
    }
    println!(
        "review status: {:?} (the CLI never marks datasets reviewed)",
        dataset.review
    );
}
