//! UNIFAC-LLE — UNIFAC group-contribution activity coefficients with the
//! **liquid–liquid-equilibrium (LLE) parameterised** group-interaction table.
//!
//! UNIFAC-LLE uses the *identical functional form* to the original (VLE)
//! UNIFAC model — the same Bondi group volumes `R_k` / surface areas `Q_k`, the
//! same Staverman–Guggenheim combinatorial term, and the same Fredenslund
//! group-residual term — but replaces the temperature-dependent VLE
//! interaction energies with a **separate, temperature-independent set of
//! `a_mn` (K)** fitted to *liquid–liquid* equilibrium data (Magnussen,
//! Rasmussen & Fredenslund 1981). These LLE parameters produce the stronger
//! positive deviations from Raoult's law needed to reproduce partial
//! miscibility, which is why this variant feeds the LLE flash rather than the
//! VLE model.
//!
//! Because the algebra is unchanged, this module **reuses** the verified
//! implementation in [`super::unifac`] (combinatorial term, group-residual
//! term, and the `γ_i = exp(ln γ^C + ln γ^R)` assembly) and only supplies the
//! LLE parameter table. Nothing in `unifac.rs` is modified.
//!
//! # Port provenance
//!
//! Ported from DWSIM (GPL-3.0), commit `1abf72d`:
//!
//! - Property-package wrapper: `DWSIM.Thermodynamics/PropertyPackages/UNIFACLL.vb`,
//!   class `UNIFACLLPropertyPackage` (`UNIFACLL.vb:28-139`) — thin wrapper that
//!   holds an `Auxiliary.UnifacLL` model (`UNIFACLL.vb:45-72`).
//! - Model class: `DWSIM.Thermodynamics/PropertyPackages/Models/UNIFAC.vb`,
//!   class `UnifacLL` (`Models/UNIFAC.vb:491-501`). `UnifacLL` **inherits**
//!   `Unifac` and differs *only* by constructing its group table with the LLE
//!   flag set — `UnifGroups = New UnifacGroups(True)` (`Models/UNIFAC.vb:497`).
//! - Group-table loader with the LLE branch: `UnifacGroups.New(ll As Boolean)`
//!   (`Models/UNIFAC.vb:508-625`); the `If ll Then …` block that layers the LLE
//!   interaction file on top is `Models/UNIFAC.vb:562-594`.
//! - The activity-coefficient algebra (`Unifac.GAMMA_MR`, `RET_Ri`, `RET_Qi`,
//!   `TAU`, …) is the same code already cited in [`super::unifac`]'s header and
//!   is reused here unchanged.
//!
//! # Parameter-table provenance (public-literature LLE subset)
//!
//! The bundled table [`magnussen_lle_subset`] is a **small public-literature
//! subset** of the UNIFAC-LLE interaction matrix, not DWSIM's full asset file.
//! Sources:
//!
//! - `a_mn` LLE interaction energies (K): Magnussen, T.; Rasmussen, P.;
//!   Fredenslund, A., *"UNIFAC Parameter Table for Prediction of
//!   Liquid–Liquid Equilibria"*, **Ind. Eng. Chem. Process Des. Dev.** 20 (2),
//!   331–339 (1981), <https://doi.org/10.1021/i200013a024>. The identical
//!   values appear (comma-delimited, `main_m,name_m,main_n,name_n,a_mn,a_nm`)
//!   in DWSIM's `DWSIM.Thermodynamics/Assets/unifac_ll_ip.txt`; the specific
//!   rows replicated here are:
//!   - `1,CH2,3,ACH,-114.8,156.5`
//!   - `1,CH2,4,ACCH2,-115.7,104.4`
//!   - `1,CH2,5,OH,644.6,328.2`
//!   - `1,CH2,8,H2O,1300,342.4`
//!   - `3,ACH,4,ACCH2,167,-146.8`
//!   - `3,ACH,5,OH,703.9,-9.21`
//!   - `3,ACH,8,H2O,859.4,372.8`
//!   - `4,ACCH2,5,OH,4000,1.27`
//!   - `4,ACCH2,8,H2O,5695,203.7`
//!   - `5,OH,8,H2O,28.73,-122.4`
//! - `R_k`, `Q_k` and subgroup→main-group assignments: the Bondi-derived group
//!   parameters common to all UNIFAC variants — Hansen, Rasmussen, Fredenslund,
//!   Schiller & Gmehling, *Ind. Eng. Chem. Res.* **30**, 2352 (1991); identical
//!   to DWSIM's `DWSIM.Thermodynamics/Assets/unifac.txt` (`SUB_ID`, `Rk`, `Qk`
//!   columns). The `R`/`Q` values are the same as in [`super::unifac`]; only
//!   the interaction matrix differs between VLE and LLE.
//!
//! These tables are published, openly-cited literature data and are permitted
//! under the workspace `DATA_POLICY.md`. The subset covers only the alkane
//! (CH2), aromatic-CH (ACH), aromatic-CH2 (ACCH2), hydroxyl (OH) and water
//! (H2O) main groups — enough for the alkane / aromatic / alcohol / water LLE
//! examples and tests here.
//!
//! ## Main-group numbering — deliberate deviation from DWSIM's file merge
//!
//! DWSIM's `UnifacGroups` keys its interaction dictionary by the *VLE* main-group
//! ids from `unifac.txt` (where `H2O` is main group **7**) and then overlays the
//! LLE rows from `unifac_ll_ip.txt` *keyed by the LLE ids* (where `H2O` is main
//! group **8**), which are not the same numbering for ids ≥ 6. This port does
//! **not** replicate that cross-numbering overlay. Instead it assigns every
//! subgroup a main-group id from the **LLE (Magnussen 1981) numbering**
//! (`CH2 = 1`, `ACH = 3`, `ACCH2 = 4`, `OH = 5`, `H2O = 8`) and keys the
//! interactions with that *same* scheme, so R/Q assignment and `a_mn` lookup are
//! internally consistent. See "Honest scope" below.
//!
//! # Honest scope — untrusted AI-assisted draft, verification not validation
//!
//! **This module is an untrusted AI-assisted draft pending human V&V.** It has
//! been **verified** — the reused algebra is the same code already cross-checked
//! against an independent implementation in [`super::unifac`], and the tests
//! below confirm the model identities (pure-component and identical-molecule
//! ideality hold exactly, infinite dilution is finite, and the LLE table
//! produces strong positive deviations for phase-splitting systems). Note the
//! LLE table is **not** uniformly "stronger" than the VLE table: for
//! butanol/water at equimolar composition it is in fact slightly milder, while
//! for aromatic/aqueous pairs and at infinite dilution it produces the large
//! deviations that partial miscibility requires (see the tests for the actual
//! side-by-side LLE-vs-VLE numbers). It has **not** been *validated* against
//! experimental liquid–liquid tie-line /
//! mutual-solubility benchmarks; the reported activity coefficients are model
//! outputs, not measured phase behaviour, and must not be treated as
//! experimentally confirmed. UNIFAC-LLE is itself a correlation with known
//! limitations.
//!
//! Explicitly **excluded** from this port:
//! - The full LLE `a_mn` matrix (DWSIM's complete `unifac_ll_ip.txt`, ~32 main
//!   groups / 255 pairs); only the 10-pair subset above is bundled.
//! - DWSIM's VLE/LLE main-group cross-numbering overlay (see above) — this port
//!   uses a single, internally-consistent LLE numbering instead.
//! - The 1-propanol / 2-propanol special main groups (LLE ids 6/7) that DWSIM's
//!   LLE table distinguishes from the generic CH2/OH split.
//! - User-database interaction overrides (`Models/UNIFAC.vb:596-623`).
//! - Excess-enthalpy / heat-capacity derivatives.
//!
//! # Units
//!
//! All public functions take/return raw `f64` in the DWSIM-internal SI
//! convention: temperature in **K**, mole fractions dimensionless in `[0, 1]`
//! summing to 1. Activity coefficients `γ_i` are dimensionless and `> 0`. This
//! follows the crate `CLAUDE.md` rule of raw documented `f64` in inner
//! thermodynamic loops.

