//! **Indicative** effective dose rate \[µSv/h\] from the dispersion channel's
//! live air concentration and dry deposit, for the Map tab's "Dose rate"
//! basis (maintainer direction, 2026-09-29).
//!
//! # SCOPE — binding, do not soften
//!
//! **Indicative dose rate, research and education only. It is not a dose to
//! any real person, and it is not for emergency, regulatory, occupational or
//! medical use** (`RESPONSIBLE_USE.md`, `crates/buangkok/CLAUDE.md`). The
//! source feeding it is not a source term either: five nuclides leaked from
//! the primary circuit at ~1 %/day with TRISO-ATOPS reference failure
//! fractions and no building, filter or stack model (see
//! [`super::atmospheric_dispersion`]).
//!
//! # No second formula: every term goes through buangkok
//!
//! | Pathway | Input | buangkok function | Coefficient (buangkok `coefficients`) |
//! |---|---|---|---|
//! | cloud submersion | live air concentration \[Bq/m^3\] | `submersion_dose_rate_msv_per_s` | FGR-15 (2025) Table 4-6, adult |
//! | inhalation (committed) | live air concentration \[Bq/m^3\] | `inhalation_committed_dose_rate_msv_per_s` (adult breathing rate 8400 m^3/y) | FGR-11 Table 2.1 "Effective", adult, max over D/W/Y |
//! | ground shine | dry deposit \[Bq/m^2\] | `ground_shine_dose_rate_msv_per_s` | FGR-15 (2025) Table 4-1, adult |
//!
//! Those three buangkok functions are the coefficient products its ported
//! pyDOSEIA pathways (`submersion_dose`, `inhalation_dose`,
//! `ground_shine_dose`) are built on, so the dose rate here and a buangkok
//! dose are one formula; buangkok's code-to-code fixture still passes
//! bit-for-bit on them. Cs-137's external coefficients include Ba-137m in
//! secular equilibrium (0.944, FGR-15 Example 4) through buangkok's progeny
//! lookup.
//!
//! **Adult** (FGR-15 "Adult" column; FGR-11 is adult-only anyway). The
//! "inhalation dose rate" is the **committed** dose per hour of breathing,
//! not a dose received in that hour; it is added to the two external rates as
//! common screening practice, which also mixes ICRP 103 (FGR-15) with ICRP
//! 26/30 (FGR-11) weighting. Both caveats are on screen.
//!
//! # Missing coefficients are missing, never zero
//!
//! FGR-11 has no inhalation entry for the noble gases (Kr-85, Xe-133): their
//! inhalation dose is negligible next to submersion, which is why no
//! inhalation coefficient is published, but it is **not** zero and is not
//! shown as zero. [`DoseRateCoefficients`] holds `None` for it, the tables
//! print "missing", and the totals say which terms they leave out.
//!
//! A **zero deposit** is different: Kr and Xe do not deposit (changi's noble
//! gas deposition velocity is exactly zero), so their ground-shine rate is a
//! real zero from a coefficient that exists.
//!
//! # Semi-infinite cloud, not buangkok's finite-cloud plume shine
//!
//! Submersion uses FGR-15's **semi-infinite-cloud** coefficient times the
//! live ground-level concentration at each pixel. buangkok's finite-cloud
//! plume shine (`pydoseia::plume_shine`, on petir's CPU `qags`) was
//! considered and **not used**, for two reasons:
//!
//! 1. **No gamma-line data.** It needs per-nuclide gamma energies and
//!    intensities plus air attenuation and build-up tables, which buangkok
//!    requires the caller to supply and which no freely usable table in this
//!    workspace provides.
//! 2. **Cost.** It is a nested adaptive triple integral per gamma line per
//!    receptor (thousands of integrand evaluations per point, not measured
//!    here); at up to 512 x 512 = 262 144 pixels refreshed at 10 Hz that is
//!    orders of magnitude beyond the frame budget, where the semi-infinite
//!    rate is one multiply per pixel. It has no GPU path.
//!
//! **Direction of the approximation.** The semi-infinite cloud assumes
//! uniform air out to several photon mean free paths (~100 m at 1 MeV). Near
//! the plume centreline, where the plume is narrower than that, it
//! **over-states** submersion; beneath the elevated (40 m stack) plume before
//! it reaches the ground, where the ground-level concentration is small but
//! the cloud overhead still irradiates, it **under-states** it. Neither
//! direction is claimed as a bound overall.
//!
//! # The map pixel is the AIR pathways only
//!
//! A pixel is `chi/Q_inst x factor`, with `factor` = [`air_dose_rate_per_unit_chi_over_q`]:
//! submersion + inhalation summed over the nuclides whose coefficients exist.
//! **Ground shine is in the receptor tables only**, because the map has no
//! per-pixel deposition field: the deposit is computed by `changi`'s survey at
//! the 24 receptors. And that deposit is what one 1200 s puff run leaves, so
//! for a release held longer the ground-shine rate here **under-states** the
//! build-up.
//!
//! # GPU (petir shaders)
//!
//! The per-pixel cost of the dose-rate map is the `chi/Q` field, and that is
//! already dispatched through petir's WGSL runner
//! (`changi::puff::wgsl::field_auto`, `petir::wgsl::gpu::GpuContext`), with
//! `changi`'s CPU thread pool as the fallback when the host has no adapter.
//! The dose rate is **linear** in that field with one nuclide-independent
//! factor (decay in transit is neglected, as for the Bq/m^3 basis), so it is
//! the GPU field times a scalar in f64. A second shader pass for one multiply
//! per pixel would cost a dispatch and a read-back to save 0.06 ms; folding
//! the factor into the puff kernel would change nothing measurable either.
//! `atmospheric_dispersion::tests::the_dose_rate_field_agrees_gpu_vs_cpu`
//! (2026-09-29, 512 x 512, 120 puffs): GPU 1.5-2.8 ms, CPU pool 174-181 ms,
//! serial 282-345 ms, factor pass 0.06 ms; GPU vs serial within 7.6e-7
//! relative near the peak.
//!
//! # What the map shows (measured 2026-09-29)
//!
//! On the map puff model (inter-monsoon, class B, 1 m/s): at the default
//! 1200 K kernel the peak air dose rate over the field is **2.27e-7 µSv/h**,
//! 6.5 decades under the 0.8 µSv/h floor; at 2000 K, 3.05e-6 µSv/h, still 5.4
//! decades under. The whole field draws grey on the default scale and the map
//! says why. Table: `reference/References.md`, "Dose-rate basis".
//!
//! Per `RESPONSIBLE_USE.md`, AI-assisted draft pending human review.

