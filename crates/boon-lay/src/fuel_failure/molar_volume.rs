// SPDX-License-Identifier: GPL-3.0
//
// boon-lay fuel failure — provenance
// ----------------------------------
// Reference : Verfondern, K. & Nabielek, H., "The Mathematical Basis of the
//             PANAMA-I Code for Modeling Pressure Vessel Failure of TRISO
//             Coated Particles under Accident Conditions",
//             Forschungszentrum Jülich, HTA-IB-03/90, 1 August 1990.
//             Reprinted as Appendix C, printed pages -479- to -511-.
// Status    : the report is restricted literature with no reuse licence. Only
//             the governing EQUATIONS and their constants are reproduced here,
//             with citation, as scientific facts. No prose, figure or page of
//             that document is copied into this repository, and the PDF is not
//             tracked here. See DATA_POLICY.md.
// Nature    : boon-lay fuel failure is boon-lay's own model: a Rust
//             implementation of the PANAMA-I FORMULAS, coded agentically (by an
//             AI coding agent, then reviewed) from the published equations. It
//             is not the PANAMA code and not a port of the PANAMA Fortran
//             (closed-source, never consulted); call it "boon-lay fuel failure",
//             and keep "PANAMA-I" for the report and its own printed results.

//! **Eqs (6a)/(6b)/(6c)** — the molar volume `V_m` of the heavy metal in the
//! kernel (pages -491- and -492-).
//!
//! ```text
//! (Th,U)O2   V_m = 0.2645 [kg/mol] / 10500 [kg/m³] = 2.51905e-5 m³/mol   (6a)
//! UO2        V_m = 0.2672 [kg/mol] / 10960 [kg/m³] = 2.43796e-5 m³/mol   (6b)
//! UCO        V_m = 0.2682 [kg/mol] / 10700 [kg/m³] = 2.50654e-5 m³/mol   (6c)
//! ```
//!
//! The report defines `V_m` as the weight of one mole of the kernel compound
//! divided by its density, and Eq (3) divides by it to turn a burnup in FIMA
//! into a number of moles of gas. It is a fixed property of the compound: no
//! temperature, no burnup, no irradiation history.
//!
//! # The third equation is printed as (6b), not (6c)
//!
//! The UCO relation on page -492- carries the label **(6b)** again — the same
//! number already used for `UO₂` on page -491-. It is recorded here as (6c),
//! which is what it must be, with the defect noted rather than silently
//! renumbered. A reader chasing "Eq (6b)" in the report will find two
//! different molar volumes under it. See
//! `docs/panama-i-units-and-open-questions.md`.
//!
//! # Verification
//!
//! **Self-verifying**: each equation prints its own quotient to six
//! significant figures, so the division is a closed check on the
//! transcription of both constants. All three reproduce their printed result
//! to within 1 part in 10⁵ — see [`tests::the_printed_quotients_are_exact`].
//!
//! That is the only verification available. There is **no figure or table in
//! the report that `V_m` can be checked against independently**, and no
//! statement of which compound stoichiometry the molar masses correspond to:
//! 0.2672 kg/mol is neither `UO₂` at natural enrichment (0.2700) nor `²³⁵UO₂`
//! (0.2670), and the report does not say what mixture it assumes. Taken as
//! printed.

use uom::si::f64::MolarVolume;
use uom::si::molar_volume::cubic_meter_per_mole;

/// Which kernel compound the molar volume is for.
///
/// Three variants, because Eqs (6a)–(6c) give three different values.
/// Deliberately **not** the same type as [`super::diffusion::KernelKind`],
/// which has two: the `D_S` correlation on page -487- explicitly uses the
/// `UO₂` relation for `UCO` as well, while the molar volume does not. Merging
/// them would silently give `UCO` the `UO₂` molar volume, a 2.8 % error in
/// the gas pressure that nothing would flag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KernelCompound {
    /// `(Th,U)O₂` — Eq (6a).
    ThoriumUraniumOxide,
    /// `UO₂` — Eq (6b).
    UraniumOxide,
    /// `UCO` — Eq (6c), printed as a second "(6b)".
    UraniumOxycarbide,
}

