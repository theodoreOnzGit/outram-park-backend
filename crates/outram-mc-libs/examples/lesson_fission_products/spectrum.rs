//! The prompt fission neutron spectrum of Fission products lesson 3: the
//! evaluated MF=5/MT=18 law as this workspace's Monte Carlo samples it, and
//! the exact distribution that sampling scheme produces. Shared by `main.rs`
//! and `tests/lesson_fission_products.rs` through `#[path]`.
//!
//! **What the MC does** (`outram_mc_libs::material::nuclide`,
//! `sample_continuous_tabular_indexed`, a port of OpenMC's
//! `ContinuousTabular::sample`, `src/distribution_energy.cpp`):
//!
//! 1. find the incident bracket `i` with `E_i <= E < E_{i+1}` and
//!    `r = (E - E_i) / (E_{i+1} - E_i)`, linear in E, clamped outside the grid;
//! 2. pick table `l = i + 1` with probability `r`, else `l = i`;
//! 3. invert table `l`'s outgoing CDF;
//! 4. map the draw from table `l`'s range `[a_l, b_l]` onto the interpolated
//!    range `[E_1, E_K]`, `E_1 = a_i + r (a_{i+1} - a_i)` and likewise `E_K`
//!    ("unit-base" interpolation).
//!
//! The evaluation's TAB2 interpolation code is carried by the parser and not
//! used by the sampler (see `ChiTabular::incident_interp`). The functions
//! below compute, in closed form, the distribution steps 1-4 produce, so the
//! lesson can overlay the *exact* law on the sampled histogram.
#![allow(dead_code)]

use njoy_outram_park_fork::acer::angular::ElasticAngular;
use njoy_outram_park_fork::nuclear_data::secondary::{ChiEout, ChiTabular, FissionSpectrum, UncorrelatedEmission};
use outram_mc_libs::material::nuclide::sample_uncorrelated_emission;
use outram_mc_libs::rng::lcg::init_seed;

/// The incident bracket and interpolation factor the sampler uses (step 1),
/// clamped exactly as `sample_continuous_tabular_indexed` clamps.
pub fn bracket(chi: &ChiTabular, e_in: f64) -> (usize, f64) {
    let e = &chi.incident;
    let n = e.len();
    if n < 2 {
        return (0, 0.0);
    }
    if e_in < e[0] {
        return (0, 0.0);
    }
    if e_in > e[n - 1] {
        return (n - 2, 1.0);
    }
    let mut i = 0;
    while i + 1 < n && e[i + 1] <= e_in {
        i += 1;
    }
    let i = i.min(n - 2);
    (i, (e_in - e[i]) / (e[i + 1] - e[i]))
}

/// One table's CDF at `x` [eV], read the way the sampler inverts it: the
/// table's own cumulative `cdf[k]` plus the exact integral of the lin-lin (or
/// histogram) density over `[E_k, x]`. 0 below the table, 1 above it.
pub fn table_cdf(t: &ChiEout, x: f64) -> f64 {
    let n = t.e_out.len();
    if n == 0 || x <= t.e_out[0] {
        return 0.0;
    }
    if x >= t.e_out[n - 1] {
        return t.cdf[n - 1];
    }
    let k = t.e_out.partition_point(|&e| e <= x) - 1;
    let d = x - t.e_out[k];
    if t.linlin {
        let w = t.e_out[k + 1] - t.e_out[k];
        let f = if w > 0.0 { (t.pdf[k + 1] - t.pdf[k]) / w } else { 0.0 };
        t.cdf[k] + t.pdf[k] * d + 0.5 * f * d * d
    } else {
        t.cdf[k] + t.pdf[k] * d
    }
}

/// One table's density at `x` [1/eV]: lin-lin or histogram, 0 outside.
pub fn table_pdf(t: &ChiEout, x: f64) -> f64 {
    let n = t.e_out.len();
    if n < 2 || x < t.e_out[0] || x > t.e_out[n - 1] {
        return 0.0;
    }
    let k = (t.e_out.partition_point(|&e| e <= x).max(1) - 1).min(n - 2);
    if !t.linlin {
        return t.pdf[k];
    }
    let w = t.e_out[k + 1] - t.e_out[k];
    if w <= 0.0 {
        return t.pdf[k];
    }
    t.pdf[k] + (t.pdf[k + 1] - t.pdf[k]) * (x - t.e_out[k]) / w
}

/// The two tables the sampler may use at `e_in`, each with its weight and
/// the linear map from the interpolated range back onto the table's own
/// range: `(table index, weight, a_l, b_l, E_1, E_K)`.
pub fn components(chi: &ChiTabular, e_in: f64) -> Vec<(usize, f64, f64, f64, f64, f64)> {
    let (i, r) = bracket(chi, e_in);
    if chi.tables.len() < 2 {
        let t = &chi.tables[0];
        let (a, b) = (t.e_out[0], t.e_out[t.e_out.len() - 1]);
        return vec![(0, 1.0, a, b, a, b)];
    }
    let (ti, tj) = (&chi.tables[i], &chi.tables[i + 1]);
    let (ai, bi) = (ti.e_out[0], ti.e_out[ti.e_out.len() - 1]);
    let (aj, bj) = (tj.e_out[0], tj.e_out[tj.e_out.len() - 1]);
    let e1 = ai + r * (aj - ai);
    let ek = bi + r * (bj - bi);
    let mut out = Vec::new();
    if r < 1.0 {
        out.push((i, 1.0 - r, ai, bi, e1, ek));
    }
    if r > 0.0 {
        out.push((i + 1, r, aj, bj, e1, ek));
    }
    out
}

