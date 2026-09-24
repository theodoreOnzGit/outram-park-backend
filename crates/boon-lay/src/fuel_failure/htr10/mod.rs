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
//             with citation, as scientific facts.
// HTR-10    : geometry and design data from IAEA-TECDOC-1382 part 2 (public)
//             and JAERI-Conf 96-010 (public), cited inline. No proprietary or
//             operational data is used; see DATA_POLICY.md.

//! **HTR-10 applied to PANAMA-I — an EXTRAPOLATION, reported as one.**
//!
//! PANAMA-I was built and validated for **German** TRISO: its reactor cases
//! are HTR-Module and HTR-500, its heating experiments are FRJ2-K11/03 and
//! AVR GO 2, and page -479- claims good agreement only over 1600–2500 °C.
//! HTR-10's fuel is German-lineage — a 60 mm pebble with a 500 µm UO₂ kernel
//! and 35 µm SiC — which is why applying the model to it is *defensible*.
//! **It is not a validated application, and nothing here should be quoted as
//! one.** Two of the inputs are not published for HTR-10 at all and are taken
//! from the report's own HTR-Module column, by name.
//!
//! # What is HTR-10's, what is derived, and what is a stand-in
//!
//! | Input | Value | Where from |
//! |---|---|---|
//! | SiC layer `r_i`/`r_a` | 380 / 415 µm | IAEA-TECDOC-1382 pt 2 Table 4-17, via `tampines::pebble_bed::triso::TrisoParticle::htr10` |
//! | kernel radius | 250 µm | same |
//! | `V_k` | kernel sphere | **derived** from the above |
//! | `V_f` | ½ × buffer shell | **derived**; the report's own definition (page -485-) |
//! | `F_b` | **0.0851 FIMA** | **derived** from the published 80 000 MWd/t — see [`BURNUP_FIMA`] |
//! | `t_B` | **1080 FPD** | **derived** from 10 MW over 27 000 × 5 g HM — see [`RESIDENCE_FULL_POWER_DAYS`] |
//! | kernel compound | UO₂, 17 % enriched | IAEA-TECDOC-1382 pt 2 §4 design table |
//! | `T_B` | **an input** | HTR-10 publishes a *maximum* fuel temperature, not an average |
//! | `σ_oo`/`m_oo` | **834 MPa / 8.02** | **STAND-IN**: EO 1607, the variety the report's own HTR-Module runs use (footnote 1, page -503-) |
//! | `Γ` | **1.4·10²⁵ m⁻² EDN** | **STAND-IN**: the report's HTR-Module/HTR-500 value (Table 2, page -504-) |
//!
//! The two stand-ins are named rather than absorbed. HTR-Module's reference
//! particle is a **500 µm UO₂-LTI-TRISO** (footnote 1, page -503-), i.e. the
//! same kernel diameter as HTR-10's, at 0.08 FIMA against HTR-10's 0.0851 and
//! 1020 FPD against 1080 — so it is the closest published case there is. That
//! is an argument for the stand-in being *reasonable*, not for it being
//! HTR-10's fuel. Picking a Table 1 variety and calling it HTR-10 without
//! saying so would be putting one reactor's fuel quality under another's name.
//!
//! # Result 1 — normal operation: PANAMA must NOT replace the `f_inc` placeholder
//!
//! `htgr_sim_v1`'s `TRISO_ATOPS_REFERENCE_FAILURE_FRACTIONS` carries
//! `f_inc = 3·10⁻⁵`, documented there as a TRISO-ATOPS reference value rather
//! than HTR-10 data, and release scales linearly in it. The obvious move is to
//! compute `f_inc` with PANAMA instead. **That would be wrong, and by a very
//! large margin.**
//!
//! `φ₁` at the end of irradiation, over the whole plausible fuel-temperature
//! band (measured 2026-09-24):
//!
//! | `T_B` | `OPF` | `F_d` | `σ_t` | `φ₁` |
//! |---|---|---|---|---|
//! | 700 °C | 1.3·10⁻³ | 0.101 | 6.9 MPa | **2.8·10⁻¹⁵** |
//! | 776 °C (HTR-Module's average) | 5.7·10⁻³ | 0.196 | 15.2 MPa | **1.2·10⁻¹²** |
//! | 900 °C | 4.1·10⁻² | 0.452 | 46.3 MPa | **5.0·10⁻⁹** |
//! | 1000 °C | 0.153 | 0.707 | 103 MPa | **1.6·10⁻⁶** |
//!
//! So the pressure-vessel mechanism contributes between **10⁻¹⁵ and 10⁻⁶**,
//! against a placeholder of 3·10⁻⁵ — seven orders of magnitude at the
//! best-supported temperature. Substituting the computed value would divide
//! every activity `htgr_sim_v1` reports by about 10⁷.
//!
//! **The placeholder is not a pressure-vessel number, and that is the point.**
//! `3·10⁻⁵` is the same order as PANAMA's own as-manufactured target
//! `φ_o = 6·10⁻⁵` (page -480-) — a *manufacturing and irradiation* defect
//! population, which PANAMA takes as an **input** and does not model. The
//! right conclusion is the one already written in that module: `f_inc` there
//! needs HTR-10 fuel-qualification data, not a better model. PANAMA cannot
//! supply it and this crate must not pretend otherwise.
//!
//! # Result 2 — accident: this is where the seam is worth having
//!
//! Under accident conditions, which is what PANAMA is *for*, the number stops
//! being negligible. 200 h isothermal, `T_B = 776 °C`, measured 2026-09-24:
//!
//! | accident `T` | `F_d` | `FKOR` | `σ_t` | `φ₁` | `φ₂` (14b) |
//! |---|---|---|---|---|---|
//! | 1200 °C | 0.373 | 1.0005 | 50 MPa | 4.53·10⁻⁹ | ~0 |
//! | 1400 °C | 0.630 | 1.0030 | 103 MPa | 6.85·10⁻⁷ | ~0 |
//! | **1600 °C** | 0.889 | 1.0119 | 178 MPa | **3.10·10⁻⁵** | 3.4·10⁻¹⁵ |
//! | 1800 °C | 0.992 | 1.0363 | 262 MPa | 4.46·10⁻⁴ | 3.3·10⁻⁹ |
//! | 2000 °C | 1.000 | 1.0906 | 370 MPa | 4.79·10⁻³ | 2.78·10⁻⁴ |
//! | 2200 °C | 1.000 | 1.1953 | 536 MPa | 6.07·10⁻² | **0.977** |
//!
//! **The 1600 °C figure landing on 3.1·10⁻⁵, beside a 3·10⁻⁵ placeholder, is
//! a coincidence.** They are different quantities — one is 200 h at HTR-10's
//! accident temperature limit, the other is an as-manufactured defect fraction
//! for a different fuel line. Reporting the coincidence as agreement would be
//! exactly the kind of accident this file exists to avoid.
//!
//! Two things the table does show, and they are the model's own structure:
//! `φ₂` overtakes `φ₁` between 2000 and 2200 °C, matching the report's
//! statement (page -508-) that thermal decomposition governs above ~2000 °C;
//! and `F_d` saturates by 1800 °C, so above that the growth is all `FKOR` and
//! `OPF`.
//!
//! # Not verified
//!
//! **Nothing here is compared against published HTR-10 data.** The workspace's
//! local literature gives HTR-10's geometry, burnup, enrichment and power but
//! **no measured fuel failure fraction, free-uranium fraction or release
//! fraction** — so the comparison that would make this a validation could not
//! be made, and is not claimed. The nearest available check is the report's
//! own statement (page -504-) that HTR-Module depressurised stays below 10⁻⁶
//! at 200 h; a *flat* 200 h at 1600 °C gives 3.1·10⁻⁵ here, which is an upper
//! bound on a transient that only briefly reaches its peak, so the two are not
//! in conflict — but without Fig. 10's temperature history it is not a check
//! either. Digitising Fig. 10 would make it one.

