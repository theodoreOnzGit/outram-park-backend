// SPDX-License-Identifier: GPL-3.0-only
//! Gaussian-plume dispersion as pyDOSEIA computes it: the Pasquill-Gifford
//! sigmas, the release-height wind correction, the two master equations and
//! the dilution factor (`chi/Q`) for each of pyDOSEIA's release modes.
//!
//! # Provenance
//!
//! Ported from pyDOSEIA `metfunc.py` (`MetFunc.sigmay`, `sigmaz`,
//! `height_correction_factor`, `master_eq_single_plume`,
//! `master_eq_sector_averaged_plume`, `dilution_per_sector`,
//! `synthetic_TJFD_for_single_plume`, `get_max_dilution_factor`), upstream
//! <https://github.com/BiswajitSadhu/pyDOSEIA> at commit
//! `dca4cdc3bb0bef7f7e692c8991cf536c91e7a4ce`. Copyright (c) 2024 Dr. Biswajit
//! Sadhu; MIT licence (full notice in `crates/buangkok/NOTICE`). Upstream cites
//! Hukkoo and Bapat's BARC manual (eqs. 2.5, 2.7, 2.32, 2.33) for the master
//! equations and Pasquill (1974) for the height correction. Paper: Sadhu et
//! al., *Health Physics* 130(1) (2026) 94-110, doi:10.1097/HP.0000000000002014.
//!
//! The arithmetic is written in upstream's operation order so the port can be
//! compared with it to the last bit (`tests/pydoseia_code_to_code.rs`).
//!
//! # Overlap with `changi`
//!
//! `changi::puff` and `changi::activity::chi_over_q` also give Pasquill-Gifford
//! Gaussian dispersion. This is a **separate** port of pyDOSEIA's own model,
//! kept inside `buangkok` so it can be verified code-to-code against pyDOSEIA.
//! The sigma fits differ (upstream uses the BARC/AERB power-law set below, not
//! Briggs), so the two are not interchangeable and are not unified here.

use uom::si::f64::{Length, Velocity};
use uom::si::length::meter;
use uom::si::velocity::meter_per_second;

use super::met::{MetClimatology, CALM_SPEED_CLASS, SPEED_CLASS_COUNT, WSPEED_K_KMPH};
use super::units::DilutionFactor;

/// Pasquill stability class, A (most unstable) to F (most stable).
///
/// Upstream carries this as the integer 1-6; [`StabilityClass::code`] gives
/// that integer back.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StabilityClass {
    /// Extremely unstable (upstream 1).
    A,
    /// Moderately unstable (upstream 2).
    B,
    /// Slightly unstable (upstream 3).
    C,
    /// Neutral (upstream 4).
    D,
    /// Slightly stable (upstream 5).
    E,
    /// Moderately stable (upstream 6).
    F,
}

impl StabilityClass {
    /// All six classes, in upstream's order.
    pub const ALL: [StabilityClass; 6] = [Self::A, Self::B, Self::C, Self::D, Self::E, Self::F];

    /// Upstream's integer code, 1 (A) to 6 (F).
    #[must_use]
    pub const fn code(self) -> u8 {
        self as u8 + 1
    }

    /// Zero-based index, 0 (A) to 5 (F).
    #[must_use]
    pub const fn index(self) -> usize {
        self as usize
    }

    /// From upstream's integer code (1-6). `None` outside that range.
    #[must_use]
    pub const fn from_code(code: u8) -> Option<Self> {
        match code {
            1 => Some(Self::A),
            2 => Some(Self::B),
            3 => Some(Self::C),
            4 => Some(Self::D),
            5 => Some(Self::E),
            6 => Some(Self::F),
            _ => None,
        }
    }
}

/// Lateral plume spread `sigma_y = A_y x^0.9031`, m, for downwind distance `x`.
///
/// Upstream (`MetFunc.sigmay`) also computes a sampling-time correction factor
/// but never applies it (the multiplication is commented out at `dca4cdc3`);
/// this port likewise does not apply one.
#[must_use]
pub fn sigma_y(stability: StabilityClass, x: Length) -> Length {
    const AY: [f64; 6] = [0.3658, 0.2751, 0.2089, 0.1471, 0.1046, 0.0722];
    let x1 = x.get::<meter>();
    Length::new::<meter>(AY[stability.index()] * x1.powf(0.9031))
}

