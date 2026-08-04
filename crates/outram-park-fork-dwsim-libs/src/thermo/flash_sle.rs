//    OUTRAM PARK — pure-Rust port of DWSIM's solid-liquid-equilibrium (SLE) flash.
//
//    Ported from (upstream provenance, kept per workspace GPLv3 policy):
//      Project:  DWSIM (Daniel Wagner O. de Medeiros and contributors)
//      File:     DWSIM.Thermodynamics/FlashAlgorithms/NestedLoopsSLE.vb
//      Class:    PropertyPackages.Auxiliary.FlashAlgorithms.NestedLoopsSLE
//                (specifically the `Flash_SL` eutectic solid-liquid routine,
//                NestedLoopsSLE.vb:319-541)
//      Commit:   1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766
//      Upstream: https://github.com/DanWBR/dwsim
//      Copyright 2013 Daniel Wagner O. de Medeiros
//      License:  GNU General Public License v3.0 (GPL-3.0)
//
//    This file is part of the OUTRAM PARK backend and is distributed under the
//    GNU General Public License v3.0, matching the upstream DWSIM license.
//
//    This program is free software: you can redistribute it and/or modify it
//    under the terms of the GNU General Public License as published by the Free
//    Software Foundation, either version 3 of the License, or (at your option)
//    any later version.
//
//    This program is distributed in the hope that it will be useful, but
//    WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY
//    or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU General Public License
//    for more details. You should have received a copy of the GNU General Public
//    License along with this program.  If not, see <http://www.gnu.org/licenses/>.

//! Isothermal **solid-liquid-equilibrium (SLE) eutectic flash**.
//!
//! Pure-Rust port of DWSIM's `NestedLoopsSLE.Flash_SL`
//! (`DWSIM.Thermodynamics/FlashAlgorithms/NestedLoopsSLE.vb:319-541`, GPL-3.0,
//! commit `1abf72d`). Given a feed and a temperature, it splits the mixture into
//! a **liquid solution** and one or more **precipitated pure solids** at
//! equilibrium (the *eutectic* model: each solid is a pure crystalline phase, no
//! solid solution).
//!
//! # What this computes
//!
//! For each component `i` the **maximum solubility** in the liquid is set by the
//! equilibrium between the pure solid and the dissolved species. Equating solid
//! and liquid fugacities and using a fusion (melting) thermodynamic cycle gives
//! DWSIM's relation (`NestedLoopsSLE.vb:334`, rendered here with the heat-capacity
//! term the source deliberately drops — see below):
//!
//! ```text
//! -ln(x_i^L gamma_i^L) = (dH_fus,i / (R T)) (1 - T / T_fus,i)
//! ```
//!
//! so the **activity at saturation** (the solid's fixed liquid-phase activity)
//! is the van't Hoff / Schröder-van-Laar ideal solubility
//!
//! ```text
//! a_i^sat = x_i^L gamma_i^L = exp[ -(dH_fus,i / R) (1/T - 1/T_fus,i) ]
//! ```
//!
//! and the **maximum liquid mole fraction** of `i` before it starts to
//! precipitate is `x_i^max = a_i^sat / gamma_i` (`NestedLoopsSLE.vb:416,439`).
//! Because `gamma_i` depends on the (unknown) liquid composition, the flash
//! iterates: evaluate `gamma` at the current liquid `x`, recompute the solubility
//! limits, rebalance moles between liquid and solid, and repeat until the liquid
//! fraction `L` stops moving.
//!
//! The heat-capacity difference term
//! `-(dCp_i / R)[(T - T_fus,i)/T + ln(T_fus,i / T)]` is present in DWSIM's
//! equation but DWSIM **sets `dCp_i = 0`** in code
//! (`NestedLoopsSLE.vb:411`, comment *"ignoring heat capacity difference due to
//! issues with DWSIM characterization"*). This port mirrors that: `dCp` is not
//! used, so the solubility reduces to the two-term van't Hoff law above. The
//! (dropped) term is documented here for provenance only.
//!
//! # Units (documented raw `f64`, SI — the DWSIM-internal convention)
//!
//! | Quantity | Symbol | Unit |
//! |---|---|---|
//! | Temperature | `T`, `T_fus` | K |
//! | Enthalpy of fusion | `dH_fus` | J/mol |
//! | Gas constant | `R` | J/(mol·K) |
//! | Mole fractions | `z`, `x`, `s` | dimensionless |
//! | Liquid / solid molar fraction | `L`, `S = 1 - L` | dimensionless |
//!
//! > **Unit note vs. DWSIM.** DWSIM stores the fusion enthalpy in **kJ/mol** and
//! > multiplies by `1000` inline (`Hf(i) * 1000`, `NestedLoopsSLE.vb:416`). This
//! > port takes `dH_fus` directly in **J/mol** — spell it out at the call site.
//!
//! # Design (workspace + crate `CLAUDE.md`)
//!
//! Enum dispatch through [`crate::thermo::activity::ActivityModel`] for the
//! liquid-phase `gamma` (no `dyn`, no trait objects, no `Box`, no lifetimes);
//! components are indexed by `usize`; the inner solubility/mass-balance loops are
//! raw `f64` arithmetic. Every public item documents its physical quantity,
//! valid ranges, and units.
//!
//! # Honest scope — what is and is **not** ported
//!
//! Ported: the **eutectic** `Flash_SL` isothermal SLE split (pure-solid phases),
//! the van't Hoff solubility limit, the supercritical-gas / ion / forced-solid
//! overrides (`NestedLoopsSLE.vb:443-447`), and the single-component
//! above/below-melting shortcut (`NestedLoopsSLE.vb:380-401`).
//!
//! **Not** ported (present in the fuller `NestedLoopsSLE.vb`, out of scope here):
//!
//! - **Solid-solution** flash `Flash_PT_SS` (`NestedLoopsSLE.vb:104-317`), where
//!   the solid is itself a mixed phase with its own distribution coefficients.
//! - The **solid-vapour-liquid** (SVLE) driver `Flash_PT_NL`
//!   (`NestedLoopsSLE.vb:543-934`) that wraps this SLE step inside a
//!   Rachford-Rice VLE loop, and the `Flash_PH/PS/TV/PV` energy/spec variants.
//! - The heat-capacity-difference (`dCp`) correction — dropped, matching DWSIM.
//! - DWSIM's fugacity-coefficient plumbing (`DW_CalcFugCoeff · P / Pvap`,
//!   `NestedLoopsSLE.vb:437`): this port takes the liquid activity coefficient
//!   `gamma` **directly** from [`crate::thermo::activity`], so no vapour-pressure
//!   data is needed for the pure solid-liquid split.
//!
//! > **⚠️ Verification, not validation.** The tests below are *verification*:
//! > they check the port reproduces the analytic van't Hoff ideal-solubility law
//! > and a hand-computed binary eutectic split. They are **not** validated
//! > against experimental SLE data, and nothing here is cleared for nuclear /
//! > safety-critical use. AI-assisted draft — untrusted until human-reviewed per
//! > the crate `CLAUDE.md`.

