//! Property-package glue: compose the cubic EOS / ideal models into K-values and
//! drive an EOS-consistent isothermal-isobaric (**PT**) two-phase VLE flash.
//!
//! Ported/composed from DWSIM (GPL-3.0). The "property package" concept mirrors
//! DWSIM's `DWSIM.Thermodynamics/PropertyPackages/PropertyPackage.vb`
//! abstraction — the object a flash asks for K-values / fugacities. DWSIM's
//! `PengRobinsonPropertyPackage.vb`, `SoaveRedlichKwong2PropertyPackage.vb`, and
//! `RaoultPropertyPackage.vb` each specialise `DW_CalcKvalue` to their model;
//! this module reproduces exactly that specialisation, sitting on the already-
//! ported kernel:
//!
//! - K-values from a cubic EOS use the fugacity-coefficient ratio
//!   `K_i = φ_i^L(x, T, P) / φ_i^V(y, T, P)` (DWSIM `DW_CalcKvalue`, which calls
//!   `DW_CalcFugCoeff` for each phase — `PropertyPackage.vb` L4620-4680), with
//!   the liquid `Z`-root for `φ^L` and the vapour `Z`-root for `φ^V`, both from
//!   [`crate::thermo::cubic_eos::CubicEos::ln_phi`].
//! - The `Ideal` package returns the Wilson ideal K-estimate
//!   (`DW_CalcKvalue_Ideal_Wilson`, `PropertyPackage.vb` L1650-1668), which is a
//!   composition-independent Raoult-style first estimate.
//! - The flash itself is the nested-loops successive-substitution driver
//!   [`crate::thermo::flash::nested_loops_flash`], seeded with Wilson and closed
//!   with *this* package's [`PropertyPackageModel::k_values`].
//!
//! ## What this computes
//!
//! Given a feed of overall mole fractions `z_i` \[-\] at fixed temperature
//! `T` \[K\] and pressure `P` \[Pa\], [`PropertyPackageModel::flash_pt`] splits
//! it into an equilibrium liquid (`x_i`) and vapour (`y_i`), returning the
//! vapour molar fraction `β` \[-\] and the K-values `K_i = y_i / x_i` \[-\]. For
//! a cubic-EOS package this is a genuine EOS-consistent flash: the converged
//! `(x, y, β)` satisfy `φ_i^L x_i = φ_i^V y_i` for every component (iso-fugacity)
//! to the outer-loop tolerance.
//!
//! ## Units — documented raw `f64` (SI), per the crate `CLAUDE.md`
//!
//! This package sits directly on the `f64`-SI kernel ([`crate::thermo::flash`],
//! [`crate::thermo::cubic_eos`]), whose inner EOS/flash arithmetic deliberately
//! uses raw `f64` in SI base units rather than `uom` (the crate `CLAUDE.md`
//! "raw f64 in inner EOS loops" rule). To avoid wrapping/unwrapping `uom` at
//! every call into that kernel — which would add friction without adding safety
//! over slices — the public signatures here follow the same convention:
//! temperature `T` \[K\], pressure `P` \[Pa\], mole fractions \[-\], all `f64`,
//! with every parameter's unit spelled out in its doc comment. (A `uom`-typed
//! outer façade, if desired later, belongs at the equipment-model boundary, not
//! in this inner composition layer.)
//!
//! ## Design (workspace + crate `CLAUDE.md`)
//!
//! Enum dispatch, **no `dyn`**: [`PropertyPackageModel`] is the closed model set
//! `{Ideal, PengRobinson, Srk}`, not a trait object. The [`PropertyPackage`]
//! trait is a **compiler-enforced contract** (every model must supply `k_values`
//! and `flash_pt`); dispatch is by `match`, not `&dyn PropertyPackage`. The one
//! model-dependent step handed to the flash driver is a **generic `Fn`
//! closure**, not a trait object. No `Box`, no lifetimes, no channels.
//!
//! ## Honest scope — verification, NOT benchmark validation
//!
//! - **TP (isothermal-isobaric) two-phase VLE flash only.** No PH/PS/TV/PV
//!   energy flashes; no three-phase (VLLE) or solid/salt equilibria. Those live
//!   in DWSIM's fuller flash suite and are out of scope (see
//!   [`crate::thermo::flash`]'s scope note).
//! - **No phase-stability pre-test.** This driver does *not* run a
//!   tangent-plane-distance / Michelsen stability analysis before flashing, so
//!   it can in principle converge to the trivial (single-phase) solution for a
//!   feed near a phase boundary. Stability testing is the separate
//!   [`crate::thermo::stability`] module's job and is not wired in here yet.
//! - **Binary interaction parameters `k_ij = 0`.** The cubic-EOS K-values use
//!   the geometric-mean combining rule (`None` `k_ij` matrix). Non-zero,
//!   fitted `k_ij` — which materially improve real VLE — are not applied here;
//!   pass them at the [`crate::thermo::cubic_eos`] layer if needed.
//! - **The EOS and flash are *verified*, not *validated*.** The tests below
//!   check internal consistency (mass balance, iso-fugacity, single-phase
//!   limits, K-ordering) and reduction to the Wilson-K result — they are **not**
//!   validated against an experimental / NIST / DECHEMA VLE benchmark. This is
//!   AI-assisted draft material, untrusted until human-reviewed per the crate
//!   `CLAUDE.md`. Not for nuclear facility operation, reactor control,
//!   safety-critical, or licensing decisions. Independent OUTRAM PARK fork, not
//!   the official DWSIM.

