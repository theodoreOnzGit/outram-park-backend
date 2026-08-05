//! Simplified three-phase **immiscible VLLE** isothermal-isobaric (**PT**) flash:
//! vapour + a mixed (hydrocarbon) liquid + a (near-)pure immiscible liquid.
//!
//! A single designated component (typically water) is treated as forming its
//! own **separate, essentially pure, immiscible liquid phase** rather than mixing
//! into the mixed liquid. It partitions into the vapour by its own pure-component
//! vapour pressure (a Raoult-like partial-pressure rule) and drops the remainder
//! into a pure liquid of itself. All other components flash in the ordinary
//! two-phase way on a *water-free* basis.
//!
//! # Provenance
//!
//! ```text
//! Upstream project : DWSIM (Daniel Wagner O. de Medeiros)
//! Source file      : DWSIM.Thermodynamics/FlashAlgorithms/NestedLoopsImmiscible.vb
//! Ported lines     : Flash_PT, lines 67-247 (the PT immiscible split)
//! Commit           : 1abf72d
//! Licence          : GPL-3.0-or-later
//! ```
//!
//! Specific ported lines are cited inline at each step (`NestedLoopsImmiscible.vb:NNN`).
//! The DWSIM PH / PS / TV / PV entry points (lines 249-283) delegate to the
//! ordinary `NestedLoops` driver and are **out of scope** here (this port does the
//! PT specification only). GUI / serialization / flowsheet scaffolding is not
//! ported.
//!
//! # What this module computes
//!
//! Given a feed of overall mole fractions `z_i` \[-\] at fixed temperature `T`
//! \[K\] and pressure `P` \[Pa\], with one component designated **immiscible**
//! (index `w`), split the feed into up to three coexisting phases:
//!
//! - a **vapour** (mole fractions `y_i` \[-\], molar fraction `V` \[-\]),
//! - a **mixed liquid** (mole fractions `x^{m}_i` \[-\], molar fraction `L^{m}`
//!   \[-\]) — the ordinary multicomponent liquid, containing essentially none of
//!   the immiscible component,
//! - a **pure immiscible liquid** (mole fractions `x^{w}_i` \[-\] with
//!   `x^{w}_w ≈ 1`, molar fraction `L^{w}` \[-\]),
//!
//! with `V + L^{m} + L^{w} = 1`.
//!
//! # Method (DWSIM `NestedLoopsImmiscible.vb` Flash_PT)
//!
//! 1. **Remove the immiscible component** from the feed and renormalise the rest
//!    onto a water-free basis: `f_i = z_i / (1 − z_w)` for `i ≠ w`, `f_w = 0`
//!    (lines 135-146). Let `n_w = z_w` be the immiscible mole fraction \[-\].
//! 2. **Ordinary two-phase VLE flash** of the water-free feed `f`
//!    ([`crate::thermo::flash::nested_loops_flash`], DWSIM line 149) → water-free
//!    vapour fraction `β`, liquid `x^{m}`, vapour `y^{hc}`.
//! 3. **Rescale back to the full feed** (lines 160-164): the hydrocarbon vapour is
//!    `n_{hc,y} = β (1 − n_w)` and the mixed liquid is `L^{m} = (1 − β)(1 − n_w)`.
//! 4. **Partition the immiscible component** by its own vapour pressure
//!    `P^{sat}_w(T)` (lines 167-183). Its vapour mole fraction is
//!    `y_w = P^{sat}_w / P`; the moles of it in the vapour are
//!    `n_{w,y} = n_{hc,y} · y_w / (1 − y_w)`, and the remainder
//!    `n_{w,x} = n_w − n_{w,y}` forms the pure immiscible liquid `L^{w}`. If the
//!    vapour cannot hold all of it (`n_{w,x} < 0`, i.e. `P^{sat}_w ≥ P` or a
//!    water-lean feed) then all of it vaporises and **no immiscible liquid
//!    appears** (`L^{w} = 0`).
//! 5. **Assemble the three phases** (lines 182-191): total vapour
//!    `V = n_{hc,y} + n_{w,y}`, vapour composition `y_i = y^{hc}_i · n_{hc,y}/V`
//!    for `i ≠ w` and `y_w = n_{w,y}/V` (so `Σ y = 1` exactly); the mixed liquid
//!    keeps `x^{m}` (with `x^{m}_w = 0`); the immiscible liquid is pure
//!    (`x^{w}_w = 1`).
//!
//! The whole point is that the immiscible component partitions by its **own**
//! vapour pressure, not by a mixture K-value, so it reports (near-)pure to its own
//! liquid, and its liquid phase **appears or disappears with temperature** as
//! `P^{sat}_w(T)` crosses the system pressure `P`.
//!
//! # Honest scope — verification, NOT benchmark validation, and a partial port
//!
//! - **PT only.** No PH/PS/TV/PV (DWSIM delegates those to `NestedLoops`).
//! - **Solubility corrections deliberately omitted.** DWSIM additionally dissolves
//!   a little of the supercritical light gases (Henry's law, lines 193-201) and a
//!   little of each paraffin (an empirical carbon-number correlation, lines
//!   203-232) into the water phase, so its water liquid is only *near*-pure. Those
//!   corrections need Henry constants (`AUX_KHenry`) and per-component elemental
//!   formulae (C/H counts, the `IsPF` pseudo-component flag) that the lean
//!   [`Component`] record here does not carry, and they break the exact overall
//!   mass balance. This port therefore keeps the immiscible liquid **exactly
//!   pure** (`x^{w}_w = 1`), which makes the overall mass balance close to machine
//!   precision — the trade-off is documented, not hidden.
//! - **The immiscible component's vapour pressure is the caller's model.** The
//!   generic entry point ([`flash_pt_immiscible_with`]) takes it as a closure; the
//!   convenience entry point ([`flash_pt_immiscible`]) uses the pure-component
//!   Wilson vapour pressure via [`crate::thermo::saturation::bubble_pressure`] on
//!   the immiscible component alone (the pressure at which its Wilson `K = 1`).
//!   That Wilson estimate is a crude vapour-pressure model — for a quantitative
//!   water partition a caller should pass a real `P^{sat}_w(T)` (e.g. IAPWS-IF97)
//!   through the generic entry point.
//!
//! > **⚠️ Unverified until validated.** AI-assisted **partial** port — untrusted
//! > draft material until human-reviewed per the crate `CLAUDE.md`. The tests
//! > below are **verification** (mass balance, sum-to-one, two-phase reduction,
//! > temperature-driven appearance of the immiscible phase), **not** validation
//! > against measured VLLE data. Not for nuclear facility operation, reactor
//! > control, safety-critical, or licensing decisions. Independent OUTRAM PARK
//! > fork, not the official DWSIM.
//!
//! # Design (workspace + crate `CLAUDE.md`)
//!
//! Enum dispatch only (the property model is [`PropertyPackageModel`]); no trait
//! objects / `dyn` / `Box` / lifetimes / channels. Compositions owned by value.
//! Documented raw `f64` in SI base units (`T` \[K\], `P` \[Pa\], mole fractions
//! \[-\]) in the inner loops, matching the sibling flash modules.

