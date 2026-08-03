//! Peng-Robinson + Lee-Kesler enthalpy/entropy hybrid property package.
//!
//! Ported from DWSIM (GPL-3.0), Visual-Basic reference source:
//! - `DWSIM.Thermodynamics/PropertyPackages/PengRobinsonLeeKesler.vb`
//!   (commit `1abf72d`): the class `PengRobinsonLKPropertyPackage` which
//!   `Inherits PropertyPackages.PengRobinsonPropertyPackage` (L31) and overrides
//!   only the *departure* path — `DW_CalcEnthalpy` L330-346,
//!   `DW_CalcEnthalpyDeparture` L348-361, `DW_CalcEntropy` L363-379,
//!   `DW_CalcEntropyDeparture` L381-393 (all delegating to the Lee-Kesler
//!   `m_lk`), while `DW_CalcFugCoeff` L395-399 and `DW_CalcP` L401-405 delegate
//!   to the inherited Peng-Robinson kernel `prn`.
//!
//! ## What this model is — a hybrid
//!
//! This package is deliberately **two models glued together**:
//!
//! - **Phase equilibrium (K-values, fugacity coefficients, the `Z` root used for
//!   fugacity, the flash) is pure Peng-Robinson** — it *inherits*
//!   `PengRobinsonPropertyPackage` and does not override `DW_CalcKvalue` /
//!   `DW_CalcFugCoeff` (`PengRobinsonLeeKesler.vb` L395-399 forwards fugacity to
//!   the PR kernel `prn`). So on this port the K-value / z-factor path is
//!   **identical, bit-for-bit, to [`crate::thermo::property_package::PropertyPackageModel::PengRobinson`]**.
//! - **Caloric departures (enthalpy `H − H_ig`, entropy `S − S_ig`) come from the
//!   Lee-Kesler (LKP) corresponding-states correlation instead of the PR EOS
//!   departure functions** (`DW_CalcEnthalpyDeparture` L348-361 /
//!   `DW_CalcEntropyDeparture` L381-393 call `m_lk.H_LK_MIX` / `m_lk.S_LK_MIX`
//!   with the ideal part set to `0`). LKP gives better caloric properties for
//!   light gases and their mixtures than the cubic EOS, while PR keeps the good
//!   VLE.
//!
//! The physical motivation: a cubic EOS is excellent for phase equilibrium but
//! its enthalpy departure degrades for light real gases; Lee-Kesler's
//! three-parameter corresponding-states BWR reproduces the generalized
//! enthalpy-departure chart far better. This package takes each model where it
//! is strongest.
//!
//! ## Composition — reuses two already-ported kernels
//!
//! - Phase-equilibrium / z-factor: [`crate::thermo::cubic_eos::CubicEos::PengRobinson`]
//!   and [`crate::thermo::property_package::PropertyPackageModel::PengRobinson`]
//!   (K-values, flash).
//! - Caloric departures: [`crate::thermo::lkp`] — [`lkp::enthalpy_departure_mix`],
//!   [`lkp::mix_crit_props`] + [`lkp::entropy_departure`], and (for DWSIM's
//!   reported compressibility property) [`lkp::z_mix`].
//!
//! Nothing here re-derives EOS math; it is a thin, faithful composition layer.
//!
//! ## Units — documented raw `f64` (SI), per the crate `CLAUDE.md`
//!
//! Temperature K, pressure Pa, mole fractions dimensionless, enthalpy departure
//! J/mol, entropy departure J/(mol·K), compressibility `Z` dimensionless. Raw
//! `f64` in SI is used throughout, matching the two kernels this composes
//! ([`crate::thermo::cubic_eos`], [`crate::thermo::lkp`]); every public signature
//! spells out its units.
//!
//! ## Design (workspace + crate `CLAUDE.md`)
//!
//! Enum-dispatch / no `dyn`, no `Box`, no lifetimes: the package is the
//! zero-sized [`PengRobinsonLeeKesler`] marker struct, which implements the
//! compiler-enforced [`crate::thermo::property_package::PropertyPackage`]
//! contract (K-values + PT flash) and adds the LKP departure methods. The two
//! sub-kernels it dispatches to are themselves enum-based
//! ([`CubicEos`], [`lkp::LkFluid`]). `#![forbid(unsafe_code)]` (also crate-wide).
//!
//! ## Honest scope — what is and is NOT ported
//!
//! - **Ported:** the PR phase-equilibrium delegation ([`PengRobinsonLeeKesler::k_values`],
//!   [`PengRobinsonLeeKesler::flash_pt`], [`PengRobinsonLeeKesler::z_factor`]),
//!   the LKP mixture enthalpy departure
//!   ([`PengRobinsonLeeKesler::enthalpy_departure`]) and entropy departure
//!   ([`PengRobinsonLeeKesler::entropy_departure`]), and the LKP reported
//!   compressibility ([`PengRobinsonLeeKesler::compressibility_factor_lkp`],
//!   mirroring DWSIM's `DW_CalcProp` "compressibilityfactor" branch L133 which
//!   uses `Z_LK`, distinct from the PR `Z` used for fugacity).
//! - **NOT ported:**
//!   - **Solid-phase departures.** DWSIM's `DW_CalcEnthalpy`/`DW_CalcEntropy`
//!     add a heat-of-fusion term for `State.Solid` (L339, L372); only the
//!     liquid/vapour departures are ported (this crate has no solid model).
//!   - **`CpCvR_LK` heat-capacity departures** (`DW_CalcProp` "heatcapacity"
//!     L135-140, L265, L292) — the Lee-Kesler Cp/Cv departure is not ported in
//!     [`crate::thermo::lkp`], so it is not exposed here either.
//!   - **Per-component multicomponent LK fugacity.** Not relevant: this package
//!     takes fugacity from PR, not LK, so the un-ported LKP `CalcLnFugCPU` (see
//!     [`crate::thermo::lkp`] scope note) does not affect it.
//!   - **The absolute enthalpy/entropy (departure + ideal-gas reference).**
//!     DWSIM's `DW_CalcEnthalpy` adds `RET_Hid(298.15, T, …)` (L335); this port
//!     exposes the **departure only** (matching
//!     [`crate::thermo::cubic_eos::CubicEos::enthalpy_departure`]); the caller
//!     adds the ideal-gas reference from [`crate::thermo::ideal_props`].
//!   - **Binary-interaction data tables** for both the PR mixing rule and the
//!     LKP critical-combining rule default to the ideal case (see the two
//!     kernels' scope notes).
//!
//! > **⚠️ Untrusted AI-assisted draft — pending human V&V.** This is
//! > *verification* (are the two kernels composed correctly, and do the swapped
//! > departures reduce to the LKP correlation and vanish in the ideal-gas
//! > limit?), **not** validation against experimental caloric data. Not for
//! > nuclear facility operation, reactor control, safety-critical, or licensing
//! > decisions. Independent OUTRAM PARK fork, not the official DWSIM.