use std::sync::OnceLock;

use buangkok::coefficients as fgr;
use buangkok::pydoseia::dcf::AgeBracket;
use buangkok::pydoseia::dose::{
    ground_shine_dose_rate_msv_per_s, inhalation_committed_dose_rate_msv_per_s,
    submersion_dose_rate_msv_per_s,
};

use super::fission_product_release::TRACKED_NUCLIDES;

/// Number of tracked nuclides.
pub const N: usize = TRACKED_NUCLIDES.len();

/// The age bracket every coefficient is taken for.
pub const AGE: AgeBracket = AgeBracket::Adult;

/// An adult's age for buangkok's breathing-rate rule (any age above 1 year
/// gives pyDOSEIA's 8400 m^3/y). Only the bracket matters, not the number.
pub const AGE_YEARS: f64 = 30.0;

/// mSv/s -> µSv/h: `1000 µSv/mSv x 3600 s/h`. A unit conversion, nothing else.
pub const USV_PER_H_PER_MSV_PER_S: f64 = 1000.0 * 3600.0;

/// **Maintainer's colour-scale floor** \[µSv/h\]: 0.8 µSv/h, "≈ airliner at
/// cruise altitude" ("just about airline", maintainer direction 2026-09-29).
/// The maintainer's figure, not a cited value; an indicative anchor, not a
/// regulatory level.
pub const FLOOR_USV_PER_H: f64 = 0.8;

/// **Maintainer's colour-scale top** \[µSv/h\]: 1 mSv/h, "high" ("a bit more
/// dangerous", maintainer direction 2026-09-29). Indicative, not regulatory.
pub const TOP_USV_PER_H: f64 = 1000.0;