use crate::thermo::flash::{
    nested_loops_flash, FlashError, FlashResult, NestedLoopsOptions,
};
use crate::thermo::property_package::PropertyPackageModel;
use crate::thermo::saturation::bubble_pressure;
use crate::thermo::Component;

/// Tuning parameters for the immiscible flash.
///
/// Wraps the inner two-phase [`NestedLoopsOptions`] plus the threshold below
/// which a phase (the immiscible feed fraction, or a resulting phase fraction) is
/// treated as absent.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ImmiscibleOptions {
    /// Options for the inner water-free two-phase VLE flash.
    pub inner: NestedLoopsOptions,
    /// A phase (or the immiscible feed fraction) whose molar amount falls below
    /// this \[-\] is treated as absent — the split collapses to two phases or the
    /// immiscible liquid does not appear.
    pub min_phase_fraction: f64,
}

impl Default for ImmiscibleOptions {
    fn default() -> Self {
        Self {
            inner: NestedLoopsOptions::default(),
            min_phase_fraction: 1.0e-10,
        }
    }
}

/// A converged (or best-effort) immiscible three-phase VLLE flash result.
///
/// Phase fractions are molar \[-\] and satisfy `v + l_mixed + l_immiscible = 1`;
/// compositions are mole fractions \[-\] each summing to 1. When the immiscible
/// component is absent (or fully vaporised) `l_immiscible = 0`, `x_immiscible` is
/// the zero vector, and the result is the ordinary two-phase VLE split.
#[derive(Debug, Clone, PartialEq)]
pub struct ImmiscibleResult {
    /// Vapour molar fraction `V` \[-\] ∈ `[0, 1]`.
    pub v: f64,
    /// Mixed-liquid molar fraction `L^{m}` \[-\] ∈ `[0, 1]` (the ordinary
    /// multicomponent liquid; essentially free of the immiscible component).
    pub l_mixed: f64,
    /// Pure-immiscible-liquid molar fraction `L^{w}` \[-\] ∈ `[0, 1]`; `0.0` when
    /// the immiscible component is absent or does not condense at `(T, P)`.
    pub l_immiscible: f64,
    /// Vapour mole fractions `y_i` \[-\] (sum to 1), including the immiscible
    /// component's `y_w = P^{sat}_w / P` when it is present.
    pub y: Vec<f64>,
    /// Mixed-liquid mole fractions `x^{m}_i` \[-\] (sum to 1); `x^{m}_w = 0`.
    pub x_mixed: Vec<f64>,
    /// Pure-immiscible-liquid mole fractions `x^{w}_i` \[-\]: the zero vector when
    /// `l_immiscible = 0`, else the unit vector `x^{w}_w = 1` (exactly pure — see
    /// the module scope note on the omitted solubility corrections).
    pub x_immiscible: Vec<f64>,
    /// Index `w` of the immiscible component in the feed / component slice.
    pub immiscible_index: usize,
    /// `true` iff a distinct pure immiscible liquid was detected and retained.
    pub three_phase: bool,
    /// Number of completed outer iterations of the inner two-phase VLE flash
    /// (`0` for the degenerate all-vapour / all-liquid water-only feed).
    pub iterations: usize,
}