/// Vertical plume spread `sigma_z = A_z x^q + r`, m, with three distance bands
/// (`x < 100 m`, `100 <= x <= 1000 m`, `x > 1000 m`) exactly as upstream.
///
/// The bands meet only approximately at 100 m and 1000 m: the jumps are at
/// most 0.84 % (class E at 1000 m). The port keeps them. A NaN distance falls through every band; upstream
/// raises `ValueError`, the port returns NaN.
#[must_use]
pub fn sigma_z(stability: StabilityClass, x: Length) -> Length {
    let x1 = x.get::<meter>();
    let i = stability.index();
    let (az, q, r) = if x1 < 100.0 {
        const AZ: [f64; 6] = [0.192, 0.156, 0.116, 0.079, 0.063, 0.053];
        const Q: [f64; 6] = [0.936, 0.922, 0.905, 0.881, 0.871, 0.814];
        (AZ[i], Q[i], 0.0)
    } else if x1 <= 1000.0 {
        const AZ: [f64; 6] = [0.00066, 0.038, 0.113, 0.222, 0.211, 0.086];
        const Q: [f64; 6] = [1.941, 1.149, 0.911, 0.725, 0.678, 0.740];
        const R: [f64; 6] = [9.27, 3.3, 0.0, -1.7, -1.3, -0.35];
        (AZ[i], Q[i], R[i])
    } else if x1 > 1000.0 {
        const AZ: [f64; 6] = [0.00024, 0.055, 0.113, 1.26, 6.73, 18.05];
        const Q: [f64; 6] = [2.094, 1.098, 0.911, 0.516, 0.305, 0.180];
        const R: [f64; 6] = [-9.6, 2.0, 0.0, -13.0, -34.0, -48.6];
        (AZ[i], Q[i], R[i])
    } else {
        return Length::new::<meter>(f64::NAN);
    };
    Length::new::<meter>((az * x1.powf(q)) + r)
}

/// Wind-speed correction from measurement height to release height,
/// `(H / H_m)^p` with `p = n / (2 - n)`, `n = 0.2` (A-C), `0.25` (D), `0.5`
/// (E-F). A release height below 10 m is raised to 10 m first, as upstream.
///
/// Upstream multiplies the wind speed by this factor, so the dilution factor
/// is divided by it.
#[must_use]
pub fn height_correction_factor(
    stability: StabilityClass,
    release_height: Length,
    measurement_height: Length,
) -> f64 {
    let an = match stability {
        StabilityClass::A | StabilityClass::B | StabilityClass::C => 0.2,
        StabilityClass::D => 0.25,
        StabilityClass::E | StabilityClass::F => 0.50,
    };
    let p = an / (2.0 - an);
    let h = release_height.get::<meter>();
    let hm = measurement_height.get::<meter>();
    if h < 10.0 {
        (10.0 / hm).powf(p)
    } else {
        (h / hm).powf(p)
    }
}

/// Where the concentration is evaluated.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Receptor {
    /// Ground level on the plume centreline, `y = z = 0` (upstream's
    /// `max_conc_plume_central_line_gl: True`).
    GroundLevelCentreline,
    /// A crosswind offset `y` and height `z` (upstream's config `Y`, `Z`). The
    /// sector-averaged equation uses only `z`.
    Offset {
        /// Crosswind distance from the plume axis.
        y: Length,
        /// Height above ground.
        z: Length,
    },
}

impl Receptor {
    fn yz(self) -> (f64, f64) {
        match self {
            Self::GroundLevelCentreline => (0.0, 0.0),
            Self::Offset { y, z } => (y.get::<meter>(), z.get::<meter>()),
        }
    }
}

/// The two factors of upstream's master equation; the dilution factor is
/// their product (times the frequency weighting of the release mode).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MasterEquationTerms {
    /// Pre-exponential factor, 1/m^3 per (m/s) (i.e. s/m^3 at unit speed).
    pub pre_expo: f64,
    /// Exponential factor (dimensionless), including the ground reflection.
    pub expo: f64,
}

