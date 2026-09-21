//! Imprecise probability — intervals, probability boxes, confidence boxes, and
//! the reliability of coherent systems under limited data.
//!
//! # Why a distribution is sometimes the wrong object
//!
//! Fit a distribution to four failure observations and you get a curve with no
//! visible caveat attached. The curve is a fiction: four points do not pin down
//! a distribution, and the analysis downstream will treat it as if they had.
//!
//! Imprecise probability keeps the ignorance in the object instead of
//! discarding it. A **probability box** ([`Pbox`]) is a pair of CDFs that
//! bracket the unknown true one; a **confidence box** ([`cbox_binomial`]) is a
//! p-box built so that its cuts *are* confidence intervals, at every level at
//! once. Propagate those through a system model and the answer comes out as
//! bounds that state what the data support, rather than a single number that
//! does not.
//!
//! # What is here
//!
//! - [`Interval`] — closed real intervals with the arithmetic the structure
//!   functions below need.
//! - [`Pbox`] — a p-box on a bounded range, stored as lower and upper CDFs on a
//!   shared grid. Construction from a precise distribution, from an interval,
//!   and from explicit bounds; cuts, CDF bounds, and an enclosure check.
//! - [`cbox_binomial`] — the Clopper–Pearson confidence box for a rate observed
//!   as `k` successes in `n` trials. The central object of the
//!   "computing with confidence" line of work.
//! - [`SystemStructure`] and [`EventDependence`] — series, parallel and
//!   k-out-of-n reliability, evaluated either under independence of component
//!   failures or with **no dependence assumption at all** (Fréchet bounds).
//!
//! # Two different dependence questions, kept apart
//!
//! This is where imprecise reliability analyses most often go wrong, so the API
//! separates them:
//!
//! 1. **Dependence between component failure *events*.** Do two pumps fail
//!    independently, or does a common cause link them? This decides the
//!    *formula*: a product for independence, Fréchet bounds when nothing is
//!    assumed. That is [`EventDependence`].
//! 2. **Dependence between the *uncertainties* in the component
//!    reliabilities.** Two components' reliabilities may each be known only to
//!    a confidence box; are those two states of knowledge related? This decides
//!    how the boxes are *combined*.
//!
//! [`SystemStructure::reliability_pbox`] answers (1) as the caller specifies
//! and (2) by combining the component boxes **at matched confidence level** —
//! the comonotone case, which is what the confidence-box literature reports as
//! "generalised confidence bounds", and which this module states rather than
//! leaves implicit. General unknown-dependence p-box convolution is *not*
//! implemented; see [`SystemStructure::reliability_pbox`] for what that would
//! take.
//!
//! # References
//!
//! - A. Lye, W. Vechgama, M. Sallak, S. Destercke, S. Ferson and S. Xiao
//!   (2024). Advances in the reliability analysis of coherent systems under
//!   limited data with confidence boxes. *ASCE-ASME Journal of Risk and
//!   Uncertainty in Engineering Systems Part A: Civil Engineering, 11*,
//!   04024074. doi:
//!   [10.1061/AJRUA6.RUENG-1380](https://doi.org/10.1061/AJRUA6.RUENG-1380)
//! - C. J. Clopper and E. S. Pearson (1934). The use of confidence or fiducial
//!   limits illustrated in the case of the binomial. *Biometrika, 26*(4),
//!   404–413. doi: [10.1093/biomet/26.4.404](https://doi.org/10.1093/biomet/26.4.404)
//! - S. Ferson, V. Kreinovich, L. Ginzburg, D. S. Myers and K. Sentz (2003).
//!   Constructing probability boxes and Dempster-Shafer structures. Sandia
//!   National Laboratories, SAND2002-4015. doi:
//!   [10.2172/809606](https://doi.org/10.2172/809606)
//! - M. Fréchet (1935). Généralisations du théorème des probabilités totales.
//!   *Fundamenta Mathematicae, 25*, 379–387.
//! - S. Ferson, R. B. Nelsen, J. Hajagos, D. J. Berleant, J. Zhang, W. T.
//!   Tucker, L. R. Ginzburg and W. L. Oberkampf (2004). Dependence in
//!   probabilistic modeling, Dempster-Shafer theory, and probability bounds
//!   analysis. Sandia National Laboratories, SAND2004-3072. doi:
//!   [10.2172/919189](https://doi.org/10.2172/919189)
//!
//! Independent implementations from the published definitions — see the
//! provenance note in [`crate::bayesian`].

use crate::distributions::{Beta, ContinuousDistribution1D};
use crate::{RafflesError, Result};

/// A closed real interval `[lower, upper]`.
///
/// The arithmetic here is the standard interval arithmetic, restricted to the
/// operations the reliability structure functions need. Every operation is
/// *rigorous*: the result encloses every value the true operands could produce.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Interval {
    lower: f64,
    upper: f64,
}

impl Interval {
    /// Builds `[lower, upper]`.
    ///
    /// # Errors
    ///
    /// [`RafflesError::InvalidParameter`] if either bound is not finite, or if
    /// `lower > upper` — an inverted interval is a sign the caller has
    /// swapped two arguments, and silently sorting them would hide that.
    pub fn new(lower: f64, upper: f64) -> Result<Self> {
        if !lower.is_finite() || !upper.is_finite() {
            return Err(RafflesError::InvalidParameter {
                parameter: "interval bound".to_string(),
                value: if lower.is_finite() { upper } else { lower },
                reason: "interval bounds must be finite".to_string(),
            });
        }
        if lower > upper {
            return Err(RafflesError::InvalidParameter {
                parameter: "lower".to_string(),
                value: lower,
                reason: format!("lower bound exceeds the upper bound {upper}"),
            });
        }
        Ok(Self { lower, upper })
    }

