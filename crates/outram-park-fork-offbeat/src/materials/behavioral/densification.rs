// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from OFFBEAT (https://gitlab.com/foam-for-nuclear/offbeat),
// upstream commit 80e84450a115b0c411e1bfa5d166379f6bf6c084, GPL-3.0.
// Corresponds to upstream
// `offbeatLib/materials/materialModel/behavioralModels/densification/`:
//   densificationModel.{C,H}      -> DensificationModel::Zero
//   densificationEmpirical.{C,H}  -> DensificationModel::Empirical
//   densificationFRAPCON.{C,H}    -> DensificationModel::Uo2Frapcon
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! Densification models — early-life **shrinkage**, volumetric strain \[-\].
//!
//! # What densification is
//!
//! A fresh UO2 pellet is not fully dense: it is pressed and sintered to
//! typically 95% of theoretical density, and the missing 5% is fine porosity
//! left over from fabrication. Under irradiation, fission fragments knock atoms
//! across the small pores and the pores disappear — in-pile sintering. The
//! pellet therefore **shrinks** over roughly the first 5–10 MWd/kgHM, then
//! stops: the process saturates once the fine porosity is gone, and from then
//! on [swelling](super::swelling) takes over and the pellet grows again.
//!
//! Densification matters because it happens *first*. It opens the fuel/cladding
//! gap at the beginning of life, which raises the gap's thermal resistance and
//! so raises fuel temperature — exactly when the rod is at its highest power.
//!
//! # SIGN CONVENTION — densification is NEGATIVE
//!
//! **Every model in this module returns a negative number**, or zero.
//! [`SwellingModel`](super::swelling::SwellingModel) returns a **positive**
//! number for the same fuel growing. The caller sums the two, so a sign error
//! here does not crash anything — it silently cancels part of the swelling and
//! gives a gap that closes at the wrong time. The unit tests at the bottom of
//! this file assert the sign explicitly, for exactly that reason.
//!
//! # Units, and the volumetric/linear factor of three
//!
//! - [`DensificationModel::value`] returns the **volumetric** strain `ΔV/V`
//!   \[-\], matching [`MaterialState::densification`].
//! - [`DensificationModel::linear`] returns the one-dimensional strain
//!   `ΔL/L = value / 3`, which is **exactly upstream's `epsilonDensification`
//!   field**. Upstream's variable is literally named `dLOverL_max`. If you are
//!   comparing this port against an OFFBEAT run, compare
//!   [`linear`](DensificationModel::linear).
//!
//! Burnup arrives as **MWd/kgHM** ([`MaterialState::burnup`]); temperature as
//! **K**. Upstream reads burnup off the mesh in MWd/tUO2 and converts locally
//! (`/1000/0.881` for the FRAPCON model, `/0.881` for the empirical one); this
//! port receives heavy-metal burnup directly, so those conversions collapse to
//! a single factor of 1000 where the correlation wants MWd/tHM.
//!
//! # Validity ranges: `value` clamps, `value_checked` refuses
//!
//! [`value`](DensificationModel::value) **clamps** burnup and temperature to
//! the endpoints of the variant's stated validity range before evaluating.
//! [`value_checked`](DensificationModel::value_checked) returns
//! [`OffbeatError::OutOfRange`] instead, and additionally rejects a degenerate
//! parameter set with [`OffbeatError::Unphysical`]. Upstream clamps nothing —
//! it extrapolates — so this port and upstream deliberately disagree outside
//! the range.
//!
//! The ranges are **this port's stated applicability**, not upstream constants:
//! upstream declares none. Each variant says what its range is.
//!
//! # Status
//!
//! AI-assisted translation, reviewed by no human yet. Per `RESPONSIBLE_USE.md`
//! this is untrusted draft material: the tests below establish internal
//! consistency with upstream's algebra, **not** validation against measured
//! densification data.
//!
//! [`MaterialState::burnup`]: crate::materials::MaterialState::burnup
//! [`MaterialState::densification`]: crate::materials::MaterialState::densification
//! [`OffbeatError::OutOfRange`]: crate::error::OffbeatError::OutOfRange
//! [`OffbeatError::Unphysical`]: crate::error::OffbeatError::Unphysical

use crate::error::{OffbeatError, Result};
use crate::materials::MaterialState;

