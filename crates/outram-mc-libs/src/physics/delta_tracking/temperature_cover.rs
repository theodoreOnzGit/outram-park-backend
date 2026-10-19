//! **A majorant that covers a temperature range** (gh #945, stage 3):
//! `Σ_maj(E) ≥ Σ_t(E, T)` for every `T` in `[T_min, T_max]` and every `E`.
//!
//! Maintainer direction (gh #945, 2026-10-17): "chebyshev polynomial to cover
//! logE and neutron T. Don't have to fit exactly but have to cover
//! everything", refined to "a few regions", with the region count and degree
//! a cost-versus-cost choice exposed only through `advanced` (#942). It is a
//! **cover, not a fit**: looseness costs virtual collisions, an under-bound
//! anywhere is a defect.
//!
//! # The construction, in two steps
//!
//! **Step 1, the T axis: [`Majorant::over_temperature_tables`].** Process the
//! nuclides at tables spanning the range (`T_1 = T_min < ... < T_n = T_max`),
//! build a [`Majorant::bounding`] for each (a bound by construction on
//! pointwise and S(α,β) data at that table's temperature), and take their
//! **pointwise maximum on the union of all their grids**, times
//! `1 + t_margin`. Doppler broadening lowers resonance peaks and raises their
//! wings, so no end of the range is enough; the maximum over the tables is
//! the bound the issue asks for ("the largest Σ_t(E, T) over T at each E").
//!
//! *What is guaranteed and what is not.* At each table temperature the
//! bound holds by construction. **Between** tables it does not follow from
//! the construction: at a fixed `E` near a resonance's half-width crossover
//! `Σ_t(T)` is not monotone in `T` (the peak falls while the wing rises), so
//! its maximum over a temperature interval can exceed both bracketing
//! tables. That excess is bounded by `t_margin`, which is **measured, not
//! proven**: `Majorant::audit` against data processed exactly at
//! temperatures *between* the tables (`tests/majorant_temperature_cover.rs`,
//! `verification_and_validation/region_temperature_945/README.md`, section
//! of 2026-10-19). The excess shrinks as the tables get closer; the default
//! margin is set from that measurement, with a safety factor.
//!
//! **Step 2, the energy axis: [`Majorant::chebyshev_cover`].** The step-1
//! table is a node list of up to a few hundred thousand points. The cover
//! replaces it by a handful of bands, each `exp(P_b(ln E) + s_b)` with `P_b`
//! a Chebyshev polynomial of `x = ln E` scaled to `[-1, 1]` and `s_b` a shift.
//! `P_b` is the interpolant, at the Chebyshev nodes, of the band's upper
//! envelope of `ln Σ_maj`. **That is a fit, and a fit can dip below.** The
//! shift `s_b` is what makes it a bound, and it is **proven, not sampled**:
//!
//! 1. The table's value on node interval `[E_i, E_{i+1}]` is the larger of its
//!    two end nodes (what [`Majorant::at`] returns), a constant `y_i`.
//! 2. `|T_j'(x)| <= j^2` on `[-1, 1]`, so `|P'| <= L = Σ j^2 |c_j|`. On an
//!    interval of width `w` (in `x`), `min P >= min(P at its ends) - L w / 2`.
//!    Intervals wider than `W_MAX` are subdivided first, so the term stays
//!    small.
//! 3. `s_b = max_i ( y_i - (min P on interval i, as bounded in 2) )`, plus
//!    1e-12 for round-off. Then `P_b + s_b >= y_i` on every interval, hence
//!    the cover is `>=` the step-1 table everywhere, hence `>=` the bound it
//!    covers. Below the first node the 1/v extrapolation of [`Majorant::at`]
//!    is kept; above the last node the last band's end value holds.
//!
//! This is a guarantee against the step-1 table, **not** an independent
//! guarantee against the data: the data-level guarantee is step 1's.
//!
//! # Cost, which is the point of the knobs
//!
//! Resonances are thousands of times above the background in `Σ_t`. A band
//! that contains a resolved resonance must sit above its peak everywhere, so
//! the cover is **very loose** across the resolved range: that is the price
//! of replacing a few hundred thousand nodes by a few coefficients. More
//! bands cost build time and `at` time and buy tightness; the measured
//! trade-off is in the V&V record. The band edges are equal in `ln E`,
//! snapped to table nodes; the data-driven edges the maintainer named
//! (S(α,β) cutoff, resolved/unresolved limits) are a refinement the
//! equal-log default does not use.

