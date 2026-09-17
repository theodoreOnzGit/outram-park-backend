// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors

//! **HTR-10 reflector zone compositions**, IAEA-TECDOC-1382 part 2, Table 4-3
//! (`bn:op-867c.8`, gh #214).
//!
//! # Why these and not the RMC paper's
//!
//! Li, Yu & Wei (2014) **defer** reflector and structural modelling to
//! IAEA-TECDOC-1382 — which is why `htr10_rmc`'s module docs record the k-vs-height
//! curve as not reproducible without it. This is that data.
//!
//! # What the table is
//!
//! Spatially **homogenised** atom densities for the R-Z reactor-physics model of
//! Figure 4.10, zones 0..=82, in atoms/(barn·cm). Two nuclides only: carbon and
//! **natural** boron. The model is recommended to include core structures "only
//! until the carbon bricks".
//!
//! Additional parameters stated alongside the table:
//! reflector graphite density **1.76 g/cm³**, and B4C weight ratio in boronated
//! carbon brick **5 %**.
//!
//! # Two things to be careful about
//!
//! - **The boron is NATURAL boron**, not B-10. Only 19.9 at.% of it absorbs.
//!   Reading these as B-10 densities over-absorbs by ~5x — the same misreading
//!   `pebble_beds::htr10::BoronReading::AsElementalB10` exists to price, where it
//!   was worth −5711 pcm on a single pebble.
//! - **The densities are homogenised in R-Z.** TECDOC says so explicitly: for a
//!   three-dimensional model they "are to be corrected by taking into
//!   consideration of the boring geometries". Using them unadjusted in 3-D
//!   smears the control-rod and helium-flow borings uniformly, which is a real
//!   approximation and not a transcription detail.
//!
//! # Zone 5
//!
//! The top core cavity carries **no entry** in the table — it is void, and is
//! represented here by its absence rather than by zeros, so a caller that asks
//! for it gets `None` instead of a suspiciously empty material.

/// Homogenised composition of one reflector zone \[atoms/(barn·cm)\].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ZoneComposition {
    /// Carbon atom density.
    pub carbon: f64,
    /// **Natural** boron atom density — not B-10. See the module docs.
    pub natural_boron: f64,
}

/// Density of reflector graphite \[g/cm³\], stated with Table 4-3.
pub const REFLECTOR_GRAPHITE_DENSITY: f64 = 1.76;
/// Weight ratio of B4C in boronated carbon brick \[-\], stated with Table 4-3.
pub const B4C_WEIGHT_RATIO_IN_BORONATED_BRICK: f64 = 0.05;
/// Highest zone number in the table.
pub const MAX_ZONE: usize = 82;

/// Table 4-3, as `(zones, carbon, natural_boron)` rows exactly as printed.
const ROWS: &[(&[usize], f64, f64)] = &[
    (&[0], 0.851047E-01, 0.456926E-06),
    (&[1], 0.729410E-01, 0.329811E-02),
    (&[2], 0.851462E-01, 0.457148E-06),
    (&[3], 0.145350E-01, 0.780384E-07),
    (&[4], 0.802916E-01, 0.431084E-06),
    // zone 5 -- top core cavity, NO ENTRY (void)
    (&[6, 7], 0.538275E-01, 0.288999E-06),
    (&[8], 0.781408E-01, 0.419537E-06),
    (&[9], 0.823751E-01, 0.442271E-06),
    (&[10], 0.843647E-01, 0.298504E-03),
    (&[11], 0.817101E-01, 0.156416E-03),
    (&[12], 0.850790E-01, 0.209092E-03),
    (&[13], 0.819167E-01, 0.358529E-04),
    (&[14], 0.541118E-01, 0.577456E-04),
    (&[15], 0.332110E-01, 0.178309E-06),
    (&[16], 0.881811E-01, 0.358866E-04),
    (&[17, 55, 72, 74, 75, 76, 78, 79], 0.765984E-01, 0.346349E-02),
    (&[18, 56, 73], 0.797184E-01, 0.0),
    (&[19], 0.761157E-01, 0.344166E-02),
    (&[20], 0.878374E-01, 0.471597E-06),
    (&[21], 0.579696E-01, 0.311238E-06),
    (&[22, 23, 25, 49, 50, 52, 54, 66, 67, 69, 71, 80], 0.882418E-01, 0.473769E-06),
    (&[24, 51, 68], 0.879541E-01, 0.168369E-03),
    (&[26], 0.846754E-01, 0.454621E-06),
    (&[27], 0.589319E-01, 0.266468E-02),
    (&[28, 82], 0.678899E-01, 1.400000E-05),
    (&[29], 0.403794E-01, 1.400000E-05),
    (&[30, 41], 0.678899E-01, 0.364500E-06),
    (&[31, 32, 33, 34, 35, 36, 37, 38, 39, 40], 0.634459E-01, 0.340640E-06),
    (&[42], 0.676758E-01, 0.125331E-03),
    (&[43, 45], 0.861476E-01, 0.462525E-06),
    (&[44], 0.829066E-01, 0.445124E-06),
    (&[46], 0.747805E-01, 0.338129E-02),
    (&[47], 0.778265E-01, 0.0),
    (&[48], 0.582699E-01, 0.312850E-06),
    (&[53], 0.855860E-01, 0.459510E-06),
    (&[57], 0.728262E-01, 0.391003E-06),
    (&[58, 59, 61, 63], 0.760368E-01, 0.408240E-06),
    (&[60], 0.757889E-01, 0.145082E-03),
    (&[62], 0.737484E-01, 0.395954E-06),
    (&[64], 0.660039E-01, 0.298444E-02),
    (&[65], 0.686924E-01, 0.0),
    (&[70], 0.861500E-01, 0.462538E-06),
    (&[77], 0.749927E-01, 0.339088E-02),
    (&[81], 0.797184E-01, 0.0),
];