/// Single (instantaneous / short-term) Gaussian plume, Hukkoo-Bapat eq. 2.5:
/// `1 / (2 pi sigma_y sigma_z u)` times the crosswind and reflected vertical
/// Gaussians. `speed_factor` is the wind speed in m/s (upstream: unit speed
/// times [`height_correction_factor`]).
#[must_use]
pub fn master_equation_single_plume(
    sigma_y: Length,
    sigma_z: Length,
    speed_factor: f64,
    release_height: Length,
    receptor: Receptor,
) -> MasterEquationTerms {
    let (y, z) = receptor.yz();
    let sy = sigma_y.get::<meter>();
    let sz = sigma_z.get::<meter>();
    let h = release_height.get::<meter>();
    let speed = 1.0 * speed_factor;
    let pre_expo = 1.0 / (2.0 * core::f64::consts::PI * sy * sz * speed);
    let expo = (-(y * y / (2.0 * (sy * sy)))).exp()
        * ((-((z - h) * (z - h) / (2.0 * (sz * sz)))).exp()
            + (-((z + h) * (z + h) / (2.0 * (sz * sz)))).exp());
    MasterEquationTerms { pre_expo, expo }
}

/// Upstream's sector width, 22.5 degrees "in radians", **as written**:
/// `0.39275`. The exact value is 0.392699...; the port keeps upstream's
/// constant (a relative difference of 1.3e-4).
pub const SECTOR_WIDTH_RAD: f64 = 0.39275;

/// Sector-averaged (long-term) Gaussian plume, Hukkoo-Bapat eq. 2.32:
/// `1 / (sqrt(2 pi) x theta sigma_z)` times the reflected vertical Gaussian,
/// with `theta` = [`SECTOR_WIDTH_RAD`]. Wind speed enters later, in
/// [`dilution_long_term_no_met`] and [`dilution_long_term_with_met`].
#[must_use]
pub fn master_equation_sector_averaged(
    x: Length,
    sigma_z: Length,
    release_height: Length,
    receptor: Receptor,
) -> MasterEquationTerms {
    let (_, z) = receptor.yz();
    let x1 = x.get::<meter>();
    let sz = sigma_z.get::<meter>();
    let h = release_height.get::<meter>();
    let pre_expo = 1.0 / ((2.0 * core::f64::consts::PI).sqrt() * x1 * SECTOR_WIDTH_RAD * sz);
    let expo = (-((z - h) * (z - h) / (2.0 * (sz * sz)))).exp()
        + (-((z + h) * (z + h) / (2.0 * (sz * sz)))).exp();
    MasterEquationTerms { pre_expo, expo }
}

/// The release and receptor geometry shared by every dilution calculation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlumeGeometry {
    /// Effective release height `H` (upstream does not add plume rise; see the
    /// scoping note).
    pub release_height: Length,
    /// Height at which the wind speed was measured, `H_m`.
    pub measurement_height: Length,
    /// Where the concentration is evaluated.
    pub receptor: Receptor,
}

/// Optional division of the per-stability dilution factors by a mean wind
/// speed per stability class (upstream's `like_to_scale_with_mean_speed` with
/// `ask_mean_speed_data`), for the two no-met-data modes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MeanSpeedScaling {
    /// No scaling: the dilution factors are for a 1 m/s wind at the
    /// measurement height (upstream default).
    UnitSpeed,
    /// Divide class `i`'s dilution factor by `speeds[i]` (A to F).
    PerClass([Velocity; 6]),
}

/// Frequency-weighted accumulation used by all three modes, in upstream's
/// operation order: `KQIJ = pre * expo * SUMNU; KQIJ = (KQIJ * 3600) / (hours * 3600)`.
fn kqij(terms: MasterEquationTerms, sumnu: f64, hours_denominator: f64) -> f64 {
    let k = terms.pre_expo * terms.expo * sumnu;
    (k * 3600.0) / hours_denominator
}

