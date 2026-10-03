//! # AP1000 severe-accident plume-centreline TED (Dadda et al. 2024, Fig. 7)
//!
//! A **published LWR dose curve**, loaded as a comparison overlay for
//! `htgr_sim_v1`'s centreline TEDE graph (maintainer request, 2026-10-01,
//! #473). Nothing here is computed by this crate's own source-term chain: it
//! is the maintainer's digitisation of a figure, summed and scaled.
//!
//! **Source.** A. Dadda et al., "Source term analysis and impact study during
//! a hypothetical accident in Haiyang nuclear power plant", *Radiation Physics
//! and Chemistry* 218 (2024) 111542, Fig. 7, the April–August period
//! (Pasquill class B, 4.44 m/s). Data and full provenance (digitisation date,
//! log-log axis calibration, licence basis) in the header of
//! `reference/lwr/dadda2024_fig7_ap1000_centreline_ted_apr_aug.csv`, loaded
//! with `include_str!` (no runtime file I/O; wasm/Android safe).
//!
//! **Their model, not ours.** HotSpot 3.1; AP1000 at 3400 MWt; RG 1.183
//! group release fractions (an unmitigated core melt, no containment leak
//! rate stated); 100 m stack; 2 h release interval; TED = inhalation CEDE +
//! submersion + ground shine + **resuspension** (their Eq. 7). Every one of
//! those differs from `htgr_sim_v1`'s accumulated dose ([`MISMATCHES`]).
//!
//! **Summing the groups.** The eight groups were digitised at different x
//! points, so [`total_ted`] interpolates each group **linearly in
//! `(log x, log TED)`** (the axes it was read off) and sums. Outside a
//! group's own digitised x range that group contributes **0 and is listed in
//! [`TotalTed::groups_outside`]** -- nothing is extrapolated. (Ce, Ba/Sr and
//! the alkali metals start at ~0.2 km, the others at ~0.1 km.)
//!
//! **Power scaling** ([`total_ted_scaled`]): `x P / 3400 MWt`, the #450
//! convention (dose ∝ released activity ∝ thermal power) that
//! [`crate::lwr_comparison::pwr_inventory_scaled`] applies to the NuScale
//! inventory. That helper scales an inventory table, not a dose, so the same
//! linear ratio is applied here directly ([`scale_to_power`]).
//!
//! Distances are km and doses Sv as plain `f64`, following the sibling
//! [`crate::lwr_comparison`] (`x_m`, `total_sv`); the suffix carries the unit.
//!
//! > **Research, education and V&V only** (`RESPONSIBLE_USE.md`). Not a dose
//! > assessment for AP1000, Haiyang, HTR-10 or any plant.
//!
//! ## V&V: digitisation gate against Table 3
//!
//! **Methodology.** Table 3 of the paper prints each group's maximum TED, all
//! at 0.6 km: noble gases 1.5, halogens 27, alkali metals 4.1, Te 0.443,
//! Ba/Sr 0.393, noble metals 0.072, Ce 2.2, La 9.51e-3 Sv (sum 35.7 Sv). The
//! gate compares each group's **largest digitised point** with its Table 3
//! value, and the sum of those eight peaks with 35.7 Sv, as
//! `|log10(digitised / table)|`.
//!
//! **Tolerance, derived before the comparison was run:** one pixel of the
//! digitiser's y calibration. The y axis spans 1e-16 .. 1e3 Sv (19 decades)
//! over 233.489 - 27.670 = 205.819 px, i.e. **0.09231 decades/px** (one px =
//! a factor 1.237). A hand-placed marker on a curve cannot be placed more
//! finely than the pixel it lands on, so a reading off this axis is uncertain
//! by about ±1 px; a deviation larger than that is a placement error rather
//! than quantisation. The total's log deviation is bounded by the largest
//! per-group one (a weighted mean of ratios), so the same tolerance applies.
//! [`GATE_TOLERANCE_DECADES`].
//!
//! **Results (2026-10-01, digitisation of 2026-10-01T01:53:51Z):**
//!
//! | Group | Digitised peak \[Sv\] at \[km\] | Table 3 \[Sv\] | Deviation | In px |
//! |---|---|---|---|---|
//! | noble gases | 1.547 at 0.626 | 1.5 | +3.1 % | +0.14 |
//! | halogens | 31.08 at 0.592 | 27 | +15.1 % | +0.66 |
//! | alkali metals | 3.741 at 0.496 | 4.1 | −8.8 % | −0.43 |
//! | Te | 0.4344 at 0.593 | 0.443 | −1.9 % | −0.09 |
//! | Ba/Sr | 0.3721 at 0.597 | 0.393 | −5.3 % | −0.26 |
//! | noble metals | 0.08086 at 0.510 | 0.072 | +12.3 % | +0.55 |
//! | Ce | 2.107 at 0.598 | 2.2 | −4.2 % | −0.20 |
//! | La | 0.01073 at 0.575 | 9.51e-3 | +12.9 % | +0.57 |
//! | **sum of peaks** | **39.38** | **35.7** | **+10.3 %** | **+0.46** |
//!
//! **All eight groups and the total pass the ±1 px (±0.0923 decade, ×/÷1.237)
//! gate**; the worst is halogens at 0.66 px. **Interpretation:** the
//! digitisation reproduces the published peaks to within the axis's own
//! resolution, and no better -- the overlay carries a ~10–15 % reading
//! uncertainty near the peak, larger than any curve-shape detail it could be
//! used to argue from. The halogens (76 % of the total) dominate both the
//! total and its +10 % excess. The peak of the *summed, interpolated* curve
//! (≈ 39.1 Sv near 0.59 km, see the test) is slightly below the sum of
//! peaks, because the groups peak at different digitised x.
//!
//! Pinned by `tests::digitised_peaks_match_table_3_within_one_pixel`.