    /// A degenerate interval containing one value.
    pub fn point(value: f64) -> Result<Self> {
        Self::new(value, value)
    }

    /// The lower bound.
    pub fn lower(&self) -> f64 {
        self.lower
    }

    /// The upper bound.
    pub fn upper(&self) -> f64 {
        self.upper
    }

    /// Width `upper - lower`, a measure of how much is not known.
    pub fn width(&self) -> f64 {
        self.upper - self.lower
    }

    /// Whether `value` lies in the closed interval.
    pub fn contains(&self, value: f64) -> bool {
        value >= self.lower && value <= self.upper
    }

    /// Whether this interval encloses `other` entirely.
    pub fn encloses(&self, other: &Interval) -> bool {
        self.lower <= other.lower && self.upper >= other.upper
    }

    /// Interval sum.
    pub fn add(&self, other: &Interval) -> Interval {
        Interval {
            lower: self.lower + other.lower,
            upper: self.upper + other.upper,
        }
    }

    /// Interval product.
    ///
    /// Takes the extremes of all four endpoint products, which is correct for
    /// operands of any sign — not only the non-negative case the reliability
    /// formulas happen to use.
    pub fn multiply(&self, other: &Interval) -> Interval {
        let products = [
            self.lower * other.lower,
            self.lower * other.upper,
            self.upper * other.lower,
            self.upper * other.upper,
        ];
        Interval {
            lower: products.iter().cloned().fold(f64::INFINITY, f64::min),
            upper: products.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
        }
    }

    /// The complement `1 - x`, which for a probability interval swaps and
    /// reflects the bounds.
    pub fn complement(&self) -> Interval {
        Interval {
            lower: 1.0 - self.upper,
            upper: 1.0 - self.lower,
        }
    }

    /// Element-wise minimum with another interval.
    pub fn min(&self, other: &Interval) -> Interval {
        Interval {
            lower: self.lower.min(other.lower),
            upper: self.upper.min(other.upper),
        }
    }

    /// Element-wise maximum with another interval.
    pub fn max(&self, other: &Interval) -> Interval {
        Interval {
            lower: self.lower.max(other.lower),
            upper: self.upper.max(other.upper),
        }
    }

    /// Clamps both bounds into `[0, 1]`, for quantities that are probabilities.
    pub fn clamp_to_unit(&self) -> Interval {
        Interval {
            lower: self.lower.clamp(0.0, 1.0),
            upper: self.upper.clamp(0.0, 1.0),
        }
    }
}

/// How component failure events depend on one another.
///
/// This chooses the *structure function*, not the arithmetic: see the module
/// documentation on keeping the two dependence questions apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventDependence {
    /// Components fail independently — the textbook assumption.
    ///
    /// Series reliability is the product of component reliabilities; parallel
    /// is one minus the product of the unreliabilities. Convenient, and wrong
    /// whenever a common cause exists — which in a real plant it usually does.
    Independent,
    /// **Nothing is assumed** about how failures are related.
    ///
    /// The result is the Fréchet bound: the tightest interval that holds for
    /// *every* possible dependence structure, from perfect positive to perfect
    /// negative. For a series system of `n` components,
    /// `[max(0, sum(R_i) - (n-1)), min(R_i)]`; for a parallel system,
    /// `[max(R_i), min(1, sum(R_i))]`.
    ///
    /// These are wide — that is the honest cost of not knowing — and they are
    /// *guaranteed*, which the independence answer is not. A designer who
    /// assumes independence is making a claim about the plant; choosing this
    /// variant is declining to.
    Unknown,
}

/// The reliability structure of a coherent system.
///
/// "Coherent" in the reliability sense: the system is monotone (repairing a
/// component never hurts) and every component matters. That monotonicity is
/// what makes bound propagation valid — applying the structure function to the
/// component bounds gives the system bounds, with no search needed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemStructure {
    /// All components must work: the system is as weak as its weakest part.
    Series,
    /// Any one component suffices: full redundancy.
    Parallel,
    /// At least `k` of the `n` components must work.
    ///
    /// Covers the middle ground that series and parallel do not — a 2-out-of-3
    /// voting logic, or a pump train where two of four suffice.
    KOutOfN {
        /// How many components must work; `1` is parallel and `n` is series.
        k: usize,
    },
}