fn apply_scaling(values: [f64; 6], scaling: MeanSpeedScaling) -> [DilutionFactor; 6] {
    let mut out = [DilutionFactor::default(); 6];
    for (i, v) in values.iter().enumerate() {
        out[i] = DilutionFactor::new(match scaling {
            MeanSpeedScaling::UnitSpeed => *v,
            MeanSpeedScaling::PerClass(s) => *v / s[i].get::<meter_per_second>(),
        });
    }
    out
}

/// Dilution factor for an **instantaneous (single-plume) release without met
/// data**, one value per stability class A-F, s/m^3 (time-integrated
/// concentration per Bq released, at unit wind speed times the height
/// correction).
///
/// Upstream builds a synthetic joint-frequency table with one hour of calm-class
/// wind in the first sector for each class, so each class's value is simply
/// the single-plume master equation at that class.
#[must_use]
pub fn dilution_single_plume_no_met(
    x: Length,
    geometry: PlumeGeometry,
    scaling: MeanSpeedScaling,
) -> [DilutionFactor; 6] {
    let mut v = [0.0; 6];
    for s in StabilityClass::ALL {
        let factor =
            height_correction_factor(s, geometry.release_height, geometry.measurement_height);
        let terms = master_equation_single_plume(
            sigma_y(s, x),
            sigma_z(s, x),
            factor,
            geometry.release_height,
            geometry.receptor,
        );
        v[s.index()] = kqij(terms, 1.0, 60.0 * 60.0);
    }
    apply_scaling(v, scaling)
}

/// Dilution factor for a **long-term release without met data** ("conservative
/// assumptions"), one value per stability class A-F, s/m^3.
///
/// Sector-averaged master equation with the wind speed `1 m/s * factor` of
/// [`height_correction_factor`] and a frequency of one hour, per class.
#[must_use]
pub fn dilution_long_term_no_met(
    x: Length,
    geometry: PlumeGeometry,
    scaling: MeanSpeedScaling,
) -> [DilutionFactor; 6] {
    let mut v = [0.0; 6];
    for s in StabilityClass::ALL {
        let factor =
            height_correction_factor(s, geometry.release_height, geometry.measurement_height);
        let sumnu = 1.0 / (1.0 * factor);
        let terms = master_equation_sector_averaged(
            x,
            sigma_z(s, x),
            geometry.release_height,
            geometry.receptor,
        );
        v[s.index()] = kqij(terms, sumnu, 60.0 * 60.0);
    }
    apply_scaling(v, scaling)
}

/// **Divergence from upstream (D3 corrected):** the single-plume dilution
/// factor per class, divided by the class's mean wind speed from the met
/// record, converted from km/h to m/s.
///
/// Upstream's `dilution_per_sector` for a single plume **with** met data
/// divides the six per-class values by `mean_speeds[:, None]`, which
/// broadcasts to a 6 x 6 array and fails its own `shape == (6,)` assertion,
/// and the means it divides by are in km/h (the met column) while the
/// dilution factor is per 1 m/s. This does what the code evidently intends:
/// class `i` divided by `mean_i / 3.6` m/s, with the means of
/// [`super::met::speed_distribution`] (quantile 0.90) over all years. Not
/// verifiable against upstream, which cannot run this path.
#[must_use]
pub fn dilution_single_plume_with_met_speeds(
    x: Length,
    geometry: PlumeGeometry,
    met: &MetClimatology,
) -> [DilutionFactor; 6] {
    let (_, means_kmph) = super::met::speed_distribution(&met.all_records(), 0.90);
    let speeds = means_kmph.map(|v| Velocity::new::<meter_per_second>(v / 3.6));
    dilution_single_plume_no_met(x, geometry, MeanSpeedScaling::PerClass(speeds))
}

/// Which calm correction to apply in [`dilution_long_term_with_met`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalmCorrection {
    /// None (upstream `calm_correction: False`).
    Off,
    /// Upstream's factors exactly, including defect D1 (see
    /// [`super::met::calm_correction_factors`]).
    Upstream,
    /// **Divergence from upstream** — D1 corrected: `N_L` is the total count in
    /// the lowest non-calm speed class over all sectors and classes. See
    /// [`super::met::calm_correction_factors_lowest_speed_class`].
    LowestSpeedClassTotal,
}

