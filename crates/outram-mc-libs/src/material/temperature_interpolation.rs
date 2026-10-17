//! **Stochastic interpolation between tabulated temperatures** — a port of
//! OpenMC's `temperature_method = "interpolation"` (GitHub #947, step 1 of
//! #945). **Not on any transport path yet**: nothing in this crate calls it,
//! so every existing result is unchanged (pinned by
//! `tests/temperature_interpolation_947.rs::existing_paths_are_unchanged`).
//!
//! # What upstream does
//!
//! OpenMC (`/home/teddy0/Documents/research/openmc`, commit `d7d3284a`) keeps
//! one table per tabulated temperature and, at each cross-section lookup,
//! uses **one** of the two tables that bracket the particle's temperature:
//!
//! - **Loading** (`src/nuclide.cpp:171-183`): for each temperature in the
//!   model, the bracketing pair `T_j <= T < T_{j+1}` is read.
//! - **Pointwise lookup** (`src/nuclide.cpp:686-708`,
//!   `Nuclide::calculate_xs`):
//!
//!   ```text
//!   f = (kT - kT_i) / (kT_{i+1} - kT_i)
//!   if (f > prn(seed)) ++i_temp;
//!   ```
//!
//!   so the upper table is used with probability `f`. The **expected** cross
//!   section is therefore linear in `kT`, i.e. linear in `T`. ~~Upstream's
//!   interpolation is linear in `sqrt(T)`~~ — a claim made in
//!   `njoy-outram-park-fork/verification_and_validation/temperature_interpolation/`
//!   and on #269, **corrected 2026-10-17**: the code above is linear in `kT`.
//! - **Unresolved resonances** (`src/nuclide.cpp:826-828`): the probability
//!   table used is `urr_data_[micro.index_temp]`, the **same** index the
//!   pointwise draw picked. One draw serves both. Here that is automatic:
//!   [`TemperatureTables::pick`] returns a whole [`Nuclide`], whose URR
//!   tables are the ones processed at that table's temperature.
//! - **Target motion** (`src/physics.cpp:697`): free-gas kinematics use
//!   `kTs_[i_temp]`, the picked table's temperature. Here also automatic,
//!   through [`Nuclide::free_gas_kt`] on the picked table.
//! - **S(α,β)** (`src/thermal.cpp:195-215`,
//!   `ThermalScattering::calculate_xs`): the same rule with its **own**
//!   random number, and a `while (kTs_[i+1] < kT && i+1 < n-1) ++i` search.
//!   That search and `nuclide.cpp`'s give the same table at every `T`
//!   (at a tabulated interior `T_j` the first gives `i = j-1, f = 1`, the
//!   second `i = j, f = 0`; both pick `j`), so one [`bracket`] serves both.
//!
//! # Where this port departs from upstream, deliberately
//!
//! 1. **Outside the tabulated range it refuses.** Upstream snaps to the end
//!    table (`src/nuclide.cpp:688-696`). #945's acceptance says a field the
//!    data cannot cover is an error, never silently clamped:
//!    [`NjoyError::TemperatureOutOfRange`].
//! 2. **At exactly the top temperature** upstream's pointwise loop exits with
//!    `i_temp = n-1` and then reads `kTs_[n]`, one past the end
//!    (`src/nuclide.cpp:700-704`); here `T = T_max` brackets as
//!    `(n-2, f = 1)` and always picks the top table, which is what the
//!    thermal rule does.
//! 3. **The interpolation variable is a choice**, [`InterpolationVariable`]:
//!    `Kt` is upstream's and the default; `SqrtT` and `LogT` exist so #947 can
//!    measure them. Only the probability `f` changes; the table search does
//!    not, because all three are monotone in `T`.
//!
//! # What the expectation is, and what it is not
//!
//! For any quantity **linear** in the table (a cross section at one energy,
//! a reaction rate in a fixed flux), the mean over draws is the linear
//! interpolant `(1-f) x_i + f x_{i+1}` ([`TableBracket::weights`],
//! [`TemperatureTables::expected_xs`]). For anything **nonlinear** in the
//! cross sections — a collision's reaction probability `σ_x/σ_t`, the flight
//! length, a self-shielded rate — it is not: transport draws per lookup, so
//! it sees `E[g(σ)]`, not `g(E[σ])`. #947's V&V measures both.
//!
//! # The fourth arm: a polynomial in temperature
//!
//! [`chebyshev_nodes_k`] and [`chebyshev_value`] are the "smooth fit in
//! temperature" the maintainer asked #947 to consider (#945): process the data
//! at the `n` Chebyshev nodes of `[T_lo, T_hi]` in the chosen variable, and
//! evaluate the degree-`n-1` interpolating Chebyshev series at any `T` in the
//! range, energy point by energy point. It is **deterministic** (no draw),
//! uses PETIR's port of GSL's `gsl_cheb_init` (`petir::ChebSeries::new`), and
//! is a measurement aid here, not a transport path.