/// The digitised Fig. 7 series, verbatim (see the CSV header).
const FIG7_CSV: &str =
    include_str!("../reference/lwr/dadda2024_fig7_ap1000_centreline_ted_apr_aug.csv");

/// AP1000 thermal power the paper models \[MWt\].
pub const AP1000_MWTH: f64 = 3400.0;

/// The citation, for labels.
pub const CITATION: &str = "A. Dadda et al., Radiation Physics and Chemistry 218 (2024) 111542, \
     Fig. 7, April-August (class B, 4.44 m/s); maintainer's log-log digitisation, 2026-10-01";

/// What the overlay is NOT, against `htgr_sim_v1`'s accumulated dose. Shown
/// wherever the curve is drawn.
pub const MISMATCHES: &str = "AP1000 BDB: unmitigated core melt, RG 1.183 releases, no \
     containment credit stated | 2 h release interval (vs the NRC 96 h reference) | 100 m stack \
     (vs the HTR-10 ground release) | TED includes resuspension (the HTR-10 side does not) | \
     stability class B at 4.44 m/s (vs the simulator's own met) | a different code (HotSpot 3.1) \
     | compare against the HTR-10 beyond-design-basis core burn (DLOFC + air ingress, KORA) | \
     research/education only";

/// Distance of Table 3's maxima \[km\].
pub const TABLE_3_DISTANCE_KM: f64 = 0.6;

/// Table 3: maximum TED per group \[Sv\], at [`TABLE_3_DISTANCE_KM`].
pub const TABLE_3_MAX_TED_SV: [(&str, f64); 8] = [
    ("noble_gases", 1.5),
    ("halogens", 27.0),
    ("alkali_metals", 4.1),
    ("tellurium", 0.443),
    ("barium_strontium", 0.393),
    ("noble_metals", 0.072),
    ("cerium", 2.2),
    ("lanthanides", 9.51e-3),
];

/// Table 3's sum \[Sv\], as printed.
pub const TABLE_3_TOTAL_SV: f64 = 35.7;

/// The digitisation gate's tolerance \[decades\]: one y pixel of the
/// calibration, `19 / (233.48883 - 27.67020)` (module docs: derived before
/// the comparison was run).
pub const GATE_TOLERANCE_DECADES: f64 = 19.0 / (233.488_830_566_406_25 - 27.670_200_347_900_39);

