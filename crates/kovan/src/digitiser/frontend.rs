//! Shared command-line surface for the digitiser binaries.
//!
//! Belongs here: [`AutoArgs`] — the `clap` argument set that fully describes
//! one automatic digitisation run — and [`AutoArgs::run`], which executes it.
//! Both the fully automatic `kovan-digitise` CLI and the hybrid
//! `kovan-digitise-tui` parse exactly these arguments, so a TUI session can
//! be re-run headlessly by pasting the same flags onto the CLI.
//!
//! Does not belong here: any interactivity (the TUI binary owns that) or the
//! pipeline itself ([`super::auto`]).
//!
//! Compiled unconditionally, no feature gate — `clap` is already a hard
//! dependency of this crate's own `kovan` CLI, unlike when this module lived
//! in `kovan-literature` (moved 2026-08-21, see this crate's `NOTICE`), where
//! `clap` was optional and this module was gated behind `digitise-cli` /
//! `digitise-tui`.

use clap::Parser;

use super::auto::{auto_digitise, AutoDigitiseConfig, AxisPixelRefs, AxisValueSpec};
use super::calibration::{AxisRef, AxisScale};
use super::dataset::{utc_now_iso8601, DigitisedDataset, FigureSource};
use super::detect::DetectConfig;
use super::raster::PlotRaster;
use super::trace::{CurveSelector, TraceConfig, TraceStrategy};
use super::DigitiserError;

/// Arguments for one automatic digitisation pass.
///
/// Axis values are supplied by the caller (read from the figure's printed
/// labels — tick-label OCR is deliberately out of scope, see the
/// [`super`] module doc); pixel geometry is automatic unless explicit
/// `--x-ref`/`--y-ref` pairs are given.
#[derive(Debug, Clone, Parser)]
pub struct AutoArgs {
    /// Path to the plot image (PNG or JPEG).
    #[arg(long)]
    pub image: String,

    /// x-axis scale: `linear` or `log`.
    #[arg(long, default_value = "linear")]
    pub x_scale: String,
    /// y-axis scale: `linear` or `log`.
    #[arg(long, default_value = "linear")]
    pub y_scale: String,

    /// Data values at the detected frame's left and right edges, as
    /// `min,max` (e.g. `--x-range 1,1e6`). Mutually exclusive with `--x-ref`.
    #[arg(long, allow_hyphen_values = true)]
    pub x_range: Option<String>,
    /// Data values at the detected frame's bottom and top edges, as
    /// `min,max`. Mutually exclusive with `--y-ref`.
    #[arg(long, allow_hyphen_values = true)]
    pub y_range: Option<String>,
    /// Explicit x reference point as `pixel=value`; give exactly twice
    /// (e.g. `--x-ref 57=1 --x-ref 462=1000`). Overrides `--x-range`.
    #[arg(long, allow_hyphen_values = true)]
    pub x_ref: Vec<String>,
    /// Explicit y reference point as `pixel=value` (pixel row, growing
    /// downward); give exactly twice. Overrides `--y-range`.
    #[arg(long, allow_hyphen_values = true)]
    pub y_ref: Vec<String>,

    /// Figure designation as printed, e.g. `"Fig. 7"`. Required provenance.
    #[arg(long)]
    pub figure: String,
    /// `KovanDocument` id of the catalogued source, if any.
    #[arg(long)]
    pub document_id: Option<String>,
    /// Source document title / free-text citation.
    #[arg(long)]
    pub document_title: Option<String>,
    /// Page the figure appears on.
    #[arg(long)]
    pub page: Option<u32>,
    /// Free-text provenance notes (crop, curve label, known skew…).
    #[arg(long)]
    pub notes: Option<String>,
    /// x-axis label as printed (units included).
    #[arg(long, default_value = "x")]
    pub x_label: String,
    /// y-axis label as printed (units included).
    #[arg(long, default_value = "y")]
    pub y_label: String,
    /// Operator recorded as `digitised_by`.
    #[arg(long, default_value = "kovan-digitise (automatic)")]
    pub operator: String,
    /// Override the `digitised_at` timestamp (ISO 8601) for byte-reproducible
    /// output; defaults to the current UTC time.
    #[arg(long)]
    pub timestamp: Option<String>,