use crate::material::nuclide::{MicroXS, Nuclide};
use crate::physics::scatter::K_BOLTZMANN_EV_PER_K;
use njoy_outram_park_fork::NjoyError;

/// The variable the selection probability `f` is linear in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InterpolationVariable {
    /// `kT` (equivalently `T`): **OpenMC's**, `src/nuclide.cpp:704`.
    #[default]
    Kt,
    /// `sqrt(T)`: the Doppler width's own scaling. A hypothesis (#945), not
    /// upstream's.
    SqrtT,
    /// `ln(T)`: exact for a pure power law `σ ∝ T^p` in log-log, but
    /// `f` is still applied to σ linearly. Measured for #947.
    LogT,
}

impl InterpolationVariable {
    /// Every variable, upstream's first: the arms #947 measures.
    pub const ALL: [InterpolationVariable; 3] = [Self::Kt, Self::SqrtT, Self::LogT];

    /// The coordinate `x(T)` the probability is linear in, for `t_k` \[K\] > 0.
    #[inline]
    pub fn coordinate(self, t_k: f64) -> f64 {
        match self {
            Self::Kt => K_BOLTZMANN_EV_PER_K * t_k,
            Self::SqrtT => t_k.sqrt(),
            Self::LogT => t_k.ln(),
        }
    }

    /// The temperature \[K\] at coordinate `x` (inverse of [`Self::coordinate`]).
    #[inline]
    pub fn temperature(self, x: f64) -> f64 {
        match self {
            Self::Kt => x / K_BOLTZMANN_EV_PER_K,
            Self::SqrtT => x * x,
            Self::LogT => x.exp(),
        }
    }

    /// A short label for tables: `"T"`, `"sqrt T"`, `"log T"`.
    pub fn label(self) -> &'static str {
        match self {
            Self::Kt => "T",
            Self::SqrtT => "sqrt T",
            Self::LogT => "log T",
        }
    }
}

/// The two tables bracketing a temperature, and the probability of the upper.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TableBracket {
    /// Index of the lower table, `T_lower <= T`.
    pub lower: usize,
    /// Probability `f` in `[0, 1]` of using table `lower + 1`. Zero when only
    /// one table exists (and then `lower + 1` is never returned).
    pub upper_probability: f64,
}

impl TableBracket {
    /// Upstream's draw, `src/nuclide.cpp:705-707`: `if (f > prn) ++i_temp`.
    /// `xi` is a uniform variate in `[0, 1)`.
    #[inline]
    pub fn pick(self, xi: f64) -> usize {
        if self.upper_probability > xi {
            self.lower + 1
        } else {
            self.lower
        }
    }

    /// `[(lower, 1-f), (lower+1, f)]`: the probabilities [`Self::pick`]
    /// returns each index with, over a uniform `xi`.
    #[inline]
    pub fn weights(self) -> [(usize, f64); 2] {
        let f = self.upper_probability;
        [(self.lower, 1.0 - f), (self.lower + 1, f)]
    }
}

