// SPDX-License-Identifier: GPL-3.0

//! HTR-10 depressurised loss of forced cooling: **PANAMA-I fuel failure wired
//! into the TRISO-ATOPS release chain**, driven by the published HTR-10
//! equilibrium-core inventory.
//!
//! ```bash
//! cargo run --release -p sembawang --example htr10_dlofc_panama_source_term
//! ```
//!
//! # What this is
//!
//! The first case in this workspace that runs `boon-lay`'s two halves together
//! on one reactor: `fuel_failure` (PANAMA-I, HTA-IB-03/90) supplies the
//! accident-added particle failure fraction `f_inc_acc`, and
//! `triso_atops_fork` (INL TRISO-ATOPS) turns it into released activity. The
//! seam itself, the transient shape and every provenance statement are in
//! [`sembawang::htr10`] — read that module before quoting any number below.
//!
//! GitHub #296.
//!
//! # RESEARCH, EDUCATION AND V&V ONLY — and this is not a source term
//!
//! `RESPONSIBLE_USE.md` applies with full force. Nothing here may be used for
//! facility operation, licensing, safety-critical decisions, emergency
//! planning or emergency response, and no dose quantity is computed.
//!
//! Three separate reasons no number below is HTR-10's source term:
//!
//! 1. **The transient is HTR-Module's**, used as a stand-in because HTR-10's
//!    own DLOFC fuel-temperature history is not in this workspace's
//!    literature. HTR-10 is 10 MW against 200 MW, so the stand-in is the
//!    conservative direction — but it is still another reactor's transient.
//! 2. **The four normal-operation failure fractions are NP-MHTGR's**, not
//!    HTR-10 fuel-qualification data. Volatile release is linear in `f_hm`, so
//!    this sets the magnitude of every activity reported.
//! 3. **One node.** The whole core is put on the hot-node history, while the
//!    source itself says under 5 % of elements reach the peak.
//!
//! Every one of those is stated at its own table below rather than only here.

use boon_lay::triso_atops_fork::accident::AccidentFractions;
use boon_lay::triso_atops_fork::nuclide_model::nuclide_database::find_nuclide;
use changi::activity::inventory::htr10_equilibrium_core;
use sembawang::accident::release::accident_release;
use sembawang::htr10::{
    self, DlofcShape, Htr10Geometry, DLOFC_PEAK_CELSIUS, DLOFC_TABLE_44_MAXIMUM_CELSIUS,
    DLOFC_TABLE_44_NOMINAL_CELSIUS, DLOFC_TIME_TO_PEAK_HOURS, REPORTING_WINDOW_HOURS,
    STAND_IN_IRRADIATION_CELSIUS,
};
use sembawang::inventory::{CoreInventory, NuclideInventory};
use uom::si::f64::{ThermodynamicTemperature, Time};
use uom::si::radioactivity::becquerel;
use uom::si::thermodynamic_temperature::degree_celsius;
use uom::si::time::hour;

/// Time samples over the 200 h transient. 201 gives a 1 h step, which resolves
/// the 30 h rise in thirty intervals; PANAMA's step is the interval midpoint
/// temperature, so a finer grid changes `φ₁` by less than the width of the
/// printed column (checked in the module's tests).
const SAMPLES: usize = 201;

/// One radial ring and one axial node. The HTR-10 inventory is a whole-core
/// figure and the transient is uniform, so any finer node count would be
/// inventing a distribution neither input carries.
const N_RADIAL: usize = 1;
const N_AXIAL: usize = 1;

/// Isothermal holds PANAMA is reported at, for comparison with HTA-IB-03/90's
/// own statements and with `boon_lay::fuel_failure::htr10`'s tables.
const ISOTHERMAL_CELSIUS: [f64; 7] = [1200.0, 1400.0, 1500.0, 1600.0, 1800.0, 2000.0, 2200.0];