/// The three pathways, in table column order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pathway {
    /// External, from the cloud (semi-infinite).
    Submersion,
    /// Committed, per hour of breathing.
    Inhalation,
    /// External, from the dry deposit.
    GroundShine,
}

impl Pathway {
    /// All three, in column order.
    pub const ALL: [Pathway; 3] = [
        Pathway::Submersion,
        Pathway::Inhalation,
        Pathway::GroundShine,
    ];

    /// Column index.
    pub fn index(self) -> usize {
        match self {
            Pathway::Submersion => 0,
            Pathway::Inhalation => 1,
            Pathway::GroundShine => 2,
        }
    }

    /// Short label for a table heading.
    pub fn label(self) -> &'static str {
        match self {
            Pathway::Submersion => "Cloud submersion",
            Pathway::Inhalation => "Inhalation (committed)",
            Pathway::GroundShine => "Ground shine",
        }
    }
}

/// The coefficient behind each (nuclide, pathway), from buangkok's shipped
/// EPA tables through buangkok's own lookups. `None` = **missing**.
#[derive(Clone, Debug, PartialEq)]
pub struct DoseRateCoefficients {
    /// `[nuclide][pathway]`: Sv m^3 Bq^-1 s^-1 (submersion), Sv/Bq
    /// (inhalation), Sv m^2 Bq^-1 s^-1 (ground shine).
    pub by_nuclide: [[Option<f64>; 3]; N],
}

impl DoseRateCoefficients {
    /// Load the FGR-15 (2025) and FGR-11 coefficients for the tracked nuclides.
    pub fn fgr() -> Self {
        let (sub, gs, chains, inh) = (
            fgr::fgr15_air_submersion(),
            fgr::fgr15_ground_surface(),
            fgr::fgr15_short_lived_progeny(),
            fgr::fgr11_inhalation(),
        );
        Self {
            by_nuclide: core::array::from_fn(|k| {
                let n = TRACKED_NUCLIDES[k];
                [
                    fgr::external_coefficient(&sub, &chains, n, AGE),
                    fgr::fgr11_inhalation_max_over_classes(&inh, n, AGE),
                    fgr::external_coefficient(&gs, &chains, n, AGE),
                ]
            }),
        }
    }

    /// The coefficient for one nuclide and pathway, `None` when missing.
    pub fn get(&self, nuclide: usize, pathway: Pathway) -> Option<f64> {
        self.by_nuclide[nuclide][pathway.index()]
    }

    /// The (nuclide, pathway) pairs with no coefficient, for on-screen notes.
    pub fn missing(&self) -> Vec<(&'static str, Pathway)> {
        let mut out = Vec::new();
        for (k, name) in TRACKED_NUCLIDES.iter().enumerate() {
            for p in Pathway::ALL {
                if self.get(k, p).is_none() {
                    out.push((*name, p));
                }
            }
        }
        out
    }
}

/// The process-wide coefficient set, parsed once from buangkok's CSVs.
pub fn coefficients() -> &'static DoseRateCoefficients {
    static C: OnceLock<DoseRateCoefficients> = OnceLock::new();
    C.get_or_init(DoseRateCoefficients::fgr)
}

/// One nuclide's rate for one pathway \[µSv/h\], through buangkok. `input` is
/// the live air concentration \[Bq/m^3\] for submersion and inhalation, the
/// dry deposit \[Bq/m^2\] for ground shine. `None` when the coefficient is
/// missing; `NAN` when the input is not available.
pub fn pathway_rate_usv_per_h(
    pathway: Pathway,
    input: f64,
    coefficient: Option<f64>,
) -> Option<f64> {
    let c = coefficient?;
    let msv_per_s = match pathway {
        Pathway::Submersion => submersion_dose_rate_msv_per_s(input, c),
        Pathway::Inhalation => inhalation_committed_dose_rate_msv_per_s(input, c, AGE_YEARS)
            .expect("AGE_YEARS is a number"),
        Pathway::GroundShine => ground_shine_dose_rate_msv_per_s(input, c),
    };
    Some(msv_per_s * USV_PER_H_PER_MSV_PER_S)
}

