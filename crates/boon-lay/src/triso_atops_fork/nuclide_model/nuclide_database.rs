// SPDX-License-Identifier: GPL-3.0
//
// TRISO-ATOPS fork — provenance
// -----------------------------
// Upstream project : TRISO-ATOPS (INL) — https://github.com/IdahoLabResearch/TRISO-ATOPS
// Upstream commit  : de374c8
// Upstream source  : trisoatops/utility_functions/calculation_functions.py (`nuclides` dict)
// Original license : MIT — Copyright (c) 2026 Battelle Energy Alliance, LLC
// This port is distributed under GPL-3.0 as part of the combined boon-lay work;
// the MIT notice above is retained (see LICENSE.triso-atops / NOTICE.triso-atops).

//! # Supported-nuclide database
//!
//! The fixed table of fission-product nuclides TRISO-ATOPS supports. Ports the
//! module-level `nuclides` dict in `calculation_functions.py`; see User Manual
//! §2.1.3 and Table 4 ("Supported nuclides").
//!
//! Half-lives are from the IAEA Live Chart of Nuclides (User Manual §7 ref. 2)
//! and are stored in **seconds**. Parent names encode the dominant decay
//! precursor within the TRISO-ATOPS model (e.g. `Xe-135` ← `I-135`), used to
//! chain parent → daughter activity during a run.

use super::TrisoAtopsNuclide;

/// Number of distinct nuclides in the TRISO-ATOPS supported table.
///
/// Matches the number of keys in the upstream `nuclides` dict.
pub const TRISO_ATOPS_NUCLIDE_COUNT: usize = 84;

/// The full TRISO-ATOPS supported-nuclide table.
///
/// Returns every [`TrisoAtopsNuclide`] the code knows how to model, in the same
/// order as the upstream `nuclides` dict. Half-lives are IAEA values in seconds;
/// parent lists give the modelled decay precursor(s).
///
/// # Notes / faithfully-ported upstream quirks
/// - `Tc-99` is stored with mass number `A = 56` exactly as upstream — this is
///   an apparent typo in the source table (physical `A` of Tc-99 is 99); it is
///   ported verbatim so the fork stays line-for-line traceable. It does not
///   affect any calculation, which uses `Z` and the half-life only.
/// - Several long-lived nuclides carry very large half-lives (up to ~1.6e18 s);
///   these are the stable-on-reactor-timescale species.
#[must_use]
pub fn supported_nuclides() -> Vec<TrisoAtopsNuclide> {
    // Helper to keep each row compact: (name, Z, A, t½ [s], parents).
    let n = TrisoAtopsNuclide::from_seconds;
    vec![
        n("Kr-83m", 36, 83, 6588.0, &[]),
        n("Kr-85m", 36, 85, 16128.0, &[]),
        n("Kr-85", 36, 85, 338_889_828.0, &[]),
        n("Rb-86", 37, 86, 1_610_669.0, &[]),
        n("Kr-87", 36, 87, 4578.0, &[]),
        n("Kr-88", 36, 88, 10170.0, &[]),
        n("Kr-89", 36, 89, 189.0, &[]),
        n("Sr-89", 38, 89, 4_368_643.0, &["Kr-89"]),
        n("Kr-90", 36, 90, 32.32, &[]),
        n("Sr-90", 38, 90, 912_310_730.0, &["Kr-90"]),
        n("Sr-92", 38, 92, 9400.0, &[]),
        n("Y-90", 39, 90, 230_580.0, &[]),
        n("Sr-91", 38, 91, 34740.0, &[]),
        n("Y-91", 39, 91, 5_055_264.0, &["Sr-91"]),
        n("Y-92", 39, 92, 12744.0, &[]),
        n("Zr-95", 40, 95, 5_532_365.0, &[]),
        n("Nb-95", 41, 95, 3_023_222.0, &["Zr-95"]),
        n("Zr-97", 40, 97, 60296.0, &[]),
        n("Mo-99", 42, 99, 237_326.0, &[]),
        n("Tc-99m", 43, 99, 21626.0, &["Mo-99"]),
        n("Ru-103", 44, 103, 3_390_941.0, &[]),
        n("Ru-105", 44, 105, 15980.0, &[]),
        n("Rh-105", 45, 105, 127_228.0, &["Ru-105"]),
        n("Ru-106", 44, 106, 32_123_520.0, &[]),
        n("Ag-110m", 47, 110, 21_585_312.0, &[]),
        n("Ag-111", 47, 111, 643_680.0, &[]),
        n("Ag-112", 47, 112, 11268.0, &[]),
        n("Ag-113", 47, 113, 19332.0, &[]),
        n("Ag-115", 47, 115, 1200.0, &[]),
        n("Sb-125", 51, 125, 87_051_674.0, &[]),
        n("Sb-127", 51, 127, 332_640.0, &[]),
        n("Te-127", 52, 127, 33660.0, &["Sb-127"]),
        n("Te-127m", 52, 127, 9_167_040.0, &[]),
        n("Sb-129", 51, 129, 15178.0, &[]),
        n("Te-129", 52, 129, 4176.0, &[]),
        n("Te-129m", 52, 129, 2_903_040.0, &[]),
        n("Te-131m", 52, 131, 119_700.0, &[]),
        n("Te-131", 52, 131, 1500.0, &[]),
        n("I-131", 53, 131, 693_377.0, &["Te-131m"]),
        n("Te-132", 52, 132, 276_826.0, &[]),
        n("I-128", 53, 128, 1499.4, &[]),
        n("I-130", 53, 130, 44496.0, &[]),
        n("I-132m", 53, 132, 4993.0, &[]),
        n("I-132", 53, 132, 8262.0, &["Te-132"]),
        n("I-133", 53, 133, 74988.0, &[]),
        n("Xe-131m", 54, 131, 1_022_976.0, &[]),
        n("Xe-133", 54, 133, 453_384.0, &["I-133"]),
        n("Xe-133m", 54, 133, 189_907.0, &[]),
        n("I-134", 53, 134, 3150.0, &[]),
        n("Cs-134", 55, 134, 65_171_364.0, &[]),
        n("I-135", 53, 135, 23688.0, &[]),
        n("Xe-135", 54, 135, 32904.0, &["I-135"]),
        n("Xe-138", 54, 138, 848.0, &[]),
        n("Cs-134m", 55, 134, 10483.0, &[]),
        n("Cs-135m", 55, 135, 3180.0, &[]),
        n("Cs-136", 55, 136, 1_124_064.0, &[]),
        n("Cs-137", 55, 137, 949_232_333.0, &[]),
        n("Cs-138", 55, 138, 1950.0, &["Xe-138"]),
        n("Ba-140", 56, 140, 1_101_686.0, &[]),
        n("La-140", 57, 140, 145_029.0, &["Ba-140"]),
        n("Ce-141", 58, 141, 2_808_346.0, &[]),
        n("Ce-143", 58, 143, 118_940.0, &[]),
        n("Pr-143", 59, 143, 1_172_448.0, &["Ce-143"]),
        n("Ce-144", 58, 144, 24_616_224.0, &[]),
        n("Nd-147", 60, 147, 952_992.0, &[]),
        n("Pm-149", 61, 149, 191_088.0, &[]),
        n("Pm-151", 61, 151, 102_240.0, &[]),
        n("Sm-153", 62, 153, 166_622.0, &[]),
        n("Pm-147", 61, 147, 82_786_440.0, &[]),
        n("Eu-155", 63, 155, 149_990_069.0, &[]),
        n("Sm-151", 62, 151, 2_840_123_338.0, &[]),
        // Upstream stores A=56 for Tc-99 (apparent typo); ported verbatim.
        n("Tc-99", 43, 56, 6_661_667_073_236.0, &[]),
        n("Zr-93", 40, 93, 50_806_650_819_093.0, &[]),
        n("Cs-135", 55, 135, 72_580_929_741_562.0, &[]),
        n("Se-79", 34, 79, 10_319_114_793_692.0, &[]),
        n("Sn-126", 50, 126, 6_879_409_862_461.0, &[]),
        n("I-129", 53, 129, 495_443_737_801_094.0, &[]),
        n("Pd-107", 46, 107, 205_120_018_834_848.0, &[]),
        n("Rb-87", 37, 87, 1_568_379_220_937_222_400.0, &[]),
        n("Nd-144", 60, 144, 7_226_536_048_181_568e7, &[]),
        n("Cd-113", 48, 113, 25_371_768_483_571_968e7, &[]),
        n("Cd-113m", 48, 113, 444_952_656.0, &[]),
        n("Se-82", 34, 82, 3_029_464_893_560_832e12, &[]),
        n("Ce-142", 58, 142, 15_778_462_987_296e11, &[]),
    ]
}