fn htr10_geometry() -> Htr10Geometry {
    // Read from `tampines`, never retyped: IAEA-TECDOC-1382 part 2 Table 4-17.
    let particle = tampines::pebble_bed::triso::TrisoParticle::htr10();
    let pebble = tampines::pebble_bed::pebble::Pebble::htr10();
    Htr10Geometry {
        kernel_radius: particle.kernel_radius,
        sic_thickness: particle.silicon_carbide_outer_radius - particle.inner_pyc_outer_radius,
        graphite_thickness: pebble.outer_radius - pebble.fuelled_zone_radius,
    }
}

/// The published HTR-10 inventory, restricted to the nuclides TRISO-ATOPS
/// models. Returns the inventory and the names it had to drop.
fn htr10_inventory() -> (CoreInventory, Vec<(String, f64)>) {
    let mut kept = Vec::new();
    let mut dropped = Vec::new();
    for entry in htr10_equilibrium_core() {
        let bq = entry.activity.get::<becquerel>();
        if find_nuclide(entry.nuclide).is_some() {
            kept.push(NuclideInventory::uniform(
                entry.nuclide,
                entry.activity,
                N_RADIAL,
            ));
        } else {
            dropped.push((entry.nuclide.to_string(), bq));
        }
    }
    (CoreInventory::new(kept, N_RADIAL, N_AXIAL), dropped)
}

/// Total released activity per nuclide \[Bq\], summed over the release windows.
fn released_bq(term: &changi::activity::source::SourceTerm) -> Vec<(String, f64)> {
    term.nuclides
        .iter()
        .map(|r| (r.label.clone(), r.total_released().get::<becquerel>()))
        .collect()
}

/// Run the whole chain at one peak temperature and one fraction set, and
/// return `(total released Bq, per-nuclide, caveat line, window count)`.
fn run_case(
    shape: &DlofcShape,
    fractions: AccidentFractions,
) -> (f64, Vec<(String, f64)>, String, usize, Vec<String>) {
    let (inventory, _) = htr10_inventory();
    let transient = shape
        .transient(SAMPLES, N_RADIAL, N_AXIAL)
        .expect("the DLOFC shape has enough samples");
    let plant = htr10::plant_parameters(htr10_geometry(), fractions);
    let out = accident_release(&inventory, &transient, &plant).expect("the release chain runs");
    let per_nuclide = released_bq(&out.source_term);
    let total: f64 = per_nuclide.iter().map(|(_, bq)| bq).sum();
    let mut caveats = Vec::new();
    if out.caveats.first_sample_forced_fully_vented {
        caveats.push("first sample forced fully vented (upstream frac[0]=1)".to_string());
    }
    if out.caveats.negative_atom_count_seen {
        caveats.push("NEGATIVE atom count seen; total under-stated".to_string());
    }
    if out.caveats.diffusion_coefficient_clamped {
        caveats.push("diffusion coefficient CLAMPED outside 700-2400 C".to_string());
    }
    if out.caveats.venting_mask_was_gappy {
        caveats.push("venting mask was gappy".to_string());
    }
    (
        total,
        per_nuclide,
        caveats.join("; "),
        out.source_term.windows.len(),
        out.screened_out,
    )
}

