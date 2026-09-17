// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from OFFBEAT (https://gitlab.com/foam-for-nuclear/offbeat),
// upstream commit 80e84450a115b0c411e1bfa5d166379f6bf6c084, GPL-3.0.
// Corresponds to upstream
// `offbeatLib/rheology/constitutiveLaws/yieldStressModels/`
// (`yieldStressModel.C/H`, `yieldStressConstant.C/H`, `hardening.C/H`,
// `yieldStressFRAPTRAN.C/H`).
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! How strong the material is, and how much stronger yielding makes it.
//!
//! # What a yield-stress model is for
//!
//! A von Mises plasticity model needs one scalar: the stress `σ_y` at which the
//! material starts to flow irreversibly. For a real metal that scalar is not a
//! constant. It rises as the material yields (**work hardening**), it falls as
//! the material gets hotter, and for cladding it rises with accumulated fast
//! fluence (**irradiation hardening** — displacement damage pins dislocations)
//! and with the cold work left over from fabrication.
//!
//! A model here answers two questions at a given accumulated equivalent plastic
//! strain `ε_p,eq`:
//!
//! - [`yield_stress`](YieldStressModel::yield_stress) — what is `σ_y` now?
//! - [`hardening_modulus`](YieldStressModel::hardening_modulus) — what is
//!   `H = dσ_y/dε_p,eq`, the slope of the hardening curve?
//!
//! The slope is what makes the plastic return mapping solvable in one Newton
//! step for a linear curve, and quadratically convergent for a curved one.

use crate::error::{OffbeatError, Result};
use crate::mechanics::LinearElastic;

use super::state::RheologyInputs;

/// A piecewise-linear hardening curve: accumulated equivalent plastic strain
/// against yield stress.
///
/// Corresponds to upstream's `plasticStrainVsYieldStress` OpenFOAM `Table`,
/// read by `hardening.C`. Outside the tabulated range the curve is held flat at
/// the first/last value rather than extrapolated — extrapolating a measured
/// stress–strain curve past its last point is how a code invents strength that
/// the material does not have.
///
/// # Units
///
/// Abscissa: equivalent plastic strain \[-\], non-negative, strictly
/// increasing. Ordinate: yield stress \[Pa\], positive.
#[derive(Debug, Clone, PartialEq)]
pub struct HardeningCurve {
    points: Vec<(f64, f64)>,
}

impl HardeningCurve {
    /// Build from `(equivalent plastic strain [-], yield stress [Pa])` pairs.
    ///
    /// The points are sorted by strain on construction, so the caller need not
    /// supply them in order.
    ///
    /// # Errors
    ///
    /// [`OffbeatError::Unphysical`] if the curve is empty, if any strain is
    /// negative, if any yield stress is non-positive, or if two points share
    /// the same strain (which would make the slope undefined).
    ///
    /// ```
    /// use outram_park_fork_offbeat::rheology::HardeningCurve;
    ///
    /// // Linear hardening: 300 MPa at first yield, +1 GPa per unit plastic strain.
    /// let c = HardeningCurve::new(vec![(0.0, 300.0e6), (0.1, 400.0e6)]).unwrap();
    /// assert!((c.yield_stress(0.05) - 350.0e6).abs() < 1.0);
    /// assert!((c.slope(0.05) - 1.0e9).abs() < 1.0);
    ///
    /// // Held flat past the last point, not extrapolated.
    /// assert!((c.yield_stress(10.0) - 400.0e6).abs() < 1.0);
    /// assert_eq!(c.slope(10.0), 0.0);
    /// ```
    pub fn new(points: Vec<(f64, f64)>) -> Result<Self> {
        if points.is_empty() {
            return Err(OffbeatError::Unphysical {
                quantity: "hardening curve",
                value: 0.0,
                unit: "points",
                reason: "a hardening curve needs at least one point",
            });
        }
        let mut points = points;
        points.sort_by(|a, b| a.0.total_cmp(&b.0));
        for &(strain, stress) in &points {
            if !(strain >= 0.0) {
                return Err(OffbeatError::Unphysical {
                    quantity: "hardening-curve plastic strain",
                    value: strain,
                    unit: "-",
                    reason: "accumulated equivalent plastic strain cannot be negative",
                });
            }
            if !(stress > 0.0) {
                return Err(OffbeatError::Unphysical {
                    quantity: "hardening-curve yield stress",
                    value: stress,
                    unit: "Pa",
                    reason: "must be strictly positive",
                });
            }
        }
        for pair in points.windows(2) {
            if pair[0].0 == pair[1].0 {
                return Err(OffbeatError::Unphysical {
                    quantity: "hardening-curve plastic strain",
                    value: pair[0].0,
                    unit: "-",
                    reason: "two points share the same strain, so the hardening slope \
                             would be undefined there",
                });
            }
        }
        Ok(Self { points })
    }