/// Fuel densification correlations — **negative** volumetric strain \[-\] for
/// shrinking fuel.
///
/// One variant per model compiled by upstream OFFBEAT's
/// `behavioralModels/densification/`; each variant's documentation names the
/// upstream class and its `TypeName` (the string a user writes in
/// `solverDict`), so a case file can be translated variant by variant.
///
/// Dispatch is by `match`, never by a trait object, per the workspace
/// `CLAUDE.md` "No trait objects" rule.
///
/// # Sign convention
///
/// **Negative is shrinkage, and every variant is negative or zero.** See the
/// [module documentation](self) — this matters because swelling returns a
/// positive number for the same fuel and the two are summed.
///
/// # Densification does not recover
///
/// Upstream keeps `min(new, old)` each timestep, so the field never becomes
/// less negative even if the correlation would allow it. These variants are
/// pure functions and hold no history; because all three are monotonically
/// decreasing in burnup at fixed temperature, evaluating them afresh gives the
/// same answer as upstream's ratchet on a monotonically increasing burnup
/// history. On a history where the *temperature* falls (which can make the
/// FRAPCON asymptote less negative) they differ, and the caller wanting
/// upstream's exact behaviour must apply the `min` itself.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DensificationModel {
    /// No densification at all — upstream `densificationModel`,
    /// `TypeName("none")`.
    ///
    /// Selecting this upstream still creates the `epsilonDensification` field
    /// and leaves it at zero. Returns exactly `0.0` at every state; no validity
    /// range.
    Zero,

    /// Exponential approach to a user-specified final density —
    /// upstream `densificationEmpirical`, `TypeName("empirical")`.
    ///
    /// The simplest defensible form: the fuel relaxes exponentially in burnup
    /// towards a final density the user states from their own resintering
    /// measurement.
    ///
    /// `ΔV/V(Bu) = ΔV/V_max · (1 − exp(−Bu·1000 / saturation_burnup))`
    ///
    /// with `Bu` in MWd/kgHM (so `Bu·1000` is MWd/tHM, the unit
    /// `saturation_burnup` is quoted in), and
    ///
    /// `ΔV/V_max = −density_change / (density_fraction·100 + density_change)`
    ///
    /// which is minus the fractional density increase, i.e. the volume the fuel
    /// loses once all the sinterable porosity has gone. Upstream computes this
    /// divided by three, because it stores the linear strain.
    ///
    /// With the upstream example values (`density_fraction = 0.95`,
    /// `density_change = 0.5` %TD, `saturation_burnup = 1000` MWd/tHM) the
    /// asymptote is `−5.236e-3` volumetric (`−1.745e-3` linear) and it is 63%
    /// complete at 1 MWd/kgHM.
    ///
    /// Valid range: burnup `0` to `120` MWd/kgHM. No temperature dependence at
    /// all — this variant is temperature-blind, which is its main limitation
    /// against [`Uo2Frapcon`](Self::Uo2Frapcon).
    Empirical {
        /// As-fabricated fuel density as a fraction of theoretical \[-\];
        /// 0.95 for typical LWR UO2. Upstream's `densityFraction`.
        ///
        /// This is a fabrication parameter, deliberately **not** taken from
        /// [`MaterialState::porosity`](crate::materials::MaterialState::porosity):
        /// the correlation is anchored on the as-fabricated density, whereas
        /// the state's porosity evolves in service.
        density_fraction: f64,
        /// Final density increase \[%TD\] once densification is complete —
        /// upstream's `densificationDensityChange`. **Positive** for a
        /// densifying fuel (0.5 means the density rises by 0.5 percentage
        /// points of theoretical); the sign flip to a negative strain happens
        /// inside the correlation.
        density_change: f64,
        /// Burnup constant \[MWd/tHM\] of the exponential —
        /// upstream's `densificationTimeConstant`. Densification is 63%
        /// complete at this burnup and 95% complete at three times it. Must be
        /// strictly positive.
        saturation_burnup: f64,
    },

    /// UO2 densification, FRAPCON form — upstream `densificationFRAPCON`,
    /// `TypeName("UO2FRAPCON")`.
    ///
    /// A two-exponential decay in burnup towards a temperature-dependent
    /// asymptote:
    ///
    /// `ΔL/L(Bu) = ΔL/L_max + exp(−3(Bu + B)) + 2·exp(−35(Bu + B))`
    ///
    /// with `Bu` in MWd/kgHM. The fast term (decay constant 35) is the rapid
    /// removal of the finest pores in the first fraction of a MWd/kgHM; the
    /// slow term (decay constant 3) is the tail. `B` is not a free parameter —
    /// it is solved for so that `ΔL/L(0) = 0` exactly, i.e. so the correlation
    /// starts from the as-fabricated state. This port solves it with upstream's
    /// own fixed-point iteration,
    /// `B ← −ln(−2·exp(−35B) − ΔL/L_max) / 3`, to a tolerance of 1e-6 in at
    /// most 10 iterations. `ΔV/V` is three times `ΔL/L`.
    ///
    /// # The asymptote, and the resintering switch
    ///
    /// `ΔL/L_max` depends on temperature, and on whether the user has supplied
    /// a measured resintering density change:
    ///
    /// - **`resintering_density_change > 0`** — the measured route:
    ///   `ΔL/L_max = −0.0015 · r · 109.6 / 100` below 950 K and
    ///   `−0.00285 · r · 109.6 / 100` above 1050 K, linearly interpolated in
    ///   between (`r` is the resintering density change in %TD; `109.6` is
    ///   upstream's `10960/100`, the UO2 theoretical density folded into the
    ///   percentage). Hotter fuel densifies about 1.9 times as much, because
    ///   thermal sintering assists the irradiation-driven process.
    /// - **`resintering_density_change == 0`** — the fallback route, from the
    ///   as-fabricated porosity alone:
    ///   `ΔL/L_max = −22.2 · (1 − density_fraction) / (T_sinter − 1453)` below
    ///   950 K and `−66.6 · (1 − density_fraction) / (T_sinter − 1453)` above
    ///   1050 K — a factor of exactly three between the two.
    ///
    /// # Two upstream defects, reproduced deliberately
    ///
    /// 1. **The 950–1050 K interpolation is a no-op on the fallback route.**
    ///    Upstream computes both interpolation endpoints with `par1` (22.2),
    ///    so the "smooth transition" returns the *low*-temperature value across
    ///    the whole window and then jumps by a factor of three at 1050 K. This
    ///    port reproduces that exactly, because changing it would silently
    ///    disagree with any OFFBEAT result being compared against. It is
    ///    checked by a unit test below, which exists to document the defect
    ///    rather than to endorse it.
    /// 2. **A dead assignment.** Upstream computes
    ///    `−r/(density_fraction·100 + r)/3` on the measured route and then
    ///    overwrites it in all three temperature branches without using it.
    ///    Not ported — it has no effect.
    ///
    /// Valid range: burnup `0` to `120` MWd/kgHM; temperature `300` to
    /// `2000` K.
    Uo2Frapcon {
        /// As-fabricated fuel density as a fraction of theoretical \[-\];
        /// 0.95 for typical LWR UO2. Upstream's `densityFraction`. Used only
        /// on the fallback route (`resintering_density_change == 0`).
        density_fraction: f64,
        /// Resintering density change \[%TD\] — upstream's
        /// `resinteringDensityChange`, the density increase measured on an
        /// as-fabricated pellet held at the resintering temperature in a
        /// furnace. **Positive**, or exactly `0.0` to select the fallback
        /// route.
        resintering_density_change: f64,
        /// Resintering test temperature \[K\] — upstream's `Tsintering`,
        /// default 1800. Used only on the fallback route, where it enters as
        /// `T_sinter − 1453`; it must not equal 1453 K.
        sintering_temperature: f64,
    },
}