#![forbid(unsafe_code)]

use super::unifac::{
    activity_coefficients as unifac_activity_coefficients, ln_gamma_combinatorial,
    ln_gamma_residual, molecular_r_q, UnifacComponent, UnifacParameters, UnifacSubgroup,
};

/// A vector of liquid-phase activity coefficients `γ_i` (dimensionless, `> 0`),
/// one entry per component in the same order as the input `components` / `x`.
///
/// Named alias for readability at call sites (the underlying type is a plain
/// `Vec<f64>`; the semantic content is the per-component `γ_i`).
pub type ActivityCoefficients = Vec<f64>;

// --- LLE main-group ids (Magnussen 1981 numbering) -------------------------
// Interaction parameters are keyed on these *main* group ids; R/Q are per
// subgroup (below). These ids match `unifac_ll_ip.txt`'s first/third columns.

/// LLE main group 1 — aliphatic `CH2` (subgroups CH3/CH2/CH/C).
pub const MAIN_CH2: usize = 1;
/// LLE main group 3 — aromatic `ACH` (subgroups ACH/AC).
pub const MAIN_ACH: usize = 3;
/// LLE main group 4 — aromatic `ACCH2` (subgroups ACCH3/ACCH2/ACCH).
pub const MAIN_ACCH2: usize = 4;
/// LLE main group 5 — hydroxyl `OH`.
pub const MAIN_OH: usize = 5;
/// LLE main group 8 — water `H2O`.
pub const MAIN_H2O: usize = 8;

