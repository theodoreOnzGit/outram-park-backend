//! Drawing a cell field (Step 10's computed power, its temperatures) on the
//! neutronics mesh itself, as the crate's drawing rule asks: the cells the
//! solver used, cut by a plane, each filled with its value's colour.
//!
//! The mesh plotter colours by zone; this bins the field into zones of a
//! clone of the mesh and recolours, the same trick `meshing::draw` uses to
//! give a region one colour on every mesh.

use outram_blender::csg::plot::{
    annotate_slice, default_colours, ImageData, LegendEntry, PlotBasis, Rgb, SlicePlot,
};
use outram_blender::csg::position::Position;
use outram_blender::unstructured::{render_mesh_slice, MeshColourBy, MeshSlice, UnstructuredMesh, Zone};

/// Number of colour bins.
const BINS: usize = 16;

/// A sequential colour map (dark blue → teal → yellow), `t` in \[0, 1\].
fn colour(t: f64) -> Rgb {
    const STOPS: [(f64, [f64; 3]); 5] = [
        (0.0, [48.0, 18.0, 59.0]),
        (0.25, [40.0, 110.0, 200.0]),
        (0.5, [30.0, 190.0, 160.0]),
        (0.75, [170.0, 220.0, 60.0]),
        (1.0, [250.0, 200.0, 30.0]),
    ];
    let t = t.clamp(0.0, 1.0);
    let i = STOPS
        .iter()
        .rposition(|s| s.0 <= t)
        .unwrap_or(0)
        .min(STOPS.len() - 2);
    let (a, b) = (STOPS[i], STOPS[i + 1]);
    let w = (t - a.0) / (b.0 - a.0);
    let c = |k: usize| (a.1[k] + w * (b.1[k] - a.1[k])).round() as u8;
    Rgb::new(c(0), c(1), c(2))
}

/// Draw `values` (one per cell of `mesh`) on a slice through `origin_cm`.
/// Cells whose value is at most `zero_below` are grey and listed as
/// `zero_label`; the rest are binned linearly between their min and max.
#[allow(clippy::too_many_arguments)]
pub fn draw_field(
    mesh: &UnstructuredMesh,
    values: &[f64],
    basis: PlotBasis,
    origin_cm: [f64; 3],
    title: &str,
    unit: &str,
    zero_below: f64,
    zero_label: &str,
    px: usize,
) -> Result<ImageData, String> {
    if values.len() != mesh.n_cells() {
        return Err(format!(
            "{} values for {} cells",
            values.len(),
            mesh.n_cells()
        ));
    }
    let live: Vec<f64> = values.iter().copied().filter(|v| *v > zero_below).collect();
    let lo = live.iter().copied().fold(f64::MAX, f64::min);
    let hi = live.iter().copied().fold(f64::MIN, f64::max);
    let span = (hi - lo).max(1e-300);
    let mut cells: Vec<Vec<usize>> = vec![Vec::new(); BINS + 1];
    for (c, &v) in values.iter().enumerate() {
        let b = if v <= zero_below {
            BINS
        } else {
            (((v - lo) / span * BINS as f64).floor() as usize).min(BINS - 1)
        };
        cells[b].push(c);
    }
    let zones: Vec<Zone> = cells
        .into_iter()
        .enumerate()
        .map(|(i, cells)| Zone {
            name: format!("bin{i}"),
            cells,
        })
        .collect();
    let binned = mesh
        .clone()
        .with_zones(zones)
        .map_err(|e| format!("{e:?}"))?;
    let mut sl = MeshSlice::framing(&binned, basis, px, MeshColourBy::Zone);
    let n = 3 - basis.axes().0 - basis.axes().1;
    sl.origin[n] = origin_cm[n];
    let (mut img, _) = render_mesh_slice(&binned, &sl);
    let mut seed = 1u64;
    let plot_cols = default_colours(BINS + 1, &mut seed);
    let grey = Rgb::new(205, 205, 205);
    let cols: Vec<Rgb> = (0..BINS)
        .map(|i| colour((i as f64 + 0.5) / BINS as f64))
        .chain(std::iter::once(grey))
        .collect();
    for p in &mut img.pixels {
        if let Some(z) = plot_cols.iter().position(|c| c == p) {
            *p = cols[z];
        }
    }
    let mut legend: Vec<LegendEntry> = (0..BINS - 1)
        .step_by(3)
        .map(|i| {
            let a = lo + span * i as f64 / BINS as f64;
            let b = lo + span * (i + 1) as f64 / BINS as f64;
            LegendEntry::new(cols[i], format!("{a:.3}-{b:.3} {unit}"))
        })
        .collect();
    legend.push(LegendEntry::new(
        cols[BINS - 1],
        format!(
            "{:.3}-{hi:.3} {unit} (max)",
            lo + span * (BINS - 1) as f64 / BINS as f64
        ),
    ));
    if !zero_label.is_empty() {
        legend.push(LegendEntry::new(grey, zero_label.to_string()));
    }
    let frame = SlicePlot::new(
        basis,
        Position::new(sl.origin[0], sl.origin[1], sl.origin[2]),
        sl.width,
        sl.pixels,
    );
    Ok(annotate_slice(&img, &frame, title, &legend))
}

#[cfg(test)]
mod tests {
    /// The colour map runs from its first stop to its last.
    #[test]
    fn colour_map_end_points() {
        let a = super::colour(0.0);
        let b = super::colour(1.0);
        assert_eq!((a.r, a.g, a.b), (48, 18, 59));
        assert_eq!((b.r, b.g, b.b), (250, 200, 30));
    }
}