/// The validity window of one variant, used both to clamp and to check.
///
/// Private: the ranges are an implementation detail of this port, documented
/// per variant, and a public accessor would freeze them as API.
#[derive(Debug, Clone, Copy)]
struct Limits {
    /// Human-readable name of the correlation, for the error message.
    quantity: &'static str,
    /// `(low, high)` burnup \[MWd/kgHM\], if burnup is an input.
    burnup: Option<(f64, f64)>,
    /// `(low, high)` temperature \[K\], if temperature is an input.
    temperature: Option<(f64, f64)>,
}

impl DensificationModel {
    /// Volumetric densification strain `ΔV/V` \[-\], **negative for shrinking
    /// fuel**.
    ///
    /// **Inputs are clamped** to the variant's stated validity range (burnup
    /// and, where the variant uses it, temperature) before evaluation, so this
    /// always returns a finite number. Outside the range it returns the
    /// endpoint value rather than an extrapolation — which is *not* what
    /// upstream OFFBEAT does; upstream extrapolates. Use
    /// [`value_checked`](Self::value_checked) when you need to know that the
    /// clamp fired.
    ///
    /// A **degenerate parameter set** — a non-positive `saturation_burnup`, a
    /// zero denominator, an asymptote so large the offset solve has no
    /// solution — makes this return `0.0`, the only answer that cannot invent
    /// physics. [`value_checked`](Self::value_checked) reports it as
    /// [`OffbeatError::Unphysical`] instead, and that is the method to use if
    /// the parameters came from user input.
    ///
    /// ```
    /// use outram_park_fork_offbeat::materials::MaterialState;
    /// use outram_park_fork_offbeat::materials::behavioral::densification::DensificationModel;
    ///
    /// let model = DensificationModel::Uo2Frapcon {
    ///     density_fraction: 0.95,
    ///     resintering_density_change: 0.5,
    ///     sintering_temperature: 1800.0,
    /// };
    ///
    /// // Zero at zero burnup, by construction of the offset B.
    /// assert!(model.value(&MaterialState::fresh(1100.0)).abs() < 1.0e-9);
    ///
    /// // Negative — the fuel SHRINKS — once it has seen some burnup.
    /// let mut aged = MaterialState::fresh(1100.0);
    /// aged.burnup = 5.0;
    /// assert!(model.value(&aged) < 0.0);
    /// ```
    #[must_use]
    pub fn value(&self, state: &MaterialState) -> f64 {
        let state = self.clamped(state);
        match self {
            Self::Zero => 0.0,

            Self::Empirical {
                density_fraction,
                density_change,
                saturation_burnup,
            } => {
                let asymptote = empirical_asymptote(*density_fraction, *density_change);
                if !asymptote.is_finite() || *saturation_burnup <= 0.0 {
                    return 0.0;
                }
                // Upstream's time constant is quoted in MWd/tHM.
                let burnup = state.burnup * 1000.0;
                asymptote * (1.0 - (-burnup / saturation_burnup).exp())
            }

            Self::Uo2Frapcon {
                density_fraction,
                resintering_density_change,
                sintering_temperature,
            } => {
                let linear_max = frapcon_linear_asymptote(
                    *density_fraction,
                    *resintering_density_change,
                    *sintering_temperature,
                    state.temperature,
                );
                let Some(offset) = frapcon_offset(linear_max) else {
                    return 0.0;
                };
                let shifted = state.burnup + offset;
                let linear = linear_max
                    + (-FRAPCON_SLOW_DECAY * shifted).exp()
                    + FRAPCON_FAST_WEIGHT * (-FRAPCON_FAST_DECAY * shifted).exp();
                3.0 * linear
            }
        }
    }

    /// One-dimensional densification strain `ΔL/L` \[-\], **negative for
    /// shrinking fuel** — exactly one third of [`value`](Self::value).
    ///
    /// This is the quantity upstream stores in its `epsilonDensification`
    /// field, so it is the one to compare against an OFFBEAT run. Same clamping
    /// and same degenerate-parameter behaviour as [`value`](Self::value).
    #[must_use]
    pub fn linear(&self, state: &MaterialState) -> f64 {
        self.value(state) / 3.0
    }