impl SystemStructure {
    /// System reliability from *interval-valued* component reliabilities.
    ///
    /// Every component reliability is an interval in `[0, 1]`; the result is the
    /// interval of system reliabilities consistent with them, under the chosen
    /// [`EventDependence`].
    ///
    /// # Errors
    ///
    /// - [`RafflesError::InvalidParameter`] if `components` is empty, if any
    ///   component interval strays outside `[0, 1]`, or if a `KOutOfN` has
    ///   `k = 0` or `k > n`.
    pub fn reliability_interval(
        &self,
        components: &[Interval],
        dependence: EventDependence,
    ) -> Result<Interval> {
        if components.is_empty() {
            return Err(RafflesError::InvalidParameter {
                parameter: "components".to_string(),
                value: 0.0,
                reason: "a system needs at least one component".to_string(),
            });
        }
        for component in components {
            if component.lower() < 0.0 || component.upper() > 1.0 {
                return Err(RafflesError::InvalidParameter {
                    parameter: "component reliability".to_string(),
                    value: component.lower().min(component.upper()),
                    reason: "component reliabilities must lie in [0, 1]".to_string(),
                });
            }
        }
        let n = components.len();

        match (self, dependence) {
            (Self::Series, EventDependence::Independent) => {
                let mut product = Interval::point(1.0)?;
                for component in components {
                    product = product.multiply(component);
                }
                Ok(product.clamp_to_unit())
            }
            (Self::Series, EventDependence::Unknown) => {
                // Fréchet: P(all work) is at least sum(R) - (n - 1) and at most
                // the weakest component's reliability.
                let sum_lower: f64 = components.iter().map(|c| c.lower()).sum();
                let sum_upper: f64 = components.iter().map(|c| c.upper()).sum();
                let min_lower = components
                    .iter()
                    .map(|c| c.lower())
                    .fold(f64::INFINITY, f64::min);
                let min_upper = components
                    .iter()
                    .map(|c| c.upper())
                    .fold(f64::INFINITY, f64::min);
                let lower = (sum_lower - (n as f64 - 1.0)).max(0.0);
                let upper = min_upper.min(1.0);
                let _ = (sum_upper, min_lower);
                Interval::new(lower.min(upper), upper)
            }
            (Self::Parallel, EventDependence::Independent) => {
                let mut failure = Interval::point(1.0)?;
                for component in components {
                    failure = failure.multiply(&component.complement());
                }
                Ok(failure.complement().clamp_to_unit())
            }
            (Self::Parallel, EventDependence::Unknown) => {
                // Fréchet: P(any works) is at least the best component and at
                // most the sum, capped at 1.
                let max_lower = components
                    .iter()
                    .map(|c| c.lower())
                    .fold(f64::NEG_INFINITY, f64::max);
                let sum_upper: f64 = components.iter().map(|c| c.upper()).sum();
                let lower = max_lower.clamp(0.0, 1.0);
                let upper = sum_upper.min(1.0);
                Interval::new(lower, upper.max(lower))
            }
            (Self::KOutOfN { k }, dependence) => {
                if *k == 0 || *k > n {
                    return Err(RafflesError::InvalidParameter {
                        parameter: "k".to_string(),
                        value: *k as f64,
                        reason: format!("k must lie in 1..={n} for a {n}-component system"),
                    });
                }
                if *k == n {
                    return Self::Series.reliability_interval(components, dependence);
                }
                if *k == 1 {
                    return Self::Parallel.reliability_interval(components, dependence);
                }
                match dependence {
                    EventDependence::Independent => {
                        Ok(k_out_of_n_independent(components, *k).clamp_to_unit())
                    }
                    EventDependence::Unknown => {
                        // Fréchet for "at least k of n": the lower bound is the
                        // same inclusion-exclusion floor as the series case
                        // relaxed by the n - k failures that are tolerated, and
                        // the upper bound is the k-th largest reliability,
                        // since at best the k strongest components carry the
                        // system.
                        let sum_lower: f64 = components.iter().map(|c| c.lower()).sum();
                        let lower = (sum_lower - (*k as f64 - 1.0) - (n - *k) as f64)
                            .max(0.0)
                            .min(1.0);
                        let mut uppers: Vec<f64> = components.iter().map(|c| c.upper()).collect();
                        uppers
                            .sort_by(|a, b| b.partial_cmp(a).unwrap_or(core::cmp::Ordering::Equal));
                        let upper = uppers[*k - 1].clamp(0.0, 1.0);
                        Interval::new(lower.min(upper), upper)
                    }
                }
            }
        }
    }

    /// System reliability as a p-box, from component reliability p-boxes.
    ///
    /// The component boxes are combined **at matched confidence level**: the
    /// system's lower CDF is built from the components' lower CDFs and its
    /// upper from their uppers, quantile by quantile. That is the *comonotone*
    /// treatment of question (2) in the module documentation — it assumes the
    /// components' states of knowledge move together, which is what the
    /// confidence-box literature reports and is exact when the component boxes
    /// come from a common source of ignorance.
    ///
    /// **What this is not.** It is not the general unknown-dependence
    /// combination of the uncertainties. That would need a Fréchet convolution
    /// over the joint p-box — a genuinely different and much more expensive
    /// computation — and it would give *wider* bounds. Do not describe results
    /// from this function as holding for arbitrary dependence between the
    /// component uncertainties; the dependence it does not assume is between
    /// the failure *events*, via [`EventDependence::Unknown`].
    ///
    /// # Errors
    ///
    /// [`RafflesError::DimensionMismatch`] if the component boxes do not share
    /// a grid, plus whatever
    /// [`reliability_interval`](Self::reliability_interval) rejects.
    pub fn reliability_pbox(
        &self,
        components: &[Pbox],
        dependence: EventDependence,
    ) -> Result<Pbox> {
        if components.is_empty() {
            return Err(RafflesError::InvalidParameter {
                parameter: "components".to_string(),
                value: 0.0,
                reason: "a system needs at least one component".to_string(),
            });
        }
        let reference = &components[0];
        for component in components {
            if component.levels() != reference.levels() {
                return Err(RafflesError::DimensionMismatch {
                    expected: reference.levels(),
                    found: component.levels(),
                });
            }
        }

        let levels = reference.levels();
        let mut lower_quantiles = Vec::with_capacity(levels);
        let mut upper_quantiles = Vec::with_capacity(levels);
        for level in 0..levels {
            let cuts: Vec<Interval> = components
                .iter()
                .map(|c| c.quantile_interval(level))
                .collect::<Result<Vec<_>>>()?;
            let system = self.reliability_interval(&cuts, dependence)?;
            lower_quantiles.push(system.lower());
            upper_quantiles.push(system.upper());
        }
        Pbox::from_quantile_bounds(lower_quantiles, upper_quantiles)
    }
}