/// The per-nuclide, per-pathway split at one receptor \[µSv/h\]:
/// `[nuclide][pathway]`, `None` = missing coefficient.
///
/// `chi_over_q_inst` \[s/m^3\] × `rates[k]` \[Bq/s\] is nuclide `k`'s live air
/// concentration; `deposits[k]` \[Bq/m^2\] its dry deposit.
pub fn receptor_split(
    chi_over_q_inst: f64,
    rates_bq_per_s: &[f64; N],
    deposits_bq_per_m2: &[f64; N],
    c: &DoseRateCoefficients,
) -> [[Option<f64>; 3]; N] {
    core::array::from_fn(|k| {
        let air = chi_over_q_inst * rates_bq_per_s[k];
        [
            pathway_rate_usv_per_h(Pathway::Submersion, air, c.get(k, Pathway::Submersion)),
            pathway_rate_usv_per_h(Pathway::Inhalation, air, c.get(k, Pathway::Inhalation)),
            pathway_rate_usv_per_h(
                Pathway::GroundShine,
                deposits_bq_per_m2[k],
                c.get(k, Pathway::GroundShine),
            ),
        ]
    })
}

/// Sum of one pathway over the nuclides whose coefficient exists, and the
/// nuclides left out. The sum is a **partial** total when the list is not
/// empty, and the caller must say so.
pub fn pathway_total(split: &[[Option<f64>; 3]; N], pathway: Pathway) -> (f64, Vec<&'static str>) {
    let mut total = 0.0;
    let mut missing = Vec::new();
    for (k, row) in split.iter().enumerate() {
        match row[pathway.index()] {
            Some(v) => total += v,
            None => missing.push(TRACKED_NUCLIDES[k]),
        }
    }
    (total, missing)
}