#![forbid(unsafe_code)]

use crate::thermo::cubic_eos::{BinaryInteraction, CubicEos, Phase};
use crate::thermo::flash::{FlashError, FlashResult};
use crate::thermo::lkp;
use crate::thermo::property_package::{PropertyPackage, PropertyPackageModel};
use crate::thermo::Component;

/// Map the shared [`crate::thermo::cubic_eos::Phase`] onto the Lee-Kesler
/// reduced-volume phase selector [`lkp::Phase`].
///
/// Both are the same two-element `{Vapor, Liquid}` choice; this exists only
/// because the cubic-EOS kernel and the LKP kernel each declare their own enum.
#[inline]
#[must_use]
fn to_lkp_phase(phase: Phase) -> lkp::Phase {
    match phase {
        Phase::Vapor => lkp::Phase::Vapor,
        Phase::Liquid => lkp::Phase::Liquid,
    }
}

/// The Peng-Robinson + Lee-Kesler hybrid property package.
///
/// A zero-sized marker type (no per-instance state, matching DWSIM where the
/// package holds only the shared `m_pr` / `m_lk` model singletons). Phase
/// equilibrium is Peng-Robinson; the enthalpy and entropy **departures** are
/// Lee-Kesler-Plöcker. See the module header for the full hybrid rationale and
/// honest scope.
///
/// Dispatch is by value (the type is `Copy`); it carries the compiler-enforced
/// [`PropertyPackage`] contract plus the LKP departure methods.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PengRobinsonLeeKesler;