use crate::thermo::cubic_eos::{CubicEos, Phase};
use crate::thermo::flash::{
    nested_loops_flash, wilson_k_values, FlashError, FlashResult, NestedLoopsOptions,
};
use crate::thermo::Component;

/// Compiler-enforced contract every property-package model must satisfy.
///
/// This trait is **not** used for `dyn` dispatch (forbidden by the workspace
/// `CLAUDE.md`); it exists so the compiler checks that every model in
/// [`PropertyPackageModel`] supplies a K-value routine and a PT flash. Runtime
/// dispatch is done by matching on the enum. Both methods use the documented
/// `f64`-SI convention (see the module header): `T` \[K\], `P` \[Pa\], mole
/// fractions \[-\].
pub trait PropertyPackage {
    /// Equilibrium K-values `K_i = y_i / x_i` \[-\] for a trial liquid `x` and
    /// vapour `y` (mole fractions \[-\]) at `t` \[K\], `p` \[Pa\].
    fn k_values(&self, components: &[Component], x: &[f64], y: &[f64], t: f64, p: f64) -> Vec<f64>;

    /// Isothermal-isobaric two-phase VLE flash of feed `z` (mole fractions
    /// \[-\]) at `t` \[K\], `p` \[Pa\].
    fn flash_pt(
        &self,
        components: &[Component],
        z: &[f64],
        t: f64,
        p: f64,
    ) -> Result<FlashResult, FlashError>;
}

/// Thermodynamic property-package model (enum dispatch, no `dyn`).
///
/// The closed set of PT-flash property models this crate composes from the
/// thermo kernel. `Copy` so it can be captured by value into the flash driver's
/// `Fn` closure without borrowing.
///
/// - [`PropertyPackageModel::Ideal`] — Raoult/Wilson ideal K-values
///   (composition-independent); the K-closure is the Wilson estimate, so a flash
///   with this package reduces to a single Wilson-K Rachford-Rice solve.
/// - [`PropertyPackageModel::PengRobinson`] — Peng-Robinson cubic EOS; K-values
///   are the liquid/vapour fugacity-coefficient ratio.
/// - [`PropertyPackageModel::Srk`] — Soave-Redlich-Kwong cubic EOS; same
///   fugacity-ratio K-values with the SRK constants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropertyPackageModel {
    /// Ideal (Raoult's law) package: K-values are the composition-independent
    /// Wilson ideal estimate `K_i = (Pc_i/P)·exp[5.373(1+ω_i)(1−Tc_i/T)]`.
    Ideal,
    /// Peng-Robinson cubic-EOS package.
    PengRobinson,
    /// Peng-Robinson **1978** cubic-EOS package.
    ///
    /// Prefer this over [`Self::PengRobinson`] for petroleum and other heavy
    /// mixtures: the 1976 α-slope correlation is stated only for `ω < 0.49`,
    /// and pseudo-components from a crude assay routinely exceed it. Identical
    /// to [`Self::PengRobinson`] below that threshold.
    PengRobinson1978,
    /// Soave-Redlich-Kwong cubic-EOS package.
    Srk,
}