impl KernelCompound {
    /// The molar mass the report divides by, \[kg/mol\].
    pub const fn molar_mass_kg_per_mol(self) -> f64 {
        match self {
            KernelCompound::ThoriumUraniumOxide => 0.2645,
            KernelCompound::UraniumOxide => 0.2672,
            KernelCompound::UraniumOxycarbide => 0.2682,
        }
    }

    /// The kernel density the report divides by, \[kg/m³\].
    pub const fn density_kg_per_m3(self) -> f64 {
        match self {
            KernelCompound::ThoriumUraniumOxide => 10_500.0,
            KernelCompound::UraniumOxide => 10_960.0,
            KernelCompound::UraniumOxycarbide => 10_700.0,
        }
    }

    /// The value the report prints for the quotient, \[m³/mol\]. Used only to
    /// check the transcription; [`molar_volume`] recomputes it.
    pub const fn printed_molar_volume_m3_per_mol(self) -> f64 {
        match self {
            KernelCompound::ThoriumUraniumOxide => 2.51905e-5,
            KernelCompound::UraniumOxide => 2.43796e-5,
            KernelCompound::UraniumOxycarbide => 2.50654e-5,
        }
    }
}

/// **Eqs (6a)/(6b)/(6c)** — the molar volume `V_m` of the heavy metal in the
/// kernel (pages -491-, -492-).
///
/// Computed from the printed molar mass and density rather than returned as
/// the printed quotient, so that the two constants — not a third derived
/// number — are what this crate carries.
pub fn molar_volume(kernel: KernelCompound) -> MolarVolume {
    MolarVolume::new::<cubic_meter_per_mole>(
        kernel.molar_mass_kg_per_mol() / kernel.density_kg_per_m3(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [KernelCompound; 3] = [
        KernelCompound::ThoriumUraniumOxide,
        KernelCompound::UraniumOxide,
        KernelCompound::UraniumOxycarbide,
    ];

    /// **Each equation prints its own quotient, and all three are
    /// reproduced.**
    ///
    /// Methodology: Eqs (6a)–(6c) each print `mass / density = value` to six
    /// significant figures. Recomputing the quotient from the two printed
    /// constants is a closed check that both were transcribed correctly —
    /// the only self-check available for this group, since no figure or table
    /// in the report plots `V_m`.
    ///
    /// Results, 2026-09-24: relative error against the printed quotient is
    /// below 1·10⁻⁵ for all three (the printed values are rounded at the
    /// sixth figure).
    #[test]
    fn the_printed_quotients_are_exact() {
        for k in ALL {
            let got = molar_volume(k).get::<cubic_meter_per_mole>();
            let printed = k.printed_molar_volume_m3_per_mol();
            let rel = (got - printed).abs() / printed;
            assert!(
                rel < 1.0e-5,
                "{k:?}: computed {got:e} against printed {printed:e}, rel {rel:e}"
            );
        }
    }

    /// The three compounds are distinct — `UCO` must not silently inherit the
    /// `UO₂` value, which is the error the separate enum exists to prevent.
    #[test]
    fn the_three_compounds_differ() {
        let v: Vec<f64> = ALL
            .iter()
            .map(|k| molar_volume(*k).get::<cubic_meter_per_mole>())
            .collect();
        assert!((v[0] - v[1]).abs() / v[1] > 0.03, "(Th,U)O2 vs UO2");
        assert!(
            (v[2] - v[1]).abs() / v[1] > 0.02,
            "UCO vs UO2 differ by 2.8 %, which is why they are separate arms"
        );
        // UO2 is the smallest of the three: highest density, middling mass.
        assert!(v[1] < v[0] && v[1] < v[2]);
    }

    /// The UCO value is the one the report mis-labels "(6b)". Pinned so the
    /// mis-numbering is recorded in code, not only in prose.
    #[test]
    fn the_uco_value_is_the_one_printed_under_a_second_6b() {
        let uco = molar_volume(KernelCompound::UraniumOxycarbide).get::<cubic_meter_per_mole>();
        assert!((uco - 0.2682 / 10_700.0).abs() < 1e-15);
        assert!((uco - 2.50654e-5).abs() / 2.50654e-5 < 1e-5);
    }
}