fn main() {
    let t_b = htr10::stand_in_irradiation_temperature();
    let shape = DlofcShape::htr_module_jrc();

    println!("================================================================");
    println!(" HTR-10 DLOFC: PANAMA-I fuel failure -> TRISO-ATOPS release");
    println!(" boon-lay fuel_failure + triso_atops_fork, orchestrated by sembawang");
    println!(" RESEARCH, EDUCATION AND V&V ONLY. NOT a source term for HTR-10.");
    println!(" No dose computed. See sembawang::htr10 and RESPONSIBLE_USE.md.");
    println!("================================================================\n");

    // ---------------------------------------------------------------- inputs
    println!("-- INPUTS, each classified -------------------------------------");
    let g = htr10_geometry();
    println!(
        "  HTR-10's own   kernel radius        {:.1} um        IAEA-TECDOC-1382 pt 2 T4-17",
        g.kernel_radius.get::<uom::si::length::micrometer>()
    );
    println!(
        "  HTR-10's own   SiC thickness        {:.1} um        same",
        g.sic_thickness.get::<uom::si::length::micrometer>()
    );
    println!(
        "  HTR-10's own   graphite shell       {:.2} mm       same (60 mm pebble, 50 mm fuelled)",
        g.graphite_thickness.get::<uom::si::length::millimeter>()
    );
    println!("  derived        F_b = 0.0851 FIMA, t_B = 1080 FPD    from 80 000 MWd/t, 10 MW");
    println!("  STAND-IN       sigma_oo/m_oo = 834 MPa / 8.02       EO 1607, HTR-Module's own");
    println!("  STAND-IN       Gamma = 1.4e25 /m2 EDN               HTR-Module (Table 2)");
    println!(
        "  STAND-IN       T_B = {STAND_IN_IRRADIATION_CELSIUS:.0} C                        \
         HTR-Module average (Table 2)"
    );
    println!(
        "  STAND-IN       peak {DLOFC_PEAK_CELSIUS:.0} C at {DLOFC_TIME_TO_PEAK_HOURS:.0} h     \
                HTR-Module DLOFC (JRC EUR 28712 EN 9.9.2)"
    );
    println!(
        "  ESTIMATE       cooldown tau = {:.0} h                  NOT CITED - see #296",
        shape.cooldown_time_constant.get::<hour>()
    );
    println!(
        "  ESTIMATE       late-time T = {:.0} C                   NOT CITED - see #296",
        shape.late_time.get::<degree_celsius>()
    );
    println!("  NP-MHTGR       f_hm=1e-4 f_sic=1e-4 f_inc=2.3e-5 f_inc_sic=3.6e-5  NOT HTR-10");
    println!("  published      22-nuclide equilibrium core inventory  Liu & Cao 2002 Table 1\n");

    // ------------------------------------------------- the inventory, honestly
    let (inventory, dropped) = htr10_inventory();
    println!("-- INVENTORY ---------------------------------------------------");
    println!(
        "  {} of 22 published nuclides are modelled by TRISO-ATOPS.",
        inventory.nuclides.len()
    );
    if !dropped.is_empty() {
        println!("  Dropped, NOT in TRISO-ATOPS's 84-nuclide table:");
        for (n, bq) in &dropped {
            println!("    {n:<10} {bq:9.2e} Bq  -- not modelled, not zero");
        }
    }
    println!();

    // ------------------------------------------------- PANAMA over the history
    let panama = htr10::panama_over_transient(&shape, t_b, SAMPLES);
    println!("-- PANAMA-I OVER THE DLOFC HISTORY -----------------------------");
    println!(
        "  phi_1 at end of irradiation (t=0): {:.3e}   (PANAMA sets phi_1(0) to this, not 0)",
        panama.end_of_irradiation_phi_1
    );
    println!("     t [h]     T [C]       phi_1       phi_2       f_inc      f_inc - f_inc(0)");
    let f0 = panama.in_service[0];
    for i in (0..SAMPLES).step_by(20) {
        println!(
            "  {:8.1}  {:8.1}   {:9.3e}   {:9.3e}   {:9.3e}   {:9.3e}",
            panama.times[i].get::<hour>(),
            panama.temperature_celsius[i],
            panama.phi_1[i],
            panama.phi_2[i],
            panama.in_service[i],
            (panama.in_service[i] - f0).max(0.0),
        );
    }
    println!(
        "  f_inc_acc at the peak ({DLOFC_TIME_TO_PEAK_HOURS:.0} h)  : {:.4e}",
        panama.accident_increment_at_peak
    );
    println!(
        "  f_inc_acc at {REPORTING_WINDOW_HOURS:.0} h              : {:.4e}",
        panama.accident_increment_final
    );
    println!(
        "  GRID: {SAMPLES} samples is a 1 h step. The convergence study in sembawang::htr10\n           puts this 0.39 % BELOW the extrapolated limit 1.071920e-7 (second order in dt from\n           the 2 h step down; a 4 h step is 16 % low). Nothing here is quoted finer than that."
    );
    println!(
        "  phi_2 never exceeds {:.2e} here, so SiC decomposition contributes nothing at\n  \
         this peak; it takes over above ~2000 C (HTA-IB-03/90 page -508-).\n",
        panama.phi_2.iter().copied().fold(0.0_f64, f64::max)
    );

    // --------------------------------------------- isothermal reference grid
    println!("-- PANAMA-I, ISOTHERMAL 200 h HOLDS (reference grid) ----------");
    println!("  Comparable with HTA-IB-03/90's own statement that HTR-Module depressurised");
    println!("  stays below 1e-6 at 200 h, and with boon_lay::fuel_failure::htr10's tables.");
    println!("     T [C]       phi_1       phi_2       f_inc");
    for c in ISOTHERMAL_CELSIUS {
        let (p1, p2, f) = htr10::isothermal_failure(
            t_b,
            ThermodynamicTemperature::new::<degree_celsius>(c),
            Time::new::<hour>(REPORTING_WINDOW_HOURS),
            200,
        );
        println!("  {c:8.0}   {p1:9.3e}   {p2:9.3e}   {f:9.3e}");
    }
    println!();

    println!("-- THE ONE CHECK THE LITERATURE SUPPORTS -----------------------");
    println!("  HTA-IB-03/90 page -504- states that HTR-Module DEPRESSURISED stays below");
    println!("  1e-6 at 200 h. That statement is about a TRANSIENT, not a hold.");
    println!(
        "    this transient, f_inc at 200 h : {:.3e}   {}",
        panama.in_service[SAMPLES - 1],
        if panama.in_service[SAMPLES - 1] < 1.0e-6 {
            "CONSISTENT with < 1e-6"
        } else {
            "EXCEEDS 1e-6 -- disagrees"
        }
    );
    let (flat_1600, _, _) = htr10::isothermal_failure(
        t_b,
        ThermodynamicTemperature::new::<degree_celsius>(1600.0),
        Time::new::<hour>(REPORTING_WINDOW_HOURS),
        200,
    );
    println!("    flat 200 h at 1600 C          : {flat_1600:.3e}   31x ABOVE 1e-6");
    println!("  The flat hold exceeding the bound is not a disagreement: it holds the fuel");
    println!("  at its accident limit for the whole window, which no transient does. The");
    println!("  transient arm is the one the report's sentence is about, and it agrees.");
    println!("  This is the nearest thing to a check available, and it is NOT a validation:");
    println!("  it is HTR-Module's bound, checked against HTR-Module's transient, with");
    println!("  HTR-10's particle geometry and burnup. Digitising the report's Fig. 10");
    println!("  would make it one (GitHub #296).\n");

    // ------------------------------------------------------------- the seam
    let base = htr10::np_mhtgr_normal_operation_fractions();
    println!("-- THE SEAM: where PANAMA's number sits against the others -----");
    println!("  f_hm      (as-manufactured heavy metal) : {:.3e}", base.heavy_metal);
    println!("  f_inc     (normal-operation in-service) : {:.3e}", base.incremental);
    println!(
        "  f_inc_acc (PANAMA, this transient, 200 h): {:.3e}",
        panama.accident_increment_final
    );
    let volatile_without = base.heavy_metal + base.incremental;
    let volatile_with = volatile_without + panama.accident_increment_final;
    println!(
        "  Volatile path is (f_hm + f_inc + f_inc_acc): {volatile_without:.3e} -> \
         {volatile_with:.3e}, a factor {:.3}",
        volatile_with / volatile_without
    );
    println!(
        "  So at a {DLOFC_PEAK_CELSIUS:.0} C peak the accident-added failure is {:.1} % of the\n  \
         as-manufactured population, and the volatile source term is still dominated by\n  \
         as-manufactured defects rather than by accident failure.\n",
        100.0 * panama.accident_increment_final / volatile_without
    );

    // ------------------------------------------- the release, and the ablation
    println!("-- RELEASE: PANAMA ON vs ABLATED (the seam's own control) ------");
    let with_panama = htr10::with_panama_accident_increment(base, panama.accident_increment_final);
    let ablated = htr10::with_panama_accident_increment(base, 0.0);
    let (total_on, per_on, caveats_on, windows, screened) = run_case(&shape, with_panama);
    let (total_off, per_off, _, _, _) = run_case(&shape, ablated);

    println!("  {windows} release windows; venting spans only the HEATING leg, because");
    println!("  TRISO-ATOPS's coolant_release selects dT/dt >= 0. The cooldown leg vents");
    println!("  nothing, so the two ESTIMATEd cooldown parameters cannot move these numbers.");
    if !screened.is_empty() {
        println!(
            "  Screened out by the half-life test (t_half < 4 % of {REPORTING_WINDOW_HOURS:.0} h): {}",
            screened.join(", ")
        );
    }
    if !caveats_on.is_empty() {
        println!("  CAVEATS: {caveats_on}");
    }
    println!();
    println!("  nuclide     inventory [Bq]   released [Bq]   released/inv    PANAMA off [Bq]   on/off");
    let inv_lookup = |name: &str| -> f64 {
        htr10_equilibrium_core()
            .into_iter()
            .find(|e| e.nuclide == name)
            .map(|e| e.activity.get::<becquerel>())
            .unwrap_or(f64::NAN)
    };
    for (name, bq_on) in &per_on {
        let bq_off = per_off
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| *v)
            .unwrap_or(f64::NAN);
        let inv = inv_lookup(name);
        println!(
            "  {name:<10} {inv:14.3e}   {bq_on:13.3e}   {:12.3e}   {bq_off:15.3e}   {:6.4}",
            bq_on / inv,
            bq_on / bq_off,
        );
    }
    println!(
        "\n  TOTAL over the modelled nuclides: {total_on:.4e} Bq with PANAMA, \
         {total_off:.4e} Bq ablated"
    );
    println!(
        "  The ablation is the control: if the two columns were equal the seam would be\n  \
         doing nothing and none of this would mean anything. Ratio {:.4}.",
        total_on / total_off
    );
    println!("  Two structural points the table shows, and neither is a bug:");
    println!("   * released/inv is identical across Kr, Xe and I. The Booth release fraction");
    println!("     depends only on int(D dt)/r^2, the atoms<->curies conversion cancels lambda,");
    println!("     and TRISO-ATOPS gives noble gases and halogens the same kernel correlation,");
    println!("     so the RELEASED FRACTION is a property of the transport group, not of the");
    println!("     nuclide. The nuclides differ only through their inventories.");
    println!("   * Ag-110m's on/off ratio is exactly 1. TRISO-ATOPS's silver branch sets the");
    println!("     scaling fraction to 1 (the breakthrough model already embeds the failed");
    println!("     population), so PANAMA's failure fraction has NO effect on silver at all.");
    println!("   * Ag-110m's windows also carry 13 floored negative first differences, so its");
    println!("     total is OVER-stated by a factor 1.0011 (measured). Every other nuclide in");
    println!("     this run has zero negative windows. See Caveats::negative_atom_count_seen.");
    println!();

    // ------------------------------------------------- peak sensitivity
    println!("-- SENSITIVITY 1: the peak, over the report's own three values --");
    // `total_on` above IS the 1500 C case, so it is the reference and no column
    // can be NaN for want of having reached it yet.
    let reference = total_on;
    println!(
        "               (the last two columns split the growth: how much is the FAILURE\n         \x20               model, and how much is Arrhenius diffusion)"
    );
    println!("     peak [C]   f_inc_acc(200 h)   released [Bq]   vs 1500 C   from f_inc_acc   from D");
    for c in [
        DLOFC_TABLE_44_NOMINAL_CELSIUS,
        DLOFC_PEAK_CELSIUS,
        DLOFC_TABLE_44_MAXIMUM_CELSIUS,
        1800.0,
    ] {
        let s = DlofcShape::with_peak(ThermodynamicTemperature::new::<degree_celsius>(c));
        let p = htr10::panama_over_transient(&s, t_b, SAMPLES);
        let f = htr10::with_panama_accident_increment(base, p.accident_increment_final);
        let (total, _, _, _, _) = run_case(&s, f);
        // The volatile path scales linearly in (f_hm + f_inc + f_inc_acc), so the
        // share of the growth attributable to the failure model is exactly that
        // ratio; whatever is left is the release fraction, i.e. diffusion.
        let from_failure = (base.heavy_metal + base.incremental + p.accident_increment_final)
            / (base.heavy_metal + base.incremental + panama.accident_increment_final);
        println!(
            "  {c:10.0}   {:16.3e}   {total:13.3e}   {:8.3}   {from_failure:13.3}   {:6.3}",
            p.accident_increment_final,
            total / reference,
            (total / reference) / from_failure,
        );
    }
    println!("  The growth is almost entirely DIFFUSION, not fuel failure: going 1500 -> 1800 C");
    println!("  multiplies the release by ~15 while the failure model contributes ~1.2 of it.");
    println!("  So for this transient the source term's temperature sensitivity is the");
    println!("  Arrhenius release fraction's, and the seam changes the answer by <0.1 %.");
    println!(
        "  1800 C is BEYOND the 1600 C design limit and is included only to show where\n  \
         the accident mechanism starts to dominate. It is not an HTR-10 condition.\n"
    );

    // ------------------------------------------------- f_hm sensitivity
    println!("-- SENSITIVITY 2: f_hm, the German-lineage measured bracket ----");
    println!("  HTR-10's fuel is German-lineage. The measured free-uranium record");
    println!("  (Kugeler et al. 2017 Table 7, via boon_lay ...::qualification) is");
    println!("  7.8e-6 to 50.7e-6, i.e. 2 to 13 times BETTER than NP-MHTGR's 1e-4.");
    println!("     f_hm        released [Bq]   vs NP-MHTGR");
    let german =
        boon_lay::fuel_failure::htr10::qualification::FREE_URANIUM_FRACTIONS;
    let best = german.iter().copied().fold(f64::INFINITY, f64::min);
    let worst = german.iter().copied().fold(0.0_f64, f64::max);
    for f_hm in [base.heavy_metal, worst, best] {
        let fractions = AccidentFractions {
            heavy_metal: f_hm,
            ..with_panama
        };
        let (total, _, _, _, _) = run_case(&shape, fractions);
        println!("  {f_hm:9.3e}   {total:13.3e}   {:11.4}", total / total_on);
    }
    println!(
        "\n  Neither German value is HTR-10's own measurement. They bracket what a\n  \
         German-lineage line achieved, which is the nearest published thing there is.\n"
    );

    println!("================================================================");
    println!(" NOT VALIDATED. No measured HTR-10 failure fraction, free-uranium");
    println!(" fraction or release fraction exists in this workspace's literature, so");
    println!(" no code-to-data comparison was possible and none is claimed. PANAMA-I");
    println!(" was validated on German TRISO over 1600-2500 C; every number above at");
    println!(" or below 1600 C is an EXTRAPOLATION. GitHub #296, #295.");
    println!("================================================================");
}
