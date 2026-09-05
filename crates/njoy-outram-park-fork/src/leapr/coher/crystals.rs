// NEW SYNTHESIS — not a translation of NJOY2016 `leapr.f90` (which has no
// general-crystal catalogue to translate). Crystallographic inputs are
// transcribed from the published sources cited per entry. GPL-3.0-only.

//! Catalogue of crystals for the **generalized** coherent-elastic path.
//!
//! Stock LEAPR selects a lattice with card-5 `iel` from a fixed set of six.
//! The evaluations that need [`super::general`] were produced with a *modified*
//! LEAPR whose card-5 `iel` means something different (Zhu 2014, Table 3:
//! `1 = cubic approximation`, `2 = exact Debye-Waller factor`) and which reads
//! the crystal structure from a **separate `coh_input` file that is not part of
//! the distributed ENDF/B deck**. The ENDF/B-VIII.0 SiC decks therefore carry
//! `iel = 0` — "no coherent elastic" in stock LEAPR's vocabulary — and contain
//! no crystallographic information at all.
//!
//! That missing `coh_input` is what this module supplies: for a material this
//! crate recognises, the crystal structure is looked up here from the published
//! literature and handed to [`super::general::coher_general`]. It is a
//! deliberate, documented departure from "the deck is the only input" — there
//! is no alternative that produces the section at all, and inventing a lattice
//! silently would be worse than either.
//!
//! Adding a crystal here is a **data** change, so every entry must carry its
//! provenance: the source of the lattice constants, of the atomic positions,
//! and of the bound coherent scattering lengths.

use crate::leapr::coher::general::{BasisAtom, CrystalStructure};
use crate::leapr::decks::SabMaterial;

/// Bound coherent scattering length of natural carbon \[fm\].
///
/// Sears, V. F. (1992), "Neutron scattering lengths and cross sections",
/// *Neutron News* **3**(3), 26-37; as tabulated by NIST
/// (<https://www.ncnr.nist.gov/resources/n-lengths/>). Corresponds to
/// `sigma_coh = 5.551` barn.
///
/// **Not in `crates/kovan-literature`** — Sears (1992) is a copyrighted journal
/// article and no openly redistributable copy was available when this was
/// written. The value is a single published constant, cited here where it is
/// used; the gap is recorded as a bead rather than papered over.
pub const B_COH_CARBON_FM: f64 = 6.6460;

/// Bound coherent scattering length of natural silicon \[fm\].
///
/// Sears (1992) / NIST, as for [`B_COH_CARBON_FM`]. Corresponds to
/// `sigma_coh = 2.163` barn.
pub const B_COH_SILICON_FM: f64 = 4.1491;

/// Cubic lattice constant of 3C-SiC \[cm\] as used by the ENDF/B-VIII.0
/// evaluation — the **DFT-minimised, 0 K** value, not the room-temperature
/// measured one.
///
/// Zhu (2014) §4.2: a VASP/GGA relaxation starting from `a = 4.395` Å converges
/// to "a minimized 3C-SiC unit cell with a = 4.379 Å at 0 K", and that cell is
/// what the phonon and coherent-elastic calculations use. The thesis quotes the
/// *experimental* constant as `a = 4.3593` Å in §1.2; using that instead moves
/// every Bragg edge up by 0.9 %.
///
/// **Independently corroborated by the evaluated tape.** The Bragg grid of
/// `reference-data/endf/tsl-CinSiC.endf` (MF=7/MT=2) is a simple-cubic
/// reciprocal lattice whose fundamental edge sits at 1.066463e-3 eV; inverting
/// `E = h^2 / (8 m_n a^2)` gives `a = 4.37898` Å, i.e. the thesis value to five
/// figures. The lattice constant is therefore **not** the source of the
/// coherent-elastic residual against that tape — see `docs/leapr-sic-coherent-elastic-vv.md`,
/// which traces it to the evaluation's own atomic basis instead.
pub const SIC_3C_LATTICE_CM: f64 = 4.379e-8;