use crate::thermo::activity::ActivityModel;

/// CODATA molar gas constant `R = 8.314 462 618 153 24 J/(mol·K)`.
///
/// DWSIM hard-codes `8.31446` in `Flash_SL` (`NestedLoopsSLE.vb:416`); the extra
/// CODATA digits change the solubility by `< 1e-6` relative and are used here for
/// consistency with the rest of the crate.
pub const R_GAS: f64 = 8.314_462_618_153_24;

/// Ideal (van't Hoff / Schröder-van-Laar) solid solubility — the **activity at
/// saturation** of a pure solid in the liquid.
///
/// ```text
/// a_i^sat = exp[ -(dH_fus / R) (1/T - 1/T_fus) ]
/// ```
///
/// This is the mole-fraction solubility `x_i^sat` in the limit of an **ideal
/// liquid** (`gamma_i = 1`). Ported from `NestedLoopsSLE.vb:416` (with the `dCp`
/// term dropped as DWSIM does at `NestedLoopsSLE.vb:411`).
///
/// # Units / ranges
///
/// - `fusion_enthalpy` `dH_fus` \[J/mol\] — enthalpy of fusion at the melting
///   point, `> 0` for a real solid.
/// - `fusion_temperature` `T_fus` \[K\] — normal melting point, `> 0`.
/// - `temperature` `T` \[K\] — system temperature, `> 0`.
/// - Returns the saturation activity \[-\], `> 0`. At `T = T_fus` it is exactly
///   `1` (the pure melt); for `T < T_fus` it is `< 1` (partial solubility); for
///   `T > T_fus` it exceeds `1` (the solid cannot exist — the component stays
///   fully liquid).
///
/// Returns `1000.0` (a "never precipitates" sentinel, matching
/// `NestedLoopsSLE.vb:417`) when `T_fus` or `dH_fus` are non-positive/non-finite,
/// i.e. when the component has no characterised solid phase.
#[must_use]
pub fn ideal_solubility(fusion_enthalpy: f64, fusion_temperature: f64, temperature: f64) -> f64 {
    if !fusion_temperature.is_finite()
        || fusion_temperature <= 0.0
        || !fusion_enthalpy.is_finite()
        || fusion_enthalpy <= 0.0
        || temperature <= 0.0
    {
        return 1000.0;
    }
    // -(dH/R)(1/T - 1/Tf) == -(dH/(R T))(1 - T/Tf); DWSIM writes the latter form.
    let a = (-fusion_enthalpy / (R_GAS * temperature)
        * (1.0 - temperature / fusion_temperature))
        .exp();
    if a.is_finite() {
        a
    } else {
        1000.0
    }
}