use uom::si::f64::{Length, Pressure, Ratio, ThermodynamicTemperature, Time, Volume};
use uom::si::length::micrometer;
use uom::si::pressure::megapascal;
use uom::si::ratio::ratio;
use uom::si::time::day;
use uom::si::volume::cubic_meter;

use super::diffusion::KernelKind;
use super::geometry::SicLayer;
use super::history::{irradiation_tau, OxygenSource, ParticleState};
use super::molar_volume::KernelCompound;
use super::strength::{irradiated_strength, irradiated_weibull_modulus};
use super::{decomposition::DecompositionCalibration, grain_boundary::GrainBoundaryCorrosion};

/// HTR-10 kernel radius, 250 µm (IAEA-TECDOC-1382 pt 2 Table 4-17).
pub const KERNEL_RADIUS_UM: f64 = 250.0;
/// Buffer outer radius, 340 µm (same source).
pub const BUFFER_OUTER_RADIUS_UM: f64 = 340.0;
/// SiC inner radius (= IPyC outer), 380 µm (same source).
pub const SIC_INNER_RADIUS_UM: f64 = 380.0;
/// SiC outer radius, 415 µm — a 35 µm layer (same source).
pub const SIC_OUTER_RADIUS_UM: f64 = 415.0;

/// HTR-10 design mean burnup as a **FIMA fraction**, derived from the
/// published 80 000 MWd/t (IAEA-TECDOC-1382 pt 2 §4; Li et al. 2014).
///
/// Derivation, so it can be checked rather than trusted: at 17 % enrichment
/// the heavy-metal molar mass is `0.17·235 + 0.83·238 = 237.5 g/mol`, so one
/// tonne holds `2.536·10²⁷` atoms. One per cent of them fissioning at
/// 200 MeV releases `8.13·10¹⁴ J = 9404 MWd`. Hence
/// `80 000 / 9404 = 8.51 % FIMA`.
///
/// The 200 MeV is the conventional recoverable energy per fission and is the
/// only assumption not taken from the HTR-10 literature; it moves the answer
/// by about ±2 % across the usual 195–205 MeV range.
pub const BURNUP_FIMA: f64 = 0.0851;