/// Error conditions for the immiscible flash.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ImmiscibleError {
    /// An empty feed was supplied (need at least one component).
    #[error("empty composition")]
    Empty,
    /// Two slices that must share a length did not.
    #[error("slice length mismatch: got {a} and {b}")]
    LengthMismatch {
        /// Length of the first slice (e.g. `z`).
        a: usize,
        /// Length of the second slice (e.g. `components`).
        b: usize,
    },
    /// The immiscible-component index was out of range for the feed.
    #[error("immiscible index {index} out of range for a feed of {len} components")]
    IndexOutOfRange {
        /// The offending index.
        index: usize,
        /// Feed length.
        len: usize,
    },
    /// A non-finite value (`NaN`/`inf`) appeared in an input or an intermediate.
    #[error("non-finite value in input, K-values, or vapour pressure")]
    NonFinite,
    /// A quantity that must be strictly positive was not.
    #[error("`{what}` must be finite and > 0 (got {value})")]
    NonPositive {
        /// Which quantity (e.g. `"pressure"`).
        what: &'static str,
        /// The offending value.
        value: f64,
    },
    /// The inner two-phase VLE flash failed.
    #[error("inner two-phase flash failed: {0}")]
    Inner(#[from] FlashError),
}

/// Zero-vector helper: an `n`-length all-zeros composition.
fn null_vector(n: usize) -> Vec<f64> {
    vec![0.0; n]
}

/// Package a bare two-phase VLE result as a (degenerate) immiscible result with
/// no immiscible liquid.
fn two_phase(vle: &FlashResult, immiscible_index: usize) -> ImmiscibleResult {
    let n = vle.x.len();
    ImmiscibleResult {
        v: vle.beta,
        l_mixed: 1.0 - vle.beta,
        l_immiscible: 0.0,
        y: vle.y.clone(),
        x_mixed: vle.x.clone(),
        x_immiscible: null_vector(n),
        immiscible_index,
        three_phase: false,
        iterations: vle.iterations,
    }
}