/// One group's digitised curve.
#[derive(Debug, Clone, PartialEq)]
pub struct GroupCurve {
    /// NUREG-1465 group name, as in the CSV.
    pub group: String,
    /// `(distance_km, ted_sv)`, in digitised (increasing-x) order.
    pub points: Vec<(f64, f64)>,
}

impl GroupCurve {
    /// The group's TED \[Sv\] at `x_km`, interpolated linearly in
    /// `(log x, log TED)`; `None` outside the digitised x range (no
    /// extrapolation).
    pub fn at(&self, x_km: f64) -> Option<f64> {
        let p = &self.points;
        let (first, last) = (p.first()?, p.last()?);
        if !(x_km >= first.0 && x_km <= last.0) {
            return None;
        }
        let i = p.windows(2).position(|w| x_km <= w[1].0)?;
        let ((x0, y0), (x1, y1)) = (p[i], p[i + 1]);
        let t = (x_km.ln() - x0.ln()) / (x1.ln() - x0.ln());
        Some((y0.ln() + t * (y1.ln() - y0.ln())).exp())
    }

    /// The largest digitised point `(distance_km, ted_sv)`.
    pub fn peak(&self) -> (f64, f64) {
        self.points
            .iter()
            .copied()
            .fold(
                (f64::NAN, f64::NEG_INFINITY),
                |a, b| if b.1 > a.1 { b } else { a },
            )
    }
}

/// The eight digitised group curves, in CSV order.
pub fn group_curves() -> Vec<GroupCurve> {
    let mut out: Vec<GroupCurve> = Vec::new();
    for line in FIG7_CSV
        .lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .skip(1)
    {
        let f: Vec<&str> = line.split(',').map(str::trim).collect();
        let pt = (f[1].parse::<f64>().unwrap(), f[2].parse::<f64>().unwrap());
        match out.iter_mut().find(|c| c.group == f[0]) {
            Some(c) => c.points.push(pt),
            None => out.push(GroupCurve {
                group: f[0].to_string(),
                points: vec![pt],
            }),
        }
    }
    out
}

/// The summed TED at one distance.
#[derive(Debug, Clone, PartialEq)]
pub struct TotalTed {
    /// Downwind distance \[km\].
    pub distance_km: f64,
    /// Sum over the groups whose digitised range covers `distance_km` \[Sv\].
    pub total_sv: f64,
    /// Groups outside their digitised range here, counted as **0** (not
    /// extrapolated).
    pub groups_outside: Vec<String>,
}

/// The x range any group covers \[km\]: (smallest first point, largest last
/// point).
pub fn digitised_range_km() -> (f64, f64) {
    group_curves()
        .iter()
        .fold((f64::INFINITY, 0.0), |(lo, hi), c| {
            (
                lo.min(c.points.first().map_or(f64::INFINITY, |p| p.0)),
                hi.max(c.points.last().map_or(0.0, |p| p.0)),
            )
        })
}

/// The eight groups summed at each of `x_km` (the common grid), as published
/// (3400 MWt). See the module docs for the interpolation and the zero
/// outside each group's range.
pub fn total_ted(x_km: &[f64]) -> Vec<TotalTed> {
    let curves = group_curves();
    x_km.iter()
        .map(|&x| {
            let mut total_sv = 0.0;
            let mut groups_outside = Vec::new();
            for c in &curves {
                match c.at(x) {
                    Some(v) => total_sv += v,
                    None => groups_outside.push(c.group.clone()),
                }
            }
            TotalTed {
                distance_km: x,
                total_sv,
                groups_outside,
            }
        })
        .collect()
}

/// A dose \[Sv\] published for 3400 MWt, scaled to `thermal_power_mwth`
/// (#450: ∝ power).
pub fn scale_to_power(sv_at_3400_mwth: f64, thermal_power_mwth: f64) -> f64 {
    sv_at_3400_mwth * thermal_power_mwth / AP1000_MWTH
}

/// [`total_ted`] scaled to `thermal_power_mwth` by [`scale_to_power`]
/// (10 MWt for HTR-10).
pub fn total_ted_scaled(x_km: &[f64], thermal_power_mwth: f64) -> Vec<TotalTed> {
    total_ted(x_km)
        .into_iter()
        .map(|mut t| {
            t.total_sv = scale_to_power(t.total_sv, thermal_power_mwth);
            t
        })
        .collect()
}