/// HTR-10 mean fuel residence at full power, **1080 FPD**, derived.
///
/// The core holds 27 000 elements at 5 g heavy metal each (IAEA-TECDOC-1382
/// pt 2 §4 design table) — 135 kg — at 10 MW thermal, so the specific power
/// is `10/0.135 = 74.07 MW/t` and reaching the design mean 80 000 MWd/t takes
/// `80 000/74.07 = 1080` full-power days. Comparable to the report's own
/// HTR-Module case (15 passes × 68 d = 1020 FPD).
pub const RESIDENCE_FULL_POWER_DAYS: f64 = 1080.0;

/// **STAND-IN.** SiC tensile strength before irradiation, 834 MPa — EO 1607,
/// the variety the report's own HTR-Module and HTR-500 runs use (footnote 1,
/// page -503-; Table 2, page -504-). **Not HTR-10 data**; no `σ_oo` for
/// HTR-10's SiC is published in this workspace's literature.
///
/// 834/8.02 rather than Table 1's 850/8.0: the two disagree and the units doc
/// settles on the Fig. 5 / Table 2 pair for reactor reproductions.
pub const STAND_IN_STRENGTH_MPA: f64 = 834.0;

/// **STAND-IN.** Weibull modulus before irradiation, 8.02 — EO 1607, as above.
pub const STAND_IN_WEIBULL_MODULUS: f64 = 8.02;

/// **STAND-IN.** Fast fluence at discharge, `1.4·10²⁵ m⁻² EDN` — the report's
/// HTR-Module and HTR-500 value (Table 2, page -504-). **Not HTR-10 data.**
pub const STAND_IN_FLUENCE_E25_PER_M2: f64 = 1.4;

/// The HTR-10 SiC layer (IAEA-TECDOC-1382 pt 2 Table 4-17).
pub fn sic_layer() -> SicLayer {
    SicLayer {
        inner_radius: Length::new::<micrometer>(SIC_INNER_RADIUS_UM),
        outer_radius: Length::new::<micrometer>(SIC_OUTER_RADIUS_UM),
    }
}

/// The kernel volume `V_k`, from the published kernel radius.
pub fn kernel_volume() -> Volume {
    let r = KERNEL_RADIUS_UM * 1.0e-6;
    Volume::new::<cubic_meter>(4.0 / 3.0 * std::f64::consts::PI * r * r * r)
}

/// The free volume `V_f` — **half the buffer shell**, which is the report's
/// own definition of `V_f` (page -485-: "corresponding to 50 % of buffer
/// volume"), applied to HTR-10's published buffer.
pub fn free_volume() -> Volume {
    let r_k = KERNEL_RADIUS_UM * 1.0e-6;
    let r_b = BUFFER_OUTER_RADIUS_UM * 1.0e-6;
    Volume::new::<cubic_meter>(
        0.5 * 4.0 / 3.0 * std::f64::consts::PI * (r_b * r_b * r_b - r_k * r_k * r_k),
    )
}