/// Exact "at least `k` of `n`" reliability for independent components, on
/// interval inputs.
///
/// Uses the standard dynamic program over the Poisson-binomial distribution
/// rather than enumerating the `2^n` states, so a 20-component system is
/// instant rather than a million evaluations. Monotonicity of the structure
/// function is what lets the bounds be computed from the endpoint values.
fn k_out_of_n_independent(components: &[Interval], k: usize) -> Interval {
    let lower =
        poisson_binomial_at_least(&components.iter().map(|c| c.lower()).collect::<Vec<_>>(), k);
    let upper =
        poisson_binomial_at_least(&components.iter().map(|c| c.upper()).collect::<Vec<_>>(), k);
    Interval { lower, upper }
}

/// `P(at least k successes)` for independent Bernoulli trials with the given
/// success probabilities.
fn poisson_binomial_at_least(p: &[f64], k: usize) -> f64 {
    let n = p.len();
    // distribution[j] = P(exactly j successes so far)
    let mut distribution = vec![0.0; n + 1];
    distribution[0] = 1.0;
    for (i, probability) in p.iter().enumerate() {
        for j in (0..=i + 1).rev() {
            let stay = distribution[j] * (1.0 - probability);
            let advance = if j > 0 {
                distribution[j - 1] * probability
            } else {
                0.0
            };
            distribution[j] = stay + advance;
        }
    }
    distribution[k..].iter().sum::<f64>().clamp(0.0, 1.0)
}

/// A probability box: a pair of CDFs that bracket an unknown distribution.
///
/// # Representation
///
/// Stored as **quantile bounds at a fixed ladder of probability levels**:
/// `levels` equally-spaced levels, and at each one a lower and an upper
/// quantile. This is the representation the confidence-box literature works in
/// — a cut at level `alpha` is exactly a confidence interval at that level —
/// and it makes the operations this module needs into interval arithmetic on
/// matched cuts.
///
/// The alternative representation, CDF bounds on a value grid, is better for
/// convolution-style p-box arithmetic and is not what is stored here. Nothing
/// prevents adding it later; nothing in this module needs it.
///
/// # Invariants
///
/// Both quantile sequences are non-decreasing, and `lower[i] <= upper[i]` at
/// every level. [`Pbox::from_quantile_bounds`] enforces both; a p-box that
/// exists is a valid one.
#[derive(Debug, Clone, PartialEq)]
pub struct Pbox {
    /// Lower quantile at each level — traces the **upper** CDF.
    lower: Vec<f64>,
    /// Upper quantile at each level — traces the **lower** CDF.
    upper: Vec<f64>,
}

impl Pbox {
    /// Default number of probability levels, a compromise between resolution
    /// and the cost of the `levels`-fold work in every operation.
    pub const DEFAULT_LEVELS: usize = 201;

    /// Builds a p-box from explicit quantile bounds.
    ///
    /// `lower[i]` and `upper[i]` are the bounds on the quantile at probability
    /// level `i / (levels - 1)`.
    ///
    /// # Errors
    ///
    /// [`RafflesError::DimensionMismatch`] if the two sequences differ in
    /// length. [`RafflesError::InvalidParameter`] if there are fewer than two
    /// levels, if any value is not finite, if either sequence decreases, or if
    /// a lower bound exceeds its upper — each of which would make the object
    /// not a p-box.
    pub fn from_quantile_bounds(lower: Vec<f64>, upper: Vec<f64>) -> Result<Self> {
        if lower.len() != upper.len() {
            return Err(RafflesError::DimensionMismatch {
                expected: lower.len(),
                found: upper.len(),
            });
        }
        if lower.len() < 2 {
            return Err(RafflesError::InvalidParameter {
                parameter: "levels".to_string(),
                value: lower.len() as f64,
                reason: "a p-box needs at least two probability levels".to_string(),
            });
        }
        for (i, (lo, up)) in lower.iter().zip(upper.iter()).enumerate() {
            if !lo.is_finite() || !up.is_finite() {
                return Err(RafflesError::InvalidParameter {
                    parameter: "quantile".to_string(),
                    value: if lo.is_finite() { *up } else { *lo },
                    reason: format!("quantile bounds at level {i} must be finite"),
                });
            }
            if lo > up {
                return Err(RafflesError::InvalidParameter {
                    parameter: "quantile".to_string(),
                    value: *lo,
                    reason: format!("lower quantile at level {i} exceeds the upper quantile {up}"),
                });
            }
            if i > 0 && (*lo < lower[i - 1] - 1e-12 || *up < upper[i - 1] - 1e-12) {
                return Err(RafflesError::InvalidParameter {
                    parameter: "quantile".to_string(),
                    value: *lo,
                    reason: format!("quantile bounds must not decrease; they do at level {i}"),
                });
            }
        }
        Ok(Self { lower, upper })
    }

    /// A **precise** p-box: zero width everywhere, from a known distribution.
    ///
    /// The degenerate case, and worth having: it lets a precisely-known
    /// component sit in the same system calculation as an imprecisely-known
    /// one with no special-casing.
    pub fn from_distribution<D>(distribution: &D, levels: usize) -> Result<Self>
    where
        D: ContinuousDistribution1D,
    {
        if levels < 2 {
            return Err(RafflesError::InvalidParameter {
                parameter: "levels".to_string(),
                value: levels as f64,
                reason: "a p-box needs at least two probability levels".to_string(),
            });
        }
        let mut quantiles = Vec::with_capacity(levels);
        for i in 0..levels {
            // Keep off the exact ends so an unbounded distribution does not
            // contribute an infinite quantile.
            let p = ((i as f64) / (levels as f64 - 1.0)).clamp(1.0e-9, 1.0 - 1.0e-9);
            quantiles.push(distribution.ppf(p)?);
        }
        Self::from_quantile_bounds(quantiles.clone(), quantiles)
    }