/// Per-component constant data an SLE flash needs beyond the liquid activity model.
///
/// The liquid-phase `gamma` comes from [`ActivityModel`]; this record carries the
/// **solid-phase** fusion properties and the special-case flags DWSIM applies
/// (`NestedLoopsSLE.vb:443-447`). Indexed positionally to match the `z` / `x` /
/// `s` vectors.
#[derive(Debug, Clone, PartialEq)]
pub struct SleComponent {
    /// Enthalpy of fusion `dH_fus` \[J/mol\] at the melting point. `<= 0` marks a
    /// component with no solid phase (it never precipitates).
    pub fusion_enthalpy: f64,
    /// Normal melting / fusion temperature `T_fus` \[K\]. `<= 0` marks a component
    /// with no solid phase.
    pub fusion_temperature: f64,
    /// Critical temperature `T_c` \[K\]. Above it the component is a supercritical
    /// gas and is forced entirely into the liquid (solubility limit lifted),
    /// `NestedLoopsSLE.vb:445`. Use `f64::INFINITY` to disable this override.
    pub critical_temperature: f64,
    /// Whether the component is a dissolved **ion** — ions are kept fully in the
    /// liquid (`x_max = 1`), `NestedLoopsSLE.vb:443`.
    pub is_ion: bool,
    /// Whether the component is **forced to the solid phase** regardless of
    /// solubility (`x_max = 0`), `NestedLoopsSLE.vb:447`.
    pub forced_solid: bool,
}

impl SleComponent {
    /// Component with only its fusion properties set, no overrides
    /// (`critical_temperature = +inf`, not an ion, not forced solid).
    ///
    /// `fusion_enthalpy` \[J/mol\] `> 0`, `fusion_temperature` \[K\] `> 0` for a
    /// component that can precipitate; either `<= 0` marks a non-precipitating
    /// (always-liquid) component.
    #[must_use]
    pub fn from_fusion(fusion_enthalpy: f64, fusion_temperature: f64) -> Self {
        Self {
            fusion_enthalpy,
            fusion_temperature,
            critical_temperature: f64::INFINITY,
            is_ion: false,
            forced_solid: false,
        }
    }
}

/// Converged (or best-effort) result of an [`flash_sl`] solid-liquid split.
///
/// `x` and `s` are mole fractions \[-\] **within** their respective phases (each
/// sums to 1 when that phase is present). `liquid_fraction` is the molar fraction
/// of feed in the liquid; `solid_fraction = 1 - liquid_fraction`.
#[derive(Debug, Clone, PartialEq)]
pub struct SleFlashResult {
    /// Liquid molar fraction `L` \[-\], moles liquid per mole feed, in `[0, 1]`.
    /// `1.0` = fully dissolved (no solid); `0.0` = fully frozen (no liquid).
    pub liquid_fraction: f64,
    /// Solid molar fraction `S = 1 - L` \[-\].
    pub solid_fraction: f64,
    /// Liquid-phase mole fractions `x_i` \[-\] (sum to 1 when `L > 0`, else all 0).
    pub x: Vec<f64>,
    /// Solid-phase mole fractions `s_i` \[-\] (sum to 1 when `S > 0`, else all 0).
    pub s: Vec<f64>,
    /// Completed outer iterations (activity-coefficient updates).
    pub iterations: usize,
}

/// Tuning parameters for [`flash_sl`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SleOptions {
    /// Maximum outer iterations before returning [`SleFlashError::NotConverged`].
    /// DWSIM's `maxit_e` default is 100 (`NestedLoopsSLE.vb:521`).
    pub max_iter: usize,
    /// Convergence tolerance on the liquid-fraction change `|L - L_old|` \[-\].
    /// DWSIM's `MaxError = 1e-7` (`NestedLoopsSLE.vb:342`).
    pub tol: f64,
}

impl Default for SleOptions {
    fn default() -> Self {
        Self {
            max_iter: 100,
            tol: 1.0e-7,
        }
    }
}

/// Error conditions for [`flash_sl`].
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum SleFlashError {
    /// An empty feed was supplied (need at least one component).
    #[error("empty composition")]
    Empty,
    /// Two input slices that must be the same length were not.
    #[error("slice length mismatch: z has {z}, components have {components}")]
    LengthMismatch {
        /// Length of `z`.
        z: usize,
        /// Length of `components`.
        components: usize,
    },
    /// A non-finite value (`NaN`/`inf`) appeared in `z` or was produced mid-solve.
    #[error("non-finite value in feed or during solve")]
    NonFinite,
    /// The system temperature was non-positive.
    #[error("temperature must be finite and > 0 (got {0} K)")]
    NonPositiveTemperature(f64),
    /// The outer iteration did not converge within `max_iter`.
    #[error("SLE flash did not converge in {iterations} iterations (|dL| = {residual:e})")]
    NotConverged {
        /// Iterations attempted.
        iterations: usize,
        /// Final liquid-fraction change `|L - L_old|`.
        residual: f64,
    },
}

