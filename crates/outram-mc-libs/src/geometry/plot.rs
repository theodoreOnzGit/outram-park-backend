// SPDX-License-Identifier: GPL-3.0

//! **Geometry slice plotting, by emitting a standalone matplotlib script.**
//! GitHub #268.
//!
//! # Why this shape, and what is deliberately NOT ported
//!
//! Upstream `src/plot.cpp` is 2597 lines of PPM/PNG rasterisation, voxel output
//! and a colour-mapping layer. **None of it is ported**, by maintainer
//! direction. What this emits is a self-contained `.py` file that draws the
//! slice when run.
//!
//! That choice buys four things:
//!
//! - no new Rust dependency, no image encoder, nothing that has to compile for
//!   Android or wasm — the output is a text file;
//! - the script is **inspectable and editable**: change the colour map, the
//!   slice plane or the figure size without rebuilding, and `diff` two scripts
//!   to see what changed in a model;
//! - it sits naturally beside the OpenMC decks already committed under
//!   `verification_and_validation/<topic>/openmc_inputs/`, so it can be
//!   compared against `openmc.Plot` output of the same model;
//! - it is the honest scope. A geometry plot's job here is to let someone *see
//!   whether the model is the model they meant* — the same job
//!   [`crate::geometry::volume_calc`] does numerically.
//!
//! Explicitly out of scope: voxel plots, the native rasteriser, and any
//! interactive viewer (the crate already has a TUI).
//!
//! # How the data gets into the script
//!
//! The index array is **embedded in the script itself** as a nested list, so
//! the `.py` is standalone and reproducible with no Rust binary and no side
//! files. It runs on a bare `python3` with only `matplotlib` and `numpy`.

use crate::geometry::cell::SurfaceToken;
use crate::geometry::geometry::Geometry;
use crate::geometry::position::{Direction, Position};

/// What the slice is coloured by. The three upstream offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColourBy {
    /// Leaf cell index.
    Cell,
    /// Leaf material index; void reads as -1.
    Material,
    /// Leaf universe index.
    Universe,
}

impl ColourBy {
    fn label(self) -> &'static str {
        match self {
            Self::Cell => "cell",
            Self::Material => "material",
            Self::Universe => "universe",
        }
    }
}

/// A slice plane: an origin and two in-plane basis vectors, with a width along
/// each and a pixel count along each.
#[derive(Debug, Clone, Copy)]
pub struct Slice {
    /// Centre of the slice \[cm\].
    pub origin: Position,
    /// In-plane basis vector for the horizontal axis (need not be unit; it is
    /// normalised here).
    pub basis_u: Direction,
    /// In-plane basis vector for the vertical axis.
    pub basis_v: Direction,
    /// Full width along `basis_u` \[cm\].
    pub width_u: f64,
    /// Full width along `basis_v` \[cm\].
    pub width_v: f64,
    /// Pixels along `basis_u`.
    pub pixels_u: usize,
    /// Pixels along `basis_v`.
    pub pixels_v: usize,
}

/// Sample the slice, returning a `pixels_v` x `pixels_u` array of indices.
///
/// `-1` marks a point that is in no cell at all — outside the geometry — and is
/// deliberately distinct from a void cell inside it, which under
/// [`ColourBy::Material`] also reads `-1`. Under [`ColourBy::Cell`] the two are
/// distinguishable, which is why a geometry that looks wrong should be checked
/// cell-coloured first.
pub fn sample_slice(geom: &Geometry, slice: &Slice, colour_by: ColourBy) -> Vec<Vec<i64>> {
    let nu = |d: Direction| -> Direction {
        let n = (d.u * d.u + d.v * d.v + d.w * d.w).sqrt();
        Direction::new(d.u / n, d.v / n, d.w / n)
    };
    let bu = nu(slice.basis_u);
    let bv = nu(slice.basis_v);

    // The locate direction. Any direction works -- it only breaks surface-sense
    // ties -- but it must be the SAME one everywhere or two adjacent pixels on
    // a surface could resolve to different sides and the plot would show a
    // one-pixel seam that is an artefact, not geometry.
    let probe = nu(Direction::new(1.0, 1.0, 1.0));

    let mut rows = Vec::with_capacity(slice.pixels_v);
    for j in 0..slice.pixels_v {
        // Row 0 is the TOP of the image, matching `imshow`'s default origin,
        // so the emitted script needs no `origin=` argument and a reader
        // comparing against `openmc.Plot` sees the same orientation.
        let fv = (j as f64 + 0.5) / slice.pixels_v as f64 - 0.5;
        let mut row = Vec::with_capacity(slice.pixels_u);
        for i in 0..slice.pixels_u {
            let fu = (i as f64 + 0.5) / slice.pixels_u as f64 - 0.5;
            let du = fu * slice.width_u;
            let dv = -fv * slice.width_v;
            let p = Position::new(
                slice.origin.x + bu.u * du + bv.u * dv,
                slice.origin.y + bu.v * du + bv.v * dv,
                slice.origin.z + bu.w * du + bv.w * dv,
            );
            let v = match geom.locate(p, probe, SurfaceToken::NONE) {
                None => -1,
                Some(path) => match colour_by {
                    ColourBy::Material => path.material.map(|m| m as i64).unwrap_or(-1),
                    ColourBy::Cell => path
                        .levels
                        .last()
                        .map(|c| c.cell as i64)
                        .unwrap_or(-1),
                    ColourBy::Universe => path
                        .levels
                        .last()
                        .map(|c| c.universe as i64)
                        .unwrap_or(-1),
                },
            };
            row.push(v);
        }
        rows.push(row);
    }
    rows
}