/// A crystal this crate can build a generalized coherent-elastic section for.
///
/// Closed enum, dispatched by `match` (workspace rule: no trait objects).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeneralCrystal {
    /// **3C-SiC** (cubic / beta silicon carbide), zinc-blende, space group
    /// F-43m, 8 atoms per conventional cubic cell.
    ///
    /// The lattice of both ENDF/B-VIII.0 SiC thermal-scattering evaluations,
    /// MAT 43 (Si in SiC) and MAT 44 (C in SiC).
    SiliconCarbide3C,
}

impl GeneralCrystal {
    /// The crystal a LEAPR deck describes, when this crate knows one.
    ///
    /// Keyed on the deck's ENDF `MAT` number and cross-checked against its `ZA`,
    /// because that pair identifies the evaluation unambiguously and neither the
    /// title card nor `awr` does. Returns `None` for every deck whose
    /// coherent-elastic section stock LEAPR can already produce (or which
    /// genuinely has none) — this is only consulted when card 5 says `iel = 0`.
    ///
    /// | MAT | ZA | Deck | Crystal |
    /// |---|---|---|---|
    /// | 43 | 143 | `tsl-SiinSiC` | [`Self::SiliconCarbide3C`] |
    /// | 44 | 144 | `tsl-CinSiC` | [`Self::SiliconCarbide3C`] |
    ///
    /// Both SiC materials map to the **same** crystal, and that is the point:
    /// coherent elastic scattering is a Bragg property of the 3C-SiC lattice as
    /// a whole, not of one sublattice. The published tapes agree — MAT 43's and
    /// MAT 44's MF=7/MT=2 sections are byte-identical apart from the header
    /// `ZA`/`AWR`. A caller that puts **both** laws in one SiC region must
    /// therefore count MT=2 **once**, or it double-counts Bragg scattering; see
    /// `reference-data/endf/README.md`.
    pub fn for_material(mat: i32, za: f64) -> Option<Self> {
        match (mat, za.round() as i32) {
            (43, 143) | (44, 144) => Some(Self::SiliconCarbide3C),
            _ => None,
        }
    }

    /// The crystal structure — lattice vectors \[cm\] and the full atomic basis
    /// with bound coherent scattering lengths \[fm\].
    ///
    /// # 3C-SiC
    ///
    /// Conventional cubic cell, `a` = [`SIC_3C_LATTICE_CM`], eight atoms: four
    /// Si on the fcc sites `(0,0,0)`, `(0,½,½)`, `(½,0,½)`, `(½,½,0)` and four C
    /// on the same sites displaced by `(¼,¼,¼)` — the zinc-blende arrangement
    /// of Zhu (2014) Fig. 1. Scattering lengths from [`B_COH_CARBON_FM`] and
    /// [`B_COH_SILICON_FM`].
    ///
    /// Using the conventional (non-primitive) cell is deliberate: it is the
    /// cell the source literature tabulates, and [`super::general`] is
    /// insensitive to the choice — the extra reciprocal-lattice points a
    /// conventional cell introduces are exactly the ones its own structure
    /// factor extinguishes.
    pub fn structure(self) -> CrystalStructure {
        match self {
            Self::SiliconCarbide3C => {
                let a = SIC_3C_LATTICE_CM;
                let fcc = [
                    [0.0, 0.0, 0.0],
                    [0.0, 0.5, 0.5],
                    [0.5, 0.0, 0.5],
                    [0.5, 0.5, 0.0],
                ];
                let mut basis = Vec::with_capacity(8);
                for site in fcc {
                    basis.push(BasisAtom {
                        fractional: site,
                        b_coh_fm: B_COH_SILICON_FM,
                        label: "Si",
                    });
                }
                for site in fcc {
                    basis.push(BasisAtom {
                        fractional: [site[0] + 0.25, site[1] + 0.25, site[2] + 0.25],
                        b_coh_fm: B_COH_CARBON_FM,
                        label: "C",
                    });
                }
                CrystalStructure {
                    cell_cm: [[a, 0.0, 0.0], [0.0, a, 0.0], [0.0, 0.0, a]],
                    basis,
                    name: "3C-SiC (zinc-blende, F-43m), Zhu 2014 DFT cell a = 4.379 A",
                }
            }
        }
    }