    /// [`value`](Self::value), but returning an error instead of clamping or
    /// silently returning zero.
    ///
    /// # Errors
    ///
    /// - [`OffbeatError::OutOfRange`] when burnup or temperature lies outside
    ///   the variant's stated validity range. Burnup is checked first.
    /// - [`OffbeatError::Unphysical`] when the variant's parameters are
    ///   degenerate: a non-positive `saturation_burnup`, a negative
    ///   `density_change`, a zero denominator in the asymptote, a
    ///   `sintering_temperature` of exactly 1453 K, or an asymptote for which
    ///   the FRAPCON offset solve does not converge.
    ///
    /// ```
    /// use outram_park_fork_offbeat::materials::MaterialState;
    /// use outram_park_fork_offbeat::materials::behavioral::densification::DensificationModel;
    ///
    /// let broken = DensificationModel::Empirical {
    ///     density_fraction: 0.95,
    ///     density_change: 0.5,
    ///     saturation_burnup: 0.0, // must be strictly positive
    /// };
    /// assert!(broken.value_checked(&MaterialState::fresh(900.0)).is_err());
    /// ```
    pub fn value_checked(&self, state: &MaterialState) -> Result<f64> {
        self.check_range(state)?;
        self.check_parameters(state)?;
        Ok(self.value(state))
    }

    /// [`linear`](Self::linear), but returning an error instead of clamping or
    /// silently returning zero.
    ///
    /// # Errors
    ///
    /// The same errors as [`value_checked`](Self::value_checked).
    pub fn linear_checked(&self, state: &MaterialState) -> Result<f64> {
        Ok(self.value_checked(state)? / 3.0)
    }

    /// The validity window of this variant. See each variant's documentation.
    fn limits(&self) -> Limits {
        match self {
            Self::Zero => Limits {
                quantity: "densification (none)",
                burnup: None,
                temperature: None,
            },
            Self::Empirical { .. } => Limits {
                quantity: "empirical densification",
                burnup: Some((0.0, 120.0)),
                temperature: None,
            },
            Self::Uo2Frapcon { .. } => Limits {
                quantity: "UO2 FRAPCON densification",
                burnup: Some((0.0, 120.0)),
                temperature: Some((300.0, 2000.0)),
            },
        }
    }

    /// A copy of `state` with every input this variant reads clamped into its
    /// validity range.
    fn clamped(&self, state: &MaterialState) -> MaterialState {
        let limits = self.limits();
        let mut clamped = *state;
        if let Some((low, high)) = limits.burnup {
            clamped.burnup = clamped.burnup.clamp(low, high);
        }
        if let Some((low, high)) = limits.temperature {
            clamped.temperature = clamped.temperature.clamp(low, high);
        }
        clamped
    }

    /// `Ok(())` if every input this variant reads lies in its validity range.
    fn check_range(&self, state: &MaterialState) -> Result<()> {
        let limits = self.limits();
        let checks = [
            (limits.burnup, state.burnup, "MWd/kgHM"),
            (limits.temperature, state.temperature, "K"),
        ];
        for (range, value, unit) in checks {
            if let Some((low, high)) = range {
                if value < low || value > high {
                    return Err(OffbeatError::OutOfRange {
                        quantity: limits.quantity,
                        value,
                        low,
                        high,
                        unit,
                    });
                }
            }
        }
        Ok(())
    }

    /// `Ok(())` if this variant's parameters give a well-posed correlation at
    /// `state`.
    fn check_parameters(&self, state: &MaterialState) -> Result<()> {
        match self {
            Self::Zero => Ok(()),

            Self::Empirical {
                density_fraction,
                density_change,
                saturation_burnup,
            } => {
                if *saturation_burnup <= 0.0 {
                    return Err(OffbeatError::Unphysical {
                        quantity: "empirical densification saturation burnup",
                        value: *saturation_burnup,
                        unit: "MWd/tHM",
                        reason: "the exponential burnup constant must be strictly positive",
                    });
                }
                if *density_change < 0.0 {
                    return Err(OffbeatError::Unphysical {
                        quantity: "empirical densification density change",
                        value: *density_change,
                        unit: "%TD",
                        reason: "densification increases density, so this must not be negative",
                    });
                }
                let asymptote = empirical_asymptote(*density_fraction, *density_change);
                if !asymptote.is_finite() {
                    return Err(OffbeatError::Unphysical {
                        quantity: "empirical densification density fraction",
                        value: *density_fraction,
                        unit: "-",
                        reason: "density_fraction*100 + density_change must not be zero",
                    });
                }
                Ok(())
            }

            Self::Uo2Frapcon {
                density_fraction,
                resintering_density_change,
                sintering_temperature,
            } => {
                if *resintering_density_change < 0.0 {
                    return Err(OffbeatError::Unphysical {
                        quantity: "FRAPCON resintering density change",
                        value: *resintering_density_change,
                        unit: "%TD",
                        reason: "densification increases density, so this must not be negative",
                    });
                }
                if *resintering_density_change == 0.0
                    && (*sintering_temperature - 1453.0).abs() < f64::EPSILON
                {
                    return Err(OffbeatError::Unphysical {
                        quantity: "FRAPCON sintering temperature",
                        value: *sintering_temperature,
                        unit: "K",
                        reason: "the fallback asymptote divides by (T_sinter - 1453)",
                    });
                }
                let clamped = self.clamped(state);
                let linear_max = frapcon_linear_asymptote(
                    *density_fraction,
                    *resintering_density_change,
                    *sintering_temperature,
                    clamped.temperature,
                );
                if frapcon_offset(linear_max).is_none() {
                    return Err(OffbeatError::Unphysical {
                        quantity: "FRAPCON densification asymptote",
                        value: linear_max,
                        unit: "-",
                        reason: "no burnup offset B satisfies dL/L = 0 at zero burnup",
                    });
                }
                Ok(())
            }
        }
    }
}