/// Emit a standalone matplotlib script that draws this slice.
///
/// `title` names the model; `provenance` is free text written into the header
/// comment — the commit, the date, whatever makes the plot traceable later.
pub fn emit_python(
    geom: &Geometry,
    slice: &Slice,
    colour_by: ColourBy,
    title: &str,
    provenance: &str,
) -> String {
    let data = sample_slice(geom, slice, colour_by);
    let mut s = String::new();

    s.push_str("# Generated by outram-mc-libs :: geometry::plot (GitHub #268).\n");
    s.push_str("#\n");
    s.push_str(&format!("# model      : {title}\n"));
    s.push_str(&format!("# coloured by: {}\n", colour_by.label()));
    s.push_str(&format!(
        "# slice     : origin ({:.6}, {:.6}, {:.6}) cm\n",
        slice.origin.x, slice.origin.y, slice.origin.z
    ));
    s.push_str(&format!(
        "#             u ({:.6}, {:.6}, {:.6}), width {:.6} cm, {} px\n",
        slice.basis_u.u, slice.basis_u.v, slice.basis_u.w, slice.width_u, slice.pixels_u
    ));
    s.push_str(&format!(
        "#             v ({:.6}, {:.6}, {:.6}), width {:.6} cm, {} px\n",
        slice.basis_v.u, slice.basis_v.v, slice.basis_v.w, slice.width_v, slice.pixels_v
    ));
    s.push_str(&format!("# provenance : {provenance}\n"));
    s.push_str("#\n");
    s.push_str("# -1 means the point is in NO cell (outside the geometry). Under\n");
    s.push_str("# material colouring a void cell INSIDE the geometry also reads -1;\n");
    s.push_str("# re-run coloured by cell to tell the two apart.\n");
    s.push_str("#\n");
    s.push_str("# Runs on a bare python3 with only matplotlib and numpy.\n");
    s.push_str("#     python3 this_file.py\n\n");
    s.push_str("import numpy as np\nimport matplotlib.pyplot as plt\n\n");

    s.push_str("INDEX = np.array([\n");
    for row in &data {
        s.push_str("    [");
        for (i, v) in row.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            s.push_str(&v.to_string());
        }
        s.push_str("],\n");
    }
    s.push_str("], dtype=int)\n\n");

    let (hu, hv) = (slice.width_u * 0.5, slice.width_v * 0.5);
    s.push_str(&format!(
        "EXTENT = ({:.6}, {:.6}, {:.6}, {:.6})  # u_min, u_max, v_min, v_max [cm]\n\n",
        -hu, hu, -hv, hv
    ));

    s.push_str("fig, ax = plt.subplots(figsize=(7, 7))\n");
    s.push_str("vals = np.unique(INDEX)\n");
    s.push_str("# A discrete colour per distinct index, so adjacent regions are\n");
    s.push_str("# distinguishable rather than smoothly interpolated.\n");
    s.push_str("cmap = plt.get_cmap('tab20', len(vals))\n");
    s.push_str("lookup = {v: i for i, v in enumerate(vals)}\n");
    s.push_str("shown = np.vectorize(lookup.get)(INDEX)\n");
    s.push_str("im = ax.imshow(shown, extent=EXTENT, cmap=cmap, interpolation='nearest')\n");
    s.push_str(&format!(
        "ax.set_title({:?})\n",
        format!("{title} — by {}", colour_by.label())
    ));
    s.push_str("ax.set_xlabel('u [cm]')\nax.set_ylabel('v [cm]')\n");
    s.push_str("cbar = fig.colorbar(im, ax=ax, ticks=range(len(vals)))\n");
    s.push_str(&format!(
        "cbar.ax.set_yticklabels([str(v) for v in vals])\ncbar.set_label({:?})\n",
        colour_by.label()
    ));
    s.push_str("plt.tight_layout()\nplt.savefig('geometry_slice.png', dpi=150)\nprint('wrote geometry_slice.png')\n");
    s
}