// --- Subgroup ids (DWSIM `SUB_ID`, shared across UNIFAC variants) -----------

/// Subgroup id for `CH3` (main group [`MAIN_CH2`]).
pub const SUB_CH3: usize = 1;
/// Subgroup id for `CH2` (main group [`MAIN_CH2`]).
pub const SUB_CH2: usize = 2;
/// Subgroup id for `CH` (main group [`MAIN_CH2`]).
pub const SUB_CH: usize = 3;
/// Subgroup id for `C` (main group [`MAIN_CH2`]).
pub const SUB_C: usize = 4;
/// Subgroup id for aromatic `ACH` (main group [`MAIN_ACH`]).
pub const SUB_ACH: usize = 10;
/// Subgroup id for aromatic `AC` (main group [`MAIN_ACH`]).
pub const SUB_AC: usize = 11;
/// Subgroup id for aromatic `ACCH3` (main group [`MAIN_ACCH2`]).
pub const SUB_ACCH3: usize = 12;
/// Subgroup id for aromatic `ACCH2` (main group [`MAIN_ACCH2`]).
pub const SUB_ACCH2: usize = 13;
/// Subgroup id for aromatic `ACCH` (main group [`MAIN_ACCH2`]).
pub const SUB_ACCH: usize = 14;
/// Subgroup id for hydroxyl `OH` (main group [`MAIN_OH`]).
pub const SUB_OH: usize = 15;
/// Subgroup id for water `H2O` (main group [`MAIN_H2O`]).
pub const SUB_H2O: usize = 17;