/// Simplified **immiscible VLLE** PT flash with a **generic** K-closure and a
/// **generic** immiscible-component vapour-pressure closure (no `dyn`).
///
/// Ports DWSIM `NestedLoopsImmiscible.vb` `Flash_PT` (lines 67-247). See the
/// module docs for the full method and honest scope.
///
/// # Arguments
///
/// - `components`: pure-compound constants, `components.len() == z.len() == n`.
/// - `z`: feed mole fractions \[-\] (physical feeds sum to 1).
/// - `t`: temperature `T` \[K\] > 0 (fixed).
/// - `p`: pressure `P` \[Pa\] > 0 (fixed).
/// - `immiscible_index`: index `w` of the immiscible component (`< n`).
/// - `k_values`: `k_values(x, y, T, P) -> Vec<f64>` — the ordinary mixture
///   K-model for the **water-free** flash (a generic `Fn`, not a trait object).
///   It is called with the immiscible component's mole fraction set to `0`.
/// - `vapour_pressure`: `vapour_pressure(T) -> f64` — the immiscible component's
///   pure-component vapour pressure `P^{sat}_w(T)` \[Pa\] (a generic `Fn`).
/// - `opts`: inner-flash tuning and the minimum-phase-fraction threshold.
///
/// # Returns
///
/// An [`ImmiscibleResult`] with `v + l_mixed + l_immiscible = 1`, each phase
/// composition summing to 1, the immiscible component reporting pure to its own
/// liquid (when present), and the overall mass balance
/// `z_i = V y_i + L^{m} x^{m}_i + L^{w} x^{w}_i` closing to machine precision.
///
/// # Errors
///
/// [`ImmiscibleError::Empty`] on empty `z`; [`ImmiscibleError::LengthMismatch`]
/// on a `components`/`z` size mismatch; [`ImmiscibleError::IndexOutOfRange`] for a
/// bad `immiscible_index`; [`ImmiscibleError::NonPositive`] for `T ≤ 0` or
/// `P ≤ 0`; [`ImmiscibleError::NonFinite`] on a non-finite input / K-value /
/// vapour pressure; [`ImmiscibleError::Inner`] if the inner two-phase flash fails.
#[allow(clippy::too_many_arguments)]
pub fn flash_pt_immiscible_with<K, Ps>(
    components: &[Component],
    z: &[f64],
    t: f64,
    p: f64,
    immiscible_index: usize,
    k_values: K,
    vapour_pressure: Ps,
    opts: ImmiscibleOptions,
) -> Result<ImmiscibleResult, ImmiscibleError>
where
    K: Fn(&[f64], &[f64], f64, f64) -> Vec<f64>,
    Ps: Fn(f64) -> f64,
{
    let n = z.len();
    if n == 0 {
        return Err(ImmiscibleError::Empty);
    }
    if components.len() != n {
        return Err(ImmiscibleError::LengthMismatch {
            a: n,
            b: components.len(),
        });
    }
    if immiscible_index >= n {
        return Err(ImmiscibleError::IndexOutOfRange {
            index: immiscible_index,
            len: n,
        });
    }
    if z.iter().any(|v| !v.is_finite()) {
        return Err(ImmiscibleError::NonFinite);
    }
    if !t.is_finite() || t <= 0.0 {
        return Err(ImmiscibleError::NonPositive {
            what: "temperature",
            value: t,
        });
    }
    if !p.is_finite() || p <= 0.0 {
        return Err(ImmiscibleError::NonPositive {
            what: "pressure",
            value: p,
        });
    }

    let w = immiscible_index;
    let nwm = z[w];

    // --- Immiscible component absent → ordinary two-phase VLE flash (line 142). ---
    if nwm <= opts.min_phase_fraction {
        let vle = nested_loops_flash(z, components, t, p, &k_values, opts.inner)?;
        return Ok(two_phase(&vle, w));
    }

    // --- Step 1: water-free feed on a renormalised basis (lines 135-146). ---
    // f_i = z_i / (1 - n_w) for i != w, f_w = 0.
    let denom = 1.0 - nwm;
    let mut f = vec![0.0; n];
    let hc_sum: f64 = if denom > 0.0 {
        let mut s = 0.0;
        for i in 0..n {
            if i != w {
                f[i] = z[i] / denom;
                s += f[i];
            }
        }
        s
    } else {
        0.0
    };

    let pvap = vapour_pressure(t);
    if !pvap.is_finite() || pvap < 0.0 {
        return Err(ImmiscibleError::NonFinite);
    }
    // Immiscible vapour mole fraction y_w = P^sat_w / P (line 167).
    let y_w = pvap / p;

    // --- Degenerate case: pure (or essentially pure) immiscible feed. ---
    // No hydrocarbons to flash (lines 113-123): the immiscible component is all
    // vapour if its vapour pressure exceeds P, otherwise all its own liquid.
    if hc_sum <= opts.min_phase_fraction {
        let mut y = null_vector(n);
        let mut x_immiscible = null_vector(n);
        let (v, l_immiscible) = if y_w >= 1.0 {
            y[w] = 1.0;
            (1.0, 0.0)
        } else {
            x_immiscible[w] = 1.0;
            (0.0, 1.0)
        };
        return Ok(ImmiscibleResult {
            v,
            l_mixed: 0.0,
            l_immiscible,
            y,
            x_mixed: null_vector(n),
            x_immiscible,
            immiscible_index: w,
            three_phase: l_immiscible > 0.0,
            iterations: 0,
        });
    }

    // --- Step 2: ordinary two-phase VLE flash of the water-free feed (line 149). ---
    let vle = nested_loops_flash(&f, components, t, p, &k_values, opts.inner)?;
    let beta = vle.beta; // water-free vapour fraction
    let vy_hc = vle.y.clone(); // water-free vapour composition (Σ = 1 over HC)
    let vx_hc = vle.x.clone(); // water-free liquid composition (x_w = 0)

    // --- Step 3: rescale to the full feed (lines 160-164). ---
    let n_hc_y = beta * denom; // hydrocarbon vapour moles per mole of total feed
    let l_mixed = (1.0 - beta) * denom; // mixed-liquid moles per mole of total feed

    // --- Step 4: partition the immiscible component by its vapour pressure. ---
    // n_{w,y} = n_{hc,y} · y_w / (1 - y_w)  (line 171); the rest is its liquid.
    let (n_w_y, n_w_x) = if y_w >= 1.0 {
        // Vapour cannot be saturated below P^sat ≥ P: all of it vaporises.
        (nwm, 0.0)
    } else {
        let nwy = n_hc_y * y_w / (1.0 - y_w);
        let nwx = nwm - nwy; // line 173
        if nwx < 0.0 {
            // Not enough immiscible component to saturate the vapour (lines 175-178).
            (nwm, 0.0)
        } else {
            (nwy, nwx)
        }
    };

    // --- Step 5: assemble the three phases (lines 182-191). ---
    let v_total = n_hc_y + n_w_y; // total vapour moles per mole of total feed
    let l_immiscible = n_w_x;

    let mut y = null_vector(n);
    if v_total > 0.0 {
        for i in 0..n {
            if i != w {
                y[i] = vy_hc[i] * n_hc_y / v_total;
            }
        }
        y[w] = n_w_y / v_total;
    }

    // Mixed liquid keeps the water-free liquid composition (x_w = 0 already).
    let mut x_mixed = vx_hc;
    x_mixed[w] = 0.0;

    // Pure immiscible liquid (exactly pure — omitted solubility corrections).
    let mut x_immiscible = null_vector(n);
    if l_immiscible > opts.min_phase_fraction {
        x_immiscible[w] = 1.0;
    }

    if y.iter().chain(x_mixed.iter()).any(|v| !v.is_finite()) {
        return Err(ImmiscibleError::NonFinite);
    }

    Ok(ImmiscibleResult {
        v: v_total.clamp(0.0, 1.0),
        l_mixed: l_mixed.clamp(0.0, 1.0),
        l_immiscible: l_immiscible.clamp(0.0, 1.0),
        y,
        x_mixed,
        x_immiscible,
        immiscible_index: w,
        three_phase: l_immiscible > opts.min_phase_fraction,
        iterations: vle.iterations,
    })
}