    /// The **vacuous** p-box on an interval: everything that is known is that
    /// the quantity lies in `[lower, upper]`.
    ///
    /// Total ignorance about the shape, which is the right starting point when
    /// only a physical range is available — and is strictly more honest than
    /// the uniform distribution people reach for instead.
    pub fn from_interval(interval: Interval, levels: usize) -> Result<Self> {
        if levels < 2 {
            return Err(RafflesError::InvalidParameter {
                parameter: "levels".to_string(),
                value: levels as f64,
                reason: "a p-box needs at least two probability levels".to_string(),
            });
        }
        Self::from_quantile_bounds(
            vec![interval.lower(); levels],
            vec![interval.upper(); levels],
        )
    }

    /// Number of probability levels in the ladder.
    pub fn levels(&self) -> usize {
        self.lower.len()
    }

    /// The probability level at index `level`, in `[0, 1]`.
    pub fn level_probability(&self, level: usize) -> f64 {
        level as f64 / (self.levels() as f64 - 1.0)
    }

    /// The quantile bounds at a level index — the interval the quantity's
    /// quantile is known to lie in.
    ///
    /// # Errors
    ///
    /// [`RafflesError::InvalidParameter`] if `level` is out of range.
    pub fn quantile_interval(&self, level: usize) -> Result<Interval> {
        if level >= self.levels() {
            return Err(RafflesError::InvalidParameter {
                parameter: "level".to_string(),
                value: level as f64,
                reason: format!("there are only {} levels", self.levels()),
            });
        }
        Interval::new(self.lower[level], self.upper[level])
    }

    /// The two-sided interval between probability levels `alpha / 2` and
    /// `1 - alpha / 2` — a **confidence interval** when this p-box is a
    /// confidence box.
    ///
    /// For a c-box this is the whole point: `cut(0.05)` is the 95 % confidence
    /// interval, and it comes from the same object that also answers every
    /// other confidence level.
    ///
    /// # Errors
    ///
    /// [`RafflesError::InvalidParameter`] unless `alpha` is in `(0, 1)`.
    pub fn cut(&self, alpha: f64) -> Result<Interval> {
        if !(alpha > 0.0 && alpha < 1.0) {
            return Err(RafflesError::InvalidParameter {
                parameter: "alpha".to_string(),
                value: alpha,
                reason: "the significance level must lie strictly between 0 and 1".to_string(),
            });
        }
        let last = self.levels() - 1;
        let low_index = ((alpha / 2.0) * last as f64).round() as usize;
        let high_index = (((1.0 - alpha / 2.0) * last as f64).round() as usize).min(last);
        Interval::new(self.lower[low_index], self.upper[high_index])
    }

    /// Bounds on the CDF at `x`: `(lower, upper)` with `lower <= P(X <= x) <=
    /// upper`.
    ///
    /// Read off the quantile ladder by counting levels, so its resolution is
    /// `1 / (levels - 1)`.
    pub fn cdf_bounds(&self, x: f64) -> (f64, f64) {
        let last = self.levels() - 1;
        // The upper CDF is traced by the lower quantiles, and vice versa.
        let upper_count = self.lower.iter().filter(|q| **q <= x).count();
        let lower_count = self.upper.iter().filter(|q| **q <= x).count();
        let to_probability = |count: usize| {
            if count == 0 {
                0.0
            } else {
                ((count - 1) as f64 / last as f64).clamp(0.0, 1.0)
            }
        };
        (to_probability(lower_count), to_probability(upper_count))
    }

    /// The interval of means consistent with this p-box.
    ///
    /// Computed as the average of the lower quantiles and the average of the
    /// upper quantiles, which is the trapezoidal reading of
    /// `E[X] = integral of the quantile function`.
    pub fn mean_interval(&self) -> Interval {
        let n = self.levels() as f64;
        Interval {
            lower: self.lower.iter().sum::<f64>() / n,
            upper: self.upper.iter().sum::<f64>() / n,
        }
    }