/// Public-literature subset of the **UNIFAC-LLE** parameter table
/// (Magnussen, Rasmussen & Fredenslund 1981).
///
/// Returns a [`super::unifac::UnifacParameters`] populated with:
/// - the Bondi group volumes/areas `R_k`, `Q_k` for the CH3/CH2/CH/C, ACH/AC,
///   ACCH3/ACCH2/ACCH, OH and H2O subgroups (dimensionless, identical to the
///   VLE model — only the interactions differ); and
/// - the temperature-independent LLE interaction energies `a_mn` (K) among the
///   CH2 / ACH / ACCH2 / OH / H2O main groups, exactly as listed in the module
///   header.
///
/// The result plugs directly into the reused [`super::unifac`] algebra. Because
/// `a_mn` is temperature-independent for the LLE table, `Ψ_mn = exp(−a_mn / T)`
/// still varies with `T` through the `1/T` factor (as in every UNIFAC variant);
/// it is only the *fitted energies* `a_mn` that carry no explicit `T`
/// dependence. Valid over the LLE-fit range (roughly 273–373 K).
pub fn magnussen_lle_subset() -> UnifacParameters {
    let mut t = UnifacParameters::new();

    // Subgroups: (subgroup_id, main_group_id, R_k, Q_k). Values from
    // DWSIM unifac.txt (= Hansen et al. 1991); main-group ids in the
    // Magnussen 1981 LLE numbering.
    let subs = [
        (SUB_CH3, MAIN_CH2, 0.9011, 0.848),     // CH3
        (SUB_CH2, MAIN_CH2, 0.6744, 0.540),     // CH2
        (SUB_CH, MAIN_CH2, 0.4469, 0.228),      // CH
        (SUB_C, MAIN_CH2, 0.2195, 0.000),       // C
        (SUB_ACH, MAIN_ACH, 0.5313, 0.400),     // ACH (aromatic CH)
        (SUB_AC, MAIN_ACH, 0.3652, 0.120),      // AC  (aromatic C)
        (SUB_ACCH3, MAIN_ACCH2, 1.2663, 0.968), // ACCH3
        (SUB_ACCH2, MAIN_ACCH2, 1.0396, 0.660), // ACCH2
        (SUB_ACCH, MAIN_ACCH2, 0.8121, 0.348),  // ACCH
        (SUB_OH, MAIN_OH, 1.0000, 1.200),       // OH
        (SUB_H2O, MAIN_H2O, 0.9200, 1.400),     // H2O
    ];
    for (subgroup_id, main_group_id, r, q) in subs {
        t.add_subgroup(UnifacSubgroup {
            subgroup_id,
            main_group_id,
            r,
            q,
        });
    }

    // LLE main-group interaction energies a_mn (K), Magnussen et al. (1981);
    // = DWSIM unifac_ll_ip.txt rows. Directional: (m, n, a_mn, a_nm).
    let pairs = [
        (MAIN_CH2, MAIN_ACH, -114.8, 156.5),   // CH2 / ACH
        (MAIN_CH2, MAIN_ACCH2, -115.7, 104.4), // CH2 / ACCH2
        (MAIN_CH2, MAIN_OH, 644.6, 328.2),     // CH2 / OH
        (MAIN_CH2, MAIN_H2O, 1300.0, 342.4),   // CH2 / H2O
        (MAIN_ACH, MAIN_ACCH2, 167.0, -146.8), // ACH / ACCH2
        (MAIN_ACH, MAIN_OH, 703.9, -9.21),     // ACH / OH
        (MAIN_ACH, MAIN_H2O, 859.4, 372.8),    // ACH / H2O
        (MAIN_ACCH2, MAIN_OH, 4000.0, 1.27),   // ACCH2 / OH
        (MAIN_ACCH2, MAIN_H2O, 5695.0, 203.7), // ACCH2 / H2O
        (MAIN_OH, MAIN_H2O, 28.73, -122.4),    // OH / H2O
    ];
    for (m, n, a_mn, a_nm) in pairs {
        t.set_interaction(m, n, a_mn);
        t.set_interaction(n, m, a_nm);
    }

    t
}

/// Which UNIFAC-LLE parameter table to use — an enum dispatch point (no `dyn`,
/// no `Box`) so future LLE tables (e.g. the full Magnussen matrix) slot in as
/// new arms, mirroring [`super::unifac::UnifacTable`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnifacLleTable {
    /// UNIFAC-LLE, Magnussen et al. (1981) public-literature subset
    /// ([`magnussen_lle_subset`]).
    MagnussenLle,
}

impl UnifacLleTable {
    /// Materialise the chosen LLE table's parameters.
    pub fn parameters(self) -> UnifacParameters {
        match self {
            Self::MagnussenLle => magnussen_lle_subset(),
        }
    }
}