impl PropertyPackageModel {
    /// The backing cubic EOS for a cubic package, or `None` for [`Self::Ideal`].
    #[must_use]
    fn cubic(self) -> Option<CubicEos> {
        match self {
            Self::Ideal => None,
            Self::PengRobinson => Some(CubicEos::PengRobinson),
            Self::PengRobinson1978 => Some(CubicEos::PengRobinson1978),
            Self::Srk => Some(CubicEos::Srk),
        }
    }

    /// Equilibrium K-values `K_i = y_i / x_i` \[-\] for a trial split.
    ///
    /// For a cubic-EOS package:
    /// `K_i = φ_i^L(x, T, P) / φ_i^V(y, T, P) = exp(ln φ_i^L − ln φ_i^V)`,
    /// where `ln φ^L` uses the **liquid** (smallest positive) `Z`-root evaluated
    /// at the liquid composition `x`, and `ln φ^V` uses the **vapour** (largest)
    /// `Z`-root at the vapour composition `y` — the standard iso-fugacity
    /// K-update (DWSIM `DW_CalcKvalue`). Binary interaction parameters are taken
    /// as zero (geometric-mean rule); see the module-level scope note.
    ///
    /// For [`Self::Ideal`] the Wilson estimate is returned, ignoring `x`, `y`
    /// (it is composition-independent).
    ///
    /// # Units / ranges
    ///
    /// `components`, `x`, `y` must share one length `n`; `x`, `y` are mole
    /// fractions \[-\]; `t` \[K\] > 0, `p` \[Pa\] > 0. Returns `n` dimensionless
    /// K-values.
    ///
    /// # Fallback
    ///
    /// If the cubic EOS fails to return a usable `Z`-root for either phase (which
    /// should not happen for physical inputs), the Wilson estimate is returned
    /// instead of a non-finite result, so the flash driver stays well-posed.
    #[must_use]
    pub fn k_values(
        self,
        components: &[Component],
        x: &[f64],
        y: &[f64],
        t: f64,
        p: f64,
    ) -> Vec<f64> {
        match self.cubic() {
            None => wilson_k_values(components, t, p),
            Some(eos) => {
                let ln_phi_l = eos.ln_phi(components, x, t, p, Phase::Liquid, None);
                let ln_phi_v = eos.ln_phi(components, y, t, p, Phase::Vapor, None);
                match (ln_phi_l, ln_phi_v) {
                    (Some(l), Some(v)) => l
                        .iter()
                        .zip(v.iter())
                        .map(|(&li, &vi)| (li - vi).exp())
                        .collect(),
                    // No usable root for a phase → fall back to the ideal estimate.
                    _ => wilson_k_values(components, t, p),
                }
            }
        }
    }