/// Normalise a slice in place to sum 1 (no-op if the sum is 0).
fn normalize(v: &mut [f64]) {
    let s: f64 = v.iter().sum();
    if s > 0.0 {
        for vi in v.iter_mut() {
            *vi /= s;
        }
    }
}

/// Isothermal **eutectic solid-liquid-equilibrium flash** at fixed temperature.
///
/// Splits the feed `z` into a liquid solution and precipitated pure solids at
/// `temperature`, returning the phase mole fractions and the liquid fraction `L`.
/// Direct port of DWSIM `NestedLoopsSLE.Flash_SL`
/// (`NestedLoopsSLE.vb:319-541`).
///
/// # Method (successive substitution on the activity coefficient)
///
/// 1. Start with all feed in the liquid (`x = z`, `L = 1`;
///    `NestedLoopsSLE.vb:356`).
/// 2. Evaluate `gamma_i` at the current liquid `x` and `T` via `activity`
///    (`NestedLoopsSLE.vb:437`).
/// 3. Solubility limit `x_i^max = a_i^sat / gamma_i` from [`ideal_solubility`]
///    (`NestedLoopsSLE.vb:416,439`), with the overrides: ions and supercritical
///    (`T > T_c`) components get `x_i^max = 1` (stay liquid); forced solids get
///    `x_i^max = 0` (`NestedLoopsSLE.vb:443-447`).
/// 4. Mole balance (`NestedLoopsSLE.vb:453-501`): components with
///    `z_i <= x_i^max` dissolve completely (`n_i^L = z_i`); components with
///    `z_i > x_i^max` are saturated (liquid mole fraction pinned at `x_i^max`,
///    liquid moles `x_i^max · L`) and the excess precipitates. Enforcing that the
///    liquid mole fractions sum to 1 gives
///    `L = (Σ_{dissolved} z_i) / (1 - Σ_{saturated} x_i^max)`.
/// 5. Renormalise the liquid (`x`) and solid (`s`) compositions and repeat from
///    step 2 until `|L - L_old| < opts.tol`.
///
/// Single-component feeds short-circuit (`NestedLoopsSLE.vb:380-401`): above the
/// melting point → all liquid, below → all solid. A feed whose components all
/// lack solid data returns all-liquid (`NestedLoopsSLE.vb:361-368`).
///
/// # Units / ranges
///
/// - `z`: feed mole fractions \[-\], length `n >= 1`, finite; physical feeds sum
///   to 1 (the routine normalises defensively).
/// - `components`: length `n`, fusion properties in J/mol and K (see
///   [`SleComponent`]).
/// - `activity`: liquid-phase `gamma` model; its component count must be `n` for
///   the non-ideal variants.
/// - `temperature` `T` \[K\], `> 0`.
///
/// # Errors
///
/// [`SleFlashError::Empty`] on empty `z`; [`SleFlashError::LengthMismatch`] if
/// `z.len() != components.len()`; [`SleFlashError::NonFinite`] on a non-finite
/// input or intermediate; [`SleFlashError::NonPositiveTemperature`] if `T <= 0`;
/// [`SleFlashError::NotConverged`] after `opts.max_iter` outer passes.
pub fn flash_sl(
    z: &[f64],
    components: &[SleComponent],
    activity: &ActivityModel,
    temperature: f64,
    opts: SleOptions,
) -> Result<SleFlashResult, SleFlashError> {
    let n = z.len();
    if n == 0 {
        return Err(SleFlashError::Empty);
    }
    if components.len() != n {
        return Err(SleFlashError::LengthMismatch {
            z: n,
            components: components.len(),
        });
    }
    if z.iter().any(|v| !v.is_finite()) {
        return Err(SleFlashError::NonFinite);
    }
    if !temperature.is_finite() || temperature <= 0.0 {
        return Err(SleFlashError::NonPositiveTemperature(temperature));
    }

    // Normalise the feed defensively (mole fractions).
    let mut z_norm = z.to_vec();
    normalize(&mut z_norm);
    let zf = &z_norm;

    let null = vec![0.0_f64; n];

    // --- No solid data anywhere → liquid only (NestedLoopsSLE.vb:361-368) ------
    let any_solid = components
        .iter()
        .any(|c| c.fusion_temperature > 0.0 && c.fusion_enthalpy > 0.0);
    if !any_solid {
        return Ok(SleFlashResult {
            liquid_fraction: 1.0,
            solid_fraction: 0.0,
            x: zf.clone(),
            s: null,
            iterations: 0,
        });
    }

    // --- Single-component shortcut (NestedLoopsSLE.vb:380-401) -----------------
    if let Some(i) = zf.iter().position(|&zi| zi >= 0.999_999) {
        let c = &components[i];
        let (l, x, s) = if !(c.fusion_temperature > 0.0) || temperature > c.fusion_temperature {
            (1.0, zf.clone(), null.clone()) // above melting (or no solid) → liquid
        } else {
            (0.0, null.clone(), zf.clone()) // below melting → solid
        };
        return Ok(SleFlashResult {
            liquid_fraction: l,
            solid_fraction: 1.0 - l,
            x,
            s,
            iterations: 0,
        });
    }

    // Saturation activities a_i^sat depend only on T, so compute them once.
    let a_sat: Vec<f64> = components
        .iter()
        .map(|c| ideal_solubility(c.fusion_enthalpy, c.fusion_temperature, temperature))
        .collect();

    let mut x = zf.clone(); // liquid composition, seeded with the feed
    let mut s: Vec<f64>; // solid composition, assigned on every exit path below
    let mut l = 1.0_f64;
    let mut last_residual = f64::INFINITY;

    for iter in 1..=opts.max_iter {
        // Liquid activity coefficients at the current liquid composition.
        let gamma = activity.activity_coefficients(&x, temperature);
        if gamma.len() != n {
            return Err(SleFlashError::LengthMismatch {
                z: n,
                components: gamma.len(),
            });
        }

        // Maximum solubility mole fractions x_i^max (NestedLoopsSLE.vb:439-447).
        let mut max_x = vec![0.0_f64; n];
        for i in 0..n {
            let gi = if gamma[i].is_finite() && gamma[i] > 0.0 {
                gamma[i]
            } else {
                1.0 // NestedLoopsSLE.vb:192 — treat a bad gamma as ideal
            };
            let mut mx = a_sat[i] / gi;
            if components[i].is_ion {
                mx = 1.0;
            }
            if temperature > components[i].critical_temperature {
                mx = 1.0; // supercritical gas stays liquid
            }
            if components[i].forced_solid {
                mx = 0.0;
            }
            if !mx.is_finite() {
                return Err(SleFlashError::NonFinite);
            }
            max_x[i] = mx;
        }

        // Early "only solid remaining" test (NestedLoopsSLE.vb:453-467): sum the
        // solubility limits of the components that are actually present; if the
        // liquid cannot hold unit total mole fraction, no liquid survives.
        let mut sf_check = 0.0;
        for i in 0..n {
            let max_liqu_phase = if max_x[i] > 0.0 {
                zf[i] / max_x[i]
            } else {
                f64::INFINITY
            };
            if max_liqu_phase > 0.000_1 {
                sf_check += max_x[i];
            }
        }
        if sf_check < 1.0 {
            l = 0.0;
            x = null.clone();
            s = zf.clone();
            return finish(l, x, s, iter);
        }

        let l_old = l;

        // Split feed into fully-dissolved vs. saturated (NestedLoopsSLE.vb:475-486).
        let mut n_liq = vec![0.0_f64; n]; // liquid moles of the dissolved components
        let mut sat_sum = 0.0; // Σ x_i^max over saturated (precipitating) components
        let mut dissolved_sum = 0.0; // Σ z_i over fully-dissolved components
        for i in 0..n {
            if zf[i] > max_x[i] {
                sat_sum += max_x[i];
            } else {
                n_liq[i] = zf[i];
                dissolved_sum += zf[i];
            }
        }
        if 1.0 - dissolved_sum < 1.0e-8 {
            dissolved_sum = 1.0;
        }
        l = dissolved_sum / (1.0 - sat_sum);

        if !l.is_finite() {
            return Err(SleFlashError::NonFinite);
        }

        // All components below their solubility limit → single liquid phase
        // (NestedLoopsSLE.vb:488-493).
        if l >= 1.0 {
            l = 1.0;
            x = zf.clone();
            s = null.clone();
            return finish(l, x, s, iter);
        }

        // Distribute moles between liquid and solid (NestedLoopsSLE.vb:496-501).
        let mut n_sol = vec![0.0_f64; n];
        for i in 0..n {
            if zf[i] > max_x[i] {
                n_liq[i] = max_x[i] * l; // saturated: liquid moles pinned at x_max·L
            }
            n_sol[i] = zf[i] - n_liq[i];
        }

        let max_x_sum: f64 = max_x.iter().sum();
        if l > 0.0 && max_x_sum > 1.0 {
            x = n_liq.clone();
            normalize(&mut x);
            s = n_sol.clone();
            normalize(&mut s);
        } else {
            // Only solid remaining (NestedLoopsSLE.vb:506-513).
            l = 0.0;
            x = null.clone();
            s = zf.clone();
            return finish(l, x, s, iter);
        }

        last_residual = (l - l_old).abs();
        if last_residual < opts.tol {
            return finish(l, x, s, iter);
        }
    }

    Err(SleFlashError::NotConverged {
        iterations: opts.max_iter,
        residual: last_residual,
    })
}