/// Weight of the fast pore-removal term in the FRAPCON densification law
/// (upstream `par4`).
const FRAPCON_FAST_WEIGHT: f64 = 2.0;
/// Burnup decay constant \[1/(MWd/kgHM)\] of the fast term (upstream `par5`).
const FRAPCON_FAST_DECAY: f64 = 35.0;
/// Burnup decay constant \[1/(MWd/kgHM)\] of the slow term (upstream `par6`).
const FRAPCON_SLOW_DECAY: f64 = 3.0;

/// The **volumetric** densification asymptote \[-\] of the empirical model:
/// minus the fractional density increase once all sinterable porosity is gone.
///
/// `−density_change / (density_fraction·100 + density_change)`, with
/// `density_change` in %TD. Negative for a positive `density_change`. Upstream
/// divides this by three because it stores the linear strain.
fn empirical_asymptote(density_fraction: f64, density_change: f64) -> f64 {
    let denominator = density_fraction * 100.0 + density_change;
    -density_change / denominator
}

/// The **linear** densification asymptote `ΔL/L_max` \[-\] of the FRAPCON
/// model at temperature `temperature` \[K\].
///
/// Reproduces upstream's two routes and its three temperature branches
/// verbatim, including the no-op interpolation window on the fallback route —
/// see [`DensificationModel::Uo2Frapcon`] for why that is deliberate. Always
/// negative or zero.
fn frapcon_linear_asymptote(
    density_fraction: f64,
    resintering_density_change: f64,
    sintering_temperature: f64,
    temperature: f64,
) -> f64 {
    /// Low-temperature coefficient of the fallback route (upstream `par1`).
    const FALLBACK_COLD: f64 = 22.2;
    /// Reference temperature \[K\] of the fallback route (upstream `par2`).
    const FALLBACK_REFERENCE: f64 = 1453.0;
    /// High-temperature coefficient of the fallback route (upstream `par3`).
    const FALLBACK_HOT: f64 = 66.6;
    /// UO2 theoretical density folded into upstream's percentage arithmetic:
    /// `10960 / 100`.
    const TD_OVER_100: f64 = 109.6;
    /// Lower edge of upstream's interpolation window \[K\].
    const T_LOW: f64 = 950.0;
    /// Upper edge of upstream's interpolation window \[K\].
    const T_HIGH: f64 = 1050.0;

    if resintering_density_change == 0.0 {
        // Fallback route: from as-fabricated porosity.
        let cold = -FALLBACK_COLD * (1.0 - density_fraction)
            / (sintering_temperature - FALLBACK_REFERENCE);
        let hot =
            -FALLBACK_HOT * (1.0 - density_fraction) / (sintering_temperature - FALLBACK_REFERENCE);
        if temperature < T_LOW {
            cold
        } else if temperature < T_HIGH {
            // Upstream computes BOTH interpolation endpoints from `par1`, so
            // the "hot" endpoint of its interpolation is the COLD value and the
            // window is flat. Reproduced verbatim; see the variant
            // documentation, and the unit test that pins this behaviour.
            let upstream_hot_endpoint = cold;
            cold + (temperature - T_LOW) / (T_HIGH - T_LOW) * (upstream_hot_endpoint - cold)
        } else {
            hot
        }
    } else {
        // Measured route: from the resintering density change.
        let cold = -(0.0015 * resintering_density_change * TD_OVER_100) / 100.0;
        let hot = -(0.00285 * resintering_density_change * TD_OVER_100) / 100.0;
        if temperature < T_LOW {
            cold
        } else if temperature < T_HIGH {
            cold + (temperature - T_LOW) / (T_HIGH - T_LOW) * (hot - cold)
        } else {
            hot
        }
    }
}