/// Liquid-phase activity coefficients `γ_i` from the UNIFAC-**LLE** table, for
/// every component (dimensionless, `> 0`).
///
/// Convenience top-level entry point: identical to
/// [`super::unifac::activity_coefficients`] but wired to the LLE parameter set.
/// Inputs:
/// - `components` — each molecule's group counts `ν_k^i`, built against the
///   subgroup ids exposed as `SUB_*` constants in this module;
/// - `x` — mole fractions (dimensionless, should sum to ≈ 1), same order/length
///   as `components`;
/// - `temperature` — in K (LLE parameters fit over roughly 273–373 K).
///
/// A pure component (`x = [1.0]`) returns `γ = 1` exactly. Mole-fraction and
/// group-count contracts are inherited unchanged from the reused base algebra.
pub fn activity_coefficients_lle(
    components: &[UnifacComponent],
    x: &[f64],
    temperature: f64,
) -> ActivityCoefficients {
    let params = magnussen_lle_subset();
    unifac_activity_coefficients(&params, components, x, temperature)
}

/// Combinatorial part `ln γ_i^C` under the LLE table, for every component.
///
/// The combinatorial term is table-independent (it depends only on `R_k` / `Q_k`,
/// which are shared with the VLE model), so this simply forwards to
/// [`super::unifac::ln_gamma_combinatorial`] with the LLE `R`/`Q` set. Provided
/// for parity with the base module and for tests. `x` are mole fractions
/// (dimensionless).
pub fn ln_gamma_combinatorial_lle(components: &[UnifacComponent], x: &[f64]) -> Vec<f64> {
    let params = magnussen_lle_subset();
    ln_gamma_combinatorial(&params, components, x)
}

/// Residual part `ln γ_i^R` under the **LLE** interaction table, for every
/// component (this is where the LLE parameters actually enter).
///
/// Forwards to [`super::unifac::ln_gamma_residual`] with the LLE `a_mn` set;
/// `x` are mole fractions (dimensionless) and `temperature` is in K.
pub fn ln_gamma_residual_lle(
    components: &[UnifacComponent],
    x: &[f64],
    temperature: f64,
) -> Vec<f64> {
    let params = magnussen_lle_subset();
    ln_gamma_residual(&params, components, x, temperature)
}

