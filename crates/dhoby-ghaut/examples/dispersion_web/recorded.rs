//! **Recorded results** the demo shows instead of computing them live: the
//! capstone's chain (TRISO-ATOPS release through `boon-lay` and `tampines`)
//! is too heavy to build for a phone, so its numbers are quoted with their
//! provenance (the demo rule: heavy cases show recorded results, never a
//! pretend live computation).
//!
//! **Provenance.** `cargo run --release -p sembawang --example
//! htr10_air_ingress_kora_bound`, re-measured 2026-10-04 on `develop`
//! `d4428668be` (one core, 0.34 s), reproducing the example's 2026-09-30
//! record; both are in that example's doc comment. Research, education and
//! V&V only: a deliberately pessimistic bounding model, never a dose to a
//! person (`RESPONSIBLE_USE.md`).

/// Where the numbers below come from, shown in the demo beside them.
pub const SOURCE: &str = "sembawang example htr10_air_ingress_kora_bound, re-run 2026-10-04 \
     (develop d4428668be), reproducing its 2026-09-30 record";

/// The dispersion the capstone used: buangkok's single plume, ground
/// release, 1 m/s at 10 m, the largest chi/Q of the six classes (class F at
/// every distance), ground-level centreline.
pub const CHI_OVER_Q_400M_F_1MS: f64 = 2.8568e-3;

/// Pathway doses at 400 m in that dispersion, Sv: submersion (FGR-15),
/// inhalation (FGR-11), groundshine over 96 h (FGR-15).
pub const DOSE_400M_SV: [(&str, f64); 3] = [
    ("submersion", 1.350e-3),
    ("inhalation", 4.142e-2),
    ("groundshine 96 h", 2.337e-2),
];

/// The capstone's dose against distance, (m, mSv).
pub const DOSE_VS_DISTANCE: [(f64, f64); 9] = [
    (400.0, 66.137),
    (600.0, 33.530),
    (800.0, 20.752),
    (1000.0, 14.318),
    (1500.0, 7.382),
    (2000.0, 4.779),
    (3000.0, 2.671),
    (5000.0, 1.331),
    (10000.0, 0.540),
];

/// Released activity over the 96 h window, Bq, and the core inventory it is
/// a fraction of (Liu & Cao 2002 Table 1), for the headline nuclides.
pub const RELEASE: [(&str, f64, f64); 6] = [
    ("Xe-133", 3.914e12, 2.050e16),
    ("I-131", 1.864e12, 9.770e15),
    ("I-133", 4.029e12, 2.110e16),
    ("Cs-137", 7.064e11, 6.920e14),
    ("Cs-134", 4.636e11, 3.110e14),
    ("Ag-110m", 5.388e10, 2.160e12),
];

/// Total released over 96 h, Bq.
pub const RELEASE_TOTAL_BQ: f64 = 1.689e13;

#[cfg(test)]
mod tests {
    use super::*;

    /// The recorded pathways sum to the recorded 400 m dose, and the curve is
    /// the 400 m dose times the chi/Q ratio (the capstone is linear in chi/Q),
    /// to the four figures printed.
    #[test]
    fn the_recorded_numbers_hang_together() {
        let total_msv: f64 = DOSE_400M_SV.iter().map(|p| p.1).sum::<f64>() * 1e3;
        assert!(
            (total_msv - DOSE_VS_DISTANCE[0].1).abs() < 0.01,
            "{total_msv}"
        );
        for w in DOSE_VS_DISTANCE.windows(2) {
            assert!(w[1].1 < w[0].1, "dose falls with distance");
        }
    }
}
