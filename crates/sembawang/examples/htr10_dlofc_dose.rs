// SPDX-License-Identifier: GPL-3.0

//! HTR-10 depressurised loss of forced cooling: the **dose** from the
//! `htr10_dlofc_panama_source_term` release, computed with the same dispersion
//! and dose chain as `htr10_air_ingress_kora_bound`, so the two can be put side
//! by side.
//!
//! > **Research, education and V&V only** (`RESPONSIBLE_USE.md`). This is not a
//! > source term or a dose for HTR-10 or for any facility.
//!
//! # Methodology
//!
//! **Release.** Exactly the DLOFC example's chain: HTR-Module's DLOFC shape
//! (`DlofcShape::htr_module_jrc`, 1500 °C peak at 30 h, a stand-in), one node,
//! the NP-MHTGR normal-operation fractions (stand-ins) plus PANAMA's accident
//! increment, Liu & Cao 2002 Table 1 inventory, `accident_release` with the
//! default (upstream) venting and real normal-operation pools (#448).
//!
//! **One change from the DLOFC example: the transient is carried to 96 h, not
//! 200 h**, to match the KORA bound's dose period. Venting spans only the
//! heating leg (≤ 30 h), so this moves no released activity; what it changes
//! is TRISO-ATOPS's half-life screen (4 % of the window: 3.84 h instead of
//! 8 h), which now keeps I-135 and Kr-85m as the KORA run does. The 200 h
//! total is printed as a check.
//!
//! **Dose.** Copied from `htr10_air_ingress_kora_bound` sections 5-6, same
//! constants: `buangkok`'s pyDOSEIA single-plume Gaussian, ground release,
//! ground-level centreline, 1 m/s, the largest χ/Q over classes A-F; FGR-15
//! submersion, FGR-11 inhalation at 0.020 m³/min, FGR-15 groundshine over
//! 96 h with SRS-19 deposition velocities and no plume depletion. Liu & Cao
//! Table 3 circulating activity is added at 100 %, as in the KORA bound.
//!
//! **Pass criterion.** None. An estimate, not a gate.
//!
//! # Results
//!
//! `cargo run --release -p sembawang --example htr10_dlofc_dose`, 2026-09-30.
//! **Not reviewed by a human.**
//!
//! | Quantity | Value |
//! |---|---|
//! | release, 96 h window | **2.218e11 Bq** (I-135 4.2e10 and Kr-85m 1.0e10 kept by the 3.84 h screen) |
//! | release, 200 h window | 1.705e11 Bq; the nuclides common to both windows agree to 1.0000 |
//! | caveat | `negative_atom_count_seen` = true |
//!
//! **Maximum dose at 400 m, first 96 h** (class F, χ/Q = 2.857e-3 s/m³):
//!
//! | Pathway | Dose | Largest contributors |
//! |---|---|---|
//! | Submersion | 0.018 mSv | I-135, I-133 |
//! | Inhalation | 0.587 mSv | I-131 0.204, Cs-134 0.098, I-133 0.078, Cs-137 0.075, Sr-90 0.067 |
//! | Groundshine (96 h) | 0.315 mSv | Cs-134 0.093, I-133 0.076, I-131 0.057 |
//! | **Total** | **0.919 mSv** | against 66.1 mSv for the KORA bound, a factor 72 |
//!
//! | Distance | 400 m | 600 m | 800 m | 1 km | 1.5 km | 2 km | 3 km | 5 km | 10 km |
//! |---|---|---|---|---|---|---|---|---|---|
//! | Dose (mSv) | 0.919 | 0.466 | 0.288 | 0.199 | 0.103 | 0.066 | 0.037 | 0.019 | 0.0075 |
//!
//! **Interpretation.** This is the worst-case plume (class F, no direction
//! change, no depletion) on a release that is itself over-stated: the whole
//! core is put on HTR-Module's hot-node history, while under 5 % of elements
//! reach the peak. The NP-MHTGR `f_hm` is 2-13× worse than German-lineage
//! measured fuel, and the volatile release is linear in it. The gap to the
//! KORA bound (×72) is mostly **not** air: the two cases use different
//! baseline fractions (×6.5) and different venting (heating leg only vs 96 h),
//! with KORA's f_ox adding about ×2.5. That split is arithmetic on the two
//! runs (volatile release linear in the full-failure fraction; the venting
//! factor is the remainder), **not** a controlled run.

