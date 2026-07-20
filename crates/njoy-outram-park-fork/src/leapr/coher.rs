// Ported from NJOY2016 `src/leapr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! Coherent-elastic (Bragg) scattering: the `coher` reciprocal-lattice sum.
//!
//! Ported from NJOY2016 `leapr.f90::coher` (lines 2489–2814) and its helpers
//! `formf` (2924–2970), `tausq`/`taufcc`/`taubcc` (the `contains` block,
//! 2792–2812). Computes the Bragg edge energies and their structure factors for
//! a crystalline moderator (graphite, Be, BeO, Al, Pb, Fe), which THERMR/ENDF
//! MF=7/MT=2 (`LTHR=1`) later turns into the cumulative structure factor `S(E)`.
//!
//! ## Physics
//!
//! For each reciprocal-lattice vector `tau` with `|tau|^2 <= 8 m E_max / hbar^2`,
//! the Bragg edge sits at `E = |tau|^2 / (8 m / hbar^2)` and carries a structure
//! factor proportional to the crystallographic form factor `formf(hkl)` divided
//! by `|tau|`. Edges within a relative tolerance of `1e-6` in energy are merged
//! (their structure factors summed). The Debye-Waller factor is **disabled here**
//! (`wint = 0` in the Fortran, so `exp(-tau^2 t2 wint) = 1`); the temperature
//! dependence enters later in the ENDF writer via `exp(-4 W' E)`.
//!
//! ## Lattice constants — HUMAN RE-VERIFY REQUIRED
//!
//! Every lattice constant below is a hand-transcribed `data`-statement value from
//! `leapr.f90` (2508–2531). They are physical inputs (lattice spacings in cm,
//! atomic masses in amu, coherent cross sections in barn) and a transcription
//! slip would silently shift Bragg edges. They are **flagged for human
//! re-verification against the Fortran source** — see [`LatticeConstants`].
//!
//! **Untrusted AI draft** (crate policy): unit-tested for self-consistency
//! (monotone edges, non-negative structure factors, plausible first-edge energy)
//! but **not** validated against a reference LEAPR run.

use crate::common::phys::{AMASSN_AMU, AMU_G, EV_ERG, HBAR_ERG_S, PI};

/// The crystalline moderator whose Bragg edges are being computed.
///
/// Matches the LEAPR `iel` coherent-elastic option (`leapr.f90` card 5): the
/// discriminant equals the Fortran `lat` passed to `coher`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoherentLattice {
    /// Graphite (hexagonal), `iel = 1`.
    Graphite = 1,
    /// Beryllium (hexagonal), `iel = 2`.
    Beryllium = 2,
    /// Beryllium oxide (hexagonal), `iel = 3`.
    BerylliumOxide = 3,
    /// Aluminium (fcc), `iel = 4`.
    Aluminium = 4,
    /// Lead (fcc), `iel = 5`.
    Lead = 5,
    /// Iron (bcc), `iel = 6`.
    Iron = 6,
}

/// Hand-transcribed lattice/material constants for one moderator (from the
/// `data` statements in `leapr.f90::coher`, lines 2508–2531).
///
/// **These are transcription-sensitive and must be re-verified by a human**
/// against the Fortran source. Fields:
///
/// - `a`, `c` — lattice constants \[cm\] (`c` unused / `0` for cubic lattices).
/// - `amsc` — scatterer atomic mass \[amu\] (feeds the Debye-Waller `t2`, which
///   is inert here because `wint = 0`).
/// - `scoh` — characteristic coherent scattering cross section \[barn\], later
///   divided by the atom count `natom`.
struct LatticeConstants {
    a: f64,
    c: f64,
    amsc: f64,
    scoh: f64,
}