    /// Trace strategy: `continuity` (default), `largest-run`, or `centroid`.
    #[arg(long, default_value = "continuity")]
    pub strategy: String,
    /// Sample every Nth pixel column.
    #[arg(long, default_value_t = 1)]
    pub step: u32,
    /// Curve-ink luminance threshold (0–255); ignored with `--curve-rgb`.
    #[arg(long, default_value_t = 128)]
    pub threshold: u8,
    /// Trace a specific curve colour, as `r,g,b` (0–255 each).
    #[arg(long)]
    pub curve_rgb: Option<String>,
    /// RGB distance tolerance for `--curve-rgb`.
    #[arg(long, default_value_t = 60)]
    pub curve_tolerance: u16,
    /// Pixels to shrink the frame inward before tracing.
    #[arg(long, default_value_t = 3)]
    pub inset: u32,
    /// Skip columns whose ink fill exceeds this fraction (vertical gridlines).
    #[arg(long, default_value_t = 0.6)]
    pub max_column_fill: f64,
    /// Frame detection: luminance below this is axis ink.
    #[arg(long, default_value_t = 128)]
    pub dark_threshold: u8,
    /// Frame detection: min dark-run fraction of the image dimension.
    #[arg(long, default_value_t = 0.4)]
    pub min_line_fraction: f64,
}

impl AutoArgs {
    /// Load the image and run the automatic pipeline, returning the raster
    /// (for front ends that display it) and the digitised dataset.
    ///
    /// # Errors
    ///
    /// Any [`DigitiserError`] from argument parsing, image loading, or the
    /// pipeline.
    pub fn run(&self) -> Result<(PlotRaster, DigitisedDataset), DigitiserError> {
        let raster = PlotRaster::from_path(std::path::Path::new(&self.image))?;
        let config = self.pipeline_config()?;
        let mut source = FigureSource::new(self.figure.clone())?;
        source.document_id = self.document_id.clone();
        source.document_title = self.document_title.clone();
        source.page = self.page;
        source.notes = self.notes.clone();
        source.image_path = Some(self.image.clone());
        let dataset = auto_digitise(
            &raster,
            &config,
            source,
            self.x_label.clone(),
            self.y_label.clone(),
            self.operator.clone(),
            self.timestamp.clone().unwrap_or_else(utc_now_iso8601),
        )?;
        Ok((raster, dataset))
    }

    /// Build the [`AutoDigitiseConfig`] these arguments describe.
    ///
    /// # Errors
    ///
    /// [`DigitiserError::Calibration`] on unparseable axis arguments.
    pub fn pipeline_config(&self) -> Result<AutoDigitiseConfig, DigitiserError> {
        let selector = match &self.curve_rgb {
            None => CurveSelector::DarkestBand {
                max_luminance: self.threshold,
            },
            Some(s) => CurveSelector::Rgb {
                rgb: parse_rgb(s)?,
                tolerance: self.curve_tolerance,
            },
        };
        Ok(AutoDigitiseConfig {
            x: axis_spec(&self.x_scale, &self.x_range, &self.x_ref, "x")?,
            y: axis_spec(&self.y_scale, &self.y_range, &self.y_ref, "y")?,
            detect: DetectConfig {
                dark_threshold: self.dark_threshold,
                min_line_fraction: self.min_line_fraction,
            },
            trace: TraceConfig {
                selector,
                strategy: parse_strategy(&self.strategy)?,
                column_step: self.step,
                inset: self.inset,
                max_column_fill: self.max_column_fill,
            },
        })
    }
}

/// Parse `linear` / `log` (also accepts `lin` / `logarithmic`).
pub fn parse_scale(s: &str) -> Result<AxisScale, DigitiserError> {
    match s.to_ascii_lowercase().as_str() {
        "linear" | "lin" => Ok(AxisScale::Linear),
        "log" | "logarithmic" => Ok(AxisScale::Logarithmic),
        other => Err(DigitiserError::Calibration(format!(
            "unknown axis scale {other:?} (expected `linear` or `log`)"
        ))),
    }
}