use boon_lay::triso_atops_fork::nuclide_model::nuclide_database::find_nuclide;
use buangkok::coefficients::{
    external_coefficient, fgr11_inhalation, fgr11_inhalation_max_over_classes,
    fgr15_air_submersion, fgr15_ground_surface, fgr15_short_lived_progeny,
};
use buangkok::pydoseia::dcf::AgeBracket;
use buangkok::pydoseia::dispersion::{
    dilution_single_plume_no_met, MeanSpeedScaling, PlumeGeometry, Receptor, StabilityClass,
};
use buangkok::pydoseia::dose::{deposition_velocity_m_per_s, submersion_dose, Release};
use changi::activity::inventory::htr10_equilibrium_core;
use changi::activity::primary_helium::htr10_primary_helium_end_of_life;
use sembawang::accident::release::accident_release;
use sembawang::htr10::{self, DlofcShape, Htr10Geometry};
use sembawang::inventory::{CoreInventory, NuclideInventory};
use uom::si::f64::{Length, Radioactivity, Time};
use uom::si::length::meter;
use uom::si::radioactivity::becquerel;
use uom::si::time::hour;

// ---------------------------------------------------------------- the inputs

/// Dose period and transient length \[h\], the KORA bound's 96 h.
const DOSE_PERIOD_HOURS: f64 = 96.0;
/// 1 h step over 96 h, as in the DLOFC example.
const SAMPLES: usize = 97;
/// The DLOFC example's own window and step, for the check.
const DLOFC_WINDOW_HOURS: f64 = 200.0;
const DLOFC_SAMPLES: usize = 201;
const N_RADIAL: usize = 1;
const N_AXIAL: usize = 1;

/// Same receptor and plume constants as `htr10_air_ingress_kora_bound`.
const RECEPTOR_M: f64 = 400.0;
const RELEASE_HEIGHT_M: f64 = 0.0;
const MEASUREMENT_HEIGHT_M: f64 = 10.0;
const BREATHING_M3_PER_S: f64 = 0.020 / 60.0;
const SWEEP_M: [f64; 9] = [400.0, 600.0, 800.0, 1000.0, 1500.0, 2000.0, 3000.0, 5000.0, 10000.0];

fn htr10_geometry() -> Htr10Geometry {
    let particle = tampines::pebble_bed::triso::TrisoParticle::htr10();
    let pebble = tampines::pebble_bed::pebble::Pebble::htr10();
    Htr10Geometry {
        kernel_radius: particle.kernel_radius,
        sic_thickness: particle.silicon_carbide_outer_radius - particle.inner_pyc_outer_radius,
        graphite_thickness: pebble.outer_radius - pebble.fuelled_zone_radius,
    }
}

fn htr10_inventory() -> CoreInventory {
    let kept = htr10_equilibrium_core()
        .into_iter()
        .filter(|e| find_nuclide(e.nuclide).is_some())
        .map(|e| NuclideInventory::uniform(e.nuclide, e.activity, N_RADIAL))
        .collect();
    CoreInventory::new(kept, N_RADIAL, N_AXIAL)
}

