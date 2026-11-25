//! **The MSRE N-1 tallies**: the neutron balance by material, the salt's
//! fine-group spectrum with its absorption split by nuclide, and the N-4
//! axial fission and flux on an R-Z mesh (gh:#1128, #1130).
//!
//! **Moved 2026-11-17** from `examples/msre_n1_first_criticality.rs`
//! (maintainer: the validation cases live in the library, especially what can
//! be `no_std`), arithmetic unchanged. `no_std` + `alloc`: these build tallies
//! and reduce them to numbers; the example still writes the CSV rows.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
#[allow(unused_imports)]
use petir::mathf::RealMath;

use super::{H_CORE, R_GRAPHITE, R_VESSEL_IN, SALT_IDX};
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::tally::filter::{EnergyFilter, FilterKind, MaterialFilter, MeshFilter};
use outram_mc_libs::tally::mesh::{CylindricalMesh, MeshKind, MeshKindExt};
use outram_mc_libs::tally::tally::{ScoreType, Tally, TallyBin};

/// Axial z edges for the N-4 tally: 4 bins through the lower plenum, 32
/// through the graphite, 4 through the upper plenum (cold dimensions).
pub fn axial_edges() -> Vec<f64> {
    let (lp, up, h) = (super::lower_plenum_height(), super::upper_plenum_height(), H_CORE);
    let mut z: Vec<f64> = (0..4).map(|i| -lp + lp * i as f64 / 4.0).collect();
    z.extend((0..32).map(|i| h * i as f64 / 32.0));
    z.extend((0..=4).map(|i| h + up * i as f64 / 4.0));
    z
}

/// Absorption and nu-fission in every material, for the neutron balance.
/// Layout `[material * 2 + score]`.
pub fn balance_tally(n_materials: usize) -> Tally {
    Tally {
        id: 2,
        name: "N-1 neutron balance by material".into(),
        filters: vec![FilterKind::Material(MaterialFilter { material_indices: (0..n_materials).collect() })],
        scores: vec![ScoreType::Absorption, ScoreType::NuFission],
        bins: vec![TallyBin::default(); n_materials * 2],
        derivative: None,
    }
}

/// Energy-bin edges for the salt spectrum, 20 000 logarithmic bins in three
/// spans: 1 000 from 1e-5 eV to 1 eV, 17 000 from 1 eV to 100 keV (3 400 per
/// decade, the resonance range: at U-238's 6.67 eV resonance a bin is
/// 4.5e-3 eV, a fifth of its 0.025 eV width) and 2 000 from 100 keV to
/// 20 MeV. A first version with 100 000 uniform bins ran out of memory
/// (14 GB) at production statistics, because the tally is copied per batch.
pub fn salt_spectrum_edges() -> Vec<f64> {
    let span = |lo: f64, hi: f64, n: usize| -> Vec<f64> {
        (0..n).map(|i| (lo.r_ln() + (hi.r_ln() - lo.r_ln()) * i as f64 / n as f64).r_exp()).collect()
    };
    let mut e = span(1e-5, 1.0, 1_000);
    e.extend(span(1.0, 1e5, 17_000));
    e.extend(span(1e5, 2e7, 2_000));
    e.push(2e7);
    e
}

/// Flux and absorption in the salt, by [`salt_spectrum_edges`] bin. Layout
/// `[group * 2 + score]`, score 0 flux, 1 absorption.
pub fn salt_spectrum_tally() -> Tally {
    let edges = salt_spectrum_edges();
    let n = edges.len() - 1;
    Tally {
        id: 3,
        name: "N-1 salt spectrum".into(),
        filters: vec![
            FilterKind::Material(MaterialFilter { material_indices: vec![SALT_IDX] }),
            FilterKind::Energy(EnergyFilter { bins: edges }),
        ],
        scores: vec![ScoreType::Flux, ScoreType::Absorption],
        bins: vec![TallyBin::default(); n * 2], derivative: None,
    }
}

/// The salt's absorption split by nuclide, from a run's
/// [`salt_spectrum_tally`].
#[derive(Debug, Clone, PartialEq)]
pub struct SaltSplit {
    /// Per salt nuclide, in the order given: `(name, [thermal, epithermal,
    /// fast])` absorption. The three add to the nuclide's total.
    pub rows: Vec<(String, [f64; 3])>,
    /// The sum over nuclides of the reconstructed absorption.
    pub reconstructed_total: f64,
    /// The directly tallied salt absorption, for the closure check.
    pub tallied_total: f64,
}

/// Split the salt's absorption by nuclide: `A_i = sum_g N_i sigma_a,i(E_g)
/// phi_g`, with `E_g` the bin's geometric midpoint and the cross section at
/// `t_k` (infinitely dilute in the unresolved range). Each row also splits
/// the absorption into three bands by the bin's lower edge: thermal (below
/// 0.625 eV), epithermal (0.625 eV to 100 keV) and fast (above 100 keV).
/// `salt` is `(nuclide, atoms/b-cm)`; `names[i]` names `nuclides[i]`.
///
/// # Panics
/// If a salt nuclide is not in `names`.
pub fn salt_absorption_split(t: &Tally, salt: &[(&str, f64)], names: &[&str], nuclides: &[Nuclide], t_k: f64) -> SaltSplit {
    let edges = salt_spectrum_edges();
    let n = edges.len() - 1;
    let mut rows = Vec::with_capacity(salt.len());
    let mut reconstructed_total = 0.0;
    for (name, density) in salt {
        let nuc = &nuclides[names.iter().position(|k| k == name).expect("salt nuclide loaded")];
        let mut band = [0.0_f64; 3];
        for g in 0..n {
            let phi = t.bins[g * 2].sum;
            if phi != 0.0 {
                let e = (edges[g] * edges[g + 1]).sqrt();
                let b = if edges[g] < 0.625 { 0 } else if edges[g] < 1.0e5 { 1 } else { 2 };
                band[b] += density * nuc.xs_at_energy(e, t_k).absorption.max(0.0) * phi;
            }
        }
        reconstructed_total += band[0] + band[1] + band[2];
        rows.push(((*name).to_string(), band));
    }
    let tallied_total: f64 = (0..n).map(|g| t.bins[g * 2 + 1].sum).sum();
    SaltSplit { rows, reconstructed_total, tallied_total }
}