impl CoherentLattice {
    /// The hand-transcribed constants for this lattice (`leapr.f90:2546–2578`).
    ///
    /// `natom` is the number of principal scattering atoms in the compound; the
    /// coherent cross section is normalised by it, as in the Fortran
    /// (`scoh = <table>/natom`).
    fn constants(self, natom: f64) -> LatticeConstants {
        // HUMAN RE-VERIFY: every literal here mirrors a leapr.f90 `parameter`.
        match self {
            // graphite: gr1,gr2,gr3,gr4 (2508–2511)
            CoherentLattice::Graphite => {
                LatticeConstants { a: 2.4573e-8, c: 6.700e-8, amsc: 12.011, scoh: 5.50 / natom }
            }
            // beryllium: be1,be2,be3,be4 (2512–2515)
            CoherentLattice::Beryllium => {
                LatticeConstants { a: 2.2856e-8, c: 3.5832e-8, amsc: 9.01, scoh: 7.53 / natom }
            }
            // beryllium oxide: beo1,beo2,beo3,beo4 (2516–2519)
            CoherentLattice::BerylliumOxide => {
                LatticeConstants { a: 2.695e-8, c: 4.39e-8, amsc: 12.5, scoh: 1.0 / natom }
            }
            // aluminum: al1,al3,al4 (2520–2522) — cubic, no `c`
            CoherentLattice::Aluminium => {
                LatticeConstants { a: 4.04e-8, c: 0.0, amsc: 26.7495, scoh: 1.495 / natom }
            }
            // lead: pb1,pb3,pb4 (2523–2525)
            CoherentLattice::Lead => {
                LatticeConstants { a: 4.94e-8, c: 0.0, amsc: 207.0, scoh: 1.0 / natom }
            }
            // iron: fe1,fe3,fe4 (2526–2528)
            CoherentLattice::Iron => {
                LatticeConstants { a: 2.86e-8, c: 0.0, amsc: 55.454, scoh: 12.9 / natom }
            }
        }
    }
}

/// The result of a `coher` run: Bragg edges in ascending energy.
///
/// Each entry is `(E \[eV\], S_edge)` where `S_edge` is the (non-cumulative)
/// structure-factor contribution of that edge in practical units. The ENDF
/// MF=7/MT=2 writer ([`crate::leapr::endout`]) forms the cumulative,
/// temperature-weighted `S(E)` from these.
#[derive(Debug, Clone, PartialEq)]
pub struct BraggEdges {
    /// `(energy_eV, structure_factor)` pairs, ascending in energy, near-degenerate
    /// edges (within `1e-6` relative energy) already merged.
    pub edges: Vec<(f64, f64)>,
}

/// `tausq` — squared reciprocal-lattice vector for a hexagonal lattice
/// (`leapr.f90:2792–2797`).
fn tausq(m1: i64, m2: i64, m3: i64, c1: f64, c2: f64, twopis: f64) -> f64 {
    let (m1, m2, m3) = (m1 as f64, m2 as f64, m3 as f64);
    (c1 * (m1 * m1 + m2 * m2 + m1 * m2) + m3 * m3 * c2) * twopis
}

/// `taufcc` — squared reciprocal-lattice vector for an fcc lattice
/// (`leapr.f90:2799–2805`).
fn taufcc(m1: i64, m2: i64, m3: i64, c1: f64, twothd: f64, twopis: f64) -> f64 {
    let (m1, m2, m3) = (m1 as f64, m2 as f64, m3 as f64);
    c1 * (m1 * m1 + m2 * m2 + m3 * m3 + twothd * m1 * m2 + twothd * m1 * m3 - twothd * m2 * m3)
        * twopis
}

/// `taubcc` — squared reciprocal-lattice vector for a bcc lattice
/// (`leapr.f90:2807–2812`). `c1 = 2/a^2` is supplied by the caller.
fn taubcc(m1: i64, m2: i64, m3: i64, c1: f64, twopis: f64) -> f64 {
    let (m1, m2, m3) = (m1 as f64, m2 as f64, m3 as f64);
    c1 * (m1 * m1 + m2 * m2 + m3 * m3 + m1 * m2 + m2 * m3 + m1 * m3) * twopis
}