/// Molecular volume `r_i` and surface area `q_i` (both dimensionless) for one
/// component under the LLE table's `R_k` / `Q_k` (identical to the VLE values).
///
/// Forwards to [`super::unifac::molecular_r_q`]. Panics if a subgroup id in
/// `component` is not in the LLE subset table.
pub fn molecular_r_q_lle(component: &UnifacComponent) -> (f64, f64) {
    let params = magnussen_lle_subset();
    molecular_r_q(&params, component)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::thermo::unifac::{activity_coefficients as vle_activity, UnifacTable};
    use approx::assert_relative_eq;

    // Molecule builders (group counts ν_k) against the SUB_* ids.
    fn n_butanol() -> UnifacComponent {
        // 1-butanol CH3-CH2-CH2-CH2-OH = 1 CH3 + 3 CH2 + 1 OH
        UnifacComponent::new(vec![(SUB_CH3, 1.0), (SUB_CH2, 3.0), (SUB_OH, 1.0)])
    }
    fn water() -> UnifacComponent {
        UnifacComponent::new(vec![(SUB_H2O, 1.0)])
    }
    fn benzene() -> UnifacComponent {
        // benzene = 6 aromatic CH (ACH)
        UnifacComponent::new(vec![(SUB_ACH, 6.0)])
    }

    /// **Methodology.** A pure component (single molecule, `x = 1`) must give
    /// `γ = 1` exactly under the LLE table: both `ln γ^C` and `ln γ^R` vanish
    /// because the mixture *is* the reference state, independent of which
    /// interaction table is used. Checked for 1-butanol at 298.15 K.
    ///
    /// **Result (2026-08-03, Magnussen 1981 LLE subset).** `γ = 1.000000000`
    /// (deviation `< 1e-12`). Pass.
    #[test]
    fn pure_component_gamma_is_one() {
        let g = activity_coefficients_lle(&[n_butanol()], &[1.0], 298.15);
        assert_relative_eq!(g[0], 1.0, epsilon = 1e-12);
    }

    /// **Methodology.** Two molecules of *identical* group composition are
    /// thermodynamically ideal under UNIFAC regardless of the interaction
    /// table: `r_i`, `q_i` and all group fractions are equal, so
    /// `ln γ^C = ln γ^R = 0` and `γ_i = 1` at every composition. Checked with
    /// two 1-butanol copies at `x = (0.3, 0.7)`, 298.15 K.
    ///
    /// **Result (2026-08-03).** `γ = (1.000000000, 1.000000000)`
    /// (deviation `< 1e-12`). Pass.
    #[test]
    fn identical_molecules_are_ideal() {
        let g = activity_coefficients_lle(&[n_butanol(), n_butanol()], &[0.3, 0.7], 298.15);
        assert_relative_eq!(g[0], 1.0, epsilon = 1e-12);
        assert_relative_eq!(g[1], 1.0, epsilon = 1e-12);
    }

    /// **Methodology.** For a mixture whose molecules are built entirely from
    /// subgroups of the *same main group* (n-hexane = 2 CH3 + 4 CH2 and
    /// n-heptane = 2 CH3 + 5 CH2, both only main group [`MAIN_CH2`]), every
    /// `Ψ_mn = 1`, so each group residual `ln Γ_k = 0` in both mixture and
    /// reference and `ln γ^R` is **exactly zero** — the LLE interaction table
    /// never engages. Only the (small) combinatorial size term survives.
    /// Checked at `x = (0.5, 0.5)`, 298.15 K.
    ///
    /// **Result (2026-08-03).** `ln γ^R = (0.0, 0.0)` exactly. Pass.
    #[test]
    fn same_main_group_mixture_has_zero_residual() {
        let hexane = UnifacComponent::new(vec![(SUB_CH3, 2.0), (SUB_CH2, 4.0)]);
        let heptane = UnifacComponent::new(vec![(SUB_CH3, 2.0), (SUB_CH2, 5.0)]);
        let comps = [hexane, heptane];
        let ln_r = ln_gamma_residual_lle(&comps, &[0.5, 0.5], 298.15);
        assert_relative_eq!(ln_r[0], 0.0, epsilon = 1e-12);
        assert_relative_eq!(ln_r[1], 0.0, epsilon = 1e-12);
    }

    /// **Methodology — partially-miscible binary, positive deviation vs base
    /// UNIFAC.** 1-butanol(1)/water(2) is a classic *partially miscible* pair
    /// (butanol mutual solubility ≈ 7 wt%). A UNIFAC-LLE model must return
    /// activity coefficients above 1 (positive deviation from Raoult's law) —
    /// the thermodynamic signature that drives liquid–liquid splitting. This
    /// test computes `γ` at `x = (0.5, 0.5)`, 298.15 K with the LLE table and
    /// reports it alongside the base VLE table
    /// ([`crate::thermo::unifac::UnifacTable::OriginalVle`]) for the same
    /// molecules. Pass criterion: both LLE `γ > 1` (positive deviation), and the
    /// LLE and VLE tables give *genuinely different* coefficients (the tables
    /// are not aliases).
    ///
    /// **Result (2026-08-03, T = 298.15 K, x = (0.5, 0.5)).**
    /// - UNIFAC-LLE (Magnussen 1981): `γ = (1.17952, 1.91950)`.
    /// - UNIFAC-VLE (Hansen 1991):    `γ = (1.25569, 1.94538)`.
    ///
    /// **Honest finding (not an over-claim).** For *this* binary at equimolar
    /// composition the two tables are comparable and the LLE values are in fact
    /// slightly *milder*, not stronger — UNIFAC-VLE already handles
    /// butanol/water reasonably. The distinguishing "strong positive deviation"
    /// signature of the LLE table appears in the dilute limit (γ∞ ≈ 52; see
    /// [`infinite_dilution_gamma_is_finite_and_large`]) and for
    /// aromatic/aqueous pairs (see [`aromatic_water_large_positive_deviation`]),
    /// where the residual term dominates. The claim asserted here is only the
    /// defensible one: both tables give a positive deviation and they are not
    /// identical. Values are model outputs (verification), not experimental
    /// tie-lines. Pass.
    #[test]
    fn partially_miscible_binary_positive_deviation() {
        let comps = [n_butanol(), water()];
        let x = [0.5, 0.5];
        let g_lle = activity_coefficients_lle(&comps, &x, 298.15);

        // Same molecules against the base VLE table for comparison.
        let vle = UnifacTable::OriginalVle.parameters();
        let g_vle = vle_activity(&vle, &comps, &x, 298.15);

        // Positive deviation from Raoult's law under the LLE table.
        assert!(g_lle[0] > 1.0, "butanol γ_LLE = {}", g_lle[0]);
        assert!(g_lle[1] > 1.0, "water γ_LLE = {}", g_lle[1]);
        // LLE and VLE tables are genuinely different (not aliases).
        assert!((g_lle[0] - g_vle[0]).abs() > 1e-3);
        assert!((g_lle[1] - g_vle[1]).abs() > 1e-3);

        // Locked measured values (regression guard against silent drift).
        assert_relative_eq!(g_lle[0], 1.17952, epsilon = 1e-3);
        assert_relative_eq!(g_lle[1], 1.91950, epsilon = 1e-3);
        assert_relative_eq!(g_vle[0], 1.25569, epsilon = 1e-3);
        assert_relative_eq!(g_vle[1], 1.94538, epsilon = 1e-3);
    }

    /// **Methodology — infinite-dilution limit `γ∞`.** As `x_i → 0` the activity
    /// coefficient approaches its finite infinite-dilution value; the code forms
    /// the `φ_i/x_i` and `θ_i/φ_i` ratios without dividing by `x_i`, so the
    /// trace limit is numerically finite. Approximated with `x_trace = 1e-6` for
    /// 1-butanol(1)/water(2) at 298.15 K. Pass criterion: both `γ∞ > 1` (finite,
    /// positive-deviation) and butanol-in-water large (`> 10`) — the phase-
    /// splitting signature.
    ///
    /// **Result (2026-08-03, T = 298.15 K, x_trace = 1e-6).**
    /// - `γ∞(1-butanol in water) = 52.4327` (UNIFAC-VLE gives `54.0715` — the
    ///   two tables are comparable at this limit for this system).
    /// - `γ∞(water in 1-butanol) = 2.70932`.
    ///
    /// The very large `γ∞` of butanol at infinite dilution in water is the
    /// expected UNIFAC-LLE signature of a phase-splitting system. Model output,
    /// verification not validation. Pass.
    #[test]
    fn infinite_dilution_gamma_is_finite_and_large() {
        let comps = [n_butanol(), water()];
        let eps = 1e-6;

        let g = activity_coefficients_lle(&comps, &[eps, 1.0 - eps], 298.15);
        assert!(g[0] > 10.0, "butanol γ∞ = {}", g[0]);
        assert_relative_eq!(g[0], 52.4327, epsilon = 1e-2);

        let g = activity_coefficients_lle(&comps, &[1.0 - eps, eps], 298.15);
        assert!(g[1] > 1.0);
        assert_relative_eq!(g[1], 2.70932, epsilon = 1e-2);
    }

    /// **Methodology — aromatic/water immiscibility.** Benzene/water is an
    /// extremely immiscible pair (benzene solubility ≈ 0.18 wt% in water). The
    /// aromatic ACH/H2O LLE interaction (`a = 859.4 / 372.8`) drives a large
    /// positive deviation. Checked at `x = (0.5, 0.5)`, 298.15 K. Pass
    /// criterion: both `γ` large (`> 2.5`) — this is where the LLE table clearly
    /// produces the strong non-ideality of a phase-splitting system, in contrast
    /// to the milder butanol/water case above.
    ///
    /// **Result (2026-08-03, T = 298.15 K, x = (0.5, 0.5)).**
    /// `γ = (3.04507, 4.65121)` (benzene, water). Both large positive
    /// deviations, consistent with strong benzene/water immiscibility. Model
    /// output, verification not validation. Pass.
    #[test]
    fn aromatic_water_large_positive_deviation() {
        let comps = [benzene(), water()];
        let g = activity_coefficients_lle(&comps, &[0.5, 0.5], 298.15);
        assert!(g[0] > 2.5, "benzene γ = {}", g[0]);
        assert!(g[1] > 2.5, "water γ = {}", g[1]);
        assert_relative_eq!(g[0], 3.04507, epsilon = 1e-2);
        assert_relative_eq!(g[1], 4.65121, epsilon = 1e-2);
    }
}