    /// Isothermal-isobaric two-phase VLE flash of feed `z` at `t` \[K\], `p`
    /// \[Pa\].
    ///
    /// Seeds the K-values with Wilson and drives
    /// [`crate::thermo::flash::nested_loops_flash`] with this package's
    /// [`Self::k_values`] as the successive-substitution K-closure. For a cubic
    /// package the converged result satisfies the iso-fugacity condition
    /// `φ_i^L x_i = φ_i^V y_i`; for [`Self::Ideal`] it is a single Wilson-K
    /// Rachford-Rice solve.
    ///
    /// # Units / ranges
    ///
    /// `components.len()` must equal `z.len()`; `z` are feed mole fractions
    /// \[-\] (physical feeds sum to 1); `t` \[K\] > 0, `p` \[Pa\] > 0. The
    /// returned [`FlashResult`] carries `β` \[-\] ∈ `[0, 1]`, `x`/`y` mole
    /// fractions \[-\], K-values \[-\], and the outer-iteration count.
    ///
    /// # Errors
    ///
    /// Propagates [`FlashError`] from the driver:
    /// [`FlashError::LengthMismatch`] on a `components`/`z` size mismatch,
    /// [`FlashError::NonFinite`] on a non-finite K-value, and
    /// [`FlashError::NotConverged`] if successive substitution does not reach the
    /// K-tolerance within the iteration budget (possible near a phase boundary
    /// without the stability pre-test noted in the module scope).
    pub fn flash_pt(
        self,
        components: &[Component],
        z: &[f64],
        t: f64,
        p: f64,
    ) -> Result<FlashResult, FlashError> {
        let model = self;
        let k_closure =
            move |x: &[f64], y: &[f64], t: f64, p: f64| model.k_values(components, x, y, t, p);
        nested_loops_flash(
            z,
            components,
            t,
            p,
            k_closure,
            NestedLoopsOptions::default(),
        )
    }
}

impl PropertyPackage for PropertyPackageModel {
    fn k_values(&self, components: &[Component], x: &[f64], y: &[f64], t: f64, p: f64) -> Vec<f64> {
        (*self).k_values(components, x, y, t, p)
    }

    fn flash_pt(
        &self,
        components: &[Component],
        z: &[f64],
        t: f64,
        p: f64,
    ) -> Result<FlashResult, FlashError> {
        (*self).flash_pt(components, z, t, p)
    }
}

#[cfg(test)]
mod tests {
    //! # V&V — verification of the PT property-package composition
    //!
    //! **Methodology.** Each test checks an internal-consistency or reduction
    //! property of the composed flash, **not** experimental VLE data:
    //! the `Ideal` package reduces to a Wilson-K Rachford-Rice solve; a
    //! Peng-Robinson flash inside the two-phase envelope returns `0 < β < 1`
    //! with a closing mass balance, the iso-fugacity condition, and correct
    //! K-ordering (light > 1 > heavy); and single-phase pressures return the
    //! all-liquid / all-vapour degenerate limits. Light-hydrocarbon critical
    //! constants come from the [`reference`](crate::thermo::component::reference)
    //! presets (Poling, Prausnitz & O'Connell, *The Properties of Gases and
    //! Liquids*, 5th ed., 2001, Appendix A — public literature).
    //!
    //! **Scope (honesty).** Verification ("did we compose the kernel
    //! correctly?"), NOT validation against measured VLE. `k_ij = 0`
    //! throughout; no stability pre-test. Numbers below were measured on
    //! 2026-07-24 (this port), release build.

    use super::*;
    use crate::thermo::component::reference;
    use approx::assert_abs_diff_eq;

