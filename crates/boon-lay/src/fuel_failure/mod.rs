// SPDX-License-Identifier: GPL-3.0
//
// PANAMA-I reimplementation — provenance
// --------------------------------------
// Reference : Verfondern, K. & Nabielek, H., "The Mathematical Basis of the
//             PANAMA-I Code for Modeling Pressure Vessel Failure of TRISO
//             Coated Particles under Accident Conditions",
//             Forschungszentrum Jülich, HTA-IB-03/90, 1 August 1990.
//             Reprinted as Appendix C, printed pages -479- to -511-.
// Status    : the report is restricted literature with no reuse licence. Only
//             the governing EQUATIONS and their constants are reproduced here,
//             with citation, as scientific facts. No prose, figure or page of
//             that document is copied into this repository, and the PDF is not
//             tracked here. See DATA_POLICY.md.
// Nature    : an independent Rust implementation of the published model, not a
//             port of the PANAMA Fortran (which is closed-source and was never
//             consulted).

//! # TRISO coated-particle failure — the PANAMA-I pressure-vessel model
//!
//! A TRISO particle is a pressure vessel. Fission gas and CO accumulate inside
//! it, the SiC layer carries the hoop stress, and the particle fails when that
//! stress exceeds the SiC strength. PANAMA-I couples three failure populations:
//!
//! | Term | Mechanism | Where it comes from |
//! |---|---|---|
//! | `φ_o` | as-manufactured defects | not modelled; an input (see [`AS_MANUFACTURED_TARGET`]) |
//! | `φ₁` | **pressure-vessel overstress** | [`weibull_failure_fraction`], this module |
//! | `φ₂` | SiC **thermal decomposition** above ~2000 °C | NOT YET IMPLEMENTED — see below |
//!
//! combined by [`total_failure_fraction`].
//!
//! ## What is implemented here, and what is deliberately absent
//!
//! **Implemented — every equation below was read directly off the source scan
//! and is reproduced with its printed equation number:**
//!
//! | Item | Eq. | Page |
//! |---|---|---|
//! | Weibull failure probability | (1) | -483- |
//! | Induced SiC stress, thin shell | (2) | -484- |
//! | Internal gas pressure, ideal gas | (3) | -484- |
//! | Failure-population combination | unnumbered | -480- |
//!
//! **Absent, and taken as INPUTS rather than guessed.** The correlations that
//! supply `σ_o`, `m`, `OPF` and `F_d` — the fluence degradation laws (8a/8b,
//! 9a/9b), the oxygen-per-fission fits (5a–5f), the Booth `f(τ)` series, the
//! `D_S` diffusion correlations and the decomposition kinetics (11/12) — are
//! **not** in this module. Two have unresolved ambiguities in the source scan
//! (the `t_B` seconds-vs-full-power-days question on (5b)/(5c), and a grouping
//! ambiguity in the `f(τ)` series), and the rest have not yet been verified
//! against the scan by a human.
//!
//! This is the deliberate shape, not an unfinished one: the failure **chain**
//! is exact and testable today, and each correlation can be dropped in behind
//! the same signature once it is confirmed. A guessed exponent in a correlation
//! would propagate silently into an absolute pressure and then into a failure
//! fraction that still *looks* reasonable.
//!
//! ## The `ln2` in Eq (1) is load-bearing — do not reach for a stock Weibull
//!
//! Eq (1) is `φ₁ = 1 − exp[−ln2·(σ_t/σ_o)^m]`, **not** the textbook
//! `1 − exp[−(σ/σ_c)^m]`. The `ln2` normalisation makes `σ_o` the **median**
//! strength: at `σ_t = σ_o`, `φ₁ = 1 − e^(−ln2) = 0.5` exactly. A stock Weibull
//! treats its scale parameter as the *characteristic* strength, where
//! `φ = 1 − e^(−1) ≈ 0.632`. Substituting one for the other misplaces the
//! strength scale by a factor `(ln2)^(1/m)` — about 4 % at `m = 8` — in a
//! direction that flatters the answer and produces no error.
//! [`median_is_the_scale_parameter`] pins this.
//!
//! ## Units
//!
//! Public signatures are `uom`-typed. Two traps from the source's own symbol
//! list (-511-): the report prints irradiation and accident temperatures in
//! **°C** while every Arrhenius term needs **kelvin**, and it never states the
//! conversion. Using `uom` removes that ambiguity at the boundary — a caller
//! passes a `ThermodynamicTemperature` and cannot get it wrong.