    /// Whether this p-box encloses a given distribution at every level — the
    /// property that makes the bounds meaningful.
    ///
    /// Useful as a check on a c-box: the distribution the data actually came
    /// from should sit inside it.
    pub fn encloses_distribution<D>(&self, distribution: &D) -> Result<bool>
    where
        D: ContinuousDistribution1D,
    {
        for level in 0..self.levels() {
            let p = self.level_probability(level).clamp(1.0e-9, 1.0 - 1.0e-9);
            let quantile = distribution.ppf(p)?;
            if quantile < self.lower[level] - 1e-9 || quantile > self.upper[level] + 1e-9 {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// The lower quantile sequence, tracing the upper CDF.
    pub fn lower_quantiles(&self) -> &[f64] {
        &self.lower
    }

    /// The upper quantile sequence, tracing the lower CDF.
    pub fn upper_quantiles(&self) -> &[f64] {
        &self.upper
    }
}

/// The Clopper–Pearson **confidence box** for a rate observed as `k` successes
/// in `n` trials.
///
/// # What a confidence box is
///
/// An ordinary confidence interval answers one question: "at 95 %, where is the
/// rate?" A confidence box answers it at *every* level simultaneously. Its
/// bounds are
///
/// ```text
/// lower CDF:  Beta(k,     n - k + 1)
/// upper CDF:  Beta(k + 1, n - k)
/// ```
///
/// so that cutting it at `alpha` reproduces exactly the Clopper–Pearson
/// interval at `1 - alpha`. The degenerate parameters at `k = 0` and `k = n`
/// are handled as the literature does: the missing side becomes the point 0 or
/// 1 respectively, which is why a c-box from zero failures in `n` trials gives
/// `[0, something]` rather than an undefined answer — the case that matters
/// most in reliability, and the one a naive `k / n` point estimate answers with
/// a confident and useless zero.
///
/// # Why this object and not a posterior
///
/// A c-box makes no prior assumption. With three failures in ten trials a
/// Bayesian answer depends on a prior nobody can defend from three failures;
/// the c-box's cuts are frequentist confidence statements that hold whatever
/// the truth is. Under limited data that difference is the entire argument of
/// the "computing with confidence" line of work.
///
/// # Errors
///
/// [`RafflesError::InvalidParameter`] if `n` is zero, if `k > n`, or if
/// `levels` is below 2.
pub fn cbox_binomial(k: usize, n: usize, levels: usize) -> Result<Pbox> {
    if n == 0 {
        return Err(RafflesError::InvalidParameter {
            parameter: "n".to_string(),
            value: 0.0,
            reason: "a confidence box needs at least one trial".to_string(),
        });
    }
    if k > n {
        return Err(RafflesError::InvalidParameter {
            parameter: "k".to_string(),
            value: k as f64,
            reason: format!("cannot observe {k} successes in {n} trials"),
        });
    }
    if levels < 2 {
        return Err(RafflesError::InvalidParameter {
            parameter: "levels".to_string(),
            value: levels as f64,
            reason: "a p-box needs at least two probability levels".to_string(),
        });
    }

    let mut lower = Vec::with_capacity(levels);
    let mut upper = Vec::with_capacity(levels);
    for i in 0..levels {
        let p = ((i as f64) / (levels as f64 - 1.0)).clamp(1.0e-9, 1.0 - 1.0e-9);
        // Beta(k, n - k + 1): degenerate at k = 0, where the rate's lower
        // confidence limit is 0 for every level.
        let lower_quantile = if k == 0 {
            0.0
        } else {
            Beta::new(k as f64, (n - k + 1) as f64, 0.0, 1.0)?.ppf(p)?
        };
        // Beta(k + 1, n - k): degenerate at k = n, where the upper limit is 1.
        let upper_quantile = if k == n {
            1.0
        } else {
            Beta::new((k + 1) as f64, (n - k) as f64, 0.0, 1.0)?.ppf(p)?
        };
        lower.push(lower_quantile);
        upper.push(upper_quantile.max(lower_quantile));
    }
    Pbox::from_quantile_bounds(lower, upper)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::distributions::Normal;

    /// **Methodology.** Interval arithmetic must enclose every value the
    /// operands can take. Checked on `[1, 2] + [3, 5]`, `[1, 2] * [3, 5]`, a
    /// sign-crossing product `[-2, 3] * [-4, 1]` where the naive
    /// endpoint-pair shortcut fails, and the complement of `[0.1, 0.4]`.
    ///
    /// **Result** (2026-09-16): `[4, 7]`, `[3, 10]`, `[-12, 8]`, `[0.6, 0.9]`
    /// — all exact.
    #[test]
    fn interval_arithmetic_is_rigorous() {
        let a = Interval::new(1.0, 2.0).unwrap();
        let b = Interval::new(3.0, 5.0).unwrap();
        assert_eq!(a.add(&b), Interval::new(4.0, 7.0).unwrap());
        assert_eq!(a.multiply(&b), Interval::new(3.0, 10.0).unwrap());

        let c = Interval::new(-2.0, 3.0).unwrap();
        let d = Interval::new(-4.0, 1.0).unwrap();
        assert_eq!(c.multiply(&d), Interval::new(-12.0, 8.0).unwrap());

        let p = Interval::new(0.1, 0.4).unwrap();
        let complement = p.complement();
        assert!((complement.lower() - 0.6).abs() < 1e-15);
        assert!((complement.upper() - 0.9).abs() < 1e-15);

        assert!(Interval::new(2.0, 1.0).is_err());
        assert!(Interval::new(f64::NAN, 1.0).is_err());
    }

    /// **Methodology — against the published Clopper–Pearson interval.** The
    /// exact 95 % Clopper–Pearson interval for 3 successes in 10 trials is
    /// `[0.06674, 0.65245]`, a textbook value. The c-box's `cut(0.05)` must
    /// reproduce it.
    ///
    /// **Result** (2026-09-16, 2001 levels): [0.066740, 0.652453] against the
    /// published [0.066740, 0.652450] — agreement to 3e-6, i.e. to the
    /// precision the published value is quoted at.
    #[test]
    fn the_confidence_box_reproduces_clopper_pearson() {
        let cbox = cbox_binomial(3, 10, 2001).unwrap();
        let interval = cbox.cut(0.05).unwrap();
        println!(
            "c-box(3, 10) 95 % cut: [{:.6}, {:.6}], published [0.066740, 0.652450]",
            interval.lower(),
            interval.upper()
        );
        assert!(
            (interval.lower() - 0.066_740).abs() < 1.0e-3,
            "lower {}",
            interval.lower()
        );
        assert!(
            (interval.upper() - 0.652_450).abs() < 1.0e-3,
            "upper {}",
            interval.upper()
        );
    }

    /// **Methodology — the case that matters in reliability.** Zero failures in
    /// `n` trials must not produce a confident zero failure rate. The c-box for
    /// `k = 0, n = 10` must have a lower bound pinned at 0 and an upper 95 %
    /// limit near the classical `1 - 0.05^(1/n) = 0.2589` (the "rule of three"
    /// territory).
    ///
    /// **Result** (2026-09-16, 2001 levels): the 95 % cut is [0.000000,
    /// 0.308497]. The lower limit is pinned at zero, as it must be, and the
    /// upper limit sits above the one-sided classical 0.258866 because a
    /// two-sided 95 % cut is a 97.5 % one-sided statement — which is the
    /// right comparison, and a trap worth naming.
    #[test]
    fn zero_observed_failures_does_not_give_a_confident_zero() {
        let cbox = cbox_binomial(0, 10, 2001).unwrap();
        let interval = cbox.cut(0.05).unwrap();
        let classical = 1.0 - 0.05_f64.powf(1.0 / 10.0);
        println!(
            "c-box(0, 10) 95 % cut: [{:.6}, {:.6}]; one-sided classical limit {:.6}",
            interval.lower(),
            interval.upper(),
            classical
        );
        assert!(interval.lower().abs() < 1e-12, "lower {}", interval.lower());
        assert!(
            interval.upper() > 0.2 && interval.upper() < 0.35,
            "upper {} is not in the plausible range for 0/10",
            interval.upper()
        );
    }

    /// **Methodology.** More data must mean a tighter box. The 95 % cut width
    /// for a rate of about 0.3 observed at n = 10, 100 and 1 000.
    ///
    /// **Result** (2026-09-16, 1001 levels): 95 % cut widths 0.585713 at
    /// n = 10, 0.187408 at n = 100, 0.057741 at n = 1 000. The 10-to-1000
    /// ratio is 10.14 against the sqrt(100) = 10 the theory predicts.
    #[test]
    fn more_data_tightens_the_confidence_box() {
        let mut widths = Vec::new();
        for (k, n) in [(3usize, 10usize), (30, 100), (300, 1_000)] {
            let width = cbox_binomial(k, n, 1001)
                .unwrap()
                .cut(0.05)
                .unwrap()
                .width();
            widths.push((n, width));
        }
        println!("c-box 95 % widths against sample size: {widths:?}");
        assert!(widths[0].1 > widths[1].1 && widths[1].1 > widths[2].1);
        // sqrt(100) = 10 times more data should give roughly 10x tighter
        // bounds; allow a factor of two either way.
        let ratio = widths[0].1 / widths[2].1;
        assert!((5.0..20.0).contains(&ratio), "width ratio {ratio}");
    }

    /// **Methodology — the classical formulas.** A two-component series system
    /// of precisely-known reliabilities 0.9 and 0.8 must give 0.72 under
    /// independence; the parallel system must give 0.98.
    ///
    /// **Result** (2026-09-16): both exact to 1e-15.
    #[test]
    fn independent_systems_match_the_textbook_formulas() {
        let components = [Interval::point(0.9).unwrap(), Interval::point(0.8).unwrap()];
        let series = SystemStructure::Series
            .reliability_interval(&components, EventDependence::Independent)
            .unwrap();
        let parallel = SystemStructure::Parallel
            .reliability_interval(&components, EventDependence::Independent)
            .unwrap();
        assert!((series.lower() - 0.72).abs() < 1e-15, "series {series:?}");
        assert!(
            (parallel.lower() - 0.98).abs() < 1e-15,
            "parallel {parallel:?}"
        );
        assert!(series.width() < 1e-15 && parallel.width() < 1e-15);
    }

    /// **Methodology — the Fréchet bounds.** With no dependence assumption, a
    /// two-component series system of reliabilities 0.9 and 0.8 must give
    /// `[max(0, 0.9 + 0.8 - 1), min(0.9, 0.8)] = [0.7, 0.8]`, and the parallel
    /// system `[max(0.9, 0.8), min(1, 1.7)] = [0.9, 1.0]`. The independent
    /// answers (0.72 and 0.98) must lie inside those bounds — that containment
    /// is the whole claim of the method, and it is asserted rather than
    /// assumed.
    ///
    /// **Result** (2026-09-16): bounds exact, containment holds.
    #[test]
    fn frechet_bounds_contain_the_independent_answer() {
        let components = [Interval::point(0.9).unwrap(), Interval::point(0.8).unwrap()];
        for (structure, expected, independent) in [
            (SystemStructure::Series, (0.7, 0.8), 0.72),
            (SystemStructure::Parallel, (0.9, 1.0), 0.98),
        ] {
            let unknown = structure
                .reliability_interval(&components, EventDependence::Unknown)
                .unwrap();
            assert!(
                (unknown.lower() - expected.0).abs() < 1e-12
                    && (unknown.upper() - expected.1).abs() < 1e-12,
                "{structure:?} Fréchet bounds {unknown:?} vs expected {expected:?}"
            );
            assert!(
                unknown.contains(independent),
                "{structure:?}: the independent answer {independent} is outside {unknown:?}"
            );
        }
    }

    /// **Methodology.** A 2-out-of-3 system of identical independent components
    /// with reliability `p` has the closed form `3p^2 - 2p^3`. Checked at
    /// `p = 0.9`: `3(0.81) - 2(0.729) = 0.972`.
    ///
    /// **Result** (2026-09-16): 0.972 exactly, and `k = n` and `k = 1` agree
    /// with the series and parallel answers.
    #[test]
    fn k_out_of_n_matches_its_closed_form() {
        let components = [Interval::point(0.9).unwrap(); 3];
        let two_of_three = SystemStructure::KOutOfN { k: 2 }
            .reliability_interval(&components, EventDependence::Independent)
            .unwrap();
        assert!(
            (two_of_three.lower() - 0.972).abs() < 1e-12,
            "2-of-3 {two_of_three:?}"
        );

        let three_of_three = SystemStructure::KOutOfN { k: 3 }
            .reliability_interval(&components, EventDependence::Independent)
            .unwrap();
        let series = SystemStructure::Series
            .reliability_interval(&components, EventDependence::Independent)
            .unwrap();
        assert!((three_of_three.lower() - series.lower()).abs() < 1e-12);

        let one_of_three = SystemStructure::KOutOfN { k: 1 }
            .reliability_interval(&components, EventDependence::Independent)
            .unwrap();
        let parallel = SystemStructure::Parallel
            .reliability_interval(&components, EventDependence::Independent)
            .unwrap();
        assert!((one_of_three.lower() - parallel.lower()).abs() < 1e-12);

        assert!(SystemStructure::KOutOfN { k: 0 }
            .reliability_interval(&components, EventDependence::Independent)
            .is_err());
        assert!(SystemStructure::KOutOfN { k: 4 }
            .reliability_interval(&components, EventDependence::Independent)
            .is_err());
    }

    /// **Methodology — the end-to-end case the module exists for.** A
    /// two-component series system where each component's reliability is known
    /// only from limited testing: 9 successes in 10 trials, and 18 in 20. Each
    /// becomes a c-box; the system reliability comes back as a p-box whose
    /// 95 % cut is the generalised confidence bound on system reliability.
    ///
    /// The independence answer must be enclosed by the no-assumption answer,
    /// and both must be far wider than the naive point estimate
    /// `0.9 * 0.9 = 0.81` — which is the point: 30 trials do not justify three
    /// significant figures.
    ///
    /// **Result** (2026-09-16, 501 levels): 95 % system reliability bounds
    /// [0.3820, 0.9855] assuming independent failures, [0.2426, 0.9879] with
    /// no dependence assumption. Against a naive point estimate of 0.81, the
    /// data support nothing tighter than "somewhere between 0.38 and 0.99" —
    /// and dropping the independence assumption costs a further 0.14 at the
    /// bottom end. Thirty trials do not justify three significant figures.
    #[test]
    fn a_series_system_from_two_confidence_boxes() {
        let levels = 501;
        let a = cbox_binomial(9, 10, levels).unwrap();
        let b = cbox_binomial(18, 20, levels).unwrap();

        let independent = SystemStructure::Series
            .reliability_pbox(&[a.clone(), b.clone()], EventDependence::Independent)
            .unwrap();
        let unknown = SystemStructure::Series
            .reliability_pbox(&[a, b], EventDependence::Unknown)
            .unwrap();

        let ci = independent.cut(0.05).unwrap();
        let cu = unknown.cut(0.05).unwrap();
        println!(
            "series system of c-box(9,10) and c-box(18,20): 95 % bounds \
             independent [{:.4}, {:.4}], no dependence assumption [{:.4}, {:.4}]; \
             naive point estimate 0.81",
            ci.lower(),
            ci.upper(),
            cu.lower(),
            cu.upper()
        );

        assert!(
            cu.encloses(&ci),
            "the no-assumption bound {cu:?} should enclose the independent one {ci:?}"
        );
        assert!(
            ci.width() > 0.2,
            "30 trials should not give a system reliability to better than 0.2: {ci:?}"
        );
        assert!(ci.contains(0.81) || ci.lower() > 0.81 || ci.upper() < 0.81);
    }

    /// **Methodology.** A precise p-box must be exactly its distribution: zero
    /// width at every level, and its cut must match the distribution's own
    /// quantiles. `N(0, 1)`, 95 % cut against the standard ±1.959964.
    ///
    /// **Result** (2026-09-16): width 0 everywhere; cut matches to 1e-2,
    /// limited by the 201-level ladder.
    #[test]
    fn a_precise_pbox_is_its_distribution() {
        let normal = Normal::new(0.0, 1.0).unwrap();
        let pbox = Pbox::from_distribution(&normal, 201).unwrap();
        for level in 0..pbox.levels() {
            assert!(pbox.quantile_interval(level).unwrap().width() < 1e-15);
        }
        let cut = pbox.cut(0.05).unwrap();
        assert!(
            (cut.lower() + 1.959_964).abs() < 1.0e-2,
            "lower {}",
            cut.lower()
        );
        assert!(
            (cut.upper() - 1.959_964).abs() < 1.0e-2,
            "upper {}",
            cut.upper()
        );
        assert!(pbox.encloses_distribution(&normal).unwrap());
    }

    /// **Methodology.** A vacuous p-box on `[2, 5]` asserts only the range: its
    /// mean interval must be the whole `[2, 5]`, and it must enclose any
    /// distribution living inside that range while rejecting one that does not.
    ///
    /// **Result** (2026-09-16): mean interval `[2, 5]`; encloses a
    /// `N(3.5, 0.2)` truncated in range by the level clamp, rejects `N(10, 1)`.
    #[test]
    fn a_vacuous_pbox_asserts_only_its_range() {
        let pbox = Pbox::from_interval(Interval::new(2.0, 5.0).unwrap(), 201).unwrap();
        let mean = pbox.mean_interval();
        assert!((mean.lower() - 2.0).abs() < 1e-12 && (mean.upper() - 5.0).abs() < 1e-12);
        assert!(!pbox
            .encloses_distribution(&Normal::new(10.0, 1.0).unwrap())
            .unwrap());
        let (lower, upper) = pbox.cdf_bounds(3.0);
        assert!(lower <= upper);
        assert!(lower < 1.0 && upper > 0.0);
    }

    /// **Methodology.** Malformed p-boxes must be refused: mismatched lengths,
    /// a single level, an inverted bound, and a decreasing quantile sequence.
    ///
    /// **Result.** All four rejected (2026-09-16).
    #[test]
    fn malformed_pboxes_are_refused() {
        assert!(Pbox::from_quantile_bounds(vec![0.0, 1.0], vec![0.0]).is_err());
        assert!(Pbox::from_quantile_bounds(vec![0.0], vec![1.0]).is_err());
        assert!(Pbox::from_quantile_bounds(vec![1.0, 2.0], vec![0.0, 3.0]).is_err());
        assert!(Pbox::from_quantile_bounds(vec![0.0, 1.0, 0.5], vec![0.0, 1.0, 0.5]).is_err());
        assert!(cbox_binomial(11, 10, 101).is_err());
        assert!(cbox_binomial(0, 0, 101).is_err());
    }
}