/// CDF of the outgoing energy the sampler produces at incident `e_in`,
/// evaluated at `y` [eV].
pub fn law_cdf(chi: &ChiTabular, e_in: f64, y: f64) -> f64 {
    components(chi, e_in)
        .into_iter()
        .map(|(l, w, a, b, e1, ek)| {
            let x = if ek > e1 { a + (y - e1) * (b - a) / (ek - e1) } else { y };
            w * table_cdf(&chi.tables[l], x)
        })
        .sum()
}

/// Density of the sampler's outgoing energy at `y` [1/eV].
pub fn law_pdf(chi: &ChiTabular, e_in: f64, y: f64) -> f64 {
    components(chi, e_in)
        .into_iter()
        .map(|(l, w, a, b, e1, ek)| {
            if ek > e1 {
                let x = a + (y - e1) * (b - a) / (ek - e1);
                w * table_pdf(&chi.tables[l], x) * (b - a) / (ek - e1)
            } else {
                w * table_pdf(&chi.tables[l], y)
            }
        })
        .sum()
}

/// Exact mean of one table [eV]: closed form on each lin-lin or histogram
/// panel, divided by the table's total.
pub fn table_mean(t: &ChiEout) -> f64 {
    super::data::mean_energy(&t.e_out, &t.pdf, t.linlin)
}

/// Exact mean of the sampler's outgoing energy at `e_in` [eV]: each table's
/// mean, carried through its linear range map.
pub fn law_mean(chi: &ChiTabular, e_in: f64) -> f64 {
    components(chi, e_in)
        .into_iter()
        .map(|(l, w, a, b, e1, ek)| {
            let m = table_mean(&chi.tables[l]);
            w * if b > a { e1 + (m - a) * (ek - e1) / (b - a) } else { m }
        })
        .sum()
}

/// The most probable outgoing energy at `e_in` [eV]: the maximum of
/// [`law_pdf`]. Each mapped table is piecewise linear in `y` (or piecewise
/// constant), so the maximum of their weighted sum sits at a mapped node of
/// one of them; every such node is tried (for a histogram, both sides of it).
pub fn law_mode(chi: &ChiTabular, e_in: f64) -> f64 {
    let mut best = (f64::NEG_INFINITY, 0.0);
    for (l, _, a, b, e1, ek) in components(chi, e_in) {
        for &x in &chi.tables[l].e_out {
            let y = if b > a { e1 + (x - a) * (ek - e1) / (b - a) } else { x };
            for yy in [y * (1.0 - 1e-12), y, y * (1.0 + 1e-12)] {
                let p = law_pdf(chi, e_in, yy);
                if p > best.0 {
                    best = (p, yy);
                }
            }
        }
    }
    best.1
}

/// Draw `n` outgoing energies at `e_in` through the MC's own sampler
/// (`sample_uncorrelated_emission`, which calls the same `sample_chi` the
/// fission source uses; the isotropic cosine it also returns is ignored),
/// each from its own LCG stream (`init_seed(id, 0, 1)` with particle ids
/// `id_base..id_base + n`, as the transport gives each particle one; give
/// each call its own range of ids, or the calls reuse the same draws). Returns the counts in `nbins` bins of
/// width `width` [eV] from 0, the overflow count, and the sum of the draws.
pub fn sample_histogram(spectrum: &FissionSpectrum, e_in: f64, n: usize, id_base: u64, width: f64, nbins: usize) -> (Vec<u64>, u64, f64) {
    let law = UncorrelatedEmission { energy: spectrum.clone(), angular: ElasticAngular::default(), lct: 1, yield_n: 1 };
    let mut counts = vec![0u64; nbins];
    let (mut over, mut sum) = (0u64, 0.0);
    for k in 0..n {
        let mut seed = init_seed((id_base + k as u64) as i64, 0, 1);
        let (e, _) = sample_uncorrelated_emission(&law, e_in, &mut seed);
        sum += e;
        let b = (e / width) as usize;
        if b < nbins {
            counts[b] += 1;
        } else {
            over += 1;
        }
    }
    (counts, over, sum)
}

/// The exact probability of each of the same bins, and of the overflow, from
/// [`law_cdf`].
pub fn bin_probabilities(chi: &ChiTabular, e_in: f64, width: f64, nbins: usize) -> (Vec<f64>, f64) {
    let total = law_cdf(chi, e_in, f64::MAX / 4.0);
    let mut p = Vec::with_capacity(nbins);
    let mut lo = law_cdf(chi, e_in, 0.0);
    for b in 0..nbins {
        let hi = law_cdf(chi, e_in, (b + 1) as f64 * width);
        p.push((hi - lo) / total);
        lo = hi;
    }
    (p, (total - lo) / total)
}
