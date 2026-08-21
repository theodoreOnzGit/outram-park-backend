//! `kovan-digitise` — fully automatic graph digitiser CLI (the agent path).
//!
//! Loads a plot image, detects the axis frame, calibrates from
//! caller-supplied axis values (linear or log per axis), traces the curve,
//! and writes a [`kovan::digitiser::dataset::DigitisedDataset`]
//! with the complete provenance record. No human in the loop, no prompts;
//! the same image and flags always produce the same points, and the output
//! is byte-identical when `--timestamp` is pinned.
//!
//! The emitted dataset is always marked `UNREVIEWED` — human verification is
//! the job of `kovan-digitise-tui` / `kovan-digitise-gui`, which record who
//! reviewed and when rather than assuming it.
//!
//! ```text
//! # log-log decay-heat style figure, axis extremes labelled at the frame:
//! kovan-digitise --image fig7.png \
//!     --x-scale log --x-range 1,1e6 --y-scale log --y-range 0.1,10 \
//!     --figure "Fig. 7" --document-title "Tobias, Decay heat, PNE 1980" \
//!     --x-label "Time after shutdown (s)" --y-label "Decay power (MeV/fission-s)" \
//!     --json fig7.json --csv fig7.csv
//! ```

use clap::Parser;
use kovan::digitiser::frontend::AutoArgs;

/// Fully automatic plot digitiser: image in, provenance-carrying data points
/// out. Axis numeric values come from the flags (read them off the printed
/// figure); axis pixel geometry is detected automatically unless explicit
/// `--x-ref`/`--y-ref` pairs are given.
#[derive(Debug, Parser)]
#[command(
    name = "kovan-digitise",
    version,
    about = "Fully automatic graph digitiser: plot image in, provenance-carrying data points out"
)]
struct Cli {
    #[command(flatten)]
    auto: AutoArgs,

    /// Write the dataset as JSON to this path.
    #[arg(long)]
    json: Option<String>,
    /// Write the dataset as CSV (provenance embedded as `#` header lines).
    #[arg(long)]
    csv: Option<String>,
    /// Print a one-line summary to stderr instead of staying silent.
    #[arg(long)]
    verbose: bool,
}

fn main() -> std::process::ExitCode {
    let cli = Cli::parse();
    let (_raster, dataset) = match cli.auto.run() {
        Ok(v) => v,
        Err(e) => {
            eprintln!("kovan-digitise: {e}");
            return std::process::ExitCode::FAILURE;
        }
    };
    if cli.verbose {
        let frame = dataset
            .trace
            .as_ref()
            .map(|t| format!("{:?} (auto: {})", t.frame, t.frame_auto_detected))
            .unwrap_or_else(|| "none".to_string());
        eprintln!(
            "kovan-digitise: {} points traced, frame {frame}, review status: UNREVIEWED",
            dataset.points.len()
        );
    }
    if dataset.points.is_empty() {
        eprintln!(
            "kovan-digitise: warning: no curve points found — check --threshold/--curve-rgb \
             and that the image really contains a curve inside the axis frame"
        );
    }
    let mut wrote = false;
    if let Some(p) = &cli.json {
        if let Err(e) = dataset.write_json(std::path::Path::new(p)) {
            eprintln!("kovan-digitise: {e}");
            return std::process::ExitCode::FAILURE;
        }
        wrote = true;
    }
    if let Some(p) = &cli.csv {
        if let Err(e) = dataset.write_csv(std::path::Path::new(p)) {
            eprintln!("kovan-digitise: {e}");
            return std::process::ExitCode::FAILURE;
        }
        wrote = true;
    }
    if !wrote {
        // No output file requested: JSON on stdout, scriptable.
        println!("{}", dataset.to_json_string());
    }
    std::process::ExitCode::SUCCESS
}