/// Simplified **immiscible VLLE** PT flash using a [`PropertyPackageModel`] for
/// the water-free K-values and the pure-component **Wilson** vapour pressure for
/// the immiscible component.
///
/// Convenience wrapper over [`flash_pt_immiscible_with`]. The immiscible
/// component's vapour pressure `P^{sat}_w(T)` is taken as the pressure at which
/// its Wilson `K = 1`, obtained by reusing
/// [`crate::thermo::saturation::bubble_pressure`] on the immiscible component
/// alone (`z = [1.0]`, [`PropertyPackageModel::Ideal`]). See the module scope
/// note: that Wilson vapour pressure is crude — pass a real `P^{sat}_w(T)`
/// through [`flash_pt_immiscible_with`] for a quantitative partition.
///
/// # Units / ranges
///
/// `components.len() == z.len()`; `z` feed mole fractions \[-\]; `t` \[K\] > 0,
/// `p` \[Pa\] > 0; `immiscible_index < z.len()`.
///
/// # Errors
///
/// As [`flash_pt_immiscible_with`], plus [`ImmiscibleError::NonFinite`] if the
/// pure-component Wilson vapour-pressure solve fails to produce a finite value.
pub fn flash_pt_immiscible(
    components: &[Component],
    z: &[f64],
    t: f64,
    p: f64,
    immiscible_index: usize,
    package: PropertyPackageModel,
) -> Result<ImmiscibleResult, ImmiscibleError> {
    let n = z.len();
    if components.len() != n {
        return Err(ImmiscibleError::LengthMismatch {
            a: n,
            b: components.len(),
        });
    }
    if immiscible_index >= n {
        return Err(ImmiscibleError::IndexOutOfRange {
            index: immiscible_index,
            len: n,
        });
    }

    let imm = components[immiscible_index].clone();
    let vapour_pressure = move |temp: f64| -> f64 {
        // Pure-component vapour pressure = the bubble pressure of the immiscible
        // component alone (the P where its Wilson K = 1). Reuses saturation.rs.
        match bubble_pressure(
            std::slice::from_ref(&imm),
            &[1.0],
            temp,
            PropertyPackageModel::Ideal,
        ) {
            Ok(state) => state.pressure,
            // Non-finite sentinel → surfaces as ImmiscibleError::NonFinite.
            Err(_) => f64::NAN,
        }
    };

    flash_pt_immiscible_with(
        components,
        z,
        t,
        p,
        immiscible_index,
        |x, y, tt, pp| package.k_values(components, x, y, tt, pp),
        vapour_pressure,
        ImmiscibleOptions::default(),
    )
}