use super::majorant::Majorant;
use crate::material::material::Material;
use crate::material::nuclide::Nuclide;
use crate::mathf::RealMath;

/// **The default extra margin** of [`Majorant::over_temperature_tables`]:
/// 1 %. The worst between-table excess measured with no margin at all was
/// 0.127 % (graphite + UO2, two tables 1207 K apart, at 1.69 MeV, where the
/// data are not Doppler-broadened and the difference is the processing
/// tolerance of separate RECONR/BROADR runs, 1e-3), 0.049 % with three
/// tables and 0.023 % with five; U-238 alone showed none to 1e-7. So 1 % is
/// 8x the worst measured excess. It is a floor for cases not measured
/// (water, other nuclides), not a derived bound.
pub const DEFAULT_T_MARGIN: f64 = 0.01;

/// Widest sub-interval (in the band's scaled `x`, whose range is 2) over
/// which the Lipschitz step of the proof is taken.
const W_MAX: f64 = 2.0e-3;

/// **The cover's two knobs**, set through `advanced` only (gh #945, #942).
///
/// `regions` is the number of energy bands (each its own polynomial in
/// `ln E`); `degree` is each polynomial's degree. Accuracy does not change
/// with either as long as the cover holds (it is a bound, not a fit); only
/// the build cost and the virtual-collision cost do.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CoverSettings {
    /// Number of energy bands, at least 1.
    pub regions: usize,
    /// Polynomial degree in each band, at least 0.
    pub degree: usize,
}

impl Default for CoverSettings {
    /// **256 bands of degree 4**, from the sweep of 2026-10-19
    /// (`verification_and_validation/region_temperature_945/README.md`, the
    /// regions x degree tables, 5-table bound, mean `Σ_maj/Σ_t` flat in
    /// lethargy over 1e-3 eV..20 MeV at 1050 K): U-238 metal 12.8 against the
    /// table's 4.3 (3.0x), graphite + UO2 1.8 against 1.5 (1.2x); 1280
    /// coefficients in place of ~3e5 nodes; fit 0.02-0.03 s. 64 bands cost 10x
    /// (U-238) and 1.9x (mix) the table; 1024 bands buy 1.6x and 1.1x for four
    /// times the coefficients; degree 8 buys under 20 % over degree 4 at 256
    /// bands. Past ~256 bands the cover is a table with extra steps.
    fn default() -> Self {
        CoverSettings {
            regions: 256,
            degree: 4,
        }
    }
}

/// One band of a Chebyshev cover: `exp(P(x(E)) + shift)` on `[e_lo, e_hi]`.
#[derive(Debug, Clone)]
pub struct CoverBand {
    e_lo: f64,
    ln_lo: f64,
    ln_hi: f64,
    /// Chebyshev coefficients `c_j` of `P = Σ c_j T_j(x)`.
    coeffs: Vec<f64>,
    /// Added to `P` before exponentiating: the proven lift.
    shift: f64,
}

impl CoverBand {
    pub(super) fn scaled(&self, factor: f64) -> CoverBand {
        let mut b = self.clone();
        b.shift += factor.max(f64::MIN_POSITIVE).r_ln();
        b
    }

    /// `P(x)` by Clenshaw's recurrence.
    fn poly(&self, x: f64) -> f64 {
        let (mut b1, mut b2) = (0.0_f64, 0.0_f64);
        for &c in self.coeffs.iter().skip(1).rev() {
            let t = 2.0 * x * b1 - b2 + c;
            b2 = b1;
            b1 = t;
        }
        x * b1 - b2 + self.coeffs[0]
    }

    fn x_of(&self, e: f64) -> f64 {
        let span = self.ln_hi - self.ln_lo;
        if span <= 0.0 {
            return 0.0;
        }
        (2.0 * (e.r_ln() - self.ln_lo) / span - 1.0).clamp(-1.0, 1.0)
    }