/// The burnup offset `B` \[MWd/kgHM\] that makes FRAPCON's densification
/// exactly zero at zero burnup.
///
/// Solves `ΔL/L_max + exp(−3B) + 2·exp(−35B) = 0` with upstream's fixed-point
/// iteration `B ← −ln(−2·exp(−35B) − ΔL/L_max) / 3` from `B = 1`, to a
/// tolerance of 1e-6 in at most 10 iterations — upstream's own tolerance and
/// iteration cap, where exceeding the cap is a fatal error.
///
/// Returns `None` when no offset exists: a non-negative asymptote (nothing to
/// densify), or an asymptote so large that the logarithm's argument goes
/// non-positive. Callers turn that into `0.0` or an
/// [`OffbeatError::Unphysical`](crate::error::OffbeatError::Unphysical).
fn frapcon_offset(linear_max: f64) -> Option<f64> {
    if !linear_max.is_finite() || linear_max >= 0.0 {
        return None;
    }
    let mut offset = 1.0_f64;
    for _ in 0..10 {
        let argument = -FRAPCON_FAST_WEIGHT * (-FRAPCON_FAST_DECAY * offset).exp() - linear_max;
        if argument <= 0.0 {
            return None;
        }
        let next = -argument.ln() / FRAPCON_SLOW_DECAY;
        if !next.is_finite() {
            return None;
        }
        if (next - offset).abs() <= 1.0e-6 {
            return Some(next);
        }
        offset = next;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The upstream documentation's own example parameters for the FRAPCON
    /// model (`resinteringDensityChange 0.5`, `Tsintering 1800`), at 95% of
    /// theoretical density.
    fn frapcon() -> DensificationModel {
        DensificationModel::Uo2Frapcon {
            density_fraction: 0.95,
            resintering_density_change: 0.5,
            sintering_temperature: 1800.0,
        }
    }

    /// The upstream documentation's own example parameters for the empirical
    /// model (`densificationDensityChange 0.5`,
    /// `densificationTimeConstant 1000` MWd/tU).
    fn empirical() -> DensificationModel {
        DensificationModel::Empirical {
            density_fraction: 0.95,
            density_change: 0.5,
            saturation_burnup: 1000.0,
        }
    }

    fn state(temperature: f64, burnup: f64) -> MaterialState {
        let mut state = MaterialState::fresh(temperature);
        state.burnup = burnup;
        state
    }

    #[test]
    fn zero_model_never_densifies() {
        let model = DensificationModel::Zero;
        assert_eq!(model.value(&state(1100.0, 50.0)), 0.0);
        assert_eq!(model.linear(&state(1100.0, 50.0)), 0.0);
        assert_eq!(model.value_checked(&state(1100.0, 50.0)).unwrap(), 0.0);
    }

    /// **Verification against the upstream model's own stated design
    /// intent.** Upstream's comment on the offset solve reads "Iterate to find
    /// B such that dL/L=0 at Bui=0" — the correlation is *defined* to start
    /// from the as-fabricated state. That gives a genuine, non-fabricated
    /// reference value of exactly zero, independent of every parameter.
    ///
    /// # Methodology
    ///
    /// - Inputs: both routes of the FRAPCON asymptote — measured
    ///   (`resintering_density_change = 0.5` %TD) and fallback
    ///   (`= 0.0`, `density_fraction = 0.95`, `T_sinter = 1800` K) — each swept
    ///   over 700, 1000 and 1400 K so that all three temperature branches are
    ///   exercised.
    /// - Reference: `ΔL/L(Bu = 0) = 0` exactly, from the definition of `B`.
    /// - Tolerance: `|ΔV/V| < 1e-9`, three orders of magnitude tighter than
    ///   the 1e-6 convergence tolerance of the fixed-point solve.
    ///
    /// # Results (measured 2026-07-29, this port)
    ///
    /// The largest residual over the six cases is `1.04e-17` — at the level of
    /// double-precision round-off, so the fixed-point solve is converging to
    /// the exact root rather than merely to its own tolerance.
    ///
    /// # Interpretation
    ///
    /// The offset solve is correctly implemented and the two-exponential form
    /// is assembled with the right signs; a sign error in either exponential
    /// term, or a wrong decay constant, would leave a residual of order the
    /// asymptote itself (1e-3).
    #[test]
    fn frapcon_densification_is_exactly_zero_at_zero_burnup() {
        let models = [
            frapcon(),
            DensificationModel::Uo2Frapcon {
                density_fraction: 0.95,
                resintering_density_change: 0.0,
                sintering_temperature: 1800.0,
            },
        ];
        for model in models {
            for temperature in [700.0, 1000.0, 1400.0] {
                let residual = model.value(&state(temperature, 0.0)).abs();
                assert!(
                    residual < 1.0e-9,
                    "{model:?} at {temperature} K leaves {residual:e} at zero burnup"
                );
            }
        }
    }

    /// Self-consistency check, not external validation: densification is a
    /// shrinkage, so every non-`Zero` variant must be negative at every
    /// positive burnup, and must never become less negative as burnup rises.
    /// This is the test that catches a sign flip against
    /// [`super::super::swelling`].
    #[test]
    fn densification_is_negative_and_monotonically_decreasing() {
        for model in [frapcon(), empirical()] {
            let mut previous = 0.0;
            for burnup in [0.1, 0.5, 1.0, 2.0, 5.0, 10.0, 30.0, 60.0, 120.0] {
                let value = model.value(&state(1100.0, burnup));
                assert!(
                    value < 0.0,
                    "{model:?} gives {value} at {burnup} MWd/kgHM — densification \
                     must be NEGATIVE"
                );
                assert!(
                    value <= previous + 1.0e-15,
                    "{model:?} recovered from {previous} to {value} at {burnup}"
                );
                assert_eq!(model.linear(&state(1100.0, burnup)), value / 3.0);
                previous = value;
            }
        }
    }

    /// Self-consistency check, not external validation: both models saturate,
    /// which is the defining physical feature of densification — the fine
    /// porosity runs out. Measured here as "within 0.1% of the asymptote by
    /// 30 MWd/kgHM, and unchanged to 1e-9 between 60 and 120 MWd/kgHM".
    ///
    /// Asymptotes for the upstream example parameters (2026-07-29, this port):
    /// FRAPCON at 1100 K `−4.6854e-3` volumetric; empirical `−5.2356e-3`
    /// volumetric.
    #[test]
    fn densification_saturates() {
        for model in [frapcon(), empirical()] {
            let late = model.value(&state(1100.0, 30.0));
            let later = model.value(&state(1100.0, 60.0));
            let latest = model.value(&state(1100.0, 120.0));
            assert!(
                (later - latest).abs() < 1.0e-9,
                "{model:?} has not saturated: {later} then {latest}"
            );
            assert!(
                (late - latest).abs() < 1.0e-3 * latest.abs(),
                "{model:?} is not within 0.1% of its asymptote by 30 MWd/kgHM"
            );
        }
    }

    /// Self-consistency check against the closed form of the empirical model,
    /// not external validation. At `Bu = saturation_burnup` the exponential has
    /// completed exactly `1 − 1/e` of its approach; at zero burnup it is
    /// exactly zero. Both are checked to machine precision, which pins the
    /// MWd/kgHM → MWd/tHM factor of 1000 in the exponent.
    #[test]
    fn empirical_model_follows_its_exponential_exactly() {
        let model = empirical();
        let asymptote = empirical_asymptote(0.95, 0.5);
        assert!((asymptote - (-0.5 / 95.5)).abs() < 1.0e-18);

        assert_eq!(model.value(&state(900.0, 0.0)), 0.0);

        // saturation_burnup = 1000 MWd/tHM = 1 MWd/kgHM
        let at_one_constant = model.value(&state(900.0, 1.0));
        let expected = asymptote * (1.0 - (-1.0_f64).exp());
        assert!(
            (at_one_constant - expected).abs() < 1.0e-15,
            "{at_one_constant} != {expected}"
        );

        // Three constants in: 95% complete.
        let at_three = model.value(&state(900.0, 3.0));
        assert!((at_three / asymptote - 0.950_212_931_632_136).abs() < 1.0e-12);

        // The empirical model is temperature-blind by construction.
        assert_eq!(
            model.value(&state(500.0, 5.0)),
            model.value(&state(1500.0, 5.0))
        );
    }

    /// Self-consistency check on the FRAPCON asymptote's temperature
    /// dependence, not external validation. On the measured route the hot
    /// asymptote is `0.00285/0.0015 = 1.9` times the cold one; on the fallback
    /// route it is `66.6/22.2 = 3` times. Both ratios are pinned exactly.
    #[test]
    fn frapcon_asymptote_temperature_branches_have_the_documented_ratios() {
        // Measured route.
        let cold = frapcon_linear_asymptote(0.95, 0.5, 1800.0, 800.0);
        let hot = frapcon_linear_asymptote(0.95, 0.5, 1800.0, 1200.0);
        assert!(cold < 0.0 && hot < 0.0);
        assert!((hot / cold - 0.00285 / 0.0015).abs() < 1.0e-12);
        assert!((cold / (-(0.0015 * 0.5 * 109.6) / 100.0) - 1.0).abs() < 1.0e-15);

        // Fallback route.
        let cold = frapcon_linear_asymptote(0.95, 0.0, 1800.0, 800.0);
        let hot = frapcon_linear_asymptote(0.95, 0.0, 1800.0, 1200.0);
        assert!((hot / cold - 3.0).abs() < 1.0e-12);
        // `1.0 - 0.95` is not exactly 0.05 in binary floating point, so this is
        // a relative rather than an exact comparison.
        assert!((cold / (-22.2 * 0.05 / 347.0) - 1.0).abs() < 1.0e-15);

        // The measured route interpolates linearly across 950-1050 K.
        let midpoint = frapcon_linear_asymptote(0.95, 0.5, 1800.0, 1000.0);
        let expected = 0.5
            * (frapcon_linear_asymptote(0.95, 0.5, 1800.0, 949.0)
                + frapcon_linear_asymptote(0.95, 0.5, 1800.0, 1050.0));
        assert!((midpoint / expected - 1.0).abs() < 1.0e-15);
    }

    /// **Documents a defect in upstream OFFBEAT that this port reproduces
    /// deliberately.** On the fallback route
    /// (`resintering_density_change == 0`) upstream computes both endpoints of
    /// its 950–1050 K "smooth transition" from `par1`, so the transition is
    /// flat and the asymptote instead jumps by a factor of three at 1050 K.
    ///
    /// This test asserts the defective behaviour, so that anyone who fixes it
    /// upstream — or here — is forced to notice and to update the comparison
    /// against OFFBEAT results at the same time. It is *not* an endorsement of
    /// the behaviour.
    #[test]
    fn frapcon_fallback_interpolation_window_is_flat_reproducing_upstream() {
        let cold = frapcon_linear_asymptote(0.95, 0.0, 1800.0, 900.0);
        let in_window = frapcon_linear_asymptote(0.95, 0.0, 1800.0, 1000.0);
        let just_below = frapcon_linear_asymptote(0.95, 0.0, 1800.0, 1049.9);
        let just_above = frapcon_linear_asymptote(0.95, 0.0, 1800.0, 1050.1);

        assert_eq!(
            in_window, cold,
            "upstream's interpolation window is a no-op"
        );
        assert_eq!(just_below, cold);
        assert!(
            (just_above / just_below - 3.0).abs() < 1.0e-12,
            "the asymptote jumps by a factor of three at 1050 K"
        );
    }

    /// `value` clamps, `value_checked` refuses — the documented contract.
    #[test]
    fn out_of_range_inputs_clamp_in_value_and_error_in_value_checked() {
        let model = frapcon();

        let beyond = state(1100.0, 500.0);
        let at_limit = state(1100.0, 120.0);
        assert_eq!(model.value(&beyond), model.value(&at_limit));
        assert!(matches!(
            model.value_checked(&beyond),
            Err(OffbeatError::OutOfRange { .. })
        ));

        let too_hot = state(4000.0, 30.0);
        assert!(model.value(&too_hot).is_finite());
        assert!(matches!(
            model.value_checked(&too_hot),
            Err(OffbeatError::OutOfRange { .. })
        ));

        // A negative burnup clamps to zero rather than reporting swelling.
        assert_eq!(
            model.value(&state(1100.0, -10.0)),
            model.value(&state(1100.0, 0.0))
        );
    }

    /// Degenerate parameter sets are reported by `value_checked` as
    /// [`OffbeatError::Unphysical`] rather than producing a `NaN` or an
    /// invented number, and `value` degrades to a finite fallback.
    ///
    /// The two groups below are separated deliberately, because they degrade
    /// differently and a caller needs to know which is which.
    #[test]
    fn degenerate_parameters_are_reported_not_invented() {
        let s = state(1100.0, 5.0);

        // Group 1: the correlation has no solution at all, so `value` falls
        // back to exactly zero — the only answer that cannot invent physics.
        let no_solution = [
            DensificationModel::Empirical {
                density_fraction: 0.95,
                density_change: 0.5,
                saturation_burnup: 0.0,
            },
            DensificationModel::Empirical {
                density_fraction: 0.0,
                density_change: 0.0,
                saturation_burnup: 1000.0,
            },
            DensificationModel::Uo2Frapcon {
                density_fraction: 0.95,
                resintering_density_change: -0.5,
                sintering_temperature: 1800.0,
            },
            DensificationModel::Uo2Frapcon {
                density_fraction: 0.95,
                resintering_density_change: 0.0,
                sintering_temperature: 1453.0,
            },
            // Fully dense fuel on the fallback route: nothing to sinter out.
            DensificationModel::Uo2Frapcon {
                density_fraction: 1.0,
                resintering_density_change: 0.0,
                sintering_temperature: 1800.0,
            },
        ];
        for model in no_solution {
            let value = model.value(&s);
            assert!(value.is_finite(), "{model:?} produced {value}");
            assert_eq!(value, 0.0, "{model:?} should fall back to zero");
            assert!(
                matches!(
                    model.value_checked(&s),
                    Err(OffbeatError::Unphysical { .. })
                ),
                "{model:?} should report an unphysical parameter set"
            );
        }

        // Group 2: the algebra is perfectly well posed but the parameter is
        // physically backwards. `value` evaluates it faithfully — and so
        // returns a POSITIVE number from a densification model, which is the
        // sign error the module documentation warns about. Only
        // `value_checked` catches it, which is why user-supplied parameters
        // must go through `value_checked`.
        let backwards = DensificationModel::Empirical {
            density_fraction: 0.95,
            density_change: -0.5,
            saturation_burnup: 1000.0,
        };
        assert!(
            backwards.value(&s) > 0.0,
            "a negative density change makes the unchecked path report growth"
        );
        assert!(matches!(
            backwards.value_checked(&s),
            Err(OffbeatError::Unphysical { .. })
        ));
    }

    /// Self-consistency check on the offset solver: it must reproduce the root
    /// of `ΔL/L_max + exp(−3B) + 2 exp(−35B) = 0` for a spread of asymptotes,
    /// and must refuse the cases where no root exists.
    #[test]
    fn frapcon_offset_solver_finds_the_root_or_says_it_cannot() {
        for linear_max in [-1.0e-4, -1.0e-3, -5.0e-3, -2.0e-2, -1.0e-1] {
            let offset = frapcon_offset(linear_max).expect("a root exists here");
            let residual = linear_max
                + (-FRAPCON_SLOW_DECAY * offset).exp()
                + FRAPCON_FAST_WEIGHT * (-FRAPCON_FAST_DECAY * offset).exp();
            assert!(
                residual.abs() < 1.0e-9,
                "offset {offset} leaves residual {residual:e} for asymptote {linear_max}"
            );
        }
        assert!(frapcon_offset(0.0).is_none(), "nothing to densify");
        assert!(
            frapcon_offset(1.0e-3).is_none(),
            "a positive asymptote is not densification"
        );
        assert!(frapcon_offset(f64::NAN).is_none());
        // An asymptote past -3 makes the logarithm's argument non-positive.
        assert!(frapcon_offset(-5.0).is_none());
    }

    /// Self-consistency check that the two independent densification fits
    /// agree in order of magnitude for the same fuel, which they should: both
    /// are anchored on a 0.5 %TD densification of 95%-dense UO2.
    ///
    /// Measured at 1100 K and 30 MWd/kgHM (2026-07-29, this port): FRAPCON
    /// `−4.6854e-3`, empirical `−5.2356e-3` volumetric — a 10.5% difference.
    /// That is a real disagreement between two published correlations, not a
    /// port error: they parameterise the asymptote differently (FRAPCON from
    /// the resintering measurement scaled by temperature, the empirical model
    /// from the stated final density directly).
    #[test]
    fn the_two_fits_agree_in_magnitude_for_the_same_fuel() {
        let s = state(1100.0, 30.0);
        let a = frapcon().value(&s);
        let b = empirical().value(&s);
        assert!(a < 0.0 && b < 0.0);
        let ratio = a / b;
        assert!(
            (0.5..2.0).contains(&ratio),
            "the two fits differ by more than a factor of two: {a} vs {b}"
        );
    }
}