/// Dilution factor for a **long-term (continuous) release with met data**, one
/// value per 22.5-degree sector (upstream's 16 met-direction sectors, index 0
/// centred on 0 degrees), averaged over the years of `met`, s/m^3.
///
/// For each year: `sum over classes i and speed classes k >= 1` of
/// `pre * expo * N_ik / (u_k * factor_i)`, divided by the non-calm hours
/// (`days * operating hours - calm hours`), then optionally multiplied by the
/// calm-correction factors. The years are then averaged.
#[must_use]
pub fn dilution_long_term_with_met(
    x: Length,
    geometry: PlumeGeometry,
    met: &MetClimatology,
    calm: CalmCorrection,
) -> [DilutionFactor; 16] {
    let wspeed_k: [f64; SPEED_CLASS_COUNT] = WSPEED_K_KMPH.map(|s| s / 3.6);
    let op_hours = met.operation_hours_per_day();
    let mut per_year: Vec<[f64; 16]> = Vec::with_capacity(met.years.len());
    for year in &met.years {
        let tjfd = &year.missing_corrected;
        let total_calm: i64 = tjfd
            .iter()
            .map(|t| t[CALM_SPEED_CLASS].iter().sum::<i64>())
            .sum();
        let hours_without_calm = year.num_days * op_hours - total_calm;
        let denom = (hours_without_calm * 60 * 60) as f64;
        let mut kbyq = [0.0; 16];
        for (direc, out) in kbyq.iter_mut().enumerate() {
            let mut sumkq_j = 0.0;
            for s in StabilityClass::ALL {
                let sz = sigma_z(s, x);
                let factor = height_correction_factor(
                    s,
                    geometry.release_height,
                    geometry.measurement_height,
                );
                let terms = master_equation_sector_averaged(
                    x,
                    sz,
                    geometry.release_height,
                    geometry.receptor,
                );
                let mut sumnu = 0.0;
                for k in 1..SPEED_CLASS_COUNT {
                    let speed_ik = wspeed_k[k] * factor;
                    sumnu += tjfd[s.index()][k][direc] as f64 / speed_ik;
                }
                sumkq_j += kqij(terms, sumnu, denom);
            }
            *out = sumkq_j;
        }
        let factors = match calm {
            CalmCorrection::Off => None,
            CalmCorrection::Upstream => Some(super::met::calm_correction_factors(tjfd)),
            CalmCorrection::LowestSpeedClassTotal => {
                Some(super::met::calm_correction_factors_lowest_speed_class(tjfd))
            }
        };
        if let Some(f) = factors {
            for (k, c) in kbyq.iter_mut().zip(f) {
                *k *= c;
            }
        }
        per_year.push(kbyq);
    }
    let n = per_year.len() as f64;
    let mut out = [DilutionFactor::default(); 16];
    for (j, o) in out.iter_mut().enumerate() {
        *o = DilutionFactor::new(numpy_pairwise_sum(per_year.iter().map(|y| y[j])) / n);
    }
    out
}

/// Sum in the order `numpy.add.reduce` uses for short contiguous arrays (a
/// plain left-to-right sum below 8 elements; numpy's pairwise scheme only
/// changes the order for longer arrays, which a years axis never reaches).
fn numpy_pairwise_sum(values: impl Iterator<Item = f64>) -> f64 {
    values.fold(0.0, |a, b| a + b)
}

/// The largest dilution factor in a set (per stability class or per sector):
/// what upstream's driver (`get_max_dilution_factor`) passes to every dose
/// pathway for a given distance.
///
/// Returns NaN for an empty slice. A NaN entry propagates as `numpy.max` does.
#[must_use]
pub fn max_dilution_factor(values: &[DilutionFactor]) -> DilutionFactor {
    let mut it = values.iter().map(|d| d.seconds_per_cubic_meter());
    let Some(first) = it.next() else {
        return DilutionFactor::new(f64::NAN);
    };
    DilutionFactor::new(it.fold(first, |m, v| {
        if m.is_nan() || v.is_nan() {
            f64::NAN
        } else {
            m.max(v)
        }
    }))
}