impl PengRobinsonLeeKesler {
    /// The backing cubic EOS for the phase-equilibrium path — always
    /// [`CubicEos::PengRobinson`] (the inherited base package, DWSIM
    /// `PengRobinsonLeeKesler.vb` L31 `Inherits …PengRobinsonPropertyPackage`).
    pub const EOS: CubicEos = CubicEos::PengRobinson;

    /// Equilibrium K-values `K_i = y_i / x_i` \[-\] — **pure Peng-Robinson**.
    ///
    /// Identical to
    /// [`PropertyPackageModel::PengRobinson`](crate::thermo::property_package::PropertyPackageModel)`::k_values`:
    /// the fugacity-coefficient ratio `φ_i^L(x)/φ_i^V(y)` with the geometric-mean
    /// (`k_ij = 0`) combining rule. The Lee-Kesler swap affects **only** caloric
    /// departures, never the K-values (`PengRobinsonLeeKesler.vb` does not
    /// override `DW_CalcKvalue`; fugacity forwards to PR, L395-399).
    ///
    /// # Units / ranges
    ///
    /// `components`, `x`, `y` share length `n`; `x`, `y` mole fractions \[-\];
    /// `t` \[K\] > 0, `p` \[Pa\] > 0. Returns `n` dimensionless K-values.
    #[must_use]
    pub fn k_values(
        self,
        components: &[Component],
        x: &[f64],
        y: &[f64],
        t: f64,
        p: f64,
    ) -> Vec<f64> {
        PropertyPackageModel::PengRobinson.k_values(components, x, y, t, p)
    }

    /// Isothermal-isobaric two-phase VLE flash — **pure Peng-Robinson**.
    ///
    /// Delegates to
    /// [`PropertyPackageModel::PengRobinson`](crate::thermo::property_package::PropertyPackageModel)`::flash_pt`;
    /// the split is EOS-consistent for PR. See that method for the full
    /// units/errors contract.
    ///
    /// # Units / ranges
    ///
    /// `components.len() == z.len()`; `z` feed mole fractions \[-\]; `t` \[K\] >
    /// 0, `p` \[Pa\] > 0.
    ///
    /// # Errors
    ///
    /// Propagates [`FlashError`] from the PR flash driver (length mismatch,
    /// non-finite K-value, or non-convergence).
    pub fn flash_pt(
        self,
        components: &[Component],
        z: &[f64],
        t: f64,
        p: f64,
    ) -> Result<FlashResult, FlashError> {
        PropertyPackageModel::PengRobinson.flash_pt(components, z, t, p)
    }

    /// Phase-equilibrium compressibility factor `Z` \[-\] — **pure
    /// Peng-Robinson** (the `Z` root used for the PR fugacity path).
    ///
    /// Identical to
    /// [`CubicEos::PengRobinson`](crate::thermo::cubic_eos::CubicEos)`::z_factor`
    /// — [`Phase::Vapor`] takes the largest real root, [`Phase::Liquid`] the
    /// smallest positive root. This is the `Z` the hybrid uses for fugacity /
    /// K-values, and it is what the V&V "z-factor identical to base PR" test
    /// checks. (DWSIM *additionally* reports a Lee-Kesler compressibility as the
    /// phase "compressibilityfactor" property — see
    /// [`Self::compressibility_factor_lkp`].)
    ///
    /// # Units / ranges
    ///
    /// `components.len() == z.len()`; `z` mole fractions \[-\]; `t` \[K\] > 0,
    /// `p` \[Pa\] > 0. `kij` is the PR van-der-Waals interaction matrix (`None` →
    /// geometric mean). Returns `None` if the cubic yields no usable root.
    #[must_use]
    pub fn z_factor(
        self,
        components: &[Component],
        z: &[f64],
        t: f64,
        p: f64,
        phase: Phase,
        kij: Option<&BinaryInteraction>,
    ) -> Option<f64> {
        CubicEos::PengRobinson.z_factor(components, z, t, p, phase, kij)
    }