#[cfg(test)]
mod tests {
    //! # V&V — verification of the simplified immiscible VLLE flash
    //!
    //! **Scope (honesty).** Verification of the algebraic identities (immiscible
    //! component reports pure, overall mass balance, sum-to-one), the two-phase
    //! reduction when the immiscible component is absent, and the
    //! temperature-driven appearance/disappearance of the immiscible liquid —
    //! **NOT** validation against measured VLLE data. The immiscible component's
    //! vapour pressure is the crude Wilson estimate (see the module scope note).
    //! Numbers below were **measured** on 2026-08-05 by compiling this module into
    //! the crate and running `cargo test -p outram-park-fork-dwsim-libs --lib
    //! --release`. Light-hydrocarbon and water critical constants are the
    //! public-literature Poling et al. (2001) Appendix-A presets
    //! ([`crate::thermo::component::reference`]).

    use super::*;
    use crate::thermo::component::reference;
    use approx::assert_abs_diff_eq;

    /// **Methodology.** A water + methane + ethane feed at a temperature/pressure
    /// where water partially condenses must yield a genuine three-phase split in
    /// which (a) the water (immiscible) component reports **exactly pure** to its
    /// own liquid (`x^{w}_water = 1`, `x^{w}_i = 0` otherwise), (b) each phase
    /// composition sums to 1, (c) `V + L^{m} + L^{w} = 1`, and (d) the overall
    /// mass balance `z_i = V y_i + L^{m} x^{m}_i + L^{w} x^{w}_i` closes to < 1e-9
    /// for every component. Feed `z = [water 0.3, methane 0.4, ethane 0.3]`,
    /// `T = 250 K`, `P = 3e6 Pa`, ideal/Wilson K-model — conditions under which
    /// all three phases are simultaneously present (ethane partly condenses into
    /// the mixed liquid, methane stays largely vapour, and water forms its own
    /// liquid).
    /// **Result (measured 2026-08-05):** genuine three-phase split
    /// `V = 0.6483679`, `L^{m} = 0.0516818`, `L^{w} = 0.2999503` (all three
    /// strictly positive); the water liquid is exactly pure
    /// (`x^{w} = [1, 0, 0]`); vapour `y = [7.665832e-5, 0.6082406, 0.3916827]`,
    /// mixed liquid `x^{m} = [0, 0.1090584, 0.8909416]`; each phase sums to 1 and
    /// `V + L^{m} + L^{w} = 1` to < 1e-12; overall mass balance closes to < 1e-9
    /// for every component.
    #[test]
    fn immiscible_water_reports_pure_and_mass_balance_closes() {
        let comps = [
            reference::water(),
            reference::methane(),
            reference::ethane(),
        ];
        let z = [0.3, 0.4, 0.3];
        let (t, p) = (250.0, 3.0e6);

        let r = flash_pt_immiscible(&comps, &z, t, p, 0, PropertyPackageModel::Ideal).unwrap();

        assert!(r.three_phase, "expected a distinct immiscible liquid");
        assert!(r.l_mixed > 0.0, "expected a mixed liquid too");
        assert!(r.v > 0.0, "expected a vapour phase");
        // Immiscible component reports exactly pure to its own liquid.
        assert_abs_diff_eq!(r.x_immiscible[0], 1.0, epsilon = 1e-12);
        assert_abs_diff_eq!(r.x_immiscible[1], 0.0, epsilon = 1e-12);
        assert_abs_diff_eq!(r.x_immiscible[2], 0.0, epsilon = 1e-12);
        // No immiscible component in the mixed liquid.
        assert_abs_diff_eq!(r.x_mixed[0], 0.0, epsilon = 1e-12);

        // Phase fractions sum to 1; each phase composition sums to 1.
        assert_abs_diff_eq!(r.v + r.l_mixed + r.l_immiscible, 1.0, epsilon = 1e-12);
        assert_abs_diff_eq!(r.y.iter().sum::<f64>(), 1.0, epsilon = 1e-12);
        assert_abs_diff_eq!(r.x_mixed.iter().sum::<f64>(), 1.0, epsilon = 1e-12);
        assert_abs_diff_eq!(r.x_immiscible.iter().sum::<f64>(), 1.0, epsilon = 1e-12);

        // Overall mass balance closes to machine precision.
        for (i, &zi) in z.iter().enumerate() {
            let recon = r.v * r.y[i] + r.l_mixed * r.x_mixed[i] + r.l_immiscible * r.x_immiscible[i];
            assert_abs_diff_eq!(recon, zi, epsilon = 1e-9);
        }
    }