/// The bracket of `t_k` \[K\] in the ascending `temps_k` \[K\], with the
/// upper table's probability linear in `variable`.
///
/// One table: `t_k` must equal it (to 1e-9 relative), and the bracket is
/// `(0, 0)`.
///
/// # Errors
///
/// [`NjoyError::InvalidInput`] for an empty or non-ascending list or a
/// non-positive or non-finite `t_k`; [`NjoyError::TemperatureOutOfRange`]
/// for `t_k` outside `[temps_k[0], temps_k[n-1]]` (upstream snaps; see the
/// module docs).
pub fn bracket(
    temps_k: &[f64],
    t_k: f64,
    variable: InterpolationVariable,
) -> Result<TableBracket, NjoyError> {
    check_ascending(temps_k)?;
    if !(t_k.is_finite() && t_k > 0.0) {
        return Err(NjoyError::InvalidInput(format!(
            "temperature interpolation needs a positive finite temperature, got {t_k} K"
        )));
    }
    let (lo, hi) = (temps_k[0], temps_k[temps_k.len() - 1]);
    let out = || NjoyError::TemperatureOutOfRange {
        requested_k: t_k,
        min_k: lo,
        max_k: hi,
    };
    if temps_k.len() == 1 {
        return if ((t_k - lo) / lo).abs() <= 1.0e-9 {
            Ok(TableBracket {
                lower: 0,
                upper_probability: 0.0,
            })
        } else {
            Err(out())
        };
    }
    if t_k < lo || t_k > hi {
        return Err(out());
    }
    // thermal.cpp's search: the first i with kT_{i+1} >= kT, capped at n-2.
    let mut i = 0;
    while temps_k[i + 1] < t_k && i + 1 < temps_k.len() - 1 {
        i += 1;
    }
    let (x0, x1, x) = (
        variable.coordinate(temps_k[i]),
        variable.coordinate(temps_k[i + 1]),
        variable.coordinate(t_k),
    );
    let f = ((x - x0) / (x1 - x0)).clamp(0.0, 1.0);
    Ok(TableBracket {
        lower: i,
        upper_probability: f,
    })
}

fn check_ascending(temps_k: &[f64]) -> Result<(), NjoyError> {
    if temps_k.is_empty() {
        return Err(NjoyError::InvalidInput("no tabulated temperatures".into()));
    }
    if temps_k.iter().any(|t| !(t.is_finite() && *t > 0.0)) {
        return Err(NjoyError::InvalidInput(format!(
            "tabulated temperatures must be positive and finite: {temps_k:?}"
        )));
    }
    if temps_k.windows(2).any(|w| w[1] <= w[0]) {
        return Err(NjoyError::InvalidInput(format!(
            "tabulated temperatures must be strictly ascending: {temps_k:?}"
        )));
    }
    Ok(())
}

/// One kind of table (a [`Nuclide`], a
/// [`ThermalScattering`](crate::material::thermal::ThermalScattering), ...)
/// at several temperatures, chosen between stochastically as OpenMC does.
///
/// Generic rather than a trait object (workspace rule): `D` is the concrete
/// table type.
#[derive(Debug, Clone)]
pub struct TemperatureTables<D> {
    temps_k: Vec<f64>,
    tables: Vec<D>,
    variable: InterpolationVariable,
}

impl<D> TemperatureTables<D> {
    /// Hold `tables[j]`, processed at `temps_k[j]` \[K\], interpolated in
    /// `variable` ([`InterpolationVariable::Kt`] is upstream's).
    ///
    /// # Errors
    ///
    /// [`NjoyError::InvalidInput`] if the lengths differ or the temperatures
    /// are not positive, finite and strictly ascending.
    pub fn new(
        temps_k: Vec<f64>,
        tables: Vec<D>,
        variable: InterpolationVariable,
    ) -> Result<Self, NjoyError> {
        check_ascending(&temps_k)?;
        if temps_k.len() != tables.len() {
            return Err(NjoyError::InvalidInput(format!(
                "{} temperatures but {} tables",
                temps_k.len(),
                tables.len()
            )));
        }
        Ok(Self {
            temps_k,
            tables,
            variable,
        })
    }

    /// The tabulated temperatures \[K\], ascending.
    pub fn temperatures_k(&self) -> &[f64] {
        &self.temps_k
    }

    /// The tables, in temperature order.
    pub fn tables(&self) -> &[D] {
        &self.tables
    }

    /// The interpolation variable.
    pub fn variable(&self) -> InterpolationVariable {
        self.variable
    }

    /// The bracket of `t_k` \[K\]; see [`bracket`].
    ///
    /// # Errors
    ///
    /// As [`bracket`].
    pub fn bracket(&self, t_k: f64) -> Result<TableBracket, NjoyError> {
        bracket(&self.temps_k, t_k, self.variable)
    }

    /// The table a lookup at `t_k` \[K\] uses for the uniform variate `xi`,
    /// and its index: upstream's stochastic choice.
    ///
    /// # Errors
    ///
    /// As [`bracket`].
    pub fn pick(&self, t_k: f64, xi: f64) -> Result<(usize, &D), NjoyError> {
        let j = self.bracket(t_k)?.pick(xi);
        Ok((j, &self.tables[j]))
    }