/// The map's per-pixel factor \[µSv/h per (s/m^3)\]: the air-pathway dose
/// rate (submersion + inhalation, summed over nuclides with a coefficient)
/// at unit instantaneous `chi/Q`. A pixel is `chi/Q_inst x` this.
///
/// Computed by calling the buangkok pathway functions at `chi/Q = 1 s/m^3`,
/// so it is the same formula as [`receptor_split`]; the two differ only by
/// floating-point reassociation (pinned to 1e-12 relative by
/// `tests::a_pixel_is_the_sum_of_the_buangkok_pathways`). `NAN` when any
/// per-nuclide release rate is unavailable: a partial source would read as a
/// total.
pub fn air_dose_rate_per_unit_chi_over_q(
    rates_bq_per_s: &[f64; N],
    c: &DoseRateCoefficients,
) -> f64 {
    if rates_bq_per_s.iter().any(|r| !r.is_finite()) {
        return f64::NAN;
    }
    let split = receptor_split(1.0, rates_bq_per_s, &[0.0; N], c);
    pathway_total(&split, Pathway::Submersion).0 + pathway_total(&split, Pathway::Inhalation).0
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A representative per-nuclide release [Bq/s], order of the default
    /// plant's (the 1200 K kernel case prints ~9.5 Bq/s in total). Values are
    /// arbitrary test inputs, not plant data.
    const RATES: [f64; N] = [7.0, 2.0, 0.3, 0.1, 0.05];

    /// **A dose-rate pixel is the sum of the buangkok pathway functions for
    /// the same concentration.**
    ///
    /// Methodology: for a spread of instantaneous `chi/Q`, the map pixel
    /// `chi x air_dose_rate_per_unit_chi_over_q` is compared with the sum over
    /// nuclides of buangkok's `submersion_dose_rate_msv_per_s` and
    /// `inhalation_committed_dose_rate_msv_per_s`, called directly on each
    /// nuclide's air concentration `chi x rate_k` and converted to µSv/h.
    /// Pass: relative difference <= 1e-12 (float reassociation of ~10 terms
    /// in f64 is a few 1e-16; 1e-12 leaves room without admitting any
    /// formula difference, which would show at O(1)).
    ///
    /// Results (2026-09-29): pass; printed worst relative difference below.
    #[test]
    fn a_pixel_is_the_sum_of_the_buangkok_pathways() {
        let c = coefficients();
        let factor = air_dose_rate_per_unit_chi_over_q(&RATES, c);
        let mut worst = 0.0_f64;
        for chi in [1e-12, 3.3e-9, 1.8e-5, 4.0e-3] {
            let mut direct = 0.0;
            for k in 0..N {
                let air = chi * RATES[k];
                if let Some(s) = c.get(k, Pathway::Submersion) {
                    direct += submersion_dose_rate_msv_per_s(air, s) * 1000.0 * 3600.0;
                }
                if let Some(i) = c.get(k, Pathway::Inhalation) {
                    direct += inhalation_committed_dose_rate_msv_per_s(air, i, 30.0).unwrap()
                        * 1000.0
                        * 3600.0;
                }
            }
            let pixel = chi * factor;
            let rel = ((pixel - direct) / direct).abs();
            worst = worst.max(rel);
            assert!(
                rel <= 1e-12,
                "chi {chi:e}: pixel {pixel:e} vs buangkok sum {direct:e}"
            );
        }
        println!("pixel vs buangkok pathway sum: worst relative difference {worst:e}");
    }

    /// Missing coefficients are missing, not zero: the noble gases have no
    /// FGR-11 inhalation entry, so their inhalation cell is `None`, the
    /// pathway total names them, and nothing else is missing.
    #[test]
    fn missing_coefficients_are_missing_not_zero() {
        let c = coefficients();
        assert_eq!(
            c.missing(),
            vec![
                ("Kr-85", Pathway::Inhalation),
                ("Xe-133", Pathway::Inhalation)
            ]
        );
        let split = receptor_split(1e-6, &RATES, &[1.0; N], c);
        assert_eq!(split[0][Pathway::Inhalation.index()], None);
        assert_eq!(split[1][Pathway::Inhalation.index()], None);
        let (_, left_out) = pathway_total(&split, Pathway::Inhalation);
        assert_eq!(left_out, vec!["Kr-85", "Xe-133"]);
        for row in &split {
            assert!(row[Pathway::Submersion.index()].unwrap() > 0.0);
            assert!(row[Pathway::GroundShine.index()].unwrap() > 0.0);
        }
    }

    /// The coefficients the map uses are the values in buangkok's CSVs, as
    /// read off FGR-15 (2025) Tables 4-6/4-1 and FGR-11 Table 2.1 (Adult),
    /// with Cs-137 carrying Ba-137m x 0.944.
    #[test]
    fn the_fgr_values_match_the_csv() {
        let c = coefficients();
        let exp: [[Option<f64>; 3]; N] = [
            [Some(2.40e-16), None, Some(9.62e-18)],          // Kr-85
            [Some(1.22e-15), None, Some(1.85e-17)],          // Xe-133
            [Some(1.68e-14), Some(8.89e-9), Some(2.43e-16)], // I-131
            [
                Some(9.37e-17 + 2.68e-14 * 0.944),
                Some(8.63e-9),
                Some(3.01e-18 + 3.87e-16 * 0.944),
            ], // Cs-137 (+ Ba-137m)
            [Some(1.28e-13), Some(2.17e-8), Some(1.73e-15)], // Ag-110m
        ];
        assert_eq!(
            TRACKED_NUCLIDES,
            ["Kr-85", "Xe-133", "I-131", "Cs-137", "Ag-110m"]
        );
        assert_eq!(c.by_nuclide, exp);
    }

    /// The default scale anchors are the maintainer's: 0.8 µSv/h and 1 mSv/h.
    #[test]
    fn the_anchors_are_the_maintainers() {
        assert_eq!(FLOOR_USV_PER_H, 0.8);
        assert_eq!(TOP_USV_PER_H, 1000.0);
        assert_eq!(USV_PER_H_PER_MSV_PER_S, 3.6e6);
    }

    /// An unavailable release rate gives an unavailable (NAN) factor, never a
    /// partial sum.
    #[test]
    fn an_unavailable_rate_is_not_a_partial_sum() {
        let mut r = RATES;
        r[3] = f64::NAN;
        assert!(air_dose_rate_per_unit_chi_over_q(&r, coefficients()).is_nan());
    }
}