/// Look a nuclide up by its canonical TRISO-ATOPS name (case-sensitive).
///
/// Returns the matching [`TrisoAtopsNuclide`], or `None` if the name is not in
/// the supported table. Ports the `nuclides[name]` dictionary access used
/// throughout `calculation_functions.py` / `trisoatops.py`.
///
/// # Arguments
/// - `name` — canonical name such as `"Cs-137"` or `"Ag-110m"`.
#[must_use]
pub fn find_nuclide(name: &str) -> Option<TrisoAtopsNuclide> {
    supported_nuclides()
        .into_iter()
        .find(|nuc| nuc.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::time::second;

    #[test]
    fn table_has_expected_count() {
        assert_eq!(supported_nuclides().len(), TRISO_ATOPS_NUCLIDE_COUNT);
    }

    #[test]
    fn half_life_round_trips_to_seconds() {
        // Verifies the `f64` seconds → uom `Time` construction is unit-correct.
        let cs137 = find_nuclide("Cs-137").expect("Cs-137 present");
        assert_eq!(cs137.half_life.get::<second>(), 949_232_333.0);
        assert_eq!(cs137.z, 55);
    }

    #[test]
    fn known_parent_chains_present() {
        // Xe-135 is the daughter of I-135 in the TRISO-ATOPS model.
        let xe135 = find_nuclide("Xe-135").expect("Xe-135 present");
        assert_eq!(xe135.parents, &["I-135"]);
        // Kr-85 is a direct fission product (no parent).
        assert!(find_nuclide("Kr-85")
            .expect("Kr-85 present")
            .parents
            .is_empty());
    }

    #[test]
    fn unknown_nuclide_is_none() {
        assert!(find_nuclide("Pu-239").is_none());
    }
}