    /// The expectation over the draw of a quantity `g` computed from one
    /// table: `(1-f) g(table_i) + f g(table_{i+1})`. Exact, not sampled.
    ///
    /// # Errors
    ///
    /// As [`bracket`].
    pub fn expectation<G>(&self, t_k: f64, mut g: G) -> Result<f64, NjoyError>
    where
        G: FnMut(&D) -> f64,
    {
        let b = self.bracket(t_k)?;
        let mut sum = 0.0;
        for (j, w) in b.weights() {
            if w > 0.0 {
                sum += w * g(&self.tables[j]);
            }
        }
        Ok(sum)
    }
}

impl TemperatureTables<Nuclide> {
    /// The **expected** micro cross sections at energy `e` \[eV\] and
    /// temperature `t_k` \[K\]: each field `(1-f) σ_i + f σ_{i+1}`, the
    /// smooth (infinitely dilute) values as [`Nuclide::xs_at_energy`]. What
    /// a linear quantity sees on average; see the module docs for what it is
    /// not.
    ///
    /// # Errors
    ///
    /// As [`bracket`].
    pub fn expected_xs(&self, e: f64, t_k: f64) -> Result<MicroXS, NjoyError> {
        let b = self.bracket(t_k)?;
        let mut out = MicroXS::default();
        for (j, w) in b.weights() {
            if w > 0.0 {
                let x = self.tables[j].xs_at_energy(e, self.temps_k[j]);
                out.total += w * x.total;
                out.elastic += w * x.elastic;
                out.fission += w * x.fission;
                out.absorption += w * x.absorption;
                out.inelastic += w * x.inelastic;
                out.n2n += w * x.n2n;
                out.n3n += w * x.n3n;
                out.mt5 += w * x.mt5;
                out.other += w * x.other;
                out.nu_fission += w * x.nu_fission;
            }
        }
        Ok(out)
    }
}

/// The `n` temperatures \[K\] at the Chebyshev nodes of `[t_lo_k, t_hi_k]`
/// in `variable`, in the order `petir::ChebSeries::new` samples them
/// (`x_k = cos(π (k + ½)/n)`, descending in the coordinate, so descending in
/// `T`). Process the data at these, then [`chebyshev_value`].
///
/// # Errors
///
/// [`NjoyError::InvalidInput`] for `n == 0` or a range that is not
/// `0 < t_lo_k < t_hi_k`.
pub fn chebyshev_nodes_k(
    n: usize,
    t_lo_k: f64,
    t_hi_k: f64,
    variable: InterpolationVariable,
) -> Result<Vec<f64>, NjoyError> {
    if n == 0 || !(t_lo_k > 0.0 && t_hi_k > t_lo_k && t_hi_k.is_finite()) {
        return Err(NjoyError::InvalidInput(format!(
            "Chebyshev nodes need n >= 1 and 0 < T_lo < T_hi, got n = {n}, [{t_lo_k}, {t_hi_k}] K"
        )));
    }
    let (a, b) = (variable.coordinate(t_lo_k), variable.coordinate(t_hi_k));
    let (bma, bpa) = (0.5 * (b - a), 0.5 * (b + a));
    Ok((0..n)
        .map(|k| {
            let y = (std::f64::consts::PI * (k as f64 + 0.5) / n as f64).cos();
            variable.temperature(y * bma + bpa)
        })
        .collect())
}

/// The degree-`n-1` Chebyshev interpolant through `values[k]`, the value at
/// node `k` of [`chebyshev_nodes_k`]`(n, t_lo_k, t_hi_k, variable)`,
/// evaluated at `t_k` \[K\] inside the range.
///
/// Built by `petir::ChebSeries::new` (a port of GSL's `gsl_cheb_init`), whose
/// sampling points are those nodes; each sample is matched to its node by
/// position, never by interpolation.
///
/// # Errors
///
/// [`NjoyError::InvalidInput`] for an empty `values`, a bad range, or `t_k`
/// outside it.
pub fn chebyshev_value(
    values: &[f64],
    t_lo_k: f64,
    t_hi_k: f64,
    variable: InterpolationVariable,
    t_k: f64,
) -> Result<f64, NjoyError> {
    let nodes = chebyshev_nodes_k(values.len(), t_lo_k, t_hi_k, variable)?;
    if !(t_k >= t_lo_k && t_k <= t_hi_k) {
        return Err(NjoyError::TemperatureOutOfRange {
            requested_k: t_k,
            min_k: t_lo_k,
            max_k: t_hi_k,
        });
    }
    let (a, b) = (variable.coordinate(t_lo_k), variable.coordinate(t_hi_k));
    let xs: Vec<f64> = nodes.iter().map(|&t| variable.coordinate(t)).collect();
    let series = petir::ChebSeries::new(values.len() - 1, a, b, |x| {
        // The node nearest x: ChebSeries::new samples exactly these points.
        let k = xs
            .iter()
            .enumerate()
            .min_by(|p, q| (p.1 - x).abs().total_cmp(&(q.1 - x).abs()))
            .map_or(0, |(k, _)| k);
        values[k]
    })
    .map_err(|e| NjoyError::InvalidInput(format!("Chebyshev series: {e:?}")))?;
    Ok(series.eval(variable.coordinate(t_k)))
}