use uom::si::f64::{
    Length, MolarVolume, Pressure, Ratio, ThermodynamicTemperature, Velocity, Time, Volume,
};
use uom::si::molar_volume::cubic_meter_per_mole;
use uom::si::pressure::pascal;
use uom::si::ratio::ratio;
use uom::si::thermodynamic_temperature::kelvin;

/// Molar gas constant `R` in J/(mol·K), as printed with Eq (3) on page -485-.
///
/// The report's own value, kept rather than substituting a CODATA figure, so
/// the arithmetic reproduces the source exactly.
pub const GAS_CONSTANT_J_PER_MOL_K: f64 = 8.3143;

/// Fission yield of the stable fission gases `F_f`, printed with Eq (3)
/// (page -485-). Dimensionless, atoms per fission.
pub const STABLE_FISSION_GAS_YIELD: f64 = 0.31;

/// The as-manufactured defective fraction `φ_o` used for reactor studies,
/// `6·10⁻⁵` (page -480-).
///
/// The report's own calculations set `φ_o = 0`; this is the value it states may
/// "without difficulty" be used as a target when considering reactor concepts.
/// It is offered as a named constant, never as a default — which of the two
/// applies is the caller's modelling decision.
pub const AS_MANUFACTURED_TARGET: f64 = 6.0e-5;

/// A failed-particle fraction, dimensionless and in `[0, 1]`.
pub type FailureFraction = Ratio;

/// The SiC layer's geometry, from which Eq (2)'s `r` and `d_o` are derived.
///
/// Both radii are to the SiC layer itself — `r_i` its inner surface (the outer
/// surface of the inner PyC) and `r_a` its outer surface.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SicLayer {
    /// Inner radius `r_i` of the SiC layer.
    pub inner_radius: Length,
    /// Outer radius `r_a` of the SiC layer.
    pub outer_radius: Length,
}

impl SicLayer {
    /// The report's **average radius** `r = (0.5·(r_a³ + r_i³))^(1/3)`
    /// (page -484-).
    ///
    /// Note this is a cube-root mean, not the arithmetic mean `(r_a + r_i)/2`.
    /// The two agree to well under a percent for a real TRISO layer, which is
    /// precisely why substituting the arithmetic mean would never show up as
    /// an obvious error — so the printed definition is kept.
    pub fn mean_radius(&self) -> Length {
        let ri = self.inner_radius.value;
        let ra = self.outer_radius.value;
        Length::new::<uom::si::length::meter>((0.5 * (ra.powi(3) + ri.powi(3))).cbrt())
    }

    /// The original layer thickness `d_o = r_a − r_i` (page -484-).
    pub fn initial_thickness(&self) -> Length {
        self.outer_radius - self.inner_radius
    }

    /// The **actual** thickness after volume corrosion, `d_act = d_o·(1 − v̇·t)`
    /// (page -484-).
    ///
    /// Returns a non-positive length once `v̇·t ≥ 1`, i.e. once the layer has
    /// notionally corroded away; callers should treat that as certain failure
    /// rather than feeding it to [`induced_stress_exact`], which would divide
    /// by zero or change sign.
    pub fn actual_thickness(&self, corrosion_rate: Velocity, elapsed: Time) -> Length {
        let consumed: Ratio = corrosion_rate * elapsed / self.initial_thickness();
        self.initial_thickness() * (Ratio::new::<ratio>(1.0) - consumed)
    }
}

/// **Eq (1)** — the fraction of particles failed by pressure-vessel overstress
/// (page -483-, attributed to Nabielek 1984).
///
/// ```text
/// φ₁(t,T) = 1 − exp[ −ln2 · (σ_t / σ_o)^m ]
/// ```
///
/// - `induced_stress` — `σ_t`, the SiC hoop stress from the internal gas
///   pressure ([`induced_stress`]).
/// - `median_strength` — `σ_o`, the SiC tensile strength **at the end of
///   irradiation**. See the module docs: because of the `ln2`, this is the
///   *median* of the strength distribution, not its characteristic value.
/// - `weibull_modulus` — `m`, dimensionless.
///
/// Returns a fraction in `[0, 1]`; it is `0.5` exactly when
/// `induced_stress == median_strength`, for any `m`.
pub fn weibull_failure_fraction(
    induced_stress: Pressure,
    median_strength: Pressure,
    weibull_modulus: f64,
) -> FailureFraction {
    let sigma_o = median_strength.get::<pascal>();
    if sigma_o <= 0.0 {
        // A zero or negative strength is total failure, not a NaN. This is the
        // limit the thinning and degradation laws walk toward.
        return Ratio::new::<ratio>(1.0);
    }
    let x = (induced_stress.get::<pascal>() / sigma_o).max(0.0);
    Ratio::new::<ratio>(1.0 - (-std::f64::consts::LN_2 * x.powf(weibull_modulus)).exp())
}