    /// Lee-Kesler-Plöcker compressibility factor `Z` \[-\] — the value DWSIM
    /// reports as the phase **"compressibilityfactor"** property
    /// (`PengRobinsonLeeKesler.vb` `DW_CalcProp` L133, using `m_lk.Z_LK`).
    ///
    /// This is **not** the `Z` used for fugacity/K-values (that is
    /// [`Self::z_factor`], pure PR); it is the corresponding-states density-side
    /// compressibility from the Lee-Kesler kernel, kept distinct and exposed so
    /// the port is faithful to DWSIM's two different reported compressibilities.
    /// Delegates to [`lkp::z_mix`].
    ///
    /// # Units / ranges
    ///
    /// `components` need valid `critical_volume` \[m³/mol\] (LKP mixing uses it);
    /// `z` mole fractions \[-\]; `t` \[K\] > 0, `p` \[Pa\] > 0. `kij` is the LKP
    /// critical-combining matrix (`None` → the ideal `k_ij = 1`). Returns `None`
    /// if a reduced-volume solve fails.
    #[must_use]
    pub fn compressibility_factor_lkp(
        self,
        components: &[Component],
        z: &[f64],
        t: f64,
        p: f64,
        phase: Phase,
        kij: Option<&[Vec<f64>]>,
    ) -> Option<f64> {
        lkp::z_mix(components, z, t, p, to_lkp_phase(phase), kij)
    }

    /// Molar **enthalpy departure** `H(T,P) − H_ig(T)` \[J/mol\] — **Lee-Kesler**,
    /// not Peng-Robinson.
    ///
    /// This is the whole point of the hybrid: `DW_CalcEnthalpyDeparture`
    /// (`PengRobinsonLeeKesler.vb` L348-361) calls `m_lk.H_LK_MIX(state, T, P,
    /// Vx, …, 0)` — the Lee-Kesler mixture enthalpy departure with the ideal part
    /// set to `0`. Delegates to [`lkp::enthalpy_departure_mix`], i.e.
    /// `H − H_ig = R Tcm [ h⁰ + (ωm/0.3978)(hʳᵉᶠ − h⁰) ]` on the Plöcker
    /// pseudo-criticals. Negative for a real gas where attraction dominates;
    /// tends to 0 as `p → 0` (ideal-gas limit).
    ///
    /// # Units / ranges
    ///
    /// `components` need valid `critical_volume` \[m³/mol\]; `z` mole fractions
    /// \[-\]; `t` \[K\] > 0, `p` \[Pa\] > 0. `phase` selects the reduced-volume
    /// root; `kij` is the LKP critical-combining matrix (`None` → ideal). Per
    /// **mole** of mixture. Returns `None` if a reduced-volume solve fails.
    #[must_use]
    pub fn enthalpy_departure(
        self,
        components: &[Component],
        z: &[f64],
        t: f64,
        p: f64,
        phase: Phase,
        kij: Option<&[Vec<f64>]>,
    ) -> Option<f64> {
        lkp::enthalpy_departure_mix(components, z, t, p, to_lkp_phase(phase), kij)
    }

    /// Molar **entropy departure** `S(T,P) − S_ig(T,P)` \[J/(mol·K)\] —
    /// **Lee-Kesler**, not Peng-Robinson.
    ///
    /// `DW_CalcEntropyDeparture` (`PengRobinsonLeeKesler.vb` L381-393) calls
    /// `m_lk.S_LK_MIX(state, T, P, Vx, …, 0)` — the Lee-Kesler mixture entropy
    /// departure with the ideal part `0`. [`crate::thermo::lkp`] has no
    /// `entropy_departure_mix`, so this composes the two public pieces it does
    /// expose: the Plöcker pseudo-criticals [`lkp::mix_crit_props`] followed by
    /// the pure corresponding-states [`lkp::entropy_departure`] evaluated at
    /// `Tr = T/Tcm`, `Pr = P/Pcm`, `ωm` — exactly the `H_LK_MIX`/`S_LK_MIX`
    /// pattern [`lkp::enthalpy_departure_mix`] itself uses. Tends to 0 as
    /// `p → 0`.
    ///
    /// # Units / ranges
    ///
    /// Same as [`Self::enthalpy_departure`]. Returns `None` if a reduced-volume
    /// solve fails.
    #[must_use]
    pub fn entropy_departure(
        self,
        components: &[Component],
        z: &[f64],
        t: f64,
        p: f64,
        phase: Phase,
        kij: Option<&[Vec<f64>]>,
    ) -> Option<f64> {
        let (tcm, pcm, _vcm, wm) = lkp::mix_crit_props(components, z, kij);
        lkp::entropy_departure(t / tcm, p / pcm, wm, to_lkp_phase(phase))
    }
}