/// The HTR-10 particle as PANAMA-I sees it, at a stated irradiation
/// temperature.
///
/// - `irradiation_temperature` — `T_B`. **An input**: HTR-10 publishes a
///   *maximum* fuel temperature (JAERI-Conf 96-010 states a 700 °C margin to
///   the 1600 °C limit) but no average, and PANAMA's `T_B` is an average. The
///   report's HTR-Module average of **776 °C** (Table 2) is the nearest
///   published figure for a comparable core and is what
///   [`tests`] sweeps around; it is not HTR-10's.
///
/// Uses [`STAND_IN_STRENGTH_MPA`], [`STAND_IN_WEIBULL_MODULUS`] and
/// [`STAND_IN_FLUENCE_E25_PER_M2`] — read their docs before quoting any
/// number this produces.
pub fn particle(irradiation_temperature: ThermodynamicTemperature) -> ParticleState {
    let burnup = Ratio::new::<ratio>(BURNUP_FIMA);
    let t_b_time = Time::new::<day>(RESIDENCE_FULL_POWER_DAYS);
    ParticleState {
        layer: sic_layer(),
        compound: KernelCompound::UraniumOxide,
        diffusion_kernel: KernelKind::UraniumOxide,
        kernel_volume: kernel_volume(),
        free_volume: free_volume(),
        burnup,
        stable_gas_yield: Ratio::new::<ratio>(super::STABLE_FISSION_GAS_YIELD),
        dimensionless_irradiation_time: irradiation_tau(
            KernelKind::UraniumOxide,
            irradiation_temperature,
            t_b_time,
            burnup,
        ),
        median_strength: irradiated_strength(
            Pressure::new::<megapascal>(STAND_IN_STRENGTH_MPA),
            STAND_IN_FLUENCE_E25_PER_M2,
            irradiation_temperature,
        ),
        weibull_modulus: irradiated_weibull_modulus(
            STAND_IN_WEIBULL_MODULUS,
            STAND_IN_FLUENCE_E25_PER_M2,
            irradiation_temperature,
        ),
        oxygen: OxygenSource::UraniumOxide {
            irradiation_temperature,
            irradiation_time: t_b_time,
        },
        decomposition: DecompositionCalibration::ParticlesInSphere,
        grain_boundary: GrainBoundaryCorrosion::Disabled,
        as_manufactured: Ratio::new::<ratio>(0.0),
    }
}