/// `formf` — crystallographic form factor for the given lattice and Miller
/// indices (`leapr.f90:2924–2970`).
fn formf(lat: CoherentLattice, l1: i64, l2: i64, l3: i64) -> f64 {
    // BeO form-factor constants c1,c2,c3 (2939–2941). HUMAN RE-VERIFY.
    const C1: f64 = 7.54;
    const C2: f64 = 4.24;
    const C3: f64 = 11.31;
    let (f1, f2, f3) = (l1 as f64, l2 as f64, l3 as f64);
    match lat {
        CoherentLattice::Graphite => {
            // l3 even vs odd (integer test, l3 >= 0 in the hexagonal sum).
            if l3 % 2 != 0 {
                (PI * (f1 - f2) / 3.0).sin().powi(2)
            } else {
                (6.0 + 10.0 * (2.0 * PI * (f1 - f2) / 3.0).cos()) / 4.0
            }
        }
        CoherentLattice::Beryllium => 1.0 + (2.0 * PI * (2.0 * f1 + 4.0 * f2 + 3.0 * f3) / 6.0).cos(),
        CoherentLattice::BerylliumOxide => {
            (1.0 + (2.0 * PI * (2.0 * f1 + 4.0 * f2 + 3.0 * f3) / 6.0).cos())
                * (C1 + C2 + C3 * (3.0 * PI * f3 / 4.0).cos())
        }
        CoherentLattice::Aluminium | CoherentLattice::Lead => {
            let e1 = 2.0 * PI * f1;
            let e2 = 2.0 * PI * (f1 + f2);
            let e3 = 2.0 * PI * (f1 + f3);
            (1.0 + e1.cos() + e2.cos() + e3.cos()).powi(2)
                + (e1.sin() + e2.sin() + e3.sin()).powi(2)
        }
        CoherentLattice::Iron => {
            let e1 = 2.0 * PI * (f1 + f2 + f3);
            (1.0 + e1.cos()).powi(2) + e1.sin().powi(2)
        }
    }
}

/// Insert one `(tau^2, f)` edge into the working list, merging with a nearby
/// existing edge when appropriate (`leapr.f90:2630–2654`, the hexagonal path).
///
/// For small `tau^2` (below `tsqx`) or an empty list every vector opens a new
/// edge; otherwise a vector whose `tau^2` falls within `[b_i, (1+eps) b_i)` of an
/// existing edge adds its form factor to that edge instead of opening a new one.
fn insert_hex_edge(edges: &mut Vec<(f64, f64)>, tsq: f64, f: f64, tsqx: f64) {
    const EPS: f64 = 0.05;
    if edges.is_empty() || tsq <= tsqx {
        edges.push((tsq, f));
        return;
    }
    for e in edges.iter_mut() {
        if tsq >= e.0 && tsq < (1.0 + EPS) * e.0 {
            e.1 += f;
            return;
        }
    }
    edges.push((tsq, f));
}