    fn value(&self, e: f64) -> f64 {
        (self.poly(self.x_of(e)) + self.shift).r_exp()
    }

    /// `∫_{-1}^{1} (P + s) dx`, in closed form (`∫ T_j = 2/(1-j²)` for even
    /// `j`, 0 for odd): the band's total `ln` cover, to compare candidates.
    fn mean_ln(&self) -> f64 {
        let mut acc = 2.0 * (self.coeffs[0] + self.shift);
        for (j, c) in self.coeffs.iter().enumerate().skip(2).step_by(2) {
            acc += 2.0 * c / (1.0 - (j * j) as f64);
        }
        acc
    }

    /// `L = Σ j² |c_j|`, a bound on `|P'|` over `[-1, 1]`.
    fn lipschitz(&self) -> f64 {
        self.coeffs
            .iter()
            .enumerate()
            .map(|(j, c)| (j * j) as f64 * c.abs())
            .sum()
    }
}

/// The cover's value at `e` \[eV\] over `bands` (non-empty, ascending).
pub(super) fn cover_at(bands: &[CoverBand], e: f64) -> f64 {
    let first = &bands[0];
    if e < first.e_lo {
        let v = first.value(first.e_lo);
        return if e > 0.0 {
            v * (first.e_lo / e).r_powf(0.5)
        } else {
            v
        };
    }
    let i = bands.partition_point(|b| b.e_lo <= e);
    bands[i.saturating_sub(1)].value(e)
}

/// The result of [`Majorant::audit_over_temperatures`].
#[derive(Debug, Clone, Copy)]
pub struct TemperatureAudit {
    /// Worst `Σ_t / Σ_maj` over every set and energy. Above 1 is an
    /// under-bound.
    pub worst_ratio: f64,
    /// Energy \[eV\] of that worst ratio.
    pub energy_ev: f64,
    /// Temperature \[K\] (`materials[0].temperature` of the set) it was found at.
    pub temperature_k: f64,
    /// Energies checked in total, summed over the sets.
    pub energies_checked: usize,
}

impl Majorant {
    /// **The bound over a set of temperature tables**: the pointwise maximum
    /// of `tables` on the union of their grids, times `1 + t_margin`. Each
    /// table is a majorant built (by [`Self::bounding`]) over the same
    /// region's materials with nuclides processed at one temperature of the
    /// range. See the module docs for what this does and does not guarantee
    /// between tables.
    ///
    /// Each table is read through [`Self::at`], which takes the larger of
    /// the two bracketing nodes, so a union node reads each table's bound on
    /// the interval to its right; the union's own `at` then does the same.
    /// Empty `tables` gives an empty majorant.
    pub fn over_temperature_tables(tables: &[Majorant], t_margin: f64) -> Majorant {
        let mut grid: Vec<f64> = tables
            .iter()
            .flat_map(|t| t.energy.iter().copied())
            .collect();
        grid.retain(|e| e.is_finite());
        grid.sort_by(|a, b| a.partial_cmp(b).expect("finite energies"));
        grid.dedup();
        let scale = 1.0 + t_margin.max(0.0);
        let sigma = grid
            .iter()
            .map(|&e| tables.iter().map(|t| t.at(e)).fold(0.0_f64, f64::max) * scale)
            .collect();
        Majorant {
            energy: grid,
            sigma,
            cover: Vec::new(),
        }
    }

    /// **A Chebyshev cover of this majorant**: a few bands, each a
    /// polynomial in `ln E`, lifted so it is `>=` this majorant everywhere
    /// (proof in the module docs). Looser than the table, far cheaper to
    /// hold. A majorant with fewer than two nodes, or all zero, is returned
    /// unchanged.
    pub fn chebyshev_cover(&self, settings: CoverSettings) -> Majorant {
        let n = self.energy.len();
        let positive = self.sigma.iter().any(|&s| s > 0.0);
        if n < 2 || !positive || !self.cover.is_empty() || self.energy[0] <= 0.0 {
            return self.clone();
        }
        let regions = settings.regions.clamp(1, n - 1);
        // Band edges equal in ln E, snapped to nodes; drop duplicates.
        let (a, b) = (self.energy[0].r_ln(), self.energy[n - 1].r_ln());
        let mut cuts: Vec<usize> = (0..=regions)
            .map(|k| {
                let target = (a + (b - a) * k as f64 / regions as f64).r_exp();
                self.energy.partition_point(|&g| g < target).min(n - 1)
            })
            .collect();
        cuts[0] = 0;
        cuts[regions] = n - 1;
        cuts.dedup();
        let bands = cuts
            .windows(2)
            .map(|w| self.fit_band(w[0], w[1], settings.degree))
            .collect();
        Majorant {
            energy: self.energy.clone(),
            sigma: self.sigma.clone(),
            cover: bands,
        }
    }

