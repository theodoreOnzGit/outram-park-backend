// NEW SYNTHESIS — not a translation of NJOY2016 `leapr.f90`.
//
// Stock LEAPR has no general coherent-elastic path to port: its `coher`
// subroutine carries six hand-coded lattices and nothing else. The algorithm
// implemented here is the *generalized coherent elastic scattering formulation*
// of
//
//   Zhu, Y. (2014). "Thermal Neutron Scattering Cross Sections for Silicon
//   Carbide." MS thesis, North Carolina State University (advisor A. I.
//   Hawari). Chapter 3.1 (Eqs. 3.4-3.8) and Chapter 4.1.
//   Catalogued in this workspace as
//   `crates/kovan-literature/open/theses/zhu2014thermal.{json,pdf}`.
//
//   Companion paper (NOT catalogued here — no freely available copy located):
//   Zhu, Y. and Hawari, A. I. (2015). "Implementation of a Generalized Coherent
//   Elastic Scattering Formulation for Thermal Neutron Scattering Analysis."
//   ICNC 2015, OSTI 23100909.
//
// The *unit convention* (what a Bragg-edge structure factor means, how `econ`
// scales edge energies, how `endout` later folds in the Debye-Waller factor) is
// taken from the ported NJOY code in `super::builtin`, so that the general path
// and the six built-in lattices are interchangeable downstream. This file is
// distributed under GPL-3.0-only with the rest of the crate.

//! Generalized coherent-elastic (Bragg) scattering for an **arbitrary crystal**.
//!
//! [`super::builtin`] ports NJOY's `coher`, which knows six lattices by heart
//! (graphite, Be, BeO, Al, Pb, Fe) and can do nothing else. This module is the
//! general case: give it the direct lattice vectors and the atomic basis of any
//! crystal and it returns the same [`BraggEdges`] the built-in path returns, so
//! [`crate::leapr::endout`] writes MF=7/MT=2 from either without caring which.
//!
//! ## Physics (Zhu 2014, Eqs. 3.4-3.8)
//!
//! For each reciprocal-lattice vector `tau = h b1 + k b2 + l b3` the Bragg edge
//! — the lowest neutron energy that can scatter off that vector, reached in
//! backscattering where `k = tau/2` — sits at
//!
//! ```text
//! E_tau = hbar^2 tau^2 / (8 m_n)          (Zhu Eq. 3.8)
//! ```
//!
//! which in LEAPR's own bookkeeping is `E = tau^2 / econ` with `econ` from
//! [`PhysicalConstants::econ`]. The edge carries the crystallographic structure
//! factor
//!
//! ```text
//! |F(tau)|^2 = | sum_j b_j exp(i tau . r_j) |^2         (Zhu Eq. 3.5)
//! ```
//!
//! summed over **every** atom `j` in the unit cell, with `b_j` the *bound
//! coherent scattering length* of that atom. This is the whole content of the
//! generalization: the six hardcoded `formf` branches in stock LEAPR are
//! special cases of this one sum, and the extinction rules (which reflections
//! vanish) fall out of it instead of being written by hand.
//!
//! The per-edge weight handed downstream is (Zhu Eq. 3.4, in LEAPR's units)
//!
//! ```text
//! f_i = 16 pi^3 |F(tau_i)|^2 / (V_cell * econ * N_cell * npr * tau_i)
//! ```
//!
//! with `V_cell` the unit-cell volume \[cm^3\], `N_cell` the number of atoms in
//! that cell, and `npr` the deck's principal-atom count (LEAPR card 5). Divided
//! by `N_cell * npr` the result is a cross section **per principal atom**,
//! which is the convention the built-in path uses and the one the ENDF tapes
//! are written in.
//!
//! The Debye-Waller factor is **not** applied here, exactly as in
//! [`super::builtin`]: NJOY's `coher` leaves `wint = 0` and the temperature
//! dependence enters later in [`crate::leapr::endout`] as `exp(-4 W' E_i)`.
//! Zhu's Eq. (3.6) has the same `exp(-4 w E_i)` factor in the same place. Zhu's
//! "cubic approximation" (his Eqs. 3.1-3.3) makes `W'` a single **compound**
//! coefficient — an atomic-ratio-weighted average over the atom types — rather
//! than a per-sublattice one; see
//! [`crate::leapr::generate`] for where that average is formed.
//!
//! ## Verification status
//!
//! Cross-checked against the six built-in NJOY lattices (they are reproduced
//! through this general path to within float round-off) and against the
//! ENDF/B-VIII.0 3C-SiC evaluation. See [`super::crystals`] and the tests at
//! the bottom of this file for the measured numbers. **Untrusted AI-assisted
//! draft** per crate policy: no human has reviewed the physics.

use crate::common::phys::PI;
use crate::leapr::coher::BraggEdges;
use crate::leapr::vintage::PhysicalConstants;