/// **Eq (2)** — the SiC hoop stress induced by internal gas pressure, in the
/// report's preferred approximate form (page -484-).
///
/// ```text
/// σ_t = r·p / (2·d_o) · (1 + v̇·t/d_o)      [Pa]
/// ```
///
/// The report gives the exact thin-shell result first
/// ([`induced_stress_exact`]) and then states that this approximation
/// "describes the state of affairs more realistically, in particular for small
/// actual SiC layer thicknesses" — it is the linearisation of the exact form in
/// `v̇t/d_o`, and unlike the exact form it stays finite as the layer thins.
/// **This is the one to use**; the exact form is provided for comparison.
///
/// Note the model is **thin-shell throughout**. The report contains no
/// thick-wall (Lamé) formulation, so none is offered here.
pub fn induced_stress(
    layer: &SicLayer,
    pressure: Pressure,
    corrosion_rate: Velocity,
    elapsed: Time,
) -> Pressure {
    let d_o = layer.initial_thickness();
    let thinning: Ratio = corrosion_rate * elapsed / d_o;
    let base: Pressure = layer.mean_radius() * pressure / (2.0 * d_o);
    base * (Ratio::new::<ratio>(1.0) + thinning)
}

/// The **exact** thin-shell stress, `σ_t = r·p / (2·d_act)` (page -484-).
///
/// Returns `None` once the corroded thickness is non-positive, rather than
/// returning an infinite or negative stress. [`induced_stress`] is the form the
/// report actually recommends.
pub fn induced_stress_exact(
    layer: &SicLayer,
    pressure: Pressure,
    corrosion_rate: Velocity,
    elapsed: Time,
) -> Option<Pressure> {
    let d_act = layer.actual_thickness(corrosion_rate, elapsed);
    if d_act.value <= 0.0 {
        return None;
    }
    Some(layer.mean_radius() * pressure / (2.0 * d_act))
}

/// **Eq (3)** — internal gas pressure from the ideal gas law (page -484-).
///
/// ```text
/// p = (F_d·F_f + OPF) · F_b · R · T / [ (V_f/V_k) · V_m ]      [Pa]
/// ```
///
/// - `released_gas_fraction` — `F_d`, the relevant fraction of fission gas
///   released from the kernel (Eq (4), Allelein 1983 — **not** implemented
///   here; supply it).
/// - `stable_gas_yield` — `F_f`, atoms of stable fission gas per fission;
///   [`STABLE_FISSION_GAS_YIELD`] is the report's 0.31.
/// - `oxygen_per_fission` — `OPF`, CO-forming oxygen atoms per fission
///   (Eqs (5a)–(5f) — **not** implemented here; supply it, and see the module
///   docs on why).
/// - `burnup` — `F_b`, heavy-metal burnup in FIMA.
/// - `free_volume` / `kernel_volume` — `V_f` (buffer void) and `V_k`.
/// - `molar_volume` — `V_m`, the molar volume of the kernel compound
///   (Eqs (6a)–(6c) — supply it).
/// - `temperature` — `T`, in kelvin via `uom`.
///
/// ## The printed grouping is ambiguous; this is the dimensionally consistent
/// reading
///
/// As printed, the fraction bar appears to span `(V_f/V_k)·R·T/V_m`, which
/// would give `p ∝ 1/(R·T)` — dimensionally wrong, and it would make pressure
/// *fall* as the particle heats. Only one grouping is consistent, and it is
/// also just `p = nRT/V_f` with `n = (F_d·F_f + OPF)·F_b·V_k/V_m`:
/// `(F_d·F_f+OPF)·F_b` is dimensionless (moles of gas per mole of heavy metal),
/// `R·T` is Pa·m³/mol, `V_f/V_k` is dimensionless and `V_m` is m³/mol, leaving
/// **Pa**. That is what is implemented. [`pressure_is_the_ideal_gas_law`]
/// pins it against `nRT/V` computed independently.
pub fn internal_gas_pressure(
    released_gas_fraction: Ratio,
    stable_gas_yield: Ratio,
    oxygen_per_fission: Ratio,
    burnup: Ratio,
    free_volume: Volume,
    kernel_volume: Volume,
    molar_volume: MolarVolume,
    temperature: ThermodynamicTemperature,
) -> Pressure {
    let gas_per_heavy_metal = released_gas_fraction.get::<ratio>()
        * stable_gas_yield.get::<ratio>()
        + oxygen_per_fission.get::<ratio>();
    let void_ratio = free_volume.value / kernel_volume.value;
    let v_m = molar_volume.get::<cubic_meter_per_mole>();
    let p = gas_per_heavy_metal
        * burnup.get::<ratio>()
        * GAS_CONSTANT_J_PER_MOL_K
        * temperature.get::<kelvin>()
        / (void_ratio * v_m);
    Pressure::new::<pascal>(p)
}