/// The weights `w_k` with `chebyshev_value(values, ..) = Σ_k w_k values[k]`
/// at `t_k` \[K\], for `n` nodes: the interpolant is linear in the node
/// values, so the weights are its values on the unit vectors. Compute them
/// once per temperature and apply them to every energy point.
///
/// # Errors
///
/// As [`chebyshev_value`].
pub fn chebyshev_weights(
    n: usize,
    t_lo_k: f64,
    t_hi_k: f64,
    variable: InterpolationVariable,
    t_k: f64,
) -> Result<Vec<f64>, NjoyError> {
    (0..n)
        .map(|k| {
            let mut unit = vec![0.0; n];
            unit[k] = 1.0;
            chebyshev_value(&unit, t_lo_k, t_hi_k, variable, t_k)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEMPS: [f64; 4] = [300.0, 600.0, 900.0, 1200.0];

    #[test]
    fn bracket_is_upstreams_linear_in_kt() {
        let b = bracket(&TEMPS, 450.0, InterpolationVariable::Kt).unwrap();
        assert_eq!(b.lower, 0);
        assert!((b.upper_probability - 0.5).abs() < 1e-14);
        // nuclide.cpp:706: f > prn picks the upper table.
        assert_eq!(b.pick(0.49), 1);
        assert_eq!(b.pick(0.51), 0);
        let w = b.weights();
        assert!((w[0].1 + w[1].1 - 1.0).abs() < 1e-15);
    }

    #[test]
    fn variables_change_only_the_probability() {
        let t = 450.0;
        for v in InterpolationVariable::ALL {
            let b = bracket(&TEMPS, t, v).unwrap();
            assert_eq!(b.lower, 0, "{v:?}");
            let (x0, x1) = (v.coordinate(300.0), v.coordinate(600.0));
            let want = (v.coordinate(t) - x0) / (x1 - x0);
            assert!((b.upper_probability - want).abs() < 1e-14);
            assert!((v.temperature(v.coordinate(t)) - t).abs() < 1e-9);
            assert!(!v.label().is_empty());
        }
        // sqrt and log are concave, so the midpoint in T lies past the
        // midpoint in either: f > 0.5, and more so in log T.
        let fs = bracket(&TEMPS, t, InterpolationVariable::SqrtT)
            .unwrap()
            .upper_probability;
        let fl = bracket(&TEMPS, t, InterpolationVariable::LogT)
            .unwrap()
            .upper_probability;
        assert!(fs > 0.5 && fl > fs);
    }

    #[test]
    fn tabulated_points_pick_their_own_table() {
        // Interior: thermal.cpp's (j-1, f=1) and nuclide.cpp's (j, 0) agree.
        let b = bracket(&TEMPS, 600.0, InterpolationVariable::Kt).unwrap();
        assert_eq!(
            b,
            TableBracket {
                lower: 0,
                upper_probability: 1.0
            }
        );
        for xi in [0.0, 0.5, 0.999_999] {
            assert_eq!(b.pick(xi), 1);
        }
        // Ends: the bottom is (0, 0); the top (n-2, 1), not upstream's
        // out-of-bounds read.
        assert_eq!(
            bracket(&TEMPS, 300.0, InterpolationVariable::Kt)
                .unwrap()
                .pick(0.0),
            0
        );
        let top = bracket(&TEMPS, 1200.0, InterpolationVariable::Kt).unwrap();
        assert_eq!(
            top,
            TableBracket {
                lower: 2,
                upper_probability: 1.0
            }
        );
    }

    #[test]
    fn outside_the_range_is_refused_not_snapped() {
        for t in [299.0, 1201.0] {
            match bracket(&TEMPS, t, InterpolationVariable::Kt) {
                Err(NjoyError::TemperatureOutOfRange { requested_k, .. }) => {
                    assert_eq!(requested_k, t)
                }
                other => panic!("expected out-of-range at {t} K, got {other:?}"),
            }
        }
        assert!(bracket(&[600.0], 600.0, InterpolationVariable::Kt).is_ok());
        assert!(bracket(&[600.0], 601.0, InterpolationVariable::Kt).is_err());
        assert!(bracket(&[], 600.0, InterpolationVariable::Kt).is_err());
        assert!(bracket(&[600.0, 300.0], 450.0, InterpolationVariable::Kt).is_err());
        assert!(bracket(&TEMPS, f64::NAN, InterpolationVariable::Kt).is_err());
    }

    #[test]
    fn tables_pick_and_expectation() {
        let t = TemperatureTables::new(
            TEMPS.to_vec(),
            vec![1.0, 2.0, 4.0, 8.0],
            InterpolationVariable::Kt,
        )
        .unwrap();
        assert_eq!(t.temperatures_k(), &TEMPS);
        assert_eq!(t.tables().len(), 4);
        assert_eq!(t.variable(), InterpolationVariable::Kt);
        // 750 K: f = 0.5 between 2 and 4.
        assert!((t.expectation(750.0, |x| *x).unwrap() - 3.0).abs() < 1e-12);
        assert_eq!(*t.pick(750.0, 0.2).unwrap().1, 4.0);
        assert_eq!(t.pick(750.0, 0.7).unwrap().0, 1);
        // The sample mean converges on the expectation.
        let mut seed = 12345u64;
        let n = 200_000;
        let mean: f64 = (0..n)
            .map(|_| *t.pick(700.0, crate::rng::lcg::prn(&mut seed)).unwrap().1)
            .sum::<f64>()
            / n as f64;
        let want = t.expectation(700.0, |x| *x).unwrap();
        assert!(
            (mean - want).abs() < 4.0 * 1.0 / (n as f64).sqrt(),
            "{mean} vs {want}"
        );
        assert!(
            TemperatureTables::new(vec![300.0], vec![1.0, 2.0], InterpolationVariable::Kt).is_err()
        );
    }

    #[test]
    fn chebyshev_arm_is_exact_for_a_polynomial_and_close_for_inverse_sqrt() {
        // A quadratic in sqrt(T) is reproduced exactly by three nodes in sqrt(T).
        let (lo, hi) = (300.0, 1500.0);
        let v = InterpolationVariable::SqrtT;
        let nodes = chebyshev_nodes_k(3, lo, hi, v).unwrap();
        assert!(nodes.iter().all(|t| *t > lo && *t < hi));
        let f = |t: f64| 2.0 + 3.0 * t.sqrt() - 0.1 * t;
        let vals: Vec<f64> = nodes.iter().map(|&t| f(t)).collect();
        for t in [300.0, 450.0, 1000.0, 1500.0] {
            assert!((chebyshev_value(&vals, lo, hi, v, t).unwrap() - f(t)).abs() < 1e-10);
        }
        // A Doppler-like 1/sqrt(T): five nodes in sqrt(T) land within 1e-3.
        let g = |t: f64| 1.0 / t.sqrt();
        let nodes = chebyshev_nodes_k(5, lo, hi, v).unwrap();
        let vals: Vec<f64> = nodes.iter().map(|&t| g(t)).collect();
        for t in [300.0, 333.0, 700.0, 1499.0] {
            let rel = chebyshev_value(&vals, lo, hi, v, t).unwrap() / g(t) - 1.0;
            assert!(rel.abs() < 1e-3, "{t} K: {rel:e}");
        }
        assert!(chebyshev_value(&vals, lo, hi, v, 1600.0).is_err());
        // The weights reproduce the value, and sum to one (constants are exact).
        let w = chebyshev_weights(5, lo, hi, v, 700.0).unwrap();
        let dot: f64 = w.iter().zip(&vals).map(|(a, b)| a * b).sum();
        assert!((dot - chebyshev_value(&vals, lo, hi, v, 700.0).unwrap()).abs() < 1e-14);
        assert!((w.iter().sum::<f64>() - 1.0).abs() < 1e-12);
        assert!(chebyshev_nodes_k(0, lo, hi, v).is_err());
        assert!(chebyshev_nodes_k(3, hi, lo, v).is_err());
    }
}