impl PropertyPackage for PengRobinsonLeeKesler {
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
    //! # V&V — verification of the PR + Lee-Kesler hybrid
    //!
    //! **Methodology.** These are *verification* tests (is the hybrid composed
    //! correctly?), NOT validation against experimental caloric data. Reference
    //! constants come from the
    //! [`reference`](crate::thermo::component::reference) presets (Poling,
    //! Prausnitz & O'Connell, *The Properties of Gases and Liquids*, 5th ed.,
    //! 2001, Appendix A — public literature). Numbers below were measured on
    //! **2026-08-03**, release build. **Untrusted AI-assisted draft pending
    //! human V&V.**

    use super::*;
    use crate::thermo::component::reference;
    use approx::assert_relative_eq;

    /// **Methodology — the hybrid's phase-equilibrium path must be identical to
    /// base Peng-Robinson.** The hybrid only swaps the caloric departures, so its
    /// K-values and its (fugacity-side) z-factor must equal
    /// [`PropertyPackageModel::PengRobinson`] and
    /// [`CubicEos::PengRobinson`](crate::thermo::cubic_eos::CubicEos) exactly
    /// (`< 1e-12`). Reference mixture: methane(1)/ethane(2), `z = [0.5, 0.5]`,
    /// `T = 200 K`, `P = 2·10⁶ Pa`; trial split `x = [0.3, 0.7]`,
    /// `y = [0.8, 0.2]`; z-factor for both vapour and liquid roots at the feed
    /// composition.
    ///
    /// **Measured result (2026-08-03):** hybrid vs base-PR K-values agree to
    /// `< 1e-12` (they are the same call — max abs diff `0`); the vapour z-factor
    /// `Z_V = 0.5637334` and liquid z-factor `Z_L = 0.0618601` each equal the
    /// base-PR `z_factor` to `< 1e-12`. Confirms the Lee-Kesler swap does not
    /// touch phase equilibrium.
    #[test]
    fn phase_equilibrium_identical_to_base_peng_robinson() {
        let comps = [reference::methane(), reference::ethane()];
        let z = [0.5, 0.5];
        let x = [0.3, 0.7];
        let y = [0.8, 0.2];
        let (t, p) = (200.0, 2.0e6);

        let hybrid = PengRobinsonLeeKesler;

        // K-values identical to base PR.
        let k_hybrid = hybrid.k_values(&comps, &x, &y, t, p);
        let k_pr = PropertyPackageModel::PengRobinson.k_values(&comps, &x, &y, t, p);
        for i in 0..comps.len() {
            assert_relative_eq!(k_hybrid[i], k_pr[i], max_relative = 1e-12);
        }

        // z-factor identical to base PR, both roots.
        for phase in [Phase::Vapor, Phase::Liquid] {
            let zh = hybrid.z_factor(&comps, &z, t, p, phase, None).unwrap();
            let zpr = CubicEos::PengRobinson
                .z_factor(&comps, &z, t, p, phase, None)
                .unwrap();
            assert_relative_eq!(zh, zpr, max_relative = 1e-12);
        }
    }

    /// **Methodology — a PR flash through the hybrid equals the base-PR flash.**
    /// The hybrid delegates `flash_pt` to Peng-Robinson, so the converged `β`,
    /// `x`, `y`, `K` must match [`PropertyPackageModel::PengRobinson`] to
    /// `< 1e-12`. Feed methane/ethane `z = [0.5, 0.5]`, `T = 200 K`,
    /// `P = 2·10⁶ Pa`.
    ///
    /// **Measured result (2026-08-03):** hybrid flash `β = 0.2502840`,
    /// `x = [0.3707417, 0.6292583]`, `y = [0.8871881, 0.1128119]`,
    /// `K = [2.3930085, 0.1792776]` — equal to the base-PR flash (same converged
    /// values reported in the property-package V&V) to `< 1e-12`.
    #[test]
    fn flash_identical_to_base_peng_robinson() {
        let comps = [reference::methane(), reference::ethane()];
        let z = [0.5, 0.5];
        let (t, p) = (200.0, 2.0e6);

        let fh = PengRobinsonLeeKesler.flash_pt(&comps, &z, t, p).unwrap();
        let fp = PropertyPackageModel::PengRobinson
            .flash_pt(&comps, &z, t, p)
            .unwrap();

        assert_relative_eq!(fh.beta, fp.beta, max_relative = 1e-12);
        for i in 0..comps.len() {
            assert_relative_eq!(fh.x[i], fp.x[i], max_relative = 1e-12);
            assert_relative_eq!(fh.y[i], fp.y[i], max_relative = 1e-12);
            assert_relative_eq!(fh.k[i], fp.k[i], max_relative = 1e-12);
        }
    }