/// Parse a trace strategy name.
pub fn parse_strategy(s: &str) -> Result<TraceStrategy, DigitiserError> {
    match s.to_ascii_lowercase().as_str() {
        "continuity" | "continuity-nearest" => Ok(TraceStrategy::ContinuityNearest),
        "largest-run" | "largest" => Ok(TraceStrategy::LargestRun),
        "centroid" | "column-centroid" => Ok(TraceStrategy::ColumnCentroid),
        other => Err(DigitiserError::Trace(format!(
            "unknown strategy {other:?} (expected `continuity`, `largest-run`, or `centroid`)"
        ))),
    }
}

fn axis_spec(
    scale: &str,
    range: &Option<String>,
    refs: &[String],
    axis: &str,
) -> Result<AxisValueSpec, DigitiserError> {
    let scale = parse_scale(scale)?;
    let refs_spec = match (refs.len(), range) {
        (0, Some(r)) => {
            let (min_value, max_value) = parse_pair(r, ',', axis, "range `min,max`")?;
            AxisPixelRefs::FrameEdges {
                min_value,
                max_value,
            }
        }
        (2, _) => {
            let (p1, v1) = parse_pair(&refs[0], '=', axis, "ref `pixel=value`")?;
            let (p2, v2) = parse_pair(&refs[1], '=', axis, "ref `pixel=value`")?;
            AxisPixelRefs::Explicit {
                r1: AxisRef {
                    pixel: p1,
                    value: v1,
                },
                r2: AxisRef {
                    pixel: p2,
                    value: v2,
                },
            }
        }
        (0, None) => {
            return Err(DigitiserError::Calibration(format!(
                "{axis} axis needs either --{axis}-range min,max or two --{axis}-ref pixel=value"
            )))
        }
        (n, _) => {
            return Err(DigitiserError::Calibration(format!(
                "--{axis}-ref must be given exactly twice, got {n}"
            )))
        }
    };
    Ok(AxisValueSpec {
        scale,
        refs: refs_spec,
    })
}

fn parse_pair(s: &str, sep: char, axis: &str, what: &str) -> Result<(f64, f64), DigitiserError> {
    let err = || DigitiserError::Calibration(format!("cannot parse {axis} {what} from {s:?}"));
    let (a, b) = s.split_once(sep).ok_or_else(err)?;
    Ok((
        a.trim().parse::<f64>().map_err(|_| err())?,
        b.trim().parse::<f64>().map_err(|_| err())?,
    ))
}

fn parse_rgb(s: &str) -> Result<[u8; 3], DigitiserError> {
    let parts: Vec<_> = s.split(',').map(str::trim).collect();
    if parts.len() != 3 {
        return Err(DigitiserError::Trace(format!(
            "--curve-rgb wants `r,g,b`, got {s:?}"
        )));
    }
    let mut rgb = [0u8; 3];
    for (i, p) in parts.iter().enumerate() {
        rgb[i] = p.parse::<u8>().map_err(|_| {
            DigitiserError::Trace(format!("--curve-rgb component {p:?} is not 0-255"))
        })?;
    }
    Ok(rgb)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scales_and_strategies_parse() {
        assert_eq!(parse_scale("log").unwrap(), AxisScale::Logarithmic);
        assert_eq!(parse_scale("Linear").unwrap(), AxisScale::Linear);
        assert!(parse_scale("banana").is_err());
        assert_eq!(
            parse_strategy("largest-run").unwrap(),
            TraceStrategy::LargestRun
        );
        assert!(parse_strategy("ml-model").is_err());
    }

    #[test]
    fn axis_spec_wants_range_or_two_refs() {
        assert!(axis_spec("linear", &None, &[], "x").is_err());
        assert!(axis_spec("linear", &None, &["10=0".into()], "x").is_err());
        let frame = axis_spec("log", &Some("1,1e6".into()), &[], "x").unwrap();
        assert!(matches!(frame.refs, AxisPixelRefs::FrameEdges { .. }));
        let explicit = axis_spec("linear", &None, &["10=0".into(), "460=100".into()], "x").unwrap();
        assert!(matches!(explicit.refs, AxisPixelRefs::Explicit { .. }));
    }

    #[test]
    fn negative_and_scientific_values_parse() {
        let (a, b) = parse_pair("-1.5,2e3", ',', "x", "range").unwrap();
        assert_eq!(a, -1.5);
        assert_eq!(b, 2000.0);
    }
}