    /// Yield stress \[Pa\] at an accumulated equivalent plastic strain \[-\].
    ///
    /// Linear between tabulated points, flat outside the table.
    #[must_use]
    pub fn yield_stress(&self, eq_plastic_strain: f64) -> f64 {
        let p = &self.points;
        if eq_plastic_strain <= p[0].0 {
            return p[0].1;
        }
        if eq_plastic_strain >= p[p.len() - 1].0 {
            return p[p.len() - 1].1;
        }
        for pair in p.windows(2) {
            let (x0, y0) = pair[0];
            let (x1, y1) = pair[1];
            if eq_plastic_strain <= x1 {
                let t = (eq_plastic_strain - x0) / (x1 - x0);
                return y0 + t * (y1 - y0);
            }
        }
        p[p.len() - 1].1
    }

    /// Hardening slope `dσ_y/dε_p,eq` \[Pa\] at an accumulated equivalent
    /// plastic strain \[-\]. Zero outside the tabulated range.
    #[must_use]
    pub fn slope(&self, eq_plastic_strain: f64) -> f64 {
        let p = &self.points;
        if p.len() < 2 || eq_plastic_strain < p[0].0 || eq_plastic_strain >= p[p.len() - 1].0 {
            return 0.0;
        }
        for pair in p.windows(2) {
            let (x0, y0) = pair[0];
            let (x1, y1) = pair[1];
            if eq_plastic_strain < x1 {
                return (y1 - y0) / (x1 - x0);
            }
        }
        0.0
    }
}

/// How the yield stress of a material is determined.
///
/// Enum dispatch, per the workspace rule: the set of yield models is closed and
/// known at compile time, so adding one is a compile error at every match site
/// rather than a runtime surprise.
///
/// Corresponds to upstream's runtime-selectable `yieldStressModel` hierarchy.
#[derive(Debug, Clone, PartialEq)]
pub enum YieldStressModel {
    /// A fixed yield stress, perfectly plastic (no hardening).
    ///
    /// Upstream `yieldStressConstant`. The right choice for a verification case
    /// with a closed-form answer, and a defensible first cut for a material
    /// whose hardening curve is unknown.
    Constant {
        /// Yield stress `σ_y` \[Pa\]. Must be positive.
        sigma_y: f64,
    },

    /// A tabulated stress/plastic-strain curve, interpolated linearly.
    ///
    /// Upstream `hardening`. The general-purpose choice when a measured
    /// stress–strain curve is available.
    Hardening {
        /// The measured curve.
        curve: HardeningCurve,
    },

    /// The FRAPTRAN Zircaloy strength correlation.
    ///
    /// Upstream `yieldStressFRAPTRAN`. A power-law flow curve
    /// `σ = K ε^n (ε̇ / 1e-3)^m` in which all three coefficients depend on
    /// temperature, and `K` and `n` additionally on fast fluence and cold work.
    /// The yield point proper is where that flow curve crosses the elastic line
    /// `σ = E ε`:
    ///
    /// `σ_y0 = (K / E^n · (ε̇/1e-3)^m)^(1/(1−n))`
    ///
    /// and beyond it the flow stress `K ε_p,eq^n (ε̇/1e-3)^m` takes over.
    ///
    /// # Validity
    ///
    /// Fitted for Zircaloy-2/-4 cladding over roughly 300–2100 K. The `K`
    /// correlation has no branch above 2100 K and upstream silently leaves the
    /// previous value in place there; this port returns
    /// [`OffbeatError::OutOfRange`] instead.
    ///
    /// # Deliberate deviations from upstream
    ///
    /// - Upstream evaluates the flow-stress branch at the **total** equivalent
    ///   deviatoric strain and derives its hardening modulus from the current
    ///   equivalent stress. This port evaluates it at the **accumulated
    ///   equivalent plastic strain**, which is the standard isotropic-hardening
    ///   form and the only one consistent with the return mapping that consumes
    ///   it. Using total strain in a plastic hardening rule double-counts the
    ///   elastic part.
    /// - Upstream's `correctYieldStress` executes `phiValue_ *= 1e25;` on the
    ///   member variable at every call, so the fluence compounds by a factor of
    ///   1e25 per call. That is a defect; this port takes the fluence in SI
    ///   n/m² from [`MaterialState`](crate::materials::MaterialState) and never
    ///   mutates it.
    Fraptran,
}