    /// The decks whose phonon spectra make up the **universal (compound)
    /// Debye-Waller coefficient**, with their atomic fractions.
    ///
    /// Zhu (2014) Eq. (3.3): under the cubic approximation there is one
    /// Debye-Waller coefficient for the whole crystal, formed as
    /// `w_tot = sum_n w_n * lambda_n` over atom types `n` weighted by atomic
    /// ratio, where `lambda_n` is the coefficient computed from that atom
    /// type's own mass and phonon spectrum. That is why the published MAT 43
    /// and MAT 44 MF=7/MT=2 sections are identical: the Bragg channel of a
    /// compound has no per-sublattice Debye-Waller factor to differ by.
    ///
    /// Each LEAPR deck carries the partial phonon DOS of *its* principal
    /// scatterer, so recovering the compound coefficient means running the
    /// Debye-Waller integral once per deck listed here and combining the
    /// results with these weights. [`crate::leapr::generate`] does that.
    ///
    /// The fractions must sum to 1.
    pub fn debye_waller_decks(self) -> &'static [(SabMaterial, f64)] {
        match self {
            // SiC is 1:1, so both sublattices weigh 0.5.
            Self::SiliconCarbide3C => &[(SabMaterial::SiInSiC, 0.5), (SabMaterial::CInSiC, 0.5)],
        }
    }

    /// The LEAPR deck whose phonon spectrum supplies the Debye-Waller
    /// coefficient of one **species** of this crystal, keyed on the
    /// [`BasisAtom::label`] used in [`Self::structure`].
    ///
    /// [`Self::debye_waller_decks`] gives the *compound* average (Zhu's cubic
    /// approximation); this gives the per-species decomposition that Zhu's
    /// *exact* Debye-Waller option needs, where each atom carries its own
    /// coefficient inside the structure factor. The two must name the same set
    /// of decks — pinned by
    /// [`tests::every_species_maps_to_one_of_the_compound_decks`].
    ///
    /// Returns `None` for a label this crystal does not contain, which a caller
    /// must treat as "cannot build the exact factor" rather than substituting
    /// the compound one silently.
    pub fn debye_waller_deck_for_species(self, label: &str) -> Option<SabMaterial> {
        match (self, label) {
            (Self::SiliconCarbide3C, "Si") => Some(SabMaterial::SiInSiC),
            (Self::SiliconCarbide3C, "C") => Some(SabMaterial::CInSiC),
            _ => None,
        }
    }

    /// A short, stable label for provenance records and cache keys.
    pub const fn label(self) -> &'static str {
        match self {
            Self::SiliconCarbide3C => "3C-SiC",
        }
    }

    /// Every catalogued crystal, for iteration in tests and diagnostics.
    pub const fn all() -> [GeneralCrystal; 1] {
        [GeneralCrystal::SiliconCarbide3C]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::leapr::coher::general::coher_general;

    #[test]
    fn sic_maps_from_both_endf_materials_and_nowhere_else() {
        assert_eq!(
            GeneralCrystal::for_material(44, 144.0),
            Some(GeneralCrystal::SiliconCarbide3C)
        );
        assert_eq!(
            GeneralCrystal::for_material(43, 143.0),
            Some(GeneralCrystal::SiliconCarbide3C)
        );
        // graphite (MAT 30) has a built-in lattice and must not be diverted here
        assert_eq!(GeneralCrystal::for_material(30, 130.0), None);
        // a MAT/ZA mismatch is not a match
        assert_eq!(GeneralCrystal::for_material(44, 143.0), None);
    }

    #[test]
    fn sic_structure_is_eight_atoms_of_the_right_two_species() {
        let s = GeneralCrystal::SiliconCarbide3C.structure();
        assert_eq!(s.atoms_per_cell(), 8);
        assert_eq!(s.basis.iter().filter(|a| a.label == "Si").count(), 4);
        assert_eq!(s.basis.iter().filter(|a| a.label == "C").count(), 4);
        // sigma_coh round-trips to the published bound coherent cross sections
        let si = 4.0 * std::f64::consts::PI * B_COH_SILICON_FM.powi(2) / 100.0;
        let c = 4.0 * std::f64::consts::PI * B_COH_CARBON_FM.powi(2) / 100.0;
        assert!((si - 2.163).abs() < 5e-3, "sigma_coh(Si) = {si}");
        assert!((c - 5.551).abs() < 5e-3, "sigma_coh(C) = {c}");
    }

    /// Every species named in [`GeneralCrystal::structure`] must map, through
    /// [`GeneralCrystal::debye_waller_deck_for_species`], to one of the decks
    /// [`GeneralCrystal::debye_waller_decks`] already lists — otherwise the
    /// exact and compound Debye-Waller paths would be built from different
    /// data and could disagree for a reason no one would think to look for.
    ///
    /// **Result (measured 2026-08-21):** holds for every catalogued crystal.
    #[test]
    fn every_species_maps_to_one_of_the_compound_decks() {
        for crystal in GeneralCrystal::all() {
            let compound: Vec<_> = crystal
                .debye_waller_decks()
                .iter()
                .map(|&(m, _)| m)
                .collect();
            let structure = crystal.structure();
            for atom in &structure.basis {
                let deck = crystal
                    .debye_waller_deck_for_species(atom.label)
                    .unwrap_or_else(|| {
                        panic!(
                            "{}: species {:?} has no Debye-Waller deck",
                            crystal.label(),
                            atom.label
                        )
                    });
                assert!(
                    compound.contains(&deck),
                    "{}: species {:?} maps to {deck:?}, which is not one of the compound decks \
                     {compound:?}",
                    crystal.label(),
                    atom.label
                );
            }
            // ...and every compound deck is actually used by some species.
            for m in &compound {
                assert!(
                    structure
                        .basis
                        .iter()
                        .any(|a| crystal.debye_waller_deck_for_species(a.label) == Some(*m)),
                    "{}: compound deck {m:?} is named but no basis atom maps to it",
                    crystal.label()
                );
            }
        }
    }

    #[test]
    fn debye_waller_weights_sum_to_one() {
        for crystal in GeneralCrystal::all() {
            let total: f64 = crystal.debye_waller_decks().iter().map(|&(_, w)| w).sum();
            assert!(
                (total - 1.0).abs() < 1e-12,
                "{}: Debye-Waller weights sum to {total}",
                crystal.label()
            );
        }
    }

    /// **V&V — methodology.** The zinc-blende extinction rule (reflections with
    /// mixed-parity `(h k l)` vanish) must emerge from the structure-factor sum,
    /// so 3C-SiC's lowest *allowed* Bragg edge is `(111)`, at
    /// `E = 3 h^2 / (8 m_n a^2)`. With the evaluation's `a = 4.379` Å that is
    /// 3.199e-3 eV. Pass criterion: within 1 % of 3.199e-3 eV.
    ///
    /// **Results (measured 2026-08-19, this environment, release mode).**
    /// First allowed edge 3.19939e-3 eV — the same number, to six figures, as
    /// the `n = 3` entry of the published `tsl-CinSiC.endf` Bragg grid
    /// (3.199390e-3 eV). The published tape's grid additionally runs *below*
    /// that, and its 1.066463e-3 eV `(100)` entry carries a **live** jump in
    /// `S(E)` — a reflection zinc-blende extinguishes exactly. (Its
    /// 2.132926e-3 eV `(110)` entry is on the grid but extinguished, as it is
    /// here.) That mismatch is the root of the coherent-elastic residual
    /// against the tape and is traced in
    /// `docs/leapr-sic-coherent-elastic-vv.md`: the evaluation's MF=7/MT=2
    /// matches a basis that is not a valid crystallographic centring, so this
    /// crate keeps zinc-blende and deliberately does not reproduce it.
    #[test]
    fn sic_first_allowed_bragg_edge_is_the_111_reflection() {
        let s = GeneralCrystal::SiliconCarbide3C.structure();
        let edges = coher_general(&s, 1, 5.0);
        let first = edges.edges.iter().find(|&&(_, f)| f > 1e-12).unwrap().0;
        let expected = 3.199390e-3;
        let rel = (first - expected).abs() / expected;
        assert!(
            rel < 1.0e-2,
            "3C-SiC first allowed edge {first:e} eV vs (111) at {expected:e} eV (rel {rel:e})"
        );
    }
}