/// Fission and flux on an R-Z mesh: r in [0, graphite radius] and [graphite
/// radius, vessel inner radius], one azimuthal bin, [`axial_edges`], split at
/// 0.625 eV so the flux above it can stand in for a fast importance. Layout
/// `[(mesh bin * 2 + energy bin) * 2 + score]`.
pub fn axial_tally() -> Tally {
    let mesh = MeshKind::Cylindrical(CylindricalMesh {
        r_grid: vec![0.0, R_GRAPHITE, R_VESSEL_IN],
        phi_grid: vec![0.0, 2.0 * core::f64::consts::PI],
        z_grid: axial_edges(),
        origin: Position::ZERO,
    });
    let n = mesh.n_bins();
    Tally {
        id: 1,
        name: "N-4 axial fission and flux".into(),
        filters: vec![FilterKind::Mesh(MeshFilter { mesh }), FilterKind::Energy(EnergyFilter { bins: vec![1e-11, 0.625, 2e7] })],
        scores: vec![ScoreType::Fission, ScoreType::Flux],
        bins: vec![TallyBin::default(); n * 2 * 2], derivative: None,
    }
}

/// One z bin of [`axial_tally`]: fission and flux inside the graphite radius
/// and in the ring outside it, and the fast flux inside.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AxialRow {
    /// Bin bounds \[cm\].
    pub z_lo: f64,
    /// Upper bound \[cm\].
    pub z_hi: f64,
    /// Fission inside the graphite radius, both energy groups.
    pub fission_inner: f64,
    /// Fission in the outer ring.
    pub fission_outer: f64,
    /// Flux inside the graphite radius, both groups.
    pub flux_inner: f64,
    /// Flux in the outer ring.
    pub flux_outer: f64,
    /// Flux above 0.625 eV inside the graphite radius.
    pub flux_fast_inner: f64,
}

/// A run's [`axial_tally`], one row per z bin.
pub fn axial_rows(t: &Tally) -> Vec<AxialRow> {
    let FilterKind::Mesh(MeshFilter { mesh }) = &t.filters[0] else { unreachable!("mesh filter") };
    let z = axial_edges();
    z.windows(2)
        .map(|w| {
            let zc = 0.5 * (w[0] + w[1]);
            let bin = |r: f64| mesh.bin(Position::new(r, 0.0, zc)).expect("inside the axial mesh");
            let (bi, bo) = (bin(0.5 * R_GRAPHITE), bin(0.5 * (R_GRAPHITE + R_VESSEL_IN)));
            // Layout: [(mesh bin * 2 + energy bin) * 2 + score]; energy 0 thermal, 1 fast.
            let get = |mb: usize, e: usize, sc: usize| t.bins[(mb * 2 + e) * 2 + sc].sum;
            let both = |mb: usize, sc: usize| get(mb, 0, sc) + get(mb, 1, sc);
            AxialRow {
                z_lo: w[0],
                z_hi: w[1],
                fission_inner: both(bi, 0),
                fission_outer: both(bo, 0),
                flux_inner: both(bi, 1),
                flux_outer: both(bo, 1),
                flux_fast_inner: get(bi, 1, 1),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The edges are increasing and span what the docs say; each tally's bin
    /// count matches its filters; an axial tally with one marked bin reads
    /// back in the right row and column; and an empty spectrum splits to
    /// zero with zero tallied.
    #[test]
    fn tallies_have_their_documented_shape() {
        let z = axial_edges();
        assert_eq!(z.len(), 41);
        assert!(z.windows(2).all(|w| w[1] > w[0]));
        let e = salt_spectrum_edges();
        assert_eq!(e.len(), 20_001);
        assert!(e.windows(2).all(|w| w[1] > w[0]));
        assert!((e[0] - 1e-5).abs() < 1e-18 && e[20_000] == 2e7);
        assert_eq!(balance_tally(9).bins.len(), 18);
        assert_eq!(salt_spectrum_tally().bins.len(), 40_000);

        let mut t = axial_tally();
        assert_eq!(t.bins.len(), 2 * 40 * 2 * 2);
        let FilterKind::Mesh(MeshFilter { mesh }) = &t.filters[0] else { unreachable!() };
        let mb = mesh.bin(Position::new(0.5 * R_GRAPHITE, 0.0, 0.5 * (z[10] + z[11]))).unwrap();
        t.bins[(mb * 2 + 1) * 2].sum = 3.0; // fast fission, inner, z bin 10
        t.bins[(mb * 2 + 1) * 2 + 1].sum = 5.0; // fast flux
        let rows = axial_rows(&t);
        assert_eq!(rows.len(), 40);
        assert_eq!((rows[10].fission_inner, rows[10].flux_fast_inner, rows[10].flux_inner), (3.0, 5.0, 5.0));
        assert_eq!(rows[11].fission_inner, 0.0);

        let split = salt_absorption_split(&salt_spectrum_tally(), &[], &[], &[], 900.0);
        assert!(split.rows.is_empty() && split.reconstructed_total == 0.0 && split.tallied_total == 0.0);
    }
}