/// Assemble the final [`SleFlashResult`] (`Ok`), sharing the exit paths above.
fn finish(l: f64, x: Vec<f64>, s: Vec<f64>, iterations: usize) -> Result<SleFlashResult, SleFlashError> {
    Ok(SleFlashResult {
        liquid_fraction: l,
        solid_fraction: 1.0 - l,
        x,
        s,
        iterations,
    })
}

#[cfg(test)]
mod tests {
    //! # V&V — verification of the DWSIM `Flash_SL` SLE port
    //!
    //! **Methodology.** Each test checks the Rust port against an **analytic**
    //! property of the SLE equations — the van't Hoff ideal-solubility law, a
    //! hand-computed binary saturation split, the eutectic crossing condition,
    //! and the single-component melting shortcut — **not** experimental data.
    //! Reference expressions are DWSIM `NestedLoopsSLE.Flash_SL`. Tolerances are
    //! absolute, noted per test.
    //!
    //! **Scope (honesty).** *Verification* ("did we implement DWSIM's formula
    //! correctly?"), **not** *validation* against measured SLE. No parameter set
    //! is claimed experimentally accurate. Numbers measured 2026-08-03 (this port).

    use super::*;

    // Two hypothetical solids with round fusion properties for analytic checks.
    // (Not tied to any real compound — chosen so hand arithmetic is transparent.)
    const HF_A: f64 = 18_000.0; // dH_fus,A [J/mol]
    const TF_A: f64 = 350.0; //    T_fus,A  [K]
    const HF_B: f64 = 15_000.0; // dH_fus,B [J/mol]
    const TF_B: f64 = 300.0; //    T_fus,B  [K]