impl YieldStressModel {
    /// Yield stress `σ_y` \[Pa\] at an accumulated equivalent plastic strain.
    ///
    /// `eq_plastic_strain` is the accumulated equivalent plastic strain \[-\],
    /// non-negative. `inputs` supplies temperature, fast fluence, cold work,
    /// the elastic constants and the strain rate — only
    /// [`Fraptran`](Self::Fraptran) reads them.
    ///
    /// # Errors
    ///
    /// [`OffbeatError::Unphysical`] for a non-positive `Constant` yield stress
    /// or a non-positive temperature; [`OffbeatError::OutOfRange`] when the
    /// FRAPTRAN correlation is evaluated outside the temperature range it is
    /// fitted over.
    pub fn yield_stress(&self, eq_plastic_strain: f64, inputs: &RheologyInputs) -> Result<f64> {
        match self {
            Self::Constant { sigma_y } => {
                if !(*sigma_y > 0.0) {
                    return Err(OffbeatError::Unphysical {
                        quantity: "yield stress",
                        value: *sigma_y,
                        unit: "Pa",
                        reason: "must be strictly positive",
                    });
                }
                Ok(*sigma_y)
            }
            Self::Hardening { curve } => Ok(curve.yield_stress(eq_plastic_strain.max(0.0))),
            Self::Fraptran => {
                let f = FraptranCoefficients::new(inputs)?;
                Ok(f.yield_stress(eq_plastic_strain.max(0.0), inputs.elastic))
            }
        }
    }

    /// Hardening modulus `H = dσ_y/dε_p,eq` \[Pa\].
    ///
    /// Zero for a perfectly plastic material. Positive for a hardening one.
    /// This is upstream's `Hp` field, and it enters the return mapping as the
    /// slope that makes the consistency condition solvable.
    ///
    /// # Errors
    ///
    /// Same conditions as [`yield_stress`](Self::yield_stress).
    pub fn hardening_modulus(
        &self,
        eq_plastic_strain: f64,
        inputs: &RheologyInputs,
    ) -> Result<f64> {
        match self {
            Self::Constant { .. } => Ok(0.0),
            Self::Hardening { curve } => Ok(curve.slope(eq_plastic_strain.max(0.0))),
            Self::Fraptran => {
                let f = FraptranCoefficients::new(inputs)?;
                Ok(f.hardening_modulus(eq_plastic_strain.max(0.0), inputs.elastic))
            }
        }
    }
}

/// The three temperature-, fluence- and cold-work-dependent coefficients of the
/// FRAPTRAN Zircaloy flow curve `σ = K ε^n (ε̇ / 1e-3)^m`.
///
/// Split out so the piecewise correlations are read once, in one place, rather
/// than duplicated between the stress and the slope. Not public: it is an
/// implementation detail of [`YieldStressModel::Fraptran`].
#[derive(Debug, Clone, Copy)]
struct FraptranCoefficients {
    /// Strength coefficient `K` \[Pa\].
    k: f64,
    /// Strain-hardening exponent `n` \[-\], in `(0, 1)`.
    n: f64,
    /// Strain-rate sensitivity exponent `m` \[-\].
    m: f64,
    /// Strain-rate factor `(ε̇ / 1e-3)^m` \[-\].
    rate_factor: f64,
}