    /// The band over node intervals `lo_i..hi_i` (nodes `lo_i ..= hi_i`):
    /// of the lifted fits of degree `0..=degree`, the one with the smallest
    /// mean `ln` cover (`∫ (P + s) dx` over the band), so a higher `degree`
    /// is never looser. Every candidate carries its own proven lift.
    fn fit_band(&self, lo_i: usize, hi_i: usize, degree: usize) -> CoverBand {
        (0..=degree)
            .map(|d| self.fit_band_degree(lo_i, hi_i, d))
            .map(|b| (b.mean_ln(), b))
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, b)| b)
            .expect("at least degree 0")
    }

    /// One candidate: the degree-`degree` envelope interpolant, lifted.
    fn fit_band_degree(&self, lo_i: usize, hi_i: usize, degree: usize) -> CoverBand {
        let floor = 1.0e-30_f64;
        let (e_lo, e_hi) = (self.energy[lo_i], self.energy[hi_i]);
        let (ln_lo, ln_hi) = (e_lo.r_ln(), e_hi.r_ln());
        let span = ln_hi - ln_lo;
        let xs: Vec<f64> = (lo_i..=hi_i)
            .map(|i| 2.0 * (self.energy[i].r_ln() - ln_lo) / span - 1.0)
            .collect();
        // y_k: ln of the step value on interval lo_i + k.
        let ys: Vec<f64> = (lo_i..hi_i)
            .map(|i| self.sigma[i].max(self.sigma[i + 1]).max(floor).r_ln())
            .collect();
        let nn = degree + 1;
        // Upper-envelope samples at the Chebyshev nodes: the max of y over
        // the node's angular neighbourhood.
        let mut yk = vec![f64::NEG_INFINITY; nn];
        for (k, v) in yk.iter_mut().enumerate() {
            let th = |t: f64| (std::f64::consts::PI * t / nn as f64).cos();
            let (x_hi, x_lo) = (th(k as f64), th(k as f64 + 1.0));
            // intervals overlapping [x_lo, x_hi]
            let first = xs.partition_point(|&x| x <= x_lo).saturating_sub(1);
            let last = xs.partition_point(|&x| x < x_hi).min(ys.len());
            for y in &ys[first.min(ys.len())..last.max(first + 1).min(ys.len())] {
                *v = v.max(*y);
            }
            if !v.is_finite() {
                *v = ys.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            }
        }
        let mut coeffs = vec![0.0; nn];
        for (j, c) in coeffs.iter_mut().enumerate() {
            let s: f64 = (0..nn)
                .map(|k| {
                    let ang = std::f64::consts::PI * (k as f64 + 0.5) / nn as f64;
                    yk[k] * (j as f64 * ang).cos()
                })
                .sum();
            *c = 2.0 * s / nn as f64;
        }
        coeffs[0] *= 0.5;
        let mut band = CoverBand {
            e_lo,
            ln_lo,
            ln_hi,
            coeffs,
            shift: 0.0,
        };
        // The proven lift.
        let l = band.lipschitz();
        let mut shift = f64::NEG_INFINITY;
        let mut p_left = band.poly(xs[0]);
        for (k, &y) in ys.iter().enumerate() {
            let (x0, x1) = (xs[k], xs[k + 1]);
            let p_right = band.poly(x1);
            let pieces = (((x1 - x0) / W_MAX).ceil() as usize).max(1);
            let mut min_p = p_left.min(p_right);
            let mut prev = p_left;
            for q in 1..pieces {
                let pq = band.poly(x0 + (x1 - x0) * q as f64 / pieces as f64);
                min_p = min_p.min(pq).min(prev);
                prev = pq;
            }
            let lower = min_p - l * (x1 - x0) / pieces as f64 / 2.0;
            shift = shift.max(y - lower);
            p_left = p_right;
        }
        band.shift = shift + 1.0e-12;
        band
    }

    /// Number of bands in a Chebyshev cover (0 for a node table).
    pub fn cover_bands(&self) -> usize {
        self.cover.len()
    }

    /// **Audit this majorant across temperatures**: [`Self::audit`] against
    /// each `(materials, nuclides)` set (each set the same region's
    /// materials with nuclides processed at one temperature, e.g. at
    /// temperatures *between* the tables the bound was built from), and the
    /// worst `Σ_t / Σ_maj` over all of them.
    pub fn audit_over_temperatures(
        &self,
        sets: &[(&[Material], &[Nuclide])],
        e_lo: f64,
        e_hi: f64,
        n_log: usize,
    ) -> TemperatureAudit {
        let mut out = TemperatureAudit {
            worst_ratio: 0.0,
            energy_ev: f64::NAN,
            temperature_k: f64::NAN,
            energies_checked: 0,
        };
        for (mats, nucs) in sets {
            let a = self.audit(mats, nucs, e_lo, e_hi, n_log);
            out.energies_checked += a.energies_checked;
            if a.worst_ratio > out.worst_ratio {
                out.worst_ratio = a.worst_ratio;
                out.energy_ev = a.energy_ev;
                out.temperature_k = mats.first().map_or(f64::NAN, |m| m.temperature);
            }
        }
        out
    }

    /// **Virtual-collision overhead proxy**: `Σ_maj / Σ_t` averaged over
    /// `energies` with `weights` (e.g. a collision density), the largest
    /// `Σ_t` over `materials` at each energy. The expected virtual
    /// collisions per real one is this minus 1. Energies with `Σ_t = 0` are
    /// skipped.
    pub fn mean_overhead(
        &self,
        materials: &[Material],
        nuclides: &[Nuclide],
        energies: &[f64],
        weights: &[f64],
    ) -> f64 {
        let (mut num, mut den) = (0.0, 0.0);
        for (&e, &w) in energies.iter().zip(weights) {
            let st = materials
                .iter()
                .map(|m| m.macro_xs_total_upper_bound(e, nuclides))
                .fold(0.0_f64, f64::max);
            if st > 0.0 {
                num += w * self.at(e) / st;
                den += w;
            }
        }
        if den > 0.0 {
            num / den
        } else {
            f64::NAN
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table(points: &[(f64, f64)]) -> Majorant {
        Majorant {
            energy: points.iter().map(|p| p.0).collect(),
            sigma: points.iter().map(|p| p.1).collect(),
            cover: Vec::new(),
        }
    }

    /// A spiky synthetic table: 1/v background with narrow peaks.
    fn spiky() -> Majorant {
        let mut pts = Vec::new();
        let mut e = 1.0e-3_f64;
        while e < 1.0e7 {
            let bg = 2.0 / e.sqrt().max(1e-2);
            let peak = if (e / 6.67).ln().abs() < 0.01 {
                800.0
            } else {
                0.0
            };
            let peak2 = if (e / 3.0e3).ln().abs() < 0.003 {
                4000.0
            } else {
                0.0
            };
            pts.push((e, bg + peak + peak2));
            e *= 1.0007;
        }
        table(&pts)
    }

    /// The union-max table is the pointwise maximum, times the margin, and
    /// a node-table bound everywhere either input is.
    #[test]
    fn over_temperature_tables_is_the_pointwise_max() {
        let a = table(&[(1.0, 5.0), (10.0, 1.0), (100.0, 1.0)]);
        let b = table(&[(1.0, 1.0), (5.0, 1.0), (10.0, 4.0), (100.0, 1.0)]);
        let m = Majorant::over_temperature_tables(&[a.clone(), b.clone()], 0.1);
        for k in 0..400 {
            let e = 1.0 * (100.0_f64).powf(k as f64 / 400.0);
            assert!(m.at(e) >= a.at(e).max(b.at(e)) * 1.1 - 1e-12, "e={e}");
        }
        assert!(Majorant::over_temperature_tables(&[], 0.0).is_empty());
    }

    /// The proven lift: the cover is at or above the table at every node,
    /// at midpoints, and on a dense independent sweep, for several region
    /// counts and degrees, including degree 0 and one region.
    #[test]
    fn chebyshev_cover_is_never_below_its_table() {
        let t = spiky();
        for &(regions, degree) in &[(1, 0), (1, 6), (4, 2), (16, 4), (64, 8), (500, 3)] {
            let c = t.chebyshev_cover(CoverSettings { regions, degree });
            assert!(c.cover_bands() >= 1);
            let n = t.energy.len();
            for i in 0..n - 1 {
                let (e0, e1) = (t.energy[i], t.energy[i + 1]);
                for f in [0.0, 0.25, 0.5, 0.999] {
                    let e = e0 + (e1 - e0) * f;
                    assert!(
                        c.at(e) >= t.at(e) * (1.0 - 1e-12),
                        "regions {regions} degree {degree}: cover {} < table {} at {e}",
                        c.at(e),
                        t.at(e)
                    );
                }
            }
            // Below the first node and above the last.
            assert!(c.at(1.0e-6) >= t.at(1.0e-6) * (1.0 - 1e-12));
            assert!(c.at(5.0e7) >= t.at(5.0e7) * (1.0 - 1e-12));
        }
    }

    /// More bands make a tighter cover on the same table (the knob does
    /// what the docs say), and `scaled` scales a cover.
    #[test]
    fn more_bands_cover_tighter_and_scaling_scales() {
        let t = spiky();
        let mean = |m: &Majorant| {
            let mut s = 0.0;
            let k = 2000;
            for i in 0..k {
                let e = 1.0e-2 * (1.0e8_f64).powf(i as f64 / k as f64);
                s += m.at(e) / t.at(e);
            }
            s / k as f64
        };
        let few = t.chebyshev_cover(CoverSettings {
            regions: 2,
            degree: 4,
        });
        let many = t.chebyshev_cover(CoverSettings {
            regions: 128,
            degree: 4,
        });
        assert!(mean(&many) < mean(&few));
        let e = 6.67;
        assert!((many.scaled(2.0).at(e) / many.at(e) - 2.0).abs() < 1e-9);
        // Degenerate inputs come back unchanged.
        let flat = Majorant::uniform(3.0);
        assert_eq!(flat.chebyshev_cover(CoverSettings::default()).at(5.0), 3.0);
        assert_eq!(CoverSettings::default().regions, 256);
    }

    /// A higher allowed degree is never looser in the band's mean `ln`
    /// cover (the fit keeps the best of degrees `0..=d`).
    #[test]
    fn a_higher_degree_is_never_looser_in_mean_ln() {
        let t = spiky();
        let mean_ln = |d: usize| {
            let c = t.chebyshev_cover(CoverSettings {
                regions: 3,
                degree: d,
            });
            let k = 4000;
            (0..k)
                .map(|i| {
                    let e = 2.0e-3 * (4.0e9_f64).powf(i as f64 / k as f64);
                    (c.at(e) / t.at(e)).ln()
                })
                .sum::<f64>()
                / k as f64
        };
        let (m0, m3, m8) = (mean_ln(0), mean_ln(3), mean_ln(8));
        assert!(m3 <= m0 + 0.02 && m8 <= m3 + 0.02, "{m0} {m3} {m8}");
    }

    /// Mean overhead and the temperature audit run on empty material sets.
    #[test]
    fn audit_and_overhead_on_no_materials() {
        let t = spiky();
        assert!(t.mean_overhead(&[], &[], &[1.0], &[1.0]).is_nan());
        let a = t.audit_over_temperatures(&[], 1.0, 10.0, 10);
        assert_eq!(a.worst_ratio, 0.0);
        let sets: [(&[Material], &[Nuclide]); 1] = [(&[], &[])];
        assert_eq!(
            t.audit_over_temperatures(&sets, 1.0, 10.0, 10).worst_ratio,
            0.0
        );
    }
}