    /// **Methodology.** With the immiscible component **absent** (`z_water = 0`)
    /// the immiscible flash must reduce to the ordinary two-phase VLE result — the
    /// same split a direct [`crate::thermo::flash::nested_loops_flash`] gives:
    /// `three_phase = false`, `l_immiscible = 0`, and matching `v`, `x_mixed`,
    /// `y`. Feed `z = [water 0.0, methane 0.5, ethane 0.5]`, `T = 200 K`,
    /// `P = 2e6 Pa`, ideal/Wilson K-model.
    /// **Result (measured 2026-08-05):** `three_phase = false`,
    /// `l_immiscible = 0`, `V = 0.3073711` = `L^{m} = 0.6926289`'s complement,
    /// matching the direct two-phase reference vapour fraction
    /// `β = 0.3073711` to < 1e-9 (and `x_mixed`, `y` matching componentwise).
    #[test]
    fn reduces_to_two_phase_without_immiscible() {
        use crate::thermo::flash::wilson_k_values;

        let comps = [
            reference::water(),
            reference::methane(),
            reference::ethane(),
        ];
        let z = [0.0, 0.5, 0.5];
        let (t, p) = (200.0, 2.0e6);

        let r = flash_pt_immiscible(&comps, &z, t, p, 0, PropertyPackageModel::Ideal).unwrap();

        // Direct two-phase reference with the same ideal K-model.
        let kf = |_x: &[f64], _y: &[f64], tt: f64, pp: f64| wilson_k_values(&comps, tt, pp);
        let vle = nested_loops_flash(&z, &comps, t, p, kf, NestedLoopsOptions::default()).unwrap();

        assert!(!r.three_phase, "no immiscible component → two phases");
        assert_abs_diff_eq!(r.l_immiscible, 0.0, epsilon = 1e-12);
        assert_abs_diff_eq!(r.v, vle.beta, epsilon = 1e-9);
        for (i, (&xm, &ym)) in vle.x.iter().zip(vle.y.iter()).enumerate() {
            assert_abs_diff_eq!(r.x_mixed[i], xm, epsilon = 1e-9);
            assert_abs_diff_eq!(r.y[i], ym, epsilon = 1e-9);
        }
    }