/// The combination of the three failure populations (page -480-).
///
/// ```text
/// φ_total = 1 − (1 − φ_o)·(1 − φ₁)·(1 − φ₂)
/// ```
///
/// A particle survives only if it survives all three mechanisms, so the
/// *survival* probabilities multiply. This is why the result is not the sum:
/// summing would double-count particles failed by more than one mechanism and
/// can exceed 1.
///
/// - `as_manufactured` — `φ_o`. The report's own runs use `0`; see
///   [`AS_MANUFACTURED_TARGET`].
/// - `pressure_vessel` — `φ₁`, from [`weibull_failure_fraction`].
/// - `thermal_decomposition` — `φ₂`. **Not implemented in this module** (the
///   action-integral model, Eqs (11)/(12)); pass `Ratio::new::<ratio>(0.0)` to
///   model pressure-vessel failure alone, and be aware that doing so is
///   non-conservative above ~2000 °C, where the report attributes failure
///   principally to SiC decomposition.
pub fn total_failure_fraction(
    as_manufactured: FailureFraction,
    pressure_vessel: FailureFraction,
    thermal_decomposition: FailureFraction,
) -> FailureFraction {
    let survive = (1.0 - as_manufactured.get::<ratio>())
        * (1.0 - pressure_vessel.get::<ratio>())
        * (1.0 - thermal_decomposition.get::<ratio>());
    Ratio::new::<ratio>(1.0 - survive)
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::length::{meter, micrometer};
    use uom::si::pressure::{megapascal, pascal};
    use uom::si::thermodynamic_temperature::degree_celsius;
    use uom::si::time::{hour, second};
    use uom::si::velocity::meter_per_second;
    use uom::si::volume::cubic_meter;

    fn r(x: f64) -> Ratio {
        Ratio::new::<ratio>(x)
    }

    /// A representative TRISO SiC layer: 350 µm kernel radius plus buffer and
    /// inner PyC puts the SiC inner surface near 250 µm, with a 35 µm layer.
    /// Dimensions only; nothing downstream is calibrated to them.
    fn layer() -> SicLayer {
        SicLayer {
            inner_radius: Length::new::<micrometer>(250.0),
            outer_radius: Length::new::<micrometer>(285.0),
        }
    }

    /// **The `ln2` is the whole point of Eq (1).** At `σ_t = σ_o` the failed
    /// fraction is exactly one half, for every Weibull modulus — that is what
    /// makes `σ_o` the MEDIAN strength. A stock Weibull would give 0.632 here.
    /// If this test ever reads 0.632, someone has "simplified" the `ln2` away.
    #[test]
    fn median_is_the_scale_parameter() {
        let s = Pressure::new::<megapascal>(200.0);
        for m in [1.0, 2.0, 5.0, 8.0, 15.0] {
            let phi = weibull_failure_fraction(s, s, m).get::<ratio>();
            assert!(
                (phi - 0.5).abs() < 1e-12,
                "m = {m}: expected exactly 0.5 at sigma_t == sigma_o, got {phi}"
            );
        }
        // And explicitly NOT the characteristic-strength convention.
        let stock = 1.0 - (-1.0_f64).exp();
        assert!((0.5 - stock).abs() > 0.1, "the two conventions must differ");
    }

    /// Failure rises monotonically with stress and saturates at 1, and a
    /// vanishing stress gives a vanishing failed fraction.
    #[test]
    fn failure_is_monotone_in_stress() {
        let sigma_o = Pressure::new::<megapascal>(200.0);
        let mut last = -1.0;
        for mpa in [0.0, 50.0, 100.0, 200.0, 400.0, 800.0] {
            let phi = weibull_failure_fraction(Pressure::new::<megapascal>(mpa), sigma_o, 8.0)
                .get::<ratio>();
            assert!(phi >= last, "not monotone at {mpa} MPa: {phi} < {last}");
            assert!(
                (0.0..=1.0).contains(&phi),
                "out of range at {mpa} MPa: {phi}"
            );
            last = phi;
        }
        assert_eq!(
            weibull_failure_fraction(Pressure::new::<pascal>(0.0), sigma_o, 8.0).get::<ratio>(),
            0.0
        );
        assert!(last > 0.999_999, "should saturate well before 4x strength");
    }

    /// A higher Weibull modulus is a *tighter* strength distribution: below the
    /// median it must fail fewer particles, above it more. This is the
    /// qualitative behaviour `m` exists to express, and it is easy to invert by
    /// a sign slip in the exponent.
    #[test]
    fn a_larger_modulus_sharpens_the_transition() {
        let sigma_o = Pressure::new::<megapascal>(200.0);
        let below = Pressure::new::<megapascal>(150.0);
        let above = Pressure::new::<megapascal>(260.0);
        let (lo_m, hi_m) = (3.0, 12.0);
        assert!(
            weibull_failure_fraction(below, sigma_o, hi_m).get::<ratio>()
                < weibull_failure_fraction(below, sigma_o, lo_m).get::<ratio>(),
            "a sharper distribution must fail FEWER particles below the median"
        );
        assert!(
            weibull_failure_fraction(above, sigma_o, hi_m).get::<ratio>()
                > weibull_failure_fraction(above, sigma_o, lo_m).get::<ratio>(),
            "a sharper distribution must fail MORE particles above the median"
        );
    }

    /// `r` is the report's cube-root mean, not the arithmetic mean. The two are
    /// close for a real layer -- which is exactly why this needs pinning.
    #[test]
    fn mean_radius_is_the_cube_root_mean() {
        let l = layer();
        let r_cbrt = l.mean_radius().get::<micrometer>();
        let arithmetic = 0.5 * (250.0 + 285.0);
        assert!(
            (r_cbrt - 268.639_995).abs() < 1e-4,
            "cube-root mean should be ~268.640 um, got {r_cbrt}"
        );
        assert!(
            (r_cbrt - arithmetic).abs() > 0.1,
            "must not silently be the arithmetic mean ({arithmetic})"
        );
        assert!((l.initial_thickness().get::<micrometer>() - 35.0).abs() < 1e-9);
    }

    /// With no corrosion, Eq (2) and the exact thin-shell form agree exactly;
    /// as the layer thins they diverge, with the exact form always the larger
    /// (it divides by the shrinking thickness, the approximation multiplies by
    /// its linearisation).
    #[test]
    fn approximate_and_exact_stress_agree_at_zero_corrosion() {
        let (l, p) = (layer(), Pressure::new::<megapascal>(30.0));
        let none = Velocity::new::<meter_per_second>(0.0);
        let t0 = Time::new::<second>(0.0);
        let approx = induced_stress(&l, p, none, t0).get::<pascal>();
        let exact = induced_stress_exact(&l, p, none, t0)
            .unwrap()
            .get::<pascal>();
        assert!((approx - exact).abs() < 1e-6, "{approx} vs {exact}");

        // r*p/(2*d_o) computed independently: 115.1314 MPa.
        let expected = 268.639_995e-6 * 30.0e6 / (2.0 * 35.0e-6);
        assert!(
            (approx - expected).abs() / expected < 1e-3,
            "{approx} vs {expected}"
        );

        // Under corrosion the exact form exceeds the approximation.
        let v = Velocity::new::<meter_per_second>(1.0e-10);
        let t = Time::new::<hour>(20.0);
        let a = induced_stress(&l, p, v, t).get::<pascal>();
        let e = induced_stress_exact(&l, p, v, t).unwrap().get::<pascal>();
        assert!(
            e > a,
            "exact {e} should exceed approximate {a} under thinning"
        );
    }

    /// A fully corroded layer has no exact stress to report, and must not come
    /// back as a negative or infinite one.
    #[test]
    fn a_consumed_layer_has_no_exact_stress() {
        let l = layer();
        let v = Velocity::new::<meter_per_second>(1.0e-8);
        let t = Time::new::<hour>(10_000.0);
        assert!(l.actual_thickness(v, t).get::<meter>() <= 0.0);
        assert!(induced_stress_exact(&l, Pressure::new::<megapascal>(30.0), v, t).is_none());
    }

    /// Eq (3) must reduce to `p = nRT/V_f` with `n` the moles of gas released,
    /// computed here independently of the implementation. This is the check
    /// that settles the printed grouping ambiguity (see the fn docs).
    #[test]
    fn pressure_is_the_ideal_gas_law() {
        let v_k = Volume::new::<cubic_meter>(1.8e-13); // ~350 um radius kernel
        let v_f = Volume::new::<cubic_meter>(9.0e-13); // buffer void
        let v_m = MolarVolume::new::<cubic_meter_per_mole>(2.46e-5); // UO2-ish
        let t = ThermodynamicTemperature::new::<degree_celsius>(1600.0);
        let (f_d, f_f, opf, f_b) = (r(0.5), r(STABLE_FISSION_GAS_YIELD), r(0.4), r(0.10));

        let got = internal_gas_pressure(f_d, f_f, opf, f_b, v_f, v_k, v_m, t).get::<pascal>();

        // Independent route: moles of heavy metal, then moles of gas, then nRT/V.
        let n_hm = v_k.get::<cubic_meter>() / v_m.get::<cubic_meter_per_mole>();
        let n_gas = (0.5 * STABLE_FISSION_GAS_YIELD + 0.4) * 0.10 * n_hm;
        let expected =
            n_gas * GAS_CONSTANT_J_PER_MOL_K * (1600.0 + 273.15) / v_f.get::<cubic_meter>();

        assert!(
            (got - expected).abs() / expected < 1e-9,
            "Eq (3) should be nRT/V_f: got {got} Pa, expected {expected} Pa"
        );
        // Pressure must RISE with temperature -- the printed grouping would
        // have it fall, which is how the ambiguity was caught.
        let hotter = internal_gas_pressure(
            f_d,
            f_f,
            opf,
            f_b,
            v_f,
            v_k,
            v_m,
            ThermodynamicTemperature::new::<degree_celsius>(2000.0),
        );
        assert!(hotter.get::<pascal>() > got, "pressure must rise with T");
    }

    /// Survival probabilities multiply, so the combination is symmetric, never
    /// exceeds 1, and is strictly larger than any single term.
    #[test]
    fn failure_populations_combine_by_survival() {
        let (a, b, c) = (r(6.0e-5), r(0.3), r(0.2));
        let total = total_failure_fraction(a, b, c).get::<ratio>();
        let expected = 1.0 - (1.0 - 6.0e-5) * 0.7 * 0.8;
        assert!((total - expected).abs() < 1e-15);

        // Not the sum -- that would double-count.
        let sum = 6.0e-5 + 0.3 + 0.2;
        assert!(total < sum, "{total} should be below the naive sum {sum}");

        // Order cannot matter.
        assert!(
            (total - total_failure_fraction(c, a, b).get::<ratio>()).abs() < 1e-15,
            "combination must be symmetric"
        );

        // Any certain mechanism gives certain failure.
        assert_eq!(
            total_failure_fraction(r(0.0), r(1.0), r(0.0)).get::<ratio>(),
            1.0
        );
        // All-zero gives zero.
        assert_eq!(
            total_failure_fraction(r(0.0), r(0.0), r(0.0)).get::<ratio>(),
            0.0
        );
    }

    /// The end-to-end chain: geometry + pressure -> stress -> Weibull -> total.
    /// Values are illustrative, not validated against PANAMA output -- the
    /// point is that the units compose and the result is a sane fraction.
    #[test]
    fn the_chain_composes_end_to_end() {
        let l = layer();
        let p = internal_gas_pressure(
            r(0.5),
            r(STABLE_FISSION_GAS_YIELD),
            r(0.4),
            r(0.10),
            Volume::new::<cubic_meter>(9.0e-13),
            Volume::new::<cubic_meter>(1.8e-13),
            MolarVolume::new::<cubic_meter_per_mole>(2.46e-5),
            ThermodynamicTemperature::new::<degree_celsius>(1600.0),
        );
        let sigma_t = induced_stress(
            &l,
            p,
            Velocity::new::<meter_per_second>(0.0),
            Time::new::<second>(0.0),
        );
        let phi1 = weibull_failure_fraction(sigma_t, Pressure::new::<megapascal>(200.0), 8.0);
        let total = total_failure_fraction(r(0.0), phi1, r(0.0));
        let v = total.get::<ratio>();
        assert!(
            (0.0..=1.0).contains(&v),
            "failure fraction out of range: {v}"
        );
        assert!(p.get::<pascal>() > 0.0 && sigma_t.get::<pascal>() > 0.0);
    }
}