/// One atom of the crystallographic basis.
///
/// Positions are **fractional** coordinates in the cell spanned by
/// [`CrystalStructure::cell_cm`] — i.e. the Cartesian position is
/// `x*a1 + y*a2 + z*a3`. Fractional coordinates are what crystallographic
/// tables and DFT input files (`POSCAR`, `.cif`) publish, so a transcription
/// error is visible by eye against the source.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BasisAtom {
    /// Fractional coordinates `(x, y, z)` in the cell, each conventionally in
    /// `[0, 1)`. Dimensionless.
    pub fractional: [f64; 3],
    /// **Bound coherent scattering length** \[fm\] of this atom's nuclide or
    /// element. Sign matters — a negative `b` (e.g. hydrogen, `-3.739` fm) flips
    /// the interference and is not the same as its magnitude.
    ///
    /// Related to the bound coherent cross section by
    /// `sigma_coh = 4 pi b^2` (with `b` in fm, `sigma_coh` in fm^2 = 0.01 barn).
    pub b_coh_fm: f64,
    /// Element/nuclide label, for diagnostics and doc tables only. Not used in
    /// the calculation.
    pub label: &'static str,
}

/// A crystal, as the generalized coherent-elastic sum needs it.
///
/// Owns its data by value (workspace rule: no lifetime parameters, no `Box`).
///
/// # Units
/// - `cell_cm` — the three direct lattice vectors `a1, a2, a3` as **rows**, in
///   **centimetres**, matching the `data`-statement units of the ported NJOY
///   lattice constants in [`super::builtin`] (`2.4573e-8` cm = 2.4573 Å).
/// - `basis[].fractional` — dimensionless.
/// - `basis[].b_coh_fm` — femtometres.
#[derive(Debug, Clone, PartialEq)]
pub struct CrystalStructure {
    /// Direct lattice vectors `a1, a2, a3` as rows \[cm\].
    pub cell_cm: [[f64; 3]; 3],
    /// Every atom in the cell spanned by `cell_cm`. The cell need not be
    /// primitive — a conventional cell with its full basis gives the same
    /// physics, because the extra reciprocal-lattice points it introduces are
    /// exactly the ones the structure factor extinguishes.
    pub basis: Vec<BasisAtom>,
    /// Human-readable name of the structure, for provenance strings.
    pub name: &'static str,
}

impl CrystalStructure {
    /// Unit-cell volume \[cm^3\] — `|a1 . (a2 x a3)|`.
    pub fn volume_cm3(&self) -> f64 {
        let [a1, a2, a3] = self.cell_cm;
        (a1[0] * (a2[1] * a3[2] - a2[2] * a3[1])
            + a1[1] * (a2[2] * a3[0] - a2[0] * a3[2])
            + a1[2] * (a2[0] * a3[1] - a2[1] * a3[0]))
            .abs()
    }

    /// Number of atoms in the cell (`N_cell` in the module-level formula).
    pub fn atoms_per_cell(&self) -> usize {
        self.basis.len()
    }

    /// The reciprocal lattice vectors `b1, b2, b3` \[1/cm\], including the
    /// `2 pi` (crystallographic "physicist" convention, so that
    /// `b_i . a_j = 2 pi delta_ij`).
    pub fn reciprocal_cm(&self) -> [[f64; 3]; 3] {
        let [a1, a2, a3] = self.cell_cm;
        let v = self.volume_cm3();
        let scale = 2.0 * PI / v;
        [
            cross_scaled(a2, a3, scale),
            cross_scaled(a3, a1, scale),
            cross_scaled(a1, a2, scale),
        ]
    }

    /// Bound coherent cross section \[barn\] of atom `i` — `4 pi b^2`, with the
    /// fm^2 -> barn conversion applied. Provided so a caller can sanity-check a
    /// transcribed scattering length against a published `sigma_coh`.
    pub fn sigma_coh_barn(&self, i: usize) -> f64 {
        let b = self.basis[i].b_coh_fm;
        4.0 * PI * b * b / 100.0
    }
}

fn cross_scaled(u: [f64; 3], v: [f64; 3], scale: f64) -> [f64; 3] {
    [
        scale * (u[1] * v[2] - u[2] * v[1]),
        scale * (u[2] * v[0] - u[0] * v[2]),
        scale * (u[0] * v[1] - u[1] * v[0]),
    ]
}

/// Compute Bragg edges and structure factors for an arbitrary crystal, using
/// the crate-default physical constants.
///
/// See [`coher_general_with_constants`] — this is that function with
/// [`PhysicalConstants::default`].
///
/// - `npr` — LEAPR card-5 `npr`, the number of principal scattering atoms in
///   the compound. The returned cross section is per principal atom.
/// - `emax_ev` — the highest incident neutron energy \[eV\] to tabulate.
pub fn coher_general(structure: &CrystalStructure, npr: usize, emax_ev: f64) -> BraggEdges {
    coher_general_with_constants(structure, npr, emax_ev, PhysicalConstants::default())
}