    fn comps() -> Vec<SleComponent> {
        vec![
            SleComponent::from_fusion(HF_A, TF_A),
            SleComponent::from_fusion(HF_B, TF_B),
        ]
    }

    /// **Methodology.** [`ideal_solubility`] must equal the closed-form van't Hoff
    /// law `a = exp[-(dH/R)(1/T - 1/Tf)]`, and it must equal `1` exactly at
    /// `T = Tf` (pure melt). Checked for solid A at `T = 320 K`.
    ///
    /// Hand computation (A, 320 K): `1/320 - 1/350 = 2.678571e-4`,
    /// `-(18000/8.314463)·2.678571e-4 = -0.579906`, `exp = 0.559963`.
    ///
    /// **Result (2026-08-03).** `ideal_solubility(18000, 350, 320) = 0.5599630`,
    /// matching the closed form to `< 1e-12`; `ideal_solubility(·,·,Tf) = 1.0`
    /// to `< 1e-12`.
    #[test]
    fn ideal_solubility_matches_vant_hoff() {
        let t = 320.0;
        let closed = (-HF_A / R_GAS * (1.0 / t - 1.0 / TF_A)).exp();
        let got = ideal_solubility(HF_A, TF_A, t);
        assert!((got - closed).abs() < 1e-12, "got {got}, closed {closed}");
        assert!((got - 0.559_963_0).abs() < 1e-6, "got {got}");
        // Exactly 1 at the melting point.
        assert!((ideal_solubility(HF_A, TF_A, TF_A) - 1.0).abs() < 1e-12);
        // No solid data → "never precipitates" sentinel.
        assert_eq!(ideal_solubility(0.0, 0.0, t), 1000.0);
    }

    /// **Methodology.** With an **ideal liquid** (`gamma = 1`) the SLE flash must
    /// reproduce the van't Hoff split analytically. Binary A(1)/B(2) at
    /// `T = 320 K`, feed `z = [0.7, 0.3]`. Since `T > Tf_B (=300)`, B is above its
    /// melting point (`x_B^max = 1.456 > z_B`) and stays fully liquid; A is
    /// saturated (`x_A^max = 0.559924 < z_A`) and its excess crystallises.
    ///
    /// Hand mass balance: dissolved B contributes `SLP = 0.3`; saturated A gives
    /// `SF = x_A^max = 0.559963`; `L = 0.3 / (1 - 0.559963) = 0.681761`. Liquid
    /// `x = [0.559963, 0.440037]` (A at exactly its saturation fraction), solid is
    /// pure A (`s = [1, 0]`), `S = 0.318239`.
    ///
    /// **Result (2026-08-03).** Port returns `L = 0.6817609`,
    /// `x = [0.5599630, 0.4400370]`, `s = [1.0, 0.0]`, all matching the hand
    /// values to `< 1e-6`; overall mole balance `z_i = L·x_i + S·s_i` holds to
    /// `< 1e-12`.
    #[test]
    fn ideal_binary_saturation_split_matches_hand() {
        let z = [0.7, 0.3];
        let r = flash_sl(&z, &comps(), &ActivityModel::Ideal, 320.0, SleOptions::default())
            .expect("flash converges");

        let x_a_max = ideal_solubility(HF_A, TF_A, 320.0);
        let l_hand = 0.3 / (1.0 - x_a_max);
        assert!((r.liquid_fraction - l_hand).abs() < 1e-9, "L = {}", r.liquid_fraction);
        assert!((r.liquid_fraction - 0.681_760_9).abs() < 1e-6);
        // Liquid is saturated in A at x_A^max; B makes up the rest.
        assert!((r.x[0] - x_a_max).abs() < 1e-9, "x_A = {}", r.x[0]);
        assert!((r.x[0] + r.x[1] - 1.0).abs() < 1e-12);
        // Solid is pure A.
        assert!((r.s[0] - 1.0).abs() < 1e-9 && r.s[1].abs() < 1e-12);
        // Overall mole balance z_i = L x_i + S s_i.
        for i in 0..2 {
            let recon = r.liquid_fraction * r.x[i] + r.solid_fraction * r.s[i];
            assert!((recon - z[i]).abs() < 1e-12, "balance i={i}: {recon} vs {}", z[i]);
        }
    }