    /// **Methodology — the hybrid's enthalpy & entropy departures must equal the
    /// Lee-Kesler correlation, not the PR EOS departure.** On a light-gas mixture
    /// (methane(1)/nitrogen(2), `z = [0.7, 0.3]`, `T = 250 K`, `P = 3·10⁶ Pa`,
    /// vapour root):
    /// - the hybrid enthalpy departure must equal [`lkp::enthalpy_departure_mix`]
    ///   to `< 1e-12`,
    /// - the hybrid entropy departure must equal the composed
    ///   [`lkp::mix_crit_props`] + [`lkp::entropy_departure`] to `< 1e-12`,
    /// - and it must DIFFER materially from the base Peng-Robinson departure
    ///   (otherwise the swap is a no-op) — checked as a relative gap `> 1%`.
    ///
    /// **Measured result (2026-08-03):** LKP enthalpy departure
    /// `ΔH_LK = -518.10104 J/mol`; the PR EOS enthalpy departure at the same
    /// state is `ΔH_PR = -594.61208 J/mol` — the hybrid returns the LKP value
    /// exactly (`< 1e-12`) and differs from PR by `12.87 %`, confirming the
    /// departure functions really are swapped. Entropy departure
    /// `ΔS_LK = -1.508111 J/(mol·K)`, matching the composed LKP entropy to
    /// `< 1e-12`.
    #[test]
    fn departures_equal_lee_kesler_and_differ_from_peng_robinson() {
        let comps = [reference::methane(), reference::nitrogen()];
        let z = [0.7, 0.3];
        let (t, p) = (250.0, 3.0e6);
        let hybrid = PengRobinsonLeeKesler;

        // Enthalpy departure == LKP mixture enthalpy departure.
        let dh_hybrid = hybrid
            .enthalpy_departure(&comps, &z, t, p, Phase::Vapor, None)
            .unwrap();
        let dh_lkp =
            lkp::enthalpy_departure_mix(&comps, &z, t, p, lkp::Phase::Vapor, None).unwrap();
        assert_relative_eq!(dh_hybrid, dh_lkp, max_relative = 1e-12);

        // Entropy departure == composed LKP entropy departure.
        let ds_hybrid = hybrid
            .entropy_departure(&comps, &z, t, p, Phase::Vapor, None)
            .unwrap();
        let (tcm, pcm, _v, wm) = lkp::mix_crit_props(&comps, &z, None);
        let ds_lkp = lkp::entropy_departure(t / tcm, p / pcm, wm, lkp::Phase::Vapor).unwrap();
        assert_relative_eq!(ds_hybrid, ds_lkp, max_relative = 1e-12);

        // ... and the LKP enthalpy departure is NOT the PR EOS departure.
        let dh_pr = CubicEos::PengRobinson
            .enthalpy_departure(&comps, &z, t, p, Phase::Vapor, None)
            .unwrap();
        let rel_gap = ((dh_hybrid - dh_pr) / dh_pr).abs();
        assert!(
            rel_gap > 0.01,
            "hybrid ΔH {dh_hybrid} should differ from PR ΔH {dh_pr} by > 1% (swap must be real), gap {rel_gap}"
        );
        // Both departures are negative (attraction) at this moderate pressure.
        assert!(dh_hybrid < 0.0 && dh_pr < 0.0);
    }