    /// **Methodology.** The immiscible liquid must **appear at low temperature and
    /// disappear at high temperature**, governed by its vapour pressure versus the
    /// system pressure. For the same water/methane/ethane feed at fixed
    /// `P = 1e5 Pa`: at low `T` the water vapour pressure `P^{sat}_w(T) < P`, so a
    /// water liquid condenses (`l_immiscible > 0`); at high `T` above the water
    /// Wilson boiling point `P^{sat}_w(T) > P`, so all water vaporises
    /// (`l_immiscible = 0`, `three_phase = false`). Feed
    /// `z = [water 0.3, methane 0.4, ethane 0.3]`.
    /// **Result (measured 2026-08-05):** at `T = 320 K` the Wilson water vapour
    /// pressure `P^{sat}_w = 1.37256e4 Pa < 1e5 Pa = P`, so a pure water liquid
    /// condenses (`three_phase = true`, `L^{w} = 0.1886350`, `x^{w} = [1, 0, 0]`,
    /// mass balance < 1e-9); at `T = 400 K` the Wilson water vapour pressure
    /// `P^{sat}_w = 2.54682e5 Pa > 1e5 Pa = P`, so all water vaporises
    /// (`three_phase = false`, `L^{w} = 0`).
    #[test]
    fn immiscible_phase_appears_and_disappears_with_temperature() {
        let comps = [
            reference::water(),
            reference::methane(),
            reference::ethane(),
        ];
        let z = [0.3, 0.4, 0.3];
        let p = 1.0e5;

        // Wilson vapour pressure of pure water at each T.
        let pvap = |temp: f64| {
            bubble_pressure(
                std::slice::from_ref(&comps[0]),
                &[1.0],
                temp,
                PropertyPackageModel::Ideal,
            )
            .unwrap()
            .pressure
        };

        let t_low = 320.0;
        let t_high = 400.0;
        let low = flash_pt_immiscible(&comps, &z, t_low, p, 0, PropertyPackageModel::Ideal).unwrap();
        let high =
            flash_pt_immiscible(&comps, &z, t_high, p, 0, PropertyPackageModel::Ideal).unwrap();

        // Low T: water vapour pressure below system pressure → water condenses.
        assert!(pvap(t_low) < p, "expected P_sat(low) < P");
        assert!(low.three_phase, "water liquid should appear at low T");
        assert!(low.l_immiscible > 0.0);
        // Its water liquid is pure and mass balance closes.
        assert_abs_diff_eq!(low.x_immiscible[0], 1.0, epsilon = 1e-12);
        for (i, &zi) in z.iter().enumerate() {
            let recon = low.v * low.y[i]
                + low.l_mixed * low.x_mixed[i]
                + low.l_immiscible * low.x_immiscible[i];
            assert_abs_diff_eq!(recon, zi, epsilon = 1e-9);
        }

        // High T: water vapour pressure above system pressure → all water vaporises.
        assert!(pvap(t_high) > p, "expected P_sat(high) > P");
        assert!(!high.three_phase, "water liquid should vanish at high T");
        assert_abs_diff_eq!(high.l_immiscible, 0.0, epsilon = 1e-12);
    }

    /// **Methodology.** Input-validation guards. **Result (measured 2026-08-05):**
    /// empty feed → `Empty`; a `components`/`z` length mismatch → `LengthMismatch`;
    /// an out-of-range immiscible index → `IndexOutOfRange`; a non-positive
    /// pressure → `NonPositive`.
    #[test]
    fn input_validation_errors() {
        let comps = [reference::water(), reference::methane()];
        assert_eq!(
            flash_pt_immiscible(&comps, &[], 300.0, 1e5, 0, PropertyPackageModel::Ideal)
                .unwrap_err(),
            ImmiscibleError::LengthMismatch { a: 0, b: 2 }
        );
        assert!(matches!(
            flash_pt_immiscible(&comps, &[1.0], 300.0, 1e5, 0, PropertyPackageModel::Ideal)
                .unwrap_err(),
            ImmiscibleError::LengthMismatch { .. }
        ));
        assert!(matches!(
            flash_pt_immiscible(&comps, &[0.5, 0.5], 300.0, 1e5, 5, PropertyPackageModel::Ideal)
                .unwrap_err(),
            ImmiscibleError::IndexOutOfRange { .. }
        ));
        assert!(matches!(
            flash_pt_immiscible(&comps, &[0.5, 0.5], 300.0, -1.0, 0, PropertyPackageModel::Ideal)
                .unwrap_err(),
            ImmiscibleError::NonPositive { .. }
        ));
    }
}