impl FraptranCoefficients {
    /// Evaluate all coefficients for one cell.
    ///
    /// Reads temperature, fast fluence (n/m², E > 1 MeV) and cold work from
    /// [`RheologyInputs::material`], and the strain rate from
    /// [`RheologyInputs::equivalent_strain_rate`], floored at 1e-3 /s exactly
    /// as upstream does.
    fn new(inputs: &RheologyInputs) -> Result<Self> {
        let t = inputs.material.temperature;
        if !(t > 0.0) {
            return Err(OffbeatError::Unphysical {
                quantity: "temperature",
                value: t,
                unit: "K",
                reason: "absolute temperature must be strictly positive",
            });
        }
        if t >= 2100.0 {
            return Err(OffbeatError::OutOfRange {
                quantity: "FRAPTRAN Zircaloy strength coefficient K",
                value: t,
                low: 0.0,
                high: 2100.0,
                unit: "K",
            });
        }

        // Fast fluence, n/m² with E > 1 MeV. Upstream calls this `phi` and
        // reads it from the dictionary in units of 1e25 n/m²; we take SI.
        let phi = inputs.material.fast_fluence.max(0.0);
        let cw = inputs.material.cold_work.max(0.0);

        // Strain-rate sensitivity exponent m(T).
        let m = if t < 750.0 {
            0.015
        } else if t <= 800.0 {
            7.458e-4 * t - 0.544_338
        } else {
            3.241_24e-4 * t - 0.207_01
        };

        // Strain-hardening exponent n(T), then the fluence multiplier.
        let mut n = if t < 419.4 {
            0.114_05
        } else if t <= 1099.0722 {
            -9.490e-2 + 1.165e-3 * t - 1.992e-6 * t.powi(2) + 9.558e-10 * t.powi(3)
        } else if t <= 1600.0 {
            -0.226_551_19 + 2.5e-4 * t
        } else {
            0.173_448_80
        };
        n *= if phi < 0.1e25 {
            1.321 + 0.48e-25 * phi
        } else if phi < 2.0e25 {
            1.369 + 0.096e-25 * phi
        } else if phi < 7.5e25 {
            1.5435 + 0.008_727e-25 * phi
        } else {
            1.608_953
        };
        // The flow curve is only invertible for n < 1; the correlation never
        // approaches that, but a caller who has perturbed the constants would
        // otherwise get a silent sign flip in the 1/(1-n) exponent.
        let n = n.clamp(1.0e-6, 0.95);

        // Strength coefficient K(T) [Pa], then cold-work and fluence factors.
        let mut k = if t < 750.0 {
            1.176_28e9 + 4.548_59e5 * t - 3.281_85e3 * t.powi(2) + 1.727_52 * t.powi(3)
        } else if t < 1090.0 {
            2.522_488e6 * (2.850_002_7e6 / t.powi(2)).exp()
        } else if t < 1255.0 {
            1.841_376_039e8 - 1.434_544_8e5 * t
        } else {
            4.330e7 - 6.685e4 * t + 37.579 * t.powi(2) - 7.33e-3 * t.powi(3)
        };
        let k_cw = 0.546 * cw;
        // NOTE — deliberate correction of an upstream defect. Upstream's
        // `yieldStressFRAPTRAN.C` writes the second and third branches of this
        // chain as `phi <= 0.1e25 && phi < 2e25` and `phi <= 2e25 && phi <
        // 12e25`, which (after the first branch has already excluded
        // `phi < 0.1e25`) can only be satisfied at the exact boundary values.
        // The consequence is that for any fluence in (0.1e25, 12e25) n/m² —
        // i.e. essentially all irradiated cladding — no fluence factor is
        // applied at all, and K is silently left at its unirradiated value.
        // This port uses the evidently intended `>=` comparisons.
        let k_phi = if phi < 0.1e25 {
            -0.1461
                + 1.464e-25
                    * phi
                    * ((2.25 * (-20.0 * cw).exp() * (1.0f64).min(((t - 550.0) / 10.0).exp())) + 1.0)
        } else if phi < 2.0e25 {
            2.928e-26 * phi
        } else if phi < 12.0e25 {
            0.532_36 + 2.6618e-27 * phi
        } else {
            // Above 12e25 n/m² the correlation has no branch; hold the value at
            // the top of the fitted range rather than extrapolate.
            0.532_36 + 2.6618e-27 * 12.0e25
        };
        k *= 1.0 + k_cw + k_phi;
        let k = k.max(1.0);

        let rate = inputs.equivalent_strain_rate.max(1.0e-3);
        let rate_factor = (rate / 1.0e-3).powf(m);

        Ok(Self {
            k,
            n,
            m,
            rate_factor,
        })
    }

    /// Yield stress \[Pa\] at an accumulated equivalent plastic strain.
    ///
    /// `max(σ_y0, K ε_p,eq^n (ε̇/1e-3)^m)` where `σ_y0` is the intersection of
    /// the flow curve with the elastic line.
    fn yield_stress(&self, eq_plastic_strain: f64, elastic: LinearElastic) -> f64 {
        let sigma_y0 =
            (self.k / elastic.young.powf(self.n) * self.rate_factor).powf(1.0 / (1.0 - self.n));
        let flow = self.k * eq_plastic_strain.powf(self.n) * self.rate_factor;
        sigma_y0.max(flow)
    }

    /// Hardening slope \[Pa\], the derivative of the flow-stress branch where
    /// that branch is active, and zero on the initial-yield plateau.
    fn hardening_modulus(&self, eq_plastic_strain: f64, elastic: LinearElastic) -> f64 {
        let sigma_y0 =
            (self.k / elastic.young.powf(self.n) * self.rate_factor).powf(1.0 / (1.0 - self.n));
        let flow = self.k * eq_plastic_strain.powf(self.n) * self.rate_factor;
        if flow <= sigma_y0 || eq_plastic_strain <= 0.0 {
            return 0.0;
        }
        let slope = self.n * self.k * eq_plastic_strain.powf(self.n - 1.0) * self.rate_factor;
        if slope.is_finite() {
            slope.max(0.0)
        } else {
            0.0
        }
    }

    /// The strain-rate sensitivity exponent `m` \[-\], kept for diagnostics.
    #[allow(dead_code)]
    fn strain_rate_exponent(&self) -> f64 {
        self.m
    }
}