/// Compute Bragg edges and structure factors for coherent elastic scattering.
///
/// Faithful port of `leapr.f90::coher`. `natom` is the number of principal
/// scattering atoms in the compound (LEAPR `npr`); `emax_ev` is the maximum
/// incident energy to tabulate (LEAPR uses `5` eV).
///
/// Returns [`BraggEdges`] sorted ascending in energy with near-degenerate edges
/// merged, plus a synthetic final edge at `emax_ev` carrying the last structure
/// factor (mirroring the Fortran, which appends `(ulim, last_f)` so the ENDF
/// histogram extends to `E_max`).
///
/// # Panics
/// Does not panic. Extremely large lattices could in principle allocate a large
/// edge list; `emax_ev = 5` keeps this to a few thousand edges for the built-in
/// moderators (matching NJOY's `maxb = 60000` word budget).
pub fn coher(lat: CoherentLattice, natom: usize, emax_ev: f64) -> BraggEdges {
    const TWOTHD: f64 = 0.666_666_666_667;
    const SQRT3: f64 = 1.732_050_808;
    const TOLER: f64 = 1.0e-6;

    let LatticeConstants { a, c, amsc: _amsc, scoh } = lat.constants(natom as f64);

    // energy <-> tau^2 conversion factors (2541–2545).
    let twopis = (2.0 * PI).powi(2);
    let amne = AMASSN_AMU * AMU_G;
    let econ = EV_ERG * 8.0 * (amne / HBAR_ERG_S) / HBAR_ERG_S;
    let recon = 1.0 / econ;
    let tsqx = econ / 20.0;

    // lattice-family coefficients (2582–2592).
    let is_hex = matches!(
        lat,
        CoherentLattice::Graphite | CoherentLattice::Beryllium | CoherentLattice::BerylliumOxide
    );
    let is_fcc = matches!(lat, CoherentLattice::Aluminium | CoherentLattice::Lead);
    let (c1, c2, scon) = if is_hex {
        (
            4.0 / (3.0 * a * a),
            1.0 / (c * c),
            scoh * (4.0 * PI).powi(2) / (2.0 * a * a * c * SQRT3 * econ),
        )
    } else if is_fcc {
        (3.0 / (a * a), 0.0, scoh * (4.0 * PI).powi(2) / (16.0 * a * a * a * econ))
    } else {
        // iron (bcc)
        (2.0 / (a * a), 0.0, scoh * (4.0 * PI).powi(2) / (8.0 * a * a * a * econ))
    };

    let ulim = econ * emax_ev;
    // Debye-Waller factor is disabled in coher (wint = 0 in the Fortran, 2593),
    // so exp(-tau^2 t2 wint) = 1 everywhere; we omit it entirely.

    let mut edges: Vec<(f64, f64)> = Vec::new();

    if is_hex {
        let phi = ulim / twopis;
        let i1m = (a * phi.sqrt()) as i64 + 1;
        for i1 in 1..=i1m {
            let l1 = i1 - 1;
            let inner = 3.0 * (a * a * phi - (l1 * l1) as f64);
            let i2m = (((l1 as f64) + inner.max(0.0).sqrt()) / 2.0) as i64 + 1;
            for i2 in i1..=i2m {
                let l2 = i2 - 1;
                let x = phi - c1 * ((l1 * l1 + l2 * l2 - l1 * l2) as f64);
                let i3m = if x > 0.0 { (c * x.sqrt()) as i64 + 1 } else { 1 };
                for i3 in 1..=i3m {
                    let l3 = i3 - 1;
                    // symmetry weights (2617–2623)
                    let w1 = if l1 == l2 { 1.0 } else { 2.0 };
                    let mut w2 = if l1 == 0 || l2 == 0 { 1.0 } else { 2.0 };
                    if l1 == 0 && l2 == 0 {
                        w2 = 1.0;
                        w2 /= 2.0;
                    }
                    let w3 = if l3 == 0 { 1.0 } else { 2.0 };
                    let wgt = w1 * w2 * w3;

                    for &sign in &[1_i64, -1_i64] {
                        let l2s = sign * l2;
                        let tsq = tausq(l1, l2s, l3, c1, c2, twopis);
                        if tsq > 0.0 && tsq <= ulim {
                            let tau = tsq.sqrt();
                            let f = (wgt / tau) * formf(lat, l1, l2s, l3);
                            insert_hex_edge(&mut edges, tsq, f, tsqx);
                        }
                    }
                }
            }
        }
    } else {
        // fcc / bcc: fixed cubic index box (2698, 2725).
        const I1M: i64 = 15;
        for i1 in -I1M..=I1M {
            for i2 in -I1M..=I1M {
                for i3 in -I1M..=I1M {
                    let tsq = if is_fcc {
                        taufcc(i1, i2, i3, c1, TWOTHD, twopis)
                    } else {
                        taubcc(i1, i2, i3, c1, twopis)
                    };
                    if tsq > 0.0 && tsq <= ulim {
                        let tau = tsq.sqrt();
                        let f = (1.0 / tau) * formf(lat, i1, i2, i3);
                        edges.push((tsq, f));
                    }
                }
            }
        }
    }

    // sort ascending by tau^2 (2749–2762; the Fortran selection sort).
    edges.sort_by(|x, y| x.0.partial_cmp(&y.0).unwrap());

    // append the synthetic final edge at ulim (2763–2765).
    let last_f = edges.last().map(|e| e.1).unwrap_or(0.0);
    edges.push((ulim, last_f));

    // convert to practical units and merge near-degenerate edges (2770–2783).
    let mut merged: Vec<(f64, f64)> = Vec::with_capacity(edges.len());
    let mut bel = -1.0_f64;
    for (tsq, f) in edges {
        let be = tsq * recon;
        let bs = f * scon;
        if be - bel < TOLER {
            if let Some(last) = merged.last_mut() {
                last.1 += bs;
            } else {
                merged.push((be, bs));
                bel = be;
            }
        } else {
            merged.push((be, bs));
            bel = be;
        }
    }

    BraggEdges { edges: merged }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graphite_edges_are_monotone_and_nonnegative() {
        let br = coher(CoherentLattice::Graphite, 1, 5.0);
        assert!(br.edges.len() > 10, "graphite has many Bragg edges below 5 eV");
        // energies strictly ascending after the merge
        assert!(
            br.edges.windows(2).all(|w| w[1].0 > w[0].0),
            "Bragg edge energies strictly ascending"
        );
        // structure factors non-negative
        assert!(br.edges.iter().all(|&(_, s)| s >= 0.0), "structure factors >= 0");
        // Forbidden reflections (e.g. graphite (001), form factor 0) legitimately
        // remain in the grid with zero structure factor — NJOY keeps them.
        assert!(
            br.edges.iter().any(|&(_, s)| s == 0.0),
            "some forbidden edges (S = 0) are retained, as NJOY does"
        );
        // The first *allowed* Bragg edge (S > 0) is graphite's (002) reflection at
        // ~1.83e-3 eV (d = c/2 = 3.35 Å, lambda_max = 6.70 Å). Loose sanity window.
        let first_allowed = br.edges.iter().find(|&&(_, s)| s > 0.0).unwrap().0;
        assert!(
            (1.0e-3..3.0e-3).contains(&first_allowed),
            "graphite first allowed Bragg edge ~1.8e-3 eV, got {first_allowed:e}"
        );
    }

    #[test]
    fn all_lattices_produce_ascending_edges() {
        for lat in [
            CoherentLattice::Graphite,
            CoherentLattice::Beryllium,
            CoherentLattice::BerylliumOxide,
            CoherentLattice::Aluminium,
            CoherentLattice::Lead,
            CoherentLattice::Iron,
        ] {
            let br = coher(lat, 1, 5.0);
            assert!(!br.edges.is_empty(), "{lat:?} yields edges");
            assert!(
                br.edges.windows(2).all(|w| w[1].0 >= w[0].0),
                "{lat:?} edge energies ascending"
            );
            assert!(br.edges.iter().all(|&(_, s)| s >= 0.0), "{lat:?} structure factors >= 0");
            // final synthetic edge sits at emax after unit conversion
            let last_e = br.edges.last().unwrap().0;
            assert!(last_e <= 5.0 + 1e-6, "{lat:?} last edge <= emax");
        }
    }

    #[test]
    fn form_factor_graphite_even_odd_branches() {
        // l3 odd -> sin^2 branch; l1==l2 -> 0.
        assert!((formf(CoherentLattice::Graphite, 1, 1, 1)).abs() < 1e-12);
        // l3 even, l1-l2 multiple of 3 -> (6+10)/4 = 4.
        assert!((formf(CoherentLattice::Graphite, 3, 0, 0) - 4.0).abs() < 1e-9);
    }
}