/// Composition of reflector `zone`, or `None` for a zone the table does not
/// list — which is zone 5, the top core cavity, and any index above
/// [`MAX_ZONE`].
#[must_use]
pub fn zone_composition(zone: usize) -> Option<ZoneComposition> {
    ROWS.iter().find_map(|(zones, c, b)| {
        zones.contains(&zone).then_some(ZoneComposition {
            carbon: *c,
            natural_boron: *b,
        })
    })
}

/// Every zone the table lists, ascending.
#[must_use]
pub fn listed_zones() -> Vec<usize> {
    let mut v: Vec<usize> = ROWS.iter().flat_map(|(z, _, _)| z.iter().copied()).collect();
    v.sort_unstable();
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **Every zone 0..=82 is accounted for, exactly once, except the cavity.**
    ///
    /// A transcription of an 83-row table is only as good as its completeness
    /// check: a dropped row is a zone silently missing its absorber, and a
    /// duplicated one is two different compositions for the same place, with
    /// whichever comes first winning. Neither shows up as an error.
    ///
    /// **Results (2026-09-17):** 82 of 83 zones listed, no duplicates, the one
    /// gap being zone 5 (top core cavity, no entry in the table).
    #[test]
    fn every_zone_is_listed_exactly_once_except_the_cavity() {
        let zones = listed_zones();
        let mut dupes: Vec<usize> = Vec::new();
        for w in zones.windows(2) {
            if w[0] == w[1] {
                dupes.push(w[0]);
            }
        }
        assert!(dupes.is_empty(), "zones listed more than once: {dupes:?}");

        let missing: Vec<usize> = (0..=MAX_ZONE).filter(|z| !zones.contains(z)).collect();
        println!("{} zones listed; missing: {missing:?}", zones.len());
        assert_eq!(
            missing,
            vec![5],
            "exactly one zone should be absent -- zone 5, the top core cavity, \
             which the table does not list because it is void"
        );
        assert!(zone_composition(5).is_none(), "the cavity must report None, not zeros");
    }

    /// The boronated-brick rows must actually be boronated, and the carbon-brick
    /// rows must actually carry none. This catches a column swap or a dropped
    /// exponent, which a row count cannot.
    #[test]
    fn boronated_and_plain_bricks_are_distinguishable() {
        // Rows the table labels "Boronated carbon bricks".
        for z in [1, 19, 27, 46, 64, 77, 17, 55] {
            let c = zone_composition(z).expect("listed");
            assert!(
                c.natural_boron > 1.0e-3,
                "zone {z} is a boronated brick but carries only {:.3e} boron",
                c.natural_boron
            );
        }
        // Rows the table labels plain "Carbon bricks" -- exactly zero boron.
        for z in [18, 47, 56, 65, 73, 81] {
            let c = zone_composition(z).expect("listed");
            assert_eq!(c.natural_boron, 0.0, "zone {z} is a plain carbon brick");
        }
    }

    /// Carbon densities must be physically plausible for graphite: the stated
    /// reflector density is 1.76 g/cm3, which is 8.826e-2 atoms/b/cm of C-12,
    /// and no homogenised zone may exceed it — a zone is graphite diluted by
    /// borings and helium, never denser than solid.
    #[test]
    fn no_zone_is_denser_than_solid_reflector_graphite() {
        // N = rho * N_A / M = 1.76 * 6.02214076e23 / 12.011 barn-cm
        let solid = 1.76 * 6.022_140_76e23 / 12.011 * 1.0e-24;
        let mut worst = (0usize, 0.0_f64);
        for z in listed_zones() {
            let c = zone_composition(z).expect("listed").carbon;
            if c > worst.1 {
                worst = (z, c);
            }
            assert!(
                c <= solid * 1.001,
                "zone {z} carbon {c:.6e} exceeds solid graphite at 1.76 g/cm3 \
                 ({solid:.6e}) -- a homogenised zone cannot be denser than the \
                 material it homogenises"
            );
        }
        println!(
            "densest zone {} at {:.6e}; solid graphite at 1.76 g/cm3 is {:.6e} ({:.1} %)",
            worst.0, worst.1, solid, 100.0 * worst.1 / solid
        );
    }
}