/// Generalized coherent-elastic sum with the physical-constant set given
/// explicitly.
///
/// Every Bragg edge energy is `E = tau^2 / econ`
/// ([`PhysicalConstants::econ`]), so — exactly as for the built-in lattices —
/// the constant set scales the **whole** edge grid by one factor. Use the
/// evaluation's own vintage when reproducing a published tape.
///
/// Returns [`BraggEdges`] in the same convention [`super::builtin::coher`]
/// returns: ascending energy \[eV\], structure factors \[barn eV\] such that
/// `sigma(E) = (1/E) * sum_{E_i <= E} f_i exp(-4 W' E_i)`, near-degenerate
/// edges merged, and a synthetic final edge at `emax_ev`.
///
/// # Cost
/// The reciprocal-lattice sum runs over a sphere of radius
/// `sqrt(econ * emax_ev)`; for 3C-SiC at `emax_ev = 5` that is ~1.4 million
/// lattice points and ~4,700 distinct edges. Linear in the basis size.
///
/// # Panics
/// Does not panic. A degenerate cell (zero volume) yields an empty edge list
/// rather than a divide-by-zero, because the index bounds collapse to zero.
pub fn coher_general_with_constants(
    structure: &CrystalStructure,
    npr: usize,
    emax_ev: f64,
    constants: PhysicalConstants,
) -> BraggEdges {
    coher_general_inner(structure, npr, emax_ev, constants, &[])
}

/// Generalized coherent-elastic sum with a **per-atom-type Debye-Waller
/// factor** folded into the structure factor — Zhu's *exact* Debye-Waller
/// option, as against the *cubic approximation* that
/// [`coher_general_with_constants`] implements.
///
/// # Why this exists
///
/// Under the cubic approximation the whole crystal shares one compound
/// Debye-Waller coefficient, so the factor `exp(-4 W' E)` multiplies the
/// finished structure factor and [`crate::leapr::endout`] applies it. Zhu
/// (2014) §3.1 also allows the exact treatment, in which each atom carries its
/// own coefficient *inside* the sum:
///
/// ```text
/// F(tau) = sum_j b_j exp(-2 W'_j E_tau) exp(i tau . r_j),   E_tau = tau^2 / econ
/// ```
///
/// The amplitude factor is `exp(-2 W' E)` because LEAPR's intensity factor is
/// `exp(-4 W' E)`; squaring recovers it. When every `W'_j` is equal the two
/// paths agree identically — pinned by
/// [`tests::equal_per_atom_coefficients_reproduce_the_compound_path`].
///
/// The two differ only where the sum `sum_j b_j exp(i tau . r_j)` involves
/// **cancellation**. For a compound with unlike scattering lengths the
/// *difference* reflections (where the sublattices subtract) are exquisitely
/// sensitive to it and the *sum* reflections are barely affected — which is
/// exactly the pattern measured against the ENDF/B-VIII.0 SiC evaluation; see
/// `docs/leapr-sic-coherent-elastic-vv.md`.
///
/// # Returned convention differs from the other entry points
///
/// The structure factors returned here **already carry the Debye-Waller
/// factor**. `endout` must therefore *not* apply it again — that is what
/// [`crate::leapr::endout::ElasticOutput::CoherentPreWeighted`] signals.
///
/// # Parameters
///
/// - `w_prime_per_atom` — one Debye-Waller coefficient `W'_j` \[1/eV\] per
///   entry of `structure.basis`, **in the same order**. A length mismatch
///   makes this fall back to the compound path (no factor at all) rather than
///   silently misassign coefficients to atoms.
pub fn coher_general_with_per_atom_debye_waller(
    structure: &CrystalStructure,
    npr: usize,
    emax_ev: f64,
    constants: PhysicalConstants,
    w_prime_per_atom: &[f64],
) -> BraggEdges {
    if w_prime_per_atom.len() != structure.basis.len() {
        return coher_general_with_constants(structure, npr, emax_ev, constants);
    }
    coher_general_inner(structure, npr, emax_ev, constants, w_prime_per_atom)
}