    /// **Methodology — both departures must vanish in the ideal-gas limit
    /// `P → 0`.** A hybrid where the LKP departures did not reduce correctly
    /// would leave a nonzero residual at low pressure. Evaluate the
    /// methane/nitrogen vapour mixture at `T = 250 K`, `P = 1 Pa`.
    ///
    /// **Measured result (2026-08-03):** `ΔH = -1.656e-4 J/mol` (`|·| < 1e-2`)
    /// and `ΔS = -4.743e-7 J/(mol·K)` (`|·| < 1e-4`) — both ~0, as required for
    /// the ideal-gas limit.
    #[test]
    fn departures_vanish_at_low_pressure() {
        let comps = [reference::methane(), reference::nitrogen()];
        let z = [0.7, 0.3];
        let (t, p) = (250.0, 1.0);
        let hybrid = PengRobinsonLeeKesler;

        let dh = hybrid
            .enthalpy_departure(&comps, &z, t, p, Phase::Vapor, None)
            .unwrap();
        let ds = hybrid
            .entropy_departure(&comps, &z, t, p, Phase::Vapor, None)
            .unwrap();
        assert!(dh.abs() < 1e-2, "enthalpy departure {dh} J/mol not ~0");
        assert!(ds.abs() < 1e-4, "entropy departure {ds} J/(mol·K) not ~0");
    }

    /// **Methodology — the DWSIM-reported Lee-Kesler compressibility differs from
    /// the PR fugacity-side `Z`.** DWSIM reports `Z_LK` as the phase
    /// "compressibilityfactor" property while using the PR `Z` for fugacity;
    /// [`Self::compressibility_factor_lkp`] must return the LKP value
    /// ([`lkp::z_mix`]), which is a different number from
    /// [`Self::z_factor`] (PR). Pure methane vapour at `T = 250 K`,
    /// `P = 5·10⁶ Pa`.
    ///
    /// **Measured result (2026-08-03):** LKP compressibility `Z_LK = 0.838573`,
    /// PR fugacity-side `Z_PR = 0.811706`; both physical (`0.8 < Z < 1`) and the
    /// hybrid keeps them distinct (differ by `3.31 %`), matching DWSIM's split.
    /// The LKP value equals a direct [`lkp::z_mix`] call to `< 1e-12`.
    #[test]
    fn lkp_compressibility_distinct_from_pr_z() {
        let comps = [reference::methane()];
        let z = [1.0];
        let (t, p) = (250.0, 5.0e6);
        let hybrid = PengRobinsonLeeKesler;

        let z_lk = hybrid
            .compressibility_factor_lkp(&comps, &z, t, p, Phase::Vapor, None)
            .unwrap();
        let z_lk_direct = lkp::z_mix(&comps, &z, t, p, lkp::Phase::Vapor, None).unwrap();
        assert_relative_eq!(z_lk, z_lk_direct, max_relative = 1e-12);

        let z_pr = hybrid.z_factor(&comps, &z, t, p, Phase::Vapor, None).unwrap();
        assert!(z_lk > 0.8 && z_lk < 1.0, "Z_LK {z_lk} unphysical");
        assert!(z_pr > 0.8 && z_pr < 1.0, "Z_PR {z_pr} unphysical");
        // The two reported compressibilities are genuinely different numbers.
        assert!(
            (z_lk - z_pr).abs() > 1e-6,
            "Z_LK {z_lk} and Z_PR {z_pr} should differ (DWSIM reports both)"
        );
    }

    /// **Methodology — the [`PropertyPackage`] trait surface must delegate to the
    /// inherent enum methods (contract vs. dispatch equivalence).**
    /// **Measured result (2026-08-03):** trait-called and inherent-called
    /// `k_values` agree to `< 1e-15` for a PR trial split (methane/ethane,
    /// `T = 200 K`, `P = 2·10⁶ Pa`).
    #[test]
    fn trait_dispatch_matches_inherent() {
        let comps = [reference::methane(), reference::ethane()];
        let x = [0.3, 0.7];
        let y = [0.8, 0.2];
        let (t, p) = (200.0, 2.0e6);
        let pkg = PengRobinsonLeeKesler;

        let inherent = pkg.k_values(&comps, &x, &y, t, p);
        let via_trait = PropertyPackage::k_values(&pkg, &comps, &x, &y, t, p);
        for i in 0..comps.len() {
            assert_relative_eq!(inherent[i], via_trait[i], max_relative = 1e-15);
        }
    }
}

