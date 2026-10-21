// SPDX-License-Identifier: GPL-3.0-only
//! **A published HTR-10 release rate of fission products from the fuel
//! elements, equilibrium core, normal operation.**
//!
//! # What this is
//!
//! The rate at which each important fission product (and tritium) leaves the
//! **fuel elements** into the primary helium, per hour and per megawatt
//! thermal, for the equilibrium core in normal operation -- the source term of
//! the primary circuit, upstream of [`super::primary_helium`] (what then
//! accumulates in the helium) and [`super::airborne_release`] (what reaches the
//! environment). Unit: **Bq h^-1 MWt^-1**, stated in the table's column header.
//!
//! The source's text gives the basis: fission products released from the
//! coated particles by diffusion, the maximum fuel-centre temperature in normal
//! operation being 864 degC (section 2.3), with free uranium and defective
//! particles the dominant release source.
//!
//! # What this is NOT
//!
//! - **Not an input.** `htgr_sim_v1`'s stage-1 release V&V test compares its
//!   **uncalibrated** release against it (gh:#399); nothing is tuned to it.
//! - **Not a release to the environment**, and not a dose.
//! - `RESPONSIBLE_USE.md` applies: nothing here may be quoted as a release
//!   figure for HTR-10 or any other plant for any operational, licensing or
//!   safety purpose.
//!
//! # Provenance
//!
//! Liu Yuanzhong and Cao Jianzhu, *Nuclear Engineering and Design* **218**
//! (2002) 81-90, **Table 2** (p. 83), "Release rates of important fission
//! products from the fuel elements in the equilibrium core", 22 nuclides.
//! Transcribed 2026-09-29 from the text layer of the maintainer's copy; see
//! `crates/changi/docs/References.md`. Restricted access: cited, not
//! redistributed.

/// The table, compiled in.
const HTR10_FUEL_RELEASE_CSV: &str =
    include_str!("../../reference/htr10_fuel_element_release_rate.csv");

/// One nuclide's release rate from the fuel elements \[Bq h^-1 MWt^-1\].
#[must_use]
pub fn htr10_fuel_element_release_rate_bq_per_h_per_mwt(nuclide: &str) -> Option<f64> {
    HTR10_FUEL_RELEASE_CSV
        .lines()
        .skip(1)
        .filter_map(|line| line.split_once(','))
        .find(|(name, _)| name.trim() == nuclide)
        .and_then(|(_, v)| v.trim().parse::<f64>().ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The transcription: 22 nuclides, and spot values from the table.
    #[test]
    fn table_2_is_transcribed() {
        let n = HTR10_FUEL_RELEASE_CSV
            .lines()
            .skip(1)
            .filter(|l| !l.is_empty())
            .count();
        assert_eq!(n, 22);
        assert_eq!(
            htr10_fuel_element_release_rate_bq_per_h_per_mwt("Kr-85"),
            Some(1.5e4)
        );
        assert_eq!(
            htr10_fuel_element_release_rate_bq_per_h_per_mwt("I-131"),
            Some(4.9e6)
        );
        assert_eq!(
            htr10_fuel_element_release_rate_bq_per_h_per_mwt("Sr-90"),
            Some(0.25)
        );
        assert_eq!(
            htr10_fuel_element_release_rate_bq_per_h_per_mwt("C-14"),
            None
        );
    }
}