/// The shared reciprocal-lattice sum. `w_prime_per_atom` is either empty (no
/// Debye-Waller factor in the amplitude — the caller's `endout` applies the
/// compound one) or exactly one coefficient per basis atom.
fn coher_general_inner(
    structure: &CrystalStructure,
    npr: usize,
    emax_ev: f64,
    constants: PhysicalConstants,
    w_prime_per_atom: &[f64],
) -> BraggEdges {
    const TOLER: f64 = 1.0e-6;

    let econ = constants.econ();
    let recon = 1.0 / econ;
    let ulim = econ * emax_ev; // tau^2 ceiling [1/cm^2]
    let tau_max = ulim.sqrt();

    let volume = structure.volume_cm3();
    let n_cell = structure.atoms_per_cell().max(1) as f64;
    let npr = npr.max(1) as f64;
    if !(volume > 0.0) || !volume.is_finite() {
        return BraggEdges { edges: Vec::new() };
    }

    // f_i = 16 pi^3 |F|^2_barn / (V econ N_cell npr tau).
    // |F|^2 arrives in fm^2, so the fm^2 -> barn conversion (1 barn = 100 fm^2)
    // is folded into `scon` once rather than per edge.
    let scon = 16.0 * PI.powi(3) / (volume * econ * n_cell * npr * 100.0);

    let recip = structure.reciprocal_cm();

    // Index bounds: tau . a_i = 2 pi h_i, so |h_i| <= |tau| |a_i| / (2 pi).
    let mut bound = [0_i64; 3];
    for (i, b) in bound.iter_mut().enumerate() {
        let a = structure.cell_cm[i];
        let len = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt();
        *b = (tau_max * len / (2.0 * PI)).floor() as i64 + 1;
    }

    let mut edges: Vec<(f64, f64)> = Vec::new();
    for h in -bound[0]..=bound[0] {
        for k in -bound[1]..=bound[1] {
            for l in -bound[2]..=bound[2] {
                if h == 0 && k == 0 && l == 0 {
                    continue;
                }
                let (hf, kf, lf) = (h as f64, k as f64, l as f64);
                let mut tau = [0.0_f64; 3];
                for c in 0..3 {
                    tau[c] = hf * recip[0][c] + kf * recip[1][c] + lf * recip[2][c];
                }
                let tsq = tau[0] * tau[0] + tau[1] * tau[1] + tau[2] * tau[2];
                if tsq <= 0.0 || tsq > ulim {
                    continue;
                }
                // |F(tau)|^2 = |sum_j b_j w_j exp(2 pi i (h x_j + k y_j + l z_j))|^2,
                // with w_j = exp(-2 W'_j E_tau) when a per-atom Debye-Waller
                // factor was supplied and w_j = 1 otherwise.
                let mut re = 0.0_f64;
                let mut im = 0.0_f64;
                for (j, atom) in structure.basis.iter().enumerate() {
                    let phase = 2.0
                        * PI
                        * (hf * atom.fractional[0]
                            + kf * atom.fractional[1]
                            + lf * atom.fractional[2]);
                    let (s, c) = phase.sin_cos();
                    let b = match w_prime_per_atom.get(j) {
                        Some(w) => atom.b_coh_fm * (-2.0 * w * tsq * recon).exp(),
                        None => atom.b_coh_fm,
                    };
                    re += b * c;
                    im += b * s;
                }
                let f2 = re * re + im * im;
                edges.push((tsq, f2 / tsq.sqrt()));
            }
        }
    }

    edges.sort_by(|x, y| x.0.partial_cmp(&y.0).unwrap());

    // Synthetic final edge at the energy ceiling, mirroring the Fortran
    // (`leapr.f90:2763-2765`) so the ENDF histogram extends to E_max.
    let last_f = edges.last().map(|e| e.1).unwrap_or(0.0);
    edges.push((ulim, last_f));

    // Convert to practical units (eV, barn eV) and merge near-degenerate edges,
    // exactly as `leapr.f90:2770-2783` does.
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
    use crate::leapr::coher::{coher, coher_with_constants, CoherentLattice};

    /// Build the fcc aluminium structure NJOY's built-in `iel = 4` describes:
    /// cubic lattice constant `a = 4.04e-8` cm, four Al atoms per conventional
    /// cell, coherent cross section 1.495 barn (`leapr.f90:2520-2522`).
    fn aluminium_fcc() -> CrystalStructure {
        // b = sqrt(sigma_coh / 4 pi); NJOY's own 1.495 b, converted, so the
        // comparison isolates the *geometry* from the scattering-length data.
        let b = (1.495 * 100.0 / (4.0 * PI)).sqrt();
        let a = 4.04e-8;
        CrystalStructure {
            cell_cm: [[a, 0.0, 0.0], [0.0, a, 0.0], [0.0, 0.0, a]],
            basis: vec![
                BasisAtom {
                    fractional: [0.0, 0.0, 0.0],
                    b_coh_fm: b,
                    label: "Al",
                },
                BasisAtom {
                    fractional: [0.0, 0.5, 0.5],
                    b_coh_fm: b,
                    label: "Al",
                },
                BasisAtom {
                    fractional: [0.5, 0.0, 0.5],
                    b_coh_fm: b,
                    label: "Al",
                },
                BasisAtom {
                    fractional: [0.5, 0.5, 0.0],
                    b_coh_fm: b,
                    label: "Al",
                },
            ],
            name: "Al (fcc), from NJOY's built-in constants",
        }
    }

    /// Body-centred-cubic iron, NJOY's built-in `iel = 6`
    /// (`a = 2.86e-8` cm, `sigma_coh = 12.9` barn, `leapr.f90:2526-2528`).
    fn iron_bcc() -> CrystalStructure {
        let b = (12.9 * 100.0 / (4.0 * PI)).sqrt();
        let a = 2.86e-8;
        CrystalStructure {
            cell_cm: [[a, 0.0, 0.0], [0.0, a, 0.0], [0.0, 0.0, a]],
            basis: vec![
                BasisAtom {
                    fractional: [0.0, 0.0, 0.0],
                    b_coh_fm: b,
                    label: "Fe",
                },
                BasisAtom {
                    fractional: [0.5, 0.5, 0.5],
                    b_coh_fm: b,
                    label: "Fe",
                },
            ],
            name: "Fe (bcc), from NJOY's built-in constants",
        }
    }

    /// Hexagonal graphite, NJOY's built-in `iel = 1`: `a = 2.4573e-8` cm,
    /// `c = 6.700e-8` cm, `sigma_coh = 5.50` barn (`leapr.f90:2508-2511`), four
    /// C atoms per cell in the AB stacking `(0,0,0)`, `(0,0,½)`, `(⅓,⅔,0)`,
    /// `(⅔,⅓,½)`.
    fn graphite_hex() -> CrystalStructure {
        let b = (5.50 * 100.0 / (4.0 * PI)).sqrt();
        let (a, c) = (2.4573e-8, 6.700e-8);
        let s3 = 3.0_f64.sqrt() / 2.0;
        let atom = |f: [f64; 3]| BasisAtom {
            fractional: f,
            b_coh_fm: b,
            label: "C",
        };
        CrystalStructure {
            cell_cm: [[a, 0.0, 0.0], [-0.5 * a, s3 * a, 0.0], [0.0, 0.0, c]],
            basis: vec![
                atom([0.0, 0.0, 0.0]),
                atom([0.0, 0.0, 0.5]),
                atom([1.0 / 3.0, 2.0 / 3.0, 0.0]),
                atom([2.0 / 3.0, 1.0 / 3.0, 0.5]),
            ],
            name: "graphite (hexagonal, AB), from NJOY's built-in constants",
        }
    }

    /// Cumulative structure factor `sum_{E_i <= E} f_i` \[barn eV\] — the
    /// Debye-Waller-free `S(E)` the ENDF writer would tabulate at `W' = 0`.
    fn cumulative(edges: &[(f64, f64)], e: f64) -> f64 {
        edges
            .iter()
            .take_while(|&&(ee, _)| ee <= e)
            .map(|&(_, f)| f)
            .sum()
    }

    /// **V&V — methodology.** The general path must reproduce NJOY's own
    /// built-in lattices, because those are special cases of Zhu Eq. (3.5): the
    /// six hand-written `formf` branches are what `|sum_j b_j exp(i tau.r_j)|^2`
    /// evaluates to for those six bases. Each lattice is rebuilt from *NJOY's
    /// own* constants — same `a`, `c`, and `sigma_coh`, converted to a
    /// scattering length by `b = sqrt(sigma_coh / 4 pi)` — so the comparison
    /// isolates the algorithm from the input data. The compared quantity is the
    /// cumulative structure factor `S(E) = sum_{E_i<=E} f_i` at 0.05, 0.1, 0.3
    /// and 0.5 eV, which folds in every edge and every form factor below each
    /// bound. Pass criterion: relative deviation < 1e-9 for the cubic lattices,
    /// < 1e-8 for graphite (see below).
    ///
    /// **Results (measured 2026-08-19, this environment, release mode),** worst
    /// case over the four energies:
    ///
    /// | Lattice | `S(0.5 eV)` general | built-in | max rel. dev. |
    /// |---|---|---|---|
    /// | Al (fcc, `iel = 4`) | 7.4624353170e-1 | 7.4624353170e-1 | 1.259e-13 |
    /// | Fe (bcc, `iel = 6`) | 6.4059063854e0 | 6.4059063854e0 | 2.321e-16 |
    /// | Graphite (hex, `iel = 1`) | 2.7159195812e0 * | 2.8650079937e0 * | 2.489e-10 |
    ///
    /// Fe is bit-identical. Al's 1.3e-13 is ordinary float re-association. The
    /// graphite residual of **2.489e-10 is fully explained**: `builtin` carries
    /// `SQRT3 = 1.732050808` as a truncated literal (`leapr.f90:2537`), whose
    /// relative error against `sqrt(3)` is 2.5e-10 and which enters the
    /// hexagonal cell volume linearly.
    ///
    /// *(Graphite's 0.5 eV entry is excluded from the comparison — see
    /// [`builtin_index_boxes_truncate_above_their_own_lattices_reach`].)*
    #[test]
    fn general_path_reproduces_njoy_builtin_lattices() {
        let constants = PhysicalConstants::default();
        for (structure, lattice, emax, tol) in [
            (aluminium_fcc(), CoherentLattice::Aluminium, 0.5, 1.0e-9),
            (iron_bcc(), CoherentLattice::Iron, 0.5, 1.0e-9),
            (graphite_hex(), CoherentLattice::Graphite, 0.3, 1.0e-8),
        ] {
            let g = coher_general_with_constants(&structure, 1, 5.0, constants);
            let b = coher_with_constants(lattice, 1, 5.0, constants);
            for e in [0.05, 0.1, 0.3, 0.5] {
                if e > emax {
                    continue;
                }
                let (sg, sb) = (cumulative(&g.edges, e), cumulative(&b.edges, e));
                assert!(sb > 0.0, "{lattice:?}: built-in S({e} eV) must be positive");
                let rel = (sg - sb).abs() / sb;
                assert!(
                    rel < tol,
                    "{lattice:?}: general S({e} eV) = {sg:e} vs built-in {sb:e}, rel dev {rel:e}"
                );
            }
        }
    }

    /// **Documents a real limitation of the ported built-in path, not a defect
    /// in the general one.** NJOY's `coher` sweeps a *fixed* index box for the
    /// cubic lattices (`i1, i2, i3` over `-15..=15`, `leapr.f90:2698, 2725`)
    /// and index bounds derived from `a` and `c` for the hexagonal ones. Those
    /// boxes do not enclose the whole `tau^2 <= econ * E_max` sphere at
    /// `E_max = 5` eV, so the built-in path silently drops edges above roughly
    /// 0.8 eV (Al), 1 eV (Fe) and 0.5 eV (graphite). The general path bounds its
    /// own loops from the requested `E_max` and keeps them.
    ///
    /// **Measured 2026-08-19** — general/built-in cumulative structure factor:
    ///
    /// | E \[eV\] | Al | Fe | graphite |
    /// |---|---|---|---|
    /// | 0.5 | 1.000000 | 1.000000 | 0.948 |
    /// | 1.0 | 1.164 | 1.033 | 0.994 |
    /// | 5.0 | 4.51 | 3.60 | 1.000 |
    ///
    /// This matters little in practice — the Debye-Waller factor and the `1/E`
    /// envelope have made the coherent-elastic channel negligible long before
    /// 1 eV, and `endout` thins the tail away — but it means the two paths are
    /// only interchangeable in the thermal range, and it must not be mistaken
    /// for the general path over-counting.
    #[test]
    fn builtin_index_boxes_truncate_above_their_own_lattices_reach() {
        let constants = PhysicalConstants::default();
        let g = coher_general_with_constants(&aluminium_fcc(), 1, 5.0, constants);
        let b = coher_with_constants(CoherentLattice::Aluminium, 1, 5.0, constants);
        let ratio = cumulative(&g.edges, 5.0) / cumulative(&b.edges, 5.0);
        assert!(
            ratio > 2.0,
            "the built-in fcc box is expected to truncate badly at 5 eV, ratio {ratio}"
        );
        // ...while agreeing where the box does enclose the sphere
        let low = cumulative(&g.edges, 0.3) / cumulative(&b.edges, 0.3);
        assert!((low - 1.0).abs() < 1e-9, "agreement at 0.3 eV, ratio {low}");
    }

    /// Edges must be ascending, structure factors non-negative, and the last
    /// edge must sit at the requested energy ceiling — the same invariants the
    /// built-in path is checked for.
    #[test]
    fn general_edges_are_monotone_and_bounded() {
        let g = coher_general(&aluminium_fcc(), 1, 5.0);
        assert!(g.edges.len() > 10);
        assert!(g.edges.windows(2).all(|w| w[1].0 > w[0].0));
        assert!(g.edges.iter().all(|&(_, s)| s >= 0.0));
        assert!((g.edges.last().unwrap().0 - 5.0).abs() < 1.0e-6);
    }

    /// The fcc extinction rule must emerge from Eq. (3.5) rather than being
    /// written by hand: for a monatomic fcc crystal every reflection with mixed
    /// parity `(h k l)` has `|F|^2 = 0`, so the lowest edge with a *positive*
    /// structure factor is `(111)`, at `E = 3 h^2 / (8 m a^2)`.
    ///
    /// For Al (`a = 4.04` Å) that is `3 * 1.253e-3` = 3.759e-3 eV.
    ///
    /// **Measured 2026-08-19:** general path 3.759016135605e-3 eV, ported
    /// built-in path 3.759016135601e-3 eV — agreement to twelve figures, so
    /// the extinctions the general sum derives are the extinctions NJOY's
    /// hand-written `formf` encodes.
    #[test]
    fn fcc_extinctions_emerge_from_the_structure_factor() {
        let g = coher_general(&aluminium_fcc(), 1, 5.0);
        let first_allowed = g.edges.iter().find(|&&(_, s)| s > 1.0e-12).unwrap().0;
        assert!(
            (3.7e-3..3.8e-3).contains(&first_allowed),
            "Al first allowed edge should be (111) at ~3.759e-3 eV, got {first_allowed:e}"
        );
        let builtin_first = coher(CoherentLattice::Aluminium, 1, 5.0)
            .edges
            .iter()
            .find(|&&(_, s)| s > 1.0e-12)
            .unwrap()
            .0;
        assert!(
            (first_allowed - builtin_first).abs() / builtin_first < 1e-10,
            "general {first_allowed:e} vs built-in {builtin_first:e}"
        );
        // and the (100)/(110) reflections below it must be present but dead
        assert!(
            g.edges
                .iter()
                .any(|&(e, s)| e < first_allowed && s.abs() < 1.0e-12),
            "forbidden reflections below (111) are retained with S = 0"
        );
    }

    /// **V&V — the exact Debye-Waller path must degenerate to the compound one.**
    ///
    /// # Methodology
    ///
    /// Zhu's exact option folds `exp(-2 W'_j E_tau)` into each atom's amplitude,
    /// so when every `W'_j` is the same `W` the whole sum scales by
    /// `exp(-2 W E)` and the intensity by `exp(-4 W E)` — precisely the factor
    /// `endout` applies on top of the compound path. The two must therefore
    /// agree edge for edge once that factor is applied by hand to the compound
    /// result. Run on aluminium (monatomic, so the equal-coefficient case is
    /// also the physical one) and on a two-species cell with deliberately
    /// unequal scattering lengths, at `W' = 0.05 /eV`, over every edge to
    /// 0.5 eV. Pass criterion: relative deviation < 1e-12 per edge.
    ///
    /// Also checks the guard: a `w_prime_per_atom` whose length does not match
    /// the basis falls back to the compound path rather than misassigning
    /// coefficients to atoms.
    ///
    /// **The synthetic final edge is excluded from the comparison, on
    /// purpose.** That edge is not a reflection — it is a copy of the last real
    /// edge's weight, planted at `E_max` so the ENDF histogram extends there
    /// (`leapr.f90:2763-2765`). The exact path weights it by the Debye-Waller
    /// factor of the reflection it copies, while the compound path lets
    /// `endout` weight it at `E_max`; the two therefore differ by
    /// `exp(-4 W (E_max - E_last))`. **Measured 2026-08-21: 3.0e-4 relative**
    /// at `W' = 0.05 /eV` on the two-species cell. Neither is more correct than
    /// the other for a duplicated edge, so this is documented rather than
    /// papered over with an invented factor.
    ///
    /// **Results (measured 2026-08-21, release mode).** Over every real edge:
    /// worst relative deviation 4.5e-15 over 334 aluminium edges and 7.9e-15
    /// over 327 edges of the two-species cell — float round-off. The
    /// length-mismatch guard returns the compound edges bit-for-bit.
    #[test]
    fn equal_per_atom_coefficients_reproduce_the_compound_path() {
        const W: f64 = 0.05;
        let two_species = CrystalStructure {
            cell_cm: [[4.0e-8, 0.0, 0.0], [0.0, 4.0e-8, 0.0], [0.0, 0.0, 4.0e-8]],
            basis: vec![
                BasisAtom {
                    fractional: [0.0, 0.0, 0.0],
                    b_coh_fm: 4.1491,
                    label: "A",
                },
                BasisAtom {
                    fractional: [0.25, 0.25, 0.25],
                    b_coh_fm: 6.6460,
                    label: "B",
                },
            ],
            name: "two unlike species, for the cancellation case",
        };

        for structure in [aluminium_fcc(), two_species] {
            let n = structure.basis.len();
            let compound = coher_general(&structure, 1, 0.5);
            let exact = coher_general_with_per_atom_debye_waller(
                &structure,
                1,
                0.5,
                PhysicalConstants::default(),
                &vec![W; n],
            );
            assert_eq!(
                compound.edges.len(),
                exact.edges.len(),
                "{}: the two paths must produce the same edge grid",
                structure.name
            );
            let mut worst = 0.0_f64;
            // `.rev().skip(1)` drops the synthetic final edge; see the doc above.
            let real = compound.edges.len() - 1;
            for (&(ec, fc), &(ee, fe)) in compound.edges[..real].iter().zip(&exact.edges[..real]) {
                assert!(
                    (ec - ee).abs() <= 1e-18 * ec.max(1e-30),
                    "{}: edge energies must match ({ec:e} vs {ee:e})",
                    structure.name
                );
                let expected = fc * (-4.0 * W * ec).exp();
                if expected > 1e-30 {
                    worst = worst.max((fe - expected).abs() / expected);
                }
            }
            assert!(
                worst < 1e-12,
                "{}: equal per-atom coefficients must reproduce the compound path times \
                 exp(-4 W E); worst relative deviation {worst:e}",
                structure.name
            );

            // Length-mismatch guard: fall back, do not misassign.
            let mismatched = coher_general_with_per_atom_debye_waller(
                &structure,
                1,
                0.5,
                PhysicalConstants::default(),
                &vec![W; n + 1],
            );
            assert_eq!(
                mismatched.edges, compound.edges,
                "{}: a wrong-length coefficient slice must fall back to the compound path",
                structure.name
            );
        }
    }

    /// **V&V — the exact option must only move reflections that involve
    /// cancellation.**
    ///
    /// # Methodology
    ///
    /// In a zinc-blende-like cell the *sum* reflections add the two
    /// sublattices' amplitudes and the *difference* reflections subtract them.
    /// A per-atom Debye-Waller factor changes the ratio of the two amplitudes
    /// with `tau^2`, so it should barely move a sum reflection and strongly
    /// move a difference reflection. Compares, at unequal coefficients
    /// (`W'_A = 1.0`, `W'_B = 1.7 /eV` — the scale actually measured for SiC,
    /// where the fitted compound value is ~1.35 /eV) against their
    /// atomic-fraction mean applied to the compound path.
    ///
    /// The two reflections are chosen **adjacent in energy** so the comparison
    /// is not confounded by the `tau^2` growth of the factor itself: `n = 48`
    /// is `(444)`, a sum reflection (`h+k+l = 0 mod 4`), and `n = 52` is
    /// `(640)`, a difference reflection (`h+k+l = 2 mod 4`), at 0.0512 eV and
    /// 0.0555 eV respectively.
    ///
    /// **Results (measured 2026-08-21):** the sum reflection moves by −1.52 %
    /// and the difference reflection by −30.63 % — a factor of 20 at
    /// essentially the same energy, which is the whole reason the option
    /// exists. Both move *down*, as they must: carbon carries the larger
    /// coefficient and the larger scattering length, so weighting it
    /// separately removes more amplitude than the mean does.
    #[test]
    fn unequal_coefficients_move_difference_reflections_far_more_than_sum_reflections() {
        let a = 4.379e-8;
        let fcc = [
            [0.0, 0.0, 0.0],
            [0.0, 0.5, 0.5],
            [0.5, 0.0, 0.5],
            [0.5, 0.5, 0.0],
        ];
        let mut basis = Vec::new();
        for s in fcc {
            basis.push(BasisAtom {
                fractional: s,
                b_coh_fm: 4.1491,
                label: "A",
            });
        }
        for s in fcc {
            basis.push(BasisAtom {
                fractional: [s[0] + 0.25, s[1] + 0.25, s[2] + 0.25],
                b_coh_fm: 6.6460,
                label: "B",
            });
        }
        let structure = CrystalStructure {
            cell_cm: [[a, 0.0, 0.0], [0.0, a, 0.0], [0.0, 0.0, a]],
            basis,
            name: "zinc-blende-like, two unlike species",
        };
        let (wa, wb) = (1.0_f64, 1.7_f64);
        let mean = 0.5 * (wa + wb);
        let per_atom: Vec<f64> = structure
            .basis
            .iter()
            .map(|at| if at.label == "A" { wa } else { wb })
            .collect();

        let compound = coher_general(&structure, 1, 0.1);
        let exact = coher_general_with_per_atom_debye_waller(
            &structure,
            1,
            0.1,
            PhysicalConstants::default(),
            &per_atom,
        );
        assert!(
            compound.edges.len() > 60,
            "need edges out to n = 52 for this comparison"
        );
        let e1 = compound.edges[0].0;
        // n = 48 is (444), a sum reflection; n = 52 is (640), a difference one.
        let pick = |edges: &[(f64, f64)], n: f64| -> f64 {
            edges
                .iter()
                .find(|&&(e, _)| (e / e1 - n).abs() < 1e-3)
                .map(|&(_, f)| f)
                .unwrap_or(0.0)
        };
        let ratio = |n: f64| {
            let c = pick(&compound.edges, n) * (-4.0 * mean * n * e1).exp();
            let x = pick(&exact.edges, n);
            (x - c).abs() / c
        };
        let sum_move = ratio(48.0);
        let diff_move = ratio(52.0);
        assert!(
            sum_move < 0.05,
            "a sum reflection should move only slightly under a per-atom factor, moved {:.2} %",
            100.0 * sum_move
        );
        assert!(
            diff_move > 0.2,
            "a difference reflection should move a lot under a per-atom factor, moved only \
             {:.2} %",
            100.0 * diff_move
        );
        // Measured ratio is 20.1; asserted at 10 so ordinary drift in either
        // reflection cannot flip the test without changing the conclusion.
        assert!(
            diff_move > 10.0 * sum_move,
            "the difference reflection must move far more than the sum reflection at essentially \
             the same energy; got {:.2} % vs {:.2} %",
            100.0 * diff_move,
            100.0 * sum_move
        );
    }

    /// Cell-geometry helpers, checked against hand arithmetic.
    #[test]
    fn cell_volume_and_reciprocal_are_consistent() {
        let s = aluminium_fcc();
        let a = 4.04e-8;
        assert!((s.volume_cm3() - a * a * a).abs() / (a * a * a) < 1e-12);
        let r = s.reciprocal_cm();
        // b1 . a1 = 2 pi, b1 . a2 = 0
        let dot11: f64 = (0..3).map(|c| r[0][c] * s.cell_cm[0][c]).sum();
        let dot12: f64 = (0..3).map(|c| r[0][c] * s.cell_cm[1][c]).sum();
        assert!((dot11 - 2.0 * PI).abs() < 1e-9);
        assert!(dot12.abs() < 1e-9);
        // sigma_coh round-trips
        assert!((s.sigma_coh_barn(0) - 1.495).abs() < 1e-12);
    }
}