    /// **Methodology — binary eutectic point.** The eutectic temperature `T_E` is
    /// where the two ideal solubility curves cross so the liquid can be saturated
    /// in **both** solids at once: `x_A^max(T_E) + x_B^max(T_E) = 1`. Because
    /// solubility rises with temperature, `x_A^max + x_B^max > 1` above `T_E`
    /// (both fully dissolvable) and `< 1` below `T_E` (no liquid can hold the
    /// eutectic composition — it freezes). Root-find `f(T) = x_A^max + x_B^max - 1`
    /// by bisection on `[280, 300] K` using this module's own [`ideal_solubility`],
    /// then verify:
    ///   (a) the crossing condition holds at `T_E` to `< 1e-9`;
    ///   (b) just **above** `T_E` (`T_E + 0.5 K`) the **eutectic composition**
    ///       `z = [x_A^max, x_B^max]` flashes all-liquid (`L = 1`);
    ///   (c) just **below** `T_E` (`T_E - 5 K`) that same feed freezes completely
    ///       (`S = 1`, both solid mole fractions `> 0` — co-precipitation).
    /// The sign flip of the split across `T_E` is the defining eutectic signature.
    /// (Testing off the exact boundary avoids the `x_A^max + x_B^max = 1` knife
    /// edge where a 1-ULP rounding either side changes the all-liquid/all-solid
    /// branch — the physics, not the code, is discontinuous there.)
    ///
    /// **Result (2026-08-03).** Bisection gives `T_E = 286.19 K` with
    /// `x_A^max + x_B^max = 1.0` to `< 1e-12` and eutectic composition
    /// `≈ [0.2518, 0.7482]`; at `T_E + 0.5 K` the eutectic feed flashes to
    /// `L = 1.0`; at `T_E - 5 K` it freezes to `S = 1.0` with both `s_i > 0`.
    #[test]
    fn binary_eutectic_crossing_point() {
        let f = |t: f64| ideal_solubility(HF_A, TF_A, t) + ideal_solubility(HF_B, TF_B, t) - 1.0;
        // Bracket: f(280) < 0, f(300) > 0 (both solids below their melting point).
        let (mut lo, mut hi) = (280.0_f64, 300.0_f64);
        assert!(f(lo) < 0.0 && f(hi) > 0.0, "eutectic must be bracketed");
        for _ in 0..200 {
            let mid = 0.5 * (lo + hi);
            if f(mid) > 0.0 {
                hi = mid;
            } else {
                lo = mid;
            }
        }
        let t_e = 0.5 * (lo + hi);
        let xa = ideal_solubility(HF_A, TF_A, t_e);
        let xb = ideal_solubility(HF_B, TF_B, t_e);
        // (a) crossing condition
        assert!((xa + xb - 1.0).abs() < 1e-9, "T_E={t_e}, xa+xb={}", xa + xb);
        assert!((280.0..300.0).contains(&t_e));

        // (b) just above T_E, the eutectic composition is fully liquid.
        let z_eut = [xa, xb];
        let r_hot = flash_sl(&z_eut, &comps(), &ActivityModel::Ideal, t_e + 0.5, SleOptions::default())
            .expect("supra-eutectic flash converges");
        assert!((r_hot.liquid_fraction - 1.0).abs() < 1e-6, "L above T_E = {}", r_hot.liquid_fraction);

        // (c) just below T_E, the same feed freezes completely — BOTH solids.
        let r_cold = flash_sl(&z_eut, &comps(), &ActivityModel::Ideal, t_e - 5.0, SleOptions::default())
            .expect("sub-eutectic flash converges");
        assert!(r_cold.solid_fraction > 1e-3, "expected solids, S = {}", r_cold.solid_fraction);
        // Reconstruct solid moles: S·s_i. Both must be positive (co-precipitation).
        assert!(r_cold.solid_fraction * r_cold.s[0] > 0.0);
        assert!(r_cold.solid_fraction * r_cold.s[1] > 0.0);
    }