/// **Reproduces upstream defect D2**: what `MetFunc.max_dilution_factor` holds
/// after the two no-met-data modes, `dilution_factor_sectorwise.T[0].max()`.
///
/// On a one-dimensional array of six classes, `.T[0]` is the **first element**
/// (class A), so the "maximum" is class A's value, not the maximum. Upstream's
/// own driver does not use it (it recomputes the maximum over classes), but a
/// pathway function called without an explicit dilution factor does. Kept for
/// the code-to-code record; use [`max_dilution_factor`].
#[must_use]
pub fn upstream_internal_max_dilution_factor(per_class: &[DilutionFactor; 6]) -> DilutionFactor {
    per_class[0]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn d3_correction_divides_each_class_by_its_mean_speed_in_m_per_s() {
        use crate::pydoseia::met::{MetClimatology, RawMetRecord};
        let raw: Vec<RawMetRecord> = (0..240)
            .map(|i| RawMetRecord {
                hour: f64::from(i % 24),
                speed_kmph: Some(3.6 + f64::from(i % 7)),
                direction_deg: Some(f64::from(i * 13 % 360)),
                stability_code: Some(f64::from(i % 6 + 1)),
            })
            .collect();
        let met = MetClimatology::from_raw_years(&[(&raw, 10)], 0, 24);
        let geo = PlumeGeometry {
            release_height: Length::new::<meter>(30.0),
            measurement_height: Length::new::<meter>(10.0),
            receptor: Receptor::GroundLevelCentreline,
        };
        let x = Length::new::<meter>(400.0);
        let unit = dilution_single_plume_no_met(x, geo, MeanSpeedScaling::UnitSpeed);
        let with = dilution_single_plume_with_met_speeds(x, geo, &met);
        let (_, means) = crate::pydoseia::met::speed_distribution(&met.all_records(), 0.9);
        for i in 0..6 {
            let expect = unit[i].seconds_per_cubic_meter() / (means[i] / 3.6);
            let got = with[i].seconds_per_cubic_meter();
            assert!(((got - expect) / expect).abs() < 1e-15, "class {i}");
        }
    }

    #[test]
    fn stability_codes_round_trip() {
        for s in StabilityClass::ALL {
            assert_eq!(StabilityClass::from_code(s.code()), Some(s));
        }
        assert_eq!(StabilityClass::from_code(7), None);
    }

    /// Records how far upstream's sigma_z bands disagree where they meet. They
    /// are nearly continuous. The largest jump over all six classes and both
    /// edges is class E at 1000 m, -0.84 % (measured 2026-09-28). Not a physics
    /// requirement; a fact about the fits.
    #[test]
    fn sigma_z_band_edges_are_nearly_continuous() {
        let at = |s, x: f64| sigma_z(s, Length::new::<meter>(x)).get::<meter>();
        for s in StabilityClass::ALL {
            for edge in [100.0, 1000.0] {
                let (below, above) = (at(s, edge - 1e-9), at(s, edge + 1e-9));
                let jump = (above - below) / below;
                assert!(jump.abs() < 0.0085, "{s:?} at {edge} m: {jump:e}");
            }
        }
        let e_below = at(StabilityClass::E, 1000.0);
        let e_above = at(StabilityClass::E, 1000.0 + 1e-9);
        let jump = (e_above - e_below) / e_below;
        assert!((jump + 0.0084).abs() < 0.0002, "{jump:e}");
    }

    #[test]
    fn a_higher_release_lowers_the_centreline_ground_concentration_nearby() {
        let geo = |h: f64| PlumeGeometry {
            release_height: Length::new::<meter>(h),
            measurement_height: Length::new::<meter>(10.0),
            receptor: Receptor::GroundLevelCentreline,
        };
        let x = Length::new::<meter>(200.0);
        let low = dilution_long_term_no_met(x, geo(10.0), MeanSpeedScaling::UnitSpeed);
        let high = dilution_long_term_no_met(x, geo(80.0), MeanSpeedScaling::UnitSpeed);
        for s in StabilityClass::ALL {
            assert!(high[s.index()] < low[s.index()], "{s:?}");
        }
    }
}
