//! The `dhshort` rung's model: **the FHR unit cell of `dh_keff_vv.rs`, built
//! by `DhUniverse::pebble` under each `DhTreatment`, and drawn from
//! `DhUniverse::material_at`**, the one call every treatment answers and the
//! one the delta-tracked power iteration asks (`DhUniverse::keff` →
//! `run_keff_delta_in`). Nothing is re-modelled here.
//!
//! - **Geometry**: `PebbleParams::fhr_unit_cell()`: a 1.9 cm fuel zone at a
//!   particle packing fraction of 0.30 in a 2.0 cm pebble, FLiBe out to a
//!   reflective sphere at 3.0 cm, `TrisoSpec::FHR_HALEU_UCO`.
//! - **Materials**: `outram-mc-libs/examples/common/fhr_unit_cell.rs`, pulled in
//!   unchanged ([`fhr`]), the table `dh_keff_vv.rs` runs on. No nuclear data:
//!   drawing needs only compositions.
//! - **Seed**: `dh_keff_vv`'s first draw, `PebbleParams::seed | 1`, so the
//!   explicit arm's packing is the one its record ran on.
//!
//! The raster asks `material_at` once per pixel, row by row from the top, left
//! to right, at the pixel centres of `SlicePlot::pixel_centre` (the slice
//! plotter's own convention). For the explicit, homogenised and ring-RPT arms
//! that reads stored geometry. **For CLS and SCLS it samples**: each pixel row
//! is, to the sampler, a flight along +x, so the picture is the chords one
//! neutron would meet, and a redraw meets new ones.

use crate::raster::{Basis, RasterReq, OUTSIDE};
use egui::Color32;
use outram_mc_libs::dh_universe::{DhTreatment, DhUniverse, PebbleParams};
use outram_mc_libs::geometry::plot::{PlotBasis, SlicePlot};
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::pebble_beds::keff_delta::MaterialQuery;

/// The shared material table (compositions, temperature, nuclide order).
#[path = "../../../../outram-mc-libs/examples/common/fhr_unit_cell.rs"]
#[allow(clippy::all)]
pub mod fhr;

/// The treatments, in the order the panel lists them: the five of the lesson's
/// table, then the two kernel-level variants measured on 2026-09-18.
pub const TREATMENTS: [DhTreatment; 7] = [
    DhTreatment::DeltaTracking,
    DhTreatment::ChordLength,
    DhTreatment::Scls,
    DhTreatment::Homogenised,
    DhTreatment::RingRpt {
        inner_radius: DhTreatment::FHR_REFERENCE_RPT_INNER,
    },
    DhTreatment::ChordLengthKernel,
    DhTreatment::SclsKernel,
];

/// Short labels for the selector (phone width).
pub const SHORT: [&str; 7] = [
    "delta (exact)",
    "CLS",
    "SCLS",
    "naive smear",
    "ring-RPT",
    "CLS, kernel",
    "SCLS, kernel",
];

/// The reflective boundary of the unit cell, cm.
pub fn boundary_radius() -> f64 {
    PebbleParams::fhr_unit_cell().coolant_radius.unwrap_or(3.0)
}

/// The seed `dh_keff_vv` packs its first draw with.
pub fn seed() -> u64 {
    PebbleParams::fhr_unit_cell().seed | 1
}

/// Build the unit cell under treatment `i` of [`TREATMENTS`].
pub fn build(i: usize) -> Result<DhUniverse, String> {
    let t = *TREATMENTS
        .get(i)
        .ok_or_else(|| format!("no treatment {i}"))?;
    let params = PebbleParams {
        seed: seed(),
        ..PebbleParams::fhr_unit_cell().with_materials(fhr::materials())
    };
    DhUniverse::pebble(params, t).map_err(|e| format!("{}: {e}", t.name()))
}

/// Rasterise `req` on `u` (worker side): one byte per pixel, row 0 on top, the
/// material index `material_at` returns, or [`OUTSIDE`] beyond the reflective
/// sphere. SCLS's in-progress flight is reset once per row
/// (`MaterialQuery::begin_history`, as transport does per history); CLS's
/// sampler simply flies on from the previous row.
pub fn raster(u: &DhUniverse, req: &RasterReq) -> Vec<u8> {
    let origin = match req.basis {
        Basis::Xy => Position::new(req.centre[0], req.centre[1], req.depth),
        Basis::Xz => Position::new(req.centre[0], req.depth, req.centre[1]),
    };
    let basis = if req.basis == Basis::Xy {
        PlotBasis::Xy
    } else {
        PlotBasis::Xz
    };
    let plot = SlicePlot::new(basis, origin, req.width, req.px);
    let r_out = boundary_radius();
    let mut out = Vec::with_capacity(req.px[0] * req.px[1]);
    for y in 0..req.px[1] {
        (&u).begin_history();
        for x in 0..req.px[0] {
            let p = plot.pixel_centre(x, y);
            out.push(if p.norm() > r_out {
                OUTSIDE
            } else {
                u.material_at(p).map_or(OUTSIDE, |m| m.min(252) as u8)
            });
        }
    }
    out
}