/// `φ₁` at the **end of irradiation** — the value PANAMA assigns to `t = 0`
/// of an accident (page -482-), and the one that matters for normal
/// operation.
///
/// # Why this is not an accident step of length zero
///
/// It uses **Eq (5b)**, not Eq (5c). Eq (5c) is the `OPF` "during heating"
/// and carries `−0.404·(10⁴/T − 10⁴/(T_B + 75))`, which does **not** vanish
/// at `T = T_B`: the `+75 °C` is the report's kernel-versus-surface
/// correction, so Eq (5c) reduces to Eq (5b) at `T = T_B + 75`, the kernel
/// temperature, and not at the surface temperature `T_B`. Feeding `T_B`
/// through the accident path instead would apply a spurious 0.26-decade
/// cooling term and under-state `OPF` by a factor 1.8.
///
/// `FKOR = 1` and `τ_a = 0` here: no accident has happened, so there is no
/// corrosion and no accident-time gas release.
pub fn end_of_irradiation_failure(
    irradiation_temperature: ThermodynamicTemperature,
) -> super::FailureFraction {
    use super::booth::released_gas_fraction;
    use super::oxygen::{oxygen_per_fission_uo2, HeatingRegime};
    use super::pressure::internal_gas_pressure;
    use super::stress::induced_stress_with_thinning_factor;
    use super::weibull::weibull_failure_fraction;

    let p = particle(irradiation_temperature);
    let opf = oxygen_per_fission_uo2(
        irradiation_temperature,
        Time::new::<day>(RESIDENCE_FULL_POWER_DAYS),
        HeatingRegime::BeforeHeating,
    );
    let f_d = released_gas_fraction(p.dimensionless_irradiation_time, Ratio::new::<ratio>(0.0));
    let gas_pressure = internal_gas_pressure(
        f_d,
        p.stable_gas_yield,
        opf,
        p.burnup,
        p.free_volume,
        p.kernel_volume,
        super::molar_volume::molar_volume(p.compound),
        irradiation_temperature,
    );
    let sigma_t =
        induced_stress_with_thinning_factor(&p.layer, gas_pressure, Ratio::new::<ratio>(1.0));
    weibull_failure_fraction(sigma_t, p.median_strength, p.weibull_modulus)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fuel_failure::history::{AccidentHistory, AccidentStep};
    use uom::si::thermodynamic_temperature::degree_celsius;
    use uom::si::time::hour;

    fn t_b(c: f64) -> ThermodynamicTemperature {
        ThermodynamicTemperature::new::<degree_celsius>(c)
    }

    fn end_of_irradiation_phi_1(irradiation_c: f64) -> f64 {
        end_of_irradiation_failure(t_b(irradiation_c)).get::<ratio>()
    }

    /// The derived inputs reproduce the published design figures they come
    /// from, so the derivations can be checked rather than trusted.
    ///
    /// - 27 000 elements × 5 g = 135 kg heavy metal at 10 MW is 74.07 MW/t,
    ///   and 80 000 MWd/t at that rate is 1080 FPD.
    /// - 80 000 MWd/t at 9404 MWd per % FIMA (200 MeV/fission, 237.5 g/mol)
    ///   is 8.51 % FIMA.
    #[test]
    fn the_derived_inputs_follow_from_the_published_design_data() {
        let heavy_metal_tonnes: f64 = 27_000.0 * 5.0e-3 / 1000.0;
        assert!((heavy_metal_tonnes - 0.135).abs() < 1e-12);
        let specific_power_mw_per_t = 10.0 / heavy_metal_tonnes;
        let days = 80_000.0 / specific_power_mw_per_t;
        assert!(
            (days - RESIDENCE_FULL_POWER_DAYS).abs() < 1.0,
            "residence derives to {days:.0} FPD"
        );

        let molar_mass_g = 0.17 * 235.0 + 0.83 * 238.0;
        let atoms_per_tonne = 1.0e6 / molar_mass_g * 6.022_14e23;
        let mwd_per_percent_fima =
            atoms_per_tonne * 0.01 * 200.0 * 1.602_177e-13 / 86_400.0 / 1.0e6;
        let fima = 80_000.0 / mwd_per_percent_fima / 100.0;
        assert!(
            (fima - BURNUP_FIMA).abs() < 5.0e-4,
            "burnup derives to {:.4} FIMA at {mwd_per_percent_fima:.0} MWd/t per %",
            fima
        );

        // Geometry: 35 um SiC on a 250 um kernel, and V_f/V_k = 0.758.
        assert!((sic_layer().initial_thickness().get::<micrometer>() - 35.0).abs() < 1e-9);
        let ratio_vf_vk = free_volume().get::<cubic_meter>() / kernel_volume().get::<cubic_meter>();
        assert!(
            (ratio_vf_vk - 0.7577).abs() < 1e-3,
            "V_f/V_k = {ratio_vf_vk:.4}"
        );
    }

    /// **PANAMA must NOT replace `htgr_sim_v1`'s `f_inc = 3·10⁻⁵` for normal
    /// operation.**
    ///
    /// Methodology: evaluate `φ₁` at the end of irradiation across the whole
    /// plausible HTR-10 fuel-temperature band, since `T_B` is an input.
    ///
    /// Results, 2026-09-24: `2.8·10⁻¹⁵` at 700 °C, `1.2·10⁻¹²` at 776 °C (the
    /// HTR-Module average), `5.0·10⁻⁹` at 900 °C, `1.6·10⁻⁶` at 1000 °C —
    /// between four and thirteen orders of magnitude below the placeholder.
    ///
    /// The conclusion is **not** that the placeholder is too high. It is that
    /// the two are different quantities: `3·10⁻⁵` is an as-manufactured defect
    /// fraction, the same order as PANAMA's own `φ_o` target of `6·10⁻⁵`
    /// (page -480-), which PANAMA takes as an **input** and does not model.
    /// Substituting the computed value would divide every activity
    /// `htgr_sim_v1` reports by about 10⁷ on the strength of a model that is
    /// not answering that question.
    #[test]
    fn normal_operation_failure_is_negligible_against_the_placeholder() {
        const PLACEHOLDER: f64 = 3.0e-5;
        for (c, want) in [
            (700.0, 2.8e-15),
            (776.0, 1.2e-12),
            (900.0, 5.0e-9),
            (1000.0, 1.6e-6),
        ] {
            let got = end_of_irradiation_phi_1(c);
            assert!(
                (got / want).log10().abs() < 0.05,
                "T_B = {c} degC: phi_1 = {got:e}, expected ~{want:e}"
            );
            assert!(
                got < PLACEHOLDER,
                "the pressure-vessel mechanism must stay below the as-manufactured \
                 placeholder across the whole band"
            );
        }
        // Even the most pessimistic temperature in the band is 19x below it.
        assert!(end_of_irradiation_phi_1(1000.0) * 19.0 < PLACEHOLDER);
        // And it rises steeply with T_B -- this is a sensitivity, not a constant.
        assert!(
            end_of_irradiation_phi_1(1000.0) / end_of_irradiation_phi_1(700.0) > 1.0e8,
            "phi_1 spans more than eight decades over 700-1000 degC, which is why \
             T_B is an input and not a guess"
        );
    }

    /// **Accident conditions are where the seam is worth having** — and the
    /// 1600 °C figure landing next to the placeholder is a coincidence.
    ///
    /// Methodology: 200 h isothermal at each accident temperature, `T_B` at
    /// the HTR-Module average of 776 °C, stepped hourly through
    /// [`AccidentHistory`].
    ///
    /// Results, 2026-09-24: `φ_total` = 4.53·10⁻⁹ (1200 °C), 6.85·10⁻⁷
    /// (1400 °C), **3.10·10⁻⁵ (1600 °C)**, 4.46·10⁻⁴ (1800 °C), 5.07·10⁻³
    /// (2000 °C), 0.978 (2200 °C).
    ///
    /// The 2200 °C value is dominated by `φ₂`, which overtakes `φ₁` between
    /// 2000 and 2200 °C — matching the report's own statement (page -508-)
    /// that thermal decomposition governs above ~2000 °C. That is the one
    /// qualitative claim in this test that the report itself makes.
    #[test]
    fn the_accident_sweep_is_where_panama_has_something_to_say() {
        let run = |accident_c: f64| {
            let p = particle(t_b(776.0));
            let mut h = AccidentHistory::new(p, Ratio::new::<ratio>(0.0));
            for _ in 0..200 {
                h.step(AccidentStep {
                    duration: Time::new::<hour>(1.0),
                    mean_temperature: t_b(accident_c),
                });
            }
            h.progress()
        };

        for (c, want) in [
            (1200.0, 4.53e-9),
            (1400.0, 6.85e-7),
            (1600.0, 3.10e-5),
            (1800.0, 4.46e-4),
        ] {
            let got = run(c).total.get::<ratio>();
            assert!(
                (got / want).log10().abs() < 0.08,
                "{c} degC / 200 h: phi_total = {got:e}, expected ~{want:e}"
            );
        }

        // phi_2 overtakes phi_1 between 2000 and 2200 degC (page -508-).
        let hot = run(2000.0);
        assert!(
            hot.thermal_decomposition < hot.pressure_vessel,
            "at 2000 degC the pressure vessel still governs"
        );
        let hotter = run(2200.0);
        assert!(
            hotter.thermal_decomposition > hotter.pressure_vessel,
            "by 2200 degC thermal decomposition must govern: phi_2 = {:?}, phi_1 = {:?}",
            hotter.thermal_decomposition,
            hotter.pressure_vessel
        );

        // The 1600 degC value sits next to the 3e-5 placeholder. That is a
        // COINCIDENCE of two unrelated quantities and is pinned here only so
        // nobody later reads it as agreement.
        let at_1600 = run(1600.0).total.get::<ratio>();
        assert!(
            (at_1600 / 3.0e-5).log10().abs() < 0.1,
            "the coincidence is real ({at_1600:e} against 3e-5) -- and it is a \
             coincidence: 200 h at HTR-10's accident limit is not an \
             as-manufactured defect fraction for another fuel line"
        );
    }
}