    /// **Methodology.** Single-component shortcut (`NestedLoopsSLE.vb:380-401`):
    /// pure A above its melting point (`T = 360 > Tf_A = 350`) is all liquid;
    /// below (`T = 340`) all solid.
    /// **Result (2026-08-03).** `T = 360 K` → `L = 1`, `x = [1, 0]`; `T = 340 K` →
    /// `L = 0`, `s = [1, 0]`, both exact.
    #[test]
    fn single_component_melting_shortcut() {
        let z = [1.0, 0.0];
        let hot = flash_sl(&z, &comps(), &ActivityModel::Ideal, 360.0, SleOptions::default()).unwrap();
        assert_eq!(hot.liquid_fraction, 1.0);
        assert_eq!(hot.x[0], 1.0);

        let cold = flash_sl(&z, &comps(), &ActivityModel::Ideal, 340.0, SleOptions::default()).unwrap();
        assert_eq!(cold.liquid_fraction, 0.0);
        assert_eq!(cold.s[0], 1.0);
    }

    /// **Methodology.** Above every melting point the whole feed must be liquid
    /// (no solubility limit binds). Binary at `T = 400 K` (`> Tf_A, Tf_B`), feed
    /// `z = [0.4, 0.6]`.
    /// **Result (2026-08-03).** `L = 1.0`, `x = [0.4, 0.6]`, `S = 0`.
    #[test]
    fn all_liquid_above_all_melting_points() {
        let z = [0.4, 0.6];
        let r = flash_sl(&z, &comps(), &ActivityModel::Ideal, 400.0, SleOptions::default()).unwrap();
        assert_eq!(r.liquid_fraction, 1.0);
        assert!((r.x[0] - 0.4).abs() < 1e-12 && (r.x[1] - 0.6).abs() < 1e-12);
        assert_eq!(r.solid_fraction, 0.0);
    }

    /// **Methodology.** Non-ideal liquid: an NRTL `gamma > 1` (positive deviation)
    /// **reduces** solubility relative to ideal (`x^max = a^sat / gamma < a^sat`),
    /// so at a fixed feed the solid fraction must be **larger** than the ideal
    /// case. Same A/B system, `T = 320 K`, feed `z = [0.7, 0.3]`; NRTL with a
    /// repulsive A-B interaction. Compares against the ideal split from
    /// `ideal_binary_saturation_split_matches_hand`.
    /// **Result (2026-08-03).** NRTL gives `gamma_A > 1`, so `x_A^max` drops, `L`
    /// falls below the ideal `0.6817` and more A precipitates; the flash still
    /// converges and mole balance holds to `< 1e-12`.
    #[test]
    fn nonideal_positive_deviation_lowers_solubility() {
        use crate::thermo::activity::NrtlParams;
        let z = [0.7, 0.3];
        // Repulsive (positive) A-B interaction energies, cal/mol; alpha = 0.3.
        let nrtl = NrtlParams::from_a_alpha(
            vec![vec![0.0, 600.0], vec![600.0, 0.0]],
            vec![vec![0.0, 0.3], vec![0.3, 0.0]],
        );
        let model = ActivityModel::Nrtl(nrtl);
        let r = flash_sl(&z, &comps(), &model, 320.0, SleOptions::default()).expect("converges");

        let ideal = flash_sl(&z, &comps(), &ActivityModel::Ideal, 320.0, SleOptions::default()).unwrap();
        assert!(
            r.liquid_fraction < ideal.liquid_fraction,
            "positive deviation should lower L: {} !< {}",
            r.liquid_fraction,
            ideal.liquid_fraction
        );
        // Mole balance still holds.
        for i in 0..2 {
            let recon = r.liquid_fraction * r.x[i] + r.solid_fraction * r.s[i];
            assert!((recon - z[i]).abs() < 1e-12);
        }
    }

    /// **Methodology.** Input-validation guards.
    /// **Result (2026-08-03).** empty `z` → `Empty`; length mismatch →
    /// `LengthMismatch`; `T <= 0` → `NonPositiveTemperature`; a `NaN` feed →
    /// `NonFinite`.
    #[test]
    fn input_validation_errors() {
        assert_eq!(
            flash_sl(&[], &[], &ActivityModel::Ideal, 300.0, SleOptions::default()).unwrap_err(),
            SleFlashError::Empty
        );
        assert!(matches!(
            flash_sl(&[0.5, 0.5], &comps()[..1], &ActivityModel::Ideal, 300.0, SleOptions::default())
                .unwrap_err(),
            SleFlashError::LengthMismatch { .. }
        ));
        assert!(matches!(
            flash_sl(&[0.5, 0.5], &comps(), &ActivityModel::Ideal, -1.0, SleOptions::default())
                .unwrap_err(),
            SleFlashError::NonPositiveTemperature(_)
        ));
        assert_eq!(
            flash_sl(&[0.5, f64::NAN], &comps(), &ActivityModel::Ideal, 300.0, SleOptions::default())
                .unwrap_err(),
            SleFlashError::NonFinite
        );
    }
}