/// A log-spaced grid of `n >= 2` points over `[lo_km, hi_km]`.
pub fn log_grid_km(lo_km: f64, hi_km: f64, n: usize) -> Vec<f64> {
    let (a, b) = (lo_km.ln(), hi_km.ln());
    (0..n)
        .map(|k| (a + (b - a) * k as f64 / (n - 1) as f64).exp())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eight_groups_loaded_verbatim() {
        let c = group_curves();
        assert_eq!(c.len(), 8);
        assert_eq!(c.iter().map(|g| g.points.len()).sum::<usize>(), 137);
        let hal = c.iter().find(|g| g.group == "halogens").unwrap();
        assert_eq!(hal.points[5], (0.5920487970295696, 31.083233879280055));
        for g in &c {
            assert!(TABLE_3_MAX_TED_SV.iter().any(|(n, _)| *n == g.group));
            assert!(g.points.windows(2).all(|w| w[1].0 > w[0].0), "{}", g.group);
        }
    }

    /// The V&V gate of the module docs: tolerance one y pixel, fixed before
    /// the first run; the measured deviations are recorded there.
    #[test]
    fn digitised_peaks_match_table_3_within_one_pixel() {
        assert!((GATE_TOLERANCE_DECADES - 0.092_314).abs() < 1e-6);
        let c = group_curves();
        let mut sum = 0.0;
        for (name, table) in TABLE_3_MAX_TED_SV {
            let (x, v) = c.iter().find(|g| g.group == name).unwrap().peak();
            sum += v;
            let dev = (v / table).log10();
            assert!(
                dev.abs() <= GATE_TOLERANCE_DECADES,
                "{name}: {v} Sv at {x} km vs {table}: {dev} decades"
            );
            assert!(
                (x - TABLE_3_DISTANCE_KM).abs() < 0.15,
                "{name} peak at {x} km"
            );
        }
        let tsum: f64 = TABLE_3_MAX_TED_SV.iter().map(|(_, v)| v).sum();
        assert!((tsum - TABLE_3_TOTAL_SV).abs() < 0.05);
        assert!((sum / TABLE_3_TOTAL_SV).log10().abs() <= GATE_TOLERANCE_DECADES);
        assert!((sum - 39.376).abs() < 1e-3, "{sum}");
    }

    /// Interpolation hits the digitised points exactly, is zero (and flagged)
    /// outside a group's range, and the summed peak sits near 0.6 km.
    #[test]
    fn total_interpolates_without_extrapolating() {
        let c = group_curves();
        let ce = c.iter().find(|g| g.group == "cerium").unwrap();
        let (x0, y0) = ce.points[3];
        assert!((ce.at(x0).unwrap() / y0 - 1.0).abs() < 1e-12);
        assert_eq!(ce.at(0.15), None);
        let t = &total_ted(&[0.15])[0];
        assert!(t.groups_outside.contains(&"cerium".to_string()));
        assert!(t.groups_outside.contains(&"alkali_metals".to_string()));
        assert!(t.groups_outside.contains(&"barium_strontium".to_string()));
        assert_eq!(t.groups_outside.len(), 3);
        let far = &total_ted(&[500.0])[0];
        assert_eq!(far.groups_outside.len(), 8);
        assert_eq!(far.total_sv, 0.0);

        let (lo, hi) = digitised_range_km();
        let grid = log_grid_km(lo, hi, 2000);
        let tot = total_ted(&grid);
        let pk = tot
            .iter()
            .max_by(|a, b| a.total_sv.total_cmp(&b.total_sv))
            .unwrap();
        assert!((pk.distance_km - 0.59).abs() < 0.03, "{}", pk.distance_km);
        assert!(
            pk.total_sv > 37.0 && pk.total_sv < 39.376,
            "{}",
            pk.total_sv
        );
        let s = &total_ted_scaled(&[pk.distance_km], 10.0)[0];
        assert!((s.total_sv - pk.total_sv * 10.0 / 3400.0).abs() < 1e-15);
    }
}