/// Colour and name of every material index treatment `i` can return. Indices
/// 0–7 are the supplied table (`fhr::materials`); 8 is what the treatment
/// synthesised, which differs by treatment (`DhUniverse::pebble`).
pub fn palette(i: usize) -> Vec<(Color32, &'static str)> {
    let mut p = vec![
        (Color32::from_rgb(230, 126, 40), "UCO kernel"),
        (Color32::from_rgb(52, 52, 58), "buffer"),
        (Color32::from_rgb(160, 160, 166), "IPyC"),
        (Color32::from_rgb(214, 190, 112), "SiC"),
        (Color32::from_rgb(196, 196, 204), "OPyC"),
        (Color32::from_rgb(96, 100, 108), "matrix graphite"),
        (Color32::from_rgb(70, 74, 84), "shell graphite"),
        (Color32::from_rgb(56, 104, 168), "FLiBe coolant"),
    ];
    let synth = match TREATMENTS.get(i) {
        Some(DhTreatment::ChordLength | DhTreatment::Scls | DhTreatment::RingRpt { .. }) => Some((
            Color32::from_rgb(196, 112, 80),
            "homogenised particle (kernel + 4 coatings)",
        )),
        Some(DhTreatment::Homogenised) => Some((
            Color32::from_rgb(150, 112, 92),
            "smeared fuel zone (particles + matrix)",
        )),
        Some(DhTreatment::ChordLengthKernel | DhTreatment::SclsKernel) => Some((
            Color32::from_rgb(124, 118, 110),
            "matrix + 4 coatings, homogenised",
        )),
        _ => None,
    };
    p.extend(synth);
    p
}

/// What each treatment keeps and gives up, one line each (the lesson's table).
pub fn keeps_and_gives_up(i: usize) -> (&'static str, &'static str) {
    match TREATMENTS.get(i) {
        Some(DhTreatment::DeltaTracking) => ("every particle, at its packed position", "nothing: the reference"),
        Some(DhTreatment::ChordLength) => (
            "nothing stored: kernel-or-matrix is sampled from chord-length statistics as the neutron flies",
            "where the particles were; and here the kernel is smeared through its whole particle",
        ),
        Some(DhTreatment::Scls) => ("CLS plus the particles met recently, inside a moving window", "as CLS, with a bounded memory"),
        Some(DhTreatment::Homogenised) => ("nothing: the fuel zone is one smeared material", "all grain-level self-shielding"),
        Some(DhTreatment::RingRpt { .. }) => (
            "the same smeared particles, packed into a shell at a FITTED radius",
            "the radial shape of the fuel; the radius is fitted to another code",
        ),
        Some(DhTreatment::ChordLengthKernel) => (
            "CLS with the kernel at full density as the inclusion",
            "where the kernels were; the coatings are smeared into the matrix",
        ),
        Some(DhTreatment::SclsKernel) => ("SCLS with the kernel as the inclusion", "as kernel-level CLS, with a bounded memory"),
        None => ("", ""),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::raster::OUTSIDE;

    /// Every treatment builds from the record's parameters, and every pixel of
    /// a slice through the centre is a material its palette names (or outside
    /// the reflective sphere). The explicit arm stores the particles and the
    /// others store none, which is the saving.
    #[test]
    fn every_treatment_draws_only_materials_its_palette_names() {
        let req = RasterReq {
            id: 1,
            basis: Basis::Xy,
            centre: [0.0, 0.0],
            depth: 0.0,
            width: [6.2, 6.2],
            px: [96, 96],
            param: 0.0,
        };
        for i in 0..TREATMENTS.len() {
            let u = build(i).unwrap();
            assert_eq!(u.particle_count() > 0, i == 0, "{}", SHORT[i]);
            let pal = palette(i);
            assert_eq!(
                pal.len(),
                u.materials().len(),
                "{}: palette against the universe's table",
                SHORT[i]
            );
            let map = raster(&u, &req);
            assert_eq!(map.len(), 96 * 96);
            assert!(
                map.iter()
                    .all(|&m| m == OUTSIDE || (m as usize) < pal.len()),
                "{}",
                SHORT[i]
            );
            // The centre is fuel zone, the corner outside, and FLiBe in between.
            assert_ne!(map[48 * 96 + 48], OUTSIDE);
            assert_eq!(map[0], OUTSIDE);
            assert_eq!(map[48 * 96 + 2], 7, "{}: FLiBe at x = -2.97 cm", SHORT[i]);
        }
    }

    /// What each arm sees inside the fuel zone, on the assembled universe:
    /// explicit shows kernels and coatings, the smear one material, ring-RPT a
    /// shell of homogenised particle between two balls of matrix, CLS a mix of
    /// homogenised particle and matrix.
    #[test]
    fn each_arm_sees_its_own_fuel_zone() {
        let req = RasterReq {
            id: 1,
            basis: Basis::Xy,
            centre: [0.0, 0.0],
            depth: 0.0,
            width: [3.8, 3.8],
            px: [200, 200],
            param: 0.0,
        };
        let seen = |i: usize| {
            let map = raster(&build(i).unwrap(), &req);
            let mut s = [false; 256];
            for (k, &m) in map.iter().enumerate() {
                let (x, y) = ((k % 200) as f64 - 99.5, (k / 200) as f64 - 99.5);
                if (x * x + y * y).sqrt() * 0.019 < 1.85 {
                    s[m as usize] = true;
                }
            }
            (0..256).filter(|&m| s[m]).collect::<Vec<_>>()
        };
        assert_eq!(seen(0), vec![0, 1, 2, 3, 4, 5]);
        assert_eq!(seen(3), vec![8]);
        assert_eq!(seen(4), vec![5, 8]);
        assert_eq!(seen(1), vec![5, 8]);
        assert_eq!(seen(5), vec![0, 8]);
    }
}