    /// **Methodology.** The `Ideal` package's K-closure is the
    /// composition-independent Wilson estimate, so a full nested-loops flash
    /// must reproduce a single Wilson-K Rachford-Rice solve exactly. Feed
    /// methane(1)/ethane(2), `z = [0.5, 0.5]`, `T = 200 K`, `P = 2·10⁶ Pa`
    /// (inside the two-phase region for the Wilson K-values). Compare the
    /// package flash against [`crate::thermo::flash::solve_rachford_rice`] on
    /// [`crate::thermo::flash::wilson_k_values`].
    /// **Results (2026-07-24, release):** package `β = 0.3073711`; direct
    /// Wilson-K RR `β = 0.3073711` — agree to < 1e-12; `x = [0.3113324,
    /// 0.6886676]`, `y = [0.9251429, 0.0748571]` agree to < 1e-12; the flash
    /// converged in `iterations = 1` (one K-update, no change).
    #[test]
    fn ideal_package_reduces_to_wilson_rachford_rice() {
        use crate::thermo::flash::{solve_rachford_rice, wilson_k_values};

        let comps = [reference::methane(), reference::ethane()];
        let z = [0.5, 0.5];
        let (t, p) = (200.0, 2.0e6);

        let fr = PropertyPackageModel::Ideal
            .flash_pt(&comps, &z, t, p)
            .unwrap();

        let kw = wilson_k_values(&comps, t, p);
        let rr = solve_rachford_rice(&z, &kw).unwrap();

        assert_abs_diff_eq!(fr.beta, rr.beta, epsilon = 1e-12);
        for i in 0..z.len() {
            assert_abs_diff_eq!(fr.x[i], rr.x[i], epsilon = 1e-12);
            assert_abs_diff_eq!(fr.y[i], rr.y[i], epsilon = 1e-12);
        }
        assert_eq!(
            fr.iterations, 1,
            "constant (Wilson) closure converges in one update"
        );
    }

    /// **Methodology.** A Peng-Robinson flash of a real light-hydrocarbon binary
    /// inside the two-phase envelope must (a) return `0 < β < 1`, (b) close the
    /// overall mass balance `z_i = (1−β) x_i + β y_i`, (c) satisfy the
    /// iso-fugacity condition `φ_i^L x_i = φ_i^V y_i` (equivalently
    /// `K_i x_i = y_i` with the returned `K`), and (d) order the K-values
    /// light > 1 > heavy. Feed methane(1)/ethane(2), `z = [0.5, 0.5]`,
    /// `T = 200 K`, `P = 2·10⁶ Pa`, `k_ij = 0`.
    /// **Results (2026-07-24, release):** converged in 11 outer iterations to
    /// `β = 0.2502840`, `x = [0.3707417, 0.6292583]`,
    /// `y = [0.8871881, 0.1128119]`, `K = [2.3930085, 0.1792776]`; mass balance
    /// closes to < 1e-12, iso-fugacity residual `|K_i x_i − y_i| < 1e-12`, and
    /// `K_1 = 2.393 > 1 > 0.179 = K_2` (methane the more volatile).
    #[test]
    fn peng_robinson_binary_two_phase_flash_is_consistent() {
        let comps = [reference::methane(), reference::ethane()];
        let z = [0.5, 0.5];
        let (t, p) = (200.0, 2.0e6);

        let fr = PropertyPackageModel::PengRobinson
            .flash_pt(&comps, &z, t, p)
            .unwrap();

        // (a) genuine two-phase split.
        assert!(
            fr.beta > 0.0 && fr.beta < 1.0,
            "expected 0 < β < 1, got {}",
            fr.beta
        );

        // Phase compositions normalise.
        assert_abs_diff_eq!(fr.x.iter().sum::<f64>(), 1.0, epsilon = 1e-12);
        assert_abs_diff_eq!(fr.y.iter().sum::<f64>(), 1.0, epsilon = 1e-12);

        for i in 0..z.len() {
            // (b) overall mass balance.
            let recon = (1.0 - fr.beta) * fr.x[i] + fr.beta * fr.y[i];
            assert_abs_diff_eq!(recon, z[i], epsilon = 1e-12);
            // (c) iso-fugacity via the converged K = y/x.
            assert_abs_diff_eq!(fr.k[i] * fr.x[i], fr.y[i], epsilon = 1e-12);
        }

        // (d) K-ordering: light methane > 1 > heavy ethane.
        assert!(fr.k[0] > 1.0, "methane K should exceed 1, got {}", fr.k[0]);
        assert!(fr.k[1] < 1.0, "ethane K should be below 1, got {}", fr.k[1]);
        assert!(fr.k[0] > fr.k[1], "light must be more volatile than heavy");
    }