fn max_chi_over_q(x_m: f64, geometry: PlumeGeometry) -> (StabilityClass, f64) {
    let per_class = dilution_single_plume_no_met(
        Length::new::<meter>(x_m),
        geometry,
        MeanSpeedScaling::UnitSpeed,
    );
    StabilityClass::ALL
        .iter()
        .zip(per_class.iter())
        .map(|(c, d)| (*c, d.seconds_per_cubic_meter()))
        .fold((StabilityClass::A, 0.0), |a, b| if b.1 > a.1 { b } else { a })
}

/// DLOFC release over `window_h`, per nuclide \[Bq\], and the screened list.
fn dlofc_release(window_h: f64, samples: usize) -> (Vec<(String, f64)>, Vec<String>, bool) {
    let mut shape = DlofcShape::htr_module_jrc();
    shape.total = Time::new::<hour>(window_h);
    let panama = htr10::panama_over_transient(&shape, htr10::stand_in_irradiation_temperature(), samples);
    let fractions = htr10::with_panama_accident_increment(
        htr10::np_mhtgr_normal_operation_fractions(),
        panama.accident_increment_final,
    );
    let plant = htr10::plant_parameters(htr10_geometry(), fractions);
    let transient = shape.transient(samples, N_RADIAL, N_AXIAL).expect("enough samples");
    let out = accident_release(&htr10_inventory(), &transient, &plant).expect("the release chain runs");
    let per = out
        .source_term
        .nuclides
        .iter()
        .map(|r| (r.label.clone(), r.total_released().get::<becquerel>()))
        .collect();
    (per, out.screened_out, out.caveats.negative_atom_count_seen)
}

fn main() {
    println!("================================================================");
    println!(" HTR-10 DLOFC (HTR-Module stand-in shape) -> dose, KORA-bound dose chain");
    println!(" RESEARCH, EDUCATION AND V&V ONLY. Not a source term, not a dose.");
    println!("================================================================\n");

    // ------------------------------------------------ 1. release
    let (atops, screened, negative) = dlofc_release(DOSE_PERIOD_HOURS, SAMPLES);
    let (atops_200, screened_200, _) = dlofc_release(DLOFC_WINDOW_HOURS, DLOFC_SAMPLES);
    let circulating = htr10_primary_helium_end_of_life();
    let circ_bq = |n: &str| {
        circulating
            .iter()
            .find(|e| e.nuclide == n)
            .map_or(0.0, |e| e.activity.get::<becquerel>())
    };
    let total_96: f64 = atops.iter().map(|(_, b)| b).sum();
    let total_200: f64 = atops_200.iter().map(|(_, b)| b).sum();
    let common_96: f64 = atops
        .iter()
        .filter(|(n, _)| atops_200.iter().any(|(m, _)| m == n))
        .map(|(_, b)| b)
        .sum();
    println!("-- 1. release (TRISO-ATOPS, real pools #448)");
    println!("   {DOSE_PERIOD_HOURS} h window: total {total_96:.4e} Bq; screened {screened:?}");
    println!("   {DLOFC_WINDOW_HOURS} h window: total {total_200:.4e} Bq; screened {screened_200:?}");
    println!(
        "   check, nuclides in both windows: 96 h {common_96:.4e} vs 200 h {total_200:.4e} (ratio {:.4})",
        common_96 / total_200
    );
    println!("   negative atom count seen (96 h): {negative}\n");

    // ------------------------------------------------ 2. dose at 400 m
    let geometry = PlumeGeometry {
        release_height: Length::new::<meter>(RELEASE_HEIGHT_M),
        measurement_height: Length::new::<meter>(MEASUREMENT_HEIGHT_M),
        receptor: Receptor::GroundLevelCentreline,
    };
    let per_class = dilution_single_plume_no_met(
        Length::new::<meter>(RECEPTOR_M),
        geometry,
        MeanSpeedScaling::UnitSpeed,
    );
    let (worst_class, worst_v) = max_chi_over_q(RECEPTOR_M, geometry);
    let chi = per_class[StabilityClass::ALL.iter().position(|c| *c == worst_class).unwrap()];
    println!("-- 2. MAXIMUM dose at {RECEPTOR_M} m, first {DOSE_PERIOD_HOURS} h (class {worst_class:?}, chi/Q {worst_v:.4e} s/m3)");

    let sub_table = fgr15_air_submersion();
    let chains = fgr15_short_lived_progeny();
    let inh_table = fgr11_inhalation();
    let gs_table = fgr15_ground_surface();
    let exposure_s = DOSE_PERIOD_HOURS * 3600.0;
    println!("   nuclide     Q [Bq]       submersion [Sv]   inhalation [Sv]   groundshine [Sv]   total [Sv]");
    let (mut e_sub_sum, mut e_inh_sum, mut e_gs_sum) = (0.0, 0.0, 0.0);
    let mut missing = Vec::new();
    for (n, bq) in &atops {
        let q = bq + circ_bq(n);
        let psi = chi.seconds_per_cubic_meter() * q;
        let release = Release::Instantaneous(Radioactivity::new::<becquerel>(q));
        let e_sub = external_coefficient(&sub_table, &chains, n, AgeBracket::Adult)
            .map(|dcf| submersion_dose(chi, release, dcf).sieverts());
        let e_inh = fgr11_inhalation_max_over_classes(&inh_table, n, AgeBracket::Adult)
            .map(|dcf| psi * dcf * BREATHING_M3_PER_S);
        let element = n.split('-').next().unwrap_or("");
        let v_d = deposition_velocity_m_per_s(element);
        let lam = find_nuclide(n)
            .map(|x| x.decay_constant().get::<uom::si::frequency::hertz>())
            .unwrap_or(0.0);
        let decay_integral = if lam > 0.0 { -(-lam * exposure_s).exp_m1() / lam } else { exposure_s };
        let e_gs = if v_d == 0.0 {
            Some(0.0)
        } else {
            external_coefficient(&gs_table, &chains, n, AgeBracket::Adult)
                .map(|dcf| psi * v_d * dcf * decay_integral)
        };
        let fmt = |e: Option<f64>| e.map_or_else(|| "MISSING".to_string(), |v| format!("{v:.3e}"));
        println!(
            "   {n:<10} {q:>11.3e}   {:>15}   {:>15}   {:>16}   {:>10.3e}",
            fmt(e_sub),
            fmt(e_inh),
            fmt(e_gs),
            e_sub.unwrap_or(0.0) + e_inh.unwrap_or(0.0) + e_gs.unwrap_or(0.0)
        );
        e_sub_sum += e_sub.unwrap_or(0.0);
        e_inh_sum += e_inh.unwrap_or(0.0);
        e_gs_sum += e_gs.unwrap_or(0.0);
        if e_sub.is_none() || e_gs.is_none() {
            missing.push(n.as_str());
        }
    }
    let total = e_sub_sum + e_inh_sum + e_gs_sum;
    println!(
        "   TOTAL: submersion {:.3} mSv, inhalation {:.3} mSv, groundshine {:.3} mSv, sum {:.3} mSv",
        1e3 * e_sub_sum,
        1e3 * e_inh_sum,
        1e3 * e_gs_sum,
        1e3 * total
    );
    println!("   missing an external coefficient (NOT zero): {missing:?}");
    println!("   NOT computed: ingestion, screened-out nuclides, H-3 / Xe-135m / Rb-88.\n");

    // ------------------------------------------------ 3. dose vs distance
    let dose_per_chi = total / worst_v;
    println!("-- 3. MAXIMUM dose vs distance, first {DOSE_PERIOD_HOURS} h");
    println!("   distance [m]   class   chi/Q [s/m3]    dose [mSv]");
    for x in SWEEP_M {
        let (c, v) = max_chi_over_q(x, geometry);
        println!("   {x:>12.0}   {c:?}       {v:>11.4e}   {:>10.4}", 1e3 * dose_per_chi * v);
    }
}