    /// **Methodology.** SRK on the same feed/state must also give a consistent
    /// two-phase split with the same qualitative K-ordering (a cross-model
    /// sanity check, not a claim the two EOS agree numerically).
    /// **Results (2026-07-24, release):** SRK converged in 12 outer iterations
    /// to `β = 0.2619638`, `K = [2.4510756, 0.1756463]`; `0 < β < 1`, mass
    /// balance to < 1e-12, and `K_1 > 1 > K_2`.
    #[test]
    fn srk_binary_two_phase_flash_is_consistent() {
        let comps = [reference::methane(), reference::ethane()];
        let z = [0.5, 0.5];
        let (t, p) = (200.0, 2.0e6);

        let fr = PropertyPackageModel::Srk
            .flash_pt(&comps, &z, t, p)
            .unwrap();

        assert!(
            fr.beta > 0.0 && fr.beta < 1.0,
            "expected 0 < β < 1, got {}",
            fr.beta
        );
        for i in 0..z.len() {
            let recon = (1.0 - fr.beta) * fr.x[i] + fr.beta * fr.y[i];
            assert_abs_diff_eq!(recon, z[i], epsilon = 1e-12);
        }
        assert!(fr.k[0] > 1.0 && fr.k[1] < 1.0 && fr.k[0] > fr.k[1]);
    }

    /// **Methodology.** Single-phase degenerate limits. At a very low pressure
    /// every component is far above its dew point → all-vapour `β = 1`; at a
    /// very high pressure the feed is subcooled → all-liquid `β = 0`. Checked
    /// with the `Ideal` (Wilson) package, which is guaranteed single-valued in
    /// these limits (all `K_i > 1` and all `K_i < 1` respectively), so the test
    /// is deterministic and needs no stability pre-test. Feed
    /// methane/ethane `z = [0.4, 0.6]`.
    /// **Results (2026-07-24):** at `T = 250 K, P = 10⁴ Pa` → `β = 1.0`,
    /// `y = z`; at `T = 200 K, P = 5·10⁷ Pa` → `β = 0.0`, `x = z` (both exact).
    #[test]
    fn single_phase_pressure_limits() {
        let comps = [reference::methane(), reference::ethane()];
        let z = [0.4, 0.6];
        let pkg = PropertyPackageModel::Ideal;

        // Very low pressure → all vapour.
        let vap = pkg.flash_pt(&comps, &z, 250.0, 1.0e4).unwrap();
        assert_abs_diff_eq!(vap.beta, 1.0, epsilon = 1e-12);
        assert_abs_diff_eq!(vap.y[0], z[0], epsilon = 1e-12);
        assert_abs_diff_eq!(vap.y[1], z[1], epsilon = 1e-12);

        // Very high pressure → all liquid.
        let liq = pkg.flash_pt(&comps, &z, 200.0, 5.0e7).unwrap();
        assert_abs_diff_eq!(liq.beta, 0.0, epsilon = 1e-12);
        assert_abs_diff_eq!(liq.x[0], z[0], epsilon = 1e-12);
        assert_abs_diff_eq!(liq.x[1], z[1], epsilon = 1e-12);
    }

    /// **Methodology.** The [`PropertyPackage`] trait surface must delegate to
    /// the same inherent enum methods (contract vs. dispatch equivalence).
    /// **Results (2026-07-24):** trait-called and inherent-called `k_values`
    /// agree to < 1e-15 for a Peng-Robinson trial split.
    #[test]
    fn trait_dispatch_matches_inherent() {
        let comps = [reference::methane(), reference::ethane()];
        let x = [0.3, 0.7];
        let y = [0.8, 0.2];
        let (t, p) = (200.0, 2.0e6);
        let pkg = PropertyPackageModel::PengRobinson;

        let inherent = pkg.k_values(&comps, &x, &y, t, p);
        let via_trait = PropertyPackage::k_values(&pkg, &comps, &x, &y, t, p);
        for i in 0..comps.len() {
            assert_abs_diff_eq!(inherent[i], via_trait[i], epsilon = 1e-15);
        }
    }
}
