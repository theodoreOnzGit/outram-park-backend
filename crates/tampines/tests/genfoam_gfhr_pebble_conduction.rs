//! Code-to-code consistency check against the **GeN-Foam `3D_gFHR` tutorial's
//! own reference fuel temperature** (bead `op-2df1`).
//!
//! # Why this test is shaped the way it is
//!
//! Upstream GeN-Foam's `Tutorials/reactorCases/3D_gFHR` reports, in its
//! `Alltest` regression script, the peak fuel (TRISO kernel) temperature over
//! the pebble bed:
//!
//! | quantity | upstream value |
//! |---|---|
//! | `expectedTfmaxAvg` | 981.808 K |
//! | `expectedTfmaxMin` | 900.664 K |
//! | `expectedTfmaxMax` | 1062.87 K |
//!
//! Reproducing those numbers outright needs GeN-Foam's `nuclearSteadyStatePebble`
//! power model coupled to a porous-medium thermal-hydraulic solve. **The
//! OUTRAM PARK GeN-Foam port has neither** — its structure models are
//! `fixedPower` and `fixedTemperature` only, as
//! `genfoam::thermal_hydraulics::structure::power_model` says in as many words.
//! So this test does not claim to reproduce them.
//!
//! What it does instead is answer a narrower question that *is* answerable, and
//! is worth answering: **is upstream's reported fuel temperature consistent with
//! upstream's own stated pebble geometry, conductivities and power, under an
//! independent implementation of the same conduction physics?** If it is not,
//! either the case data or one of the two codes is wrong, and that is worth
//! knowing before anyone ports the pebble model.
//!
//! # SUPERSEDED for the full chain — read this first
//!
//! Upstream's `nuclearSteadyStatePebble` has since been ported properly, to
//! `outram_foam_appbuilder_lib::genfoam::thermal_hydraulics::structure::nuclear_steady_state_pebble`,
//! and **its numbers are the ones to cite**. Two things this file gets
//! differently from upstream, found by doing that port:
//!
//! - **The annulus term.** This file takes the fuelled annulus' *outer face to
//!   its centre*; upstream takes it *to its volume average*, because the volume
//!   average is what the dispersed particles actually sit at. That is the larger
//!   of the two differences.
//! - **The matrix conductivity.** This file uses the `k_matrix = 29` that
//!   upstream's `lumped_structure.py` hardcodes; upstream's solver computes it
//!   from a **Maxwell** mixture rule over the TRISO packing, giving 29.587 —
//!   which is a pleasing independent confirmation of the 29, but is not the same
//!   input.
//!
//! Together those put this file's total rise at 71.56 K against the ported
//! model's 61.67 K. **The 61.67 K is upstream's model; the 71.56 K is this
//! file's own formulation of the same physics.** What remains valid and exact
//! here is the shell-conductance comparison in the second test, which is a
//! like-for-like check against upstream's own closed form.
//!
//! The conduction stack is assembled from this workspace's own closed-form
//! helpers in [`tampines::pebble_bed::triso`], which were written for the HTR-10
//! pebble and know nothing of gFHR.
//!
//! # The pebble
//!
//! A gFHR pebble is **not** shaped like an HTR-10 pebble: it has an *unfuelled
//! central graphite core*, an annular fuelled matrix around it, and an unfuelled
//! outer shell. That is why [`tampines::pebble_bed::Pebble`] — whose two-node
//! profile assumes a fuelled centre — cannot be used directly here, and why the
//! annular-matrix term below is written out rather than called.
//!
//! ```text
//!   r = 0 ........ 1.38 cm ........ 1.80 cm ........ 2.00 cm
//!   | graphite core | fuelled matrix | graphite shell |
//!     (unheated)      (9022 TRISO)     (unheated)
//! ```
//!
//! # Source of every constant
//!
//! `Tutorials/reactorCases/3D_gFHR/rootCase/constant/fluidRegion/phaseProperties`
//! (the `nuclearSteadyStatePebble` block) at upstream commit `652b3da`, with the
//! reactor power and pebble count from the case's own `lumped_structure.py`.
//! Conductivities are the constant (temperature-independent) values upstream
//! states there, attributed by upstream to April Novak's thesis. Nothing is
//! taken from this workspace's own HTR-10 correlations — that would be posing a
//! different problem.
//!
//! # Results
//!
//! See the test's doc comment.

use std::f64::consts::PI;

use tampines::pebble_bed::triso::{
    solid_sphere_centre_temperature_rise, spherical_shell_temperature_rise,
};
use uom::si::f64::{Length, Power, ThermalConductivity};
use uom::si::length::meter;
use uom::si::power::watt;
use uom::si::temperature_interval::kelvin as kelvin_interval;
use uom::si::thermal_conductivity::watt_per_meter_kelvin;

// ── upstream's gFHR case constants ──────────────────────────────────────────

/// Total reactor thermal power, W (`lumped_structure.py`, `P`).
const REACTOR_POWER: f64 = 280.0e6;
/// Number of pebbles in the core (`lumped_structure.py`, `N_p`).
const PEBBLES: f64 = 250_190.0;
/// TRISO particles per pebble (`phaseProperties`, `nTRISO`).
const TRISO_PER_PEBBLE: f64 = 9022.0;

/// Unfuelled central graphite core radius, m (`pebbleCoreRadius`).
const R_CORE: f64 = 1.38e-2;
/// Outer radius of the fuelled matrix annulus, m (`pebbleMatrixRadius`).
const R_MATRIX: f64 = 1.8e-2;
/// Pebble outer radius, m (`pebbleShellRadius`).
const R_SHELL: f64 = 2.0e-2;

/// TRISO layer outer radii, m (`trisoFuelRadius` … `trisoOuterPyCRadius`).
const R_FUEL: f64 = 212.5e-6;
const R_BUFFER: f64 = 312.5e-6;
const R_IPYC: f64 = 352.5e-6;
const R_SIC: f64 = 387.5e-6;
const R_OPYC: f64 = 427.5e-6;

/// Conductivities, W/(m K) (`*KCoeffs`, constant term; all higher coefficients
/// are zero in this case, so they are genuinely constant).
const K_FUEL: f64 = 3.3073;
const K_BUFFER: f64 = 0.50;
const K_PYC: f64 = 4.00;
const K_SIC: f64 = 90.3;
const K_GRAPHITE_SHELL: f64 = 41.68;
/// Effective conductivity of the TRISO-bearing matrix, W/(m K).
///
/// `lumped_structure.py` uses `k_matrix = 29`, noting it is the graphite value
/// 41.68 degraded by the dispersed particles ("taking from python effective
/// calculation"). Upstream's own number is used rather than one computed here,
/// so the comparison is against upstream's model and not against this
/// workspace's dispersion correlations.
const K_MATRIX: f64 = 29.0;

/// Coolant inlet temperature, K — upstream's own `0/fluidRegion/T` boundary
/// condition (`bottom`, `fixedValue uniform 823.15`).
const T_COOLANT_INLET: f64 = 823.15;

/// Upstream's reference peak fuel temperatures, K (`Alltest`).
const TFMAX_AVG: f64 = 981.808;
const TFMAX_MIN: f64 = 900.664;
const TFMAX_MAX: f64 = 1062.87;

fn watts(x: f64) -> Power {
    Power::new::<watt>(x)
}
fn metres(x: f64) -> Length {
    Length::new::<meter>(x)
}
fn conductivity(x: f64) -> ThermalConductivity {
    ThermalConductivity::new::<watt_per_meter_kelvin>(x)
}

/// Temperature rise from the outer surface of a **uniformly heated spherical
/// annulus** to its inner (adiabatic) face, K.
///
/// The gFHR pebble's fuelled zone is an annulus `R_c < r < R_m` around an
/// unheated graphite core. Symmetry puts zero flux at `r = 0`, so no power
/// crosses into the core and the core is isothermal at the annulus' inner face
/// temperature. The power crossing radius `r` is then
/// `Q(r) = q''' (4/3) pi (r^3 - R_c^3)`, and integrating
/// `dT/dr = -Q(r) / (4 pi r^2 k)` from `R_m` inward gives
///
/// ```text
///   T(R_c) - T(R_m) = q'''/(3k) [ R_m^2/2 + R_c^3/R_m - (3/2) R_c^2 ]
/// ```
///
/// with `q''' = Q / ((4/3) pi (R_m^3 - R_c^3))`.
///
/// This is written out rather than called from
/// [`tampines::pebble_bed::pebble`] because that module's `Pebble` models a
/// *fuelled centre* with an unfuelled shell — the HTR-10 topology — and has no
/// annular-fuel form. Deriving it here keeps the gFHR geometry honest instead of
/// bending it to fit an existing struct.
fn heated_annulus_rise(power_w: f64) -> f64 {
    let volume = 4.0 / 3.0 * PI * (R_MATRIX.powi(3) - R_CORE.powi(3));
    let q_triple = power_w / volume;
    q_triple / (3.0 * K_MATRIX)
        * (R_MATRIX.powi(2) / 2.0 + R_CORE.powi(3) / R_MATRIX - 1.5 * R_CORE.powi(2))
}

/// **V&V — gFHR: upstream's reference peak fuel temperature is consistent with
/// upstream's own pebble data.**
///
/// ## Methodology
///
/// At the core-average pebble power `Q_p = 280 MW / 250 190 = 1119.1 W`, the
/// surface-to-peak-kernel temperature rise is assembled from four exact
/// closed-form conduction terms, in order from the pebble surface inward:
///
/// 1. **Unfuelled graphite shell**, `1.80 cm -> 2.00 cm`, carrying the whole
///    pebble power — [`spherical_shell_temperature_rise`].
/// 2. **Fuelled annular matrix**, `1.38 cm -> 1.80 cm`, uniformly heated with an
///    adiabatic inner face — [`heated_annulus_rise`], derived in its own doc.
/// 3. **The four TRISO coatings** of one particle carrying `Q_p / 9022`, each an
///    unheated shell — [`spherical_shell_temperature_rise`].
/// 4. **The UO2 kernel**, uniformly heated to its centre —
///    [`solid_sphere_centre_temperature_rise`].
///
/// Terms 3 and 4 are evaluated for a particle at the pebble's hottest point,
/// which is the bounding position, exactly as upstream's own model reports a
/// *maximum* fuel temperature.
///
/// ## Pass criterion
///
/// The computed internal rise must be **strictly positive and strictly less than
/// the total rise upstream reports from its own coolant inlet to its own average
/// peak fuel temperature**, `Tfmax_avg - T_inlet = 981.808 - 823.15 = 158.66 K`.
/// That is a hard physical bound, not a fitted tolerance: the pebble's internal
/// conduction rise is one component of that total, the remainder being the
/// coolant's own heat-up along the channel plus the convective film drop. A
/// geometry, conductivity or power wrong by an order of magnitude — the errors
/// this kind of reconstruction actually makes — breaks it immediately.
///
/// The *implied* pebble surface temperature, `Tfmax - dT`, is reported for each
/// of upstream's three statistics and must exceed the coolant inlet temperature,
/// since a pebble surface cannot be colder than the coldest coolant in the core.
///
/// ## Results (measured 2026-09-15)
///
/// Pebble power 1119.1 W; per-particle power 0.12405 W.
///
/// | term | rise (K) |
/// |---|---|
/// | graphite shell, 1.80 -> 2.00 cm | 11.87 |
/// | fuelled annulus, 1.38 -> 1.80 cm | 21.42 |
/// | TRISO coatings (buffer + IPyC + SiC + OPyC) | 31.25 |
/// | UO2 kernel centre | 7.02 |
/// | **total, surface -> peak kernel** | **71.56** |
///
/// That is **45.1 %** of upstream's 158.66 K inlet-to-peak-fuel rise, leaving
/// 87.1 K for coolant heat-up plus film drop.
///
/// Implied pebble surface temperatures, `Tfmax - 71.56 K`:
///
/// | upstream statistic | implied surface (K) |
/// |---|---|
/// | `Tfmax_min` 900.664 | 829.1 |
/// | `Tfmax_avg` 981.808 | **910.2** |
/// | `Tfmax_max` 1062.870 | 991.3 |
///
/// **Read the middle row, and treat the other two as bounds with a known bias.**
/// The rise above is computed at the *core-average* pebble power, so it pairs
/// properly only with the *average* `Tfmax`. The hottest pebble carries more
/// than average power, so its true internal rise exceeds 71.56 K and 991.3 K
/// *overstates* its surface temperature; the coldest carries less, so 829.1 K
/// *understates* its surface temperature — which is the direction that keeps the
/// assertion below meaningful rather than lucky.
///
/// The average row is the informative one: **910.2 K**, sitting between the
/// 823.15 K inlet and the roughly 923 K outlet a gFHR runs at, and nearer the
/// outlet — where a bed-averaged pebble surface temperature should sit, since
/// most of the core lies downstream of the inlet.
///
/// **Interpretation.** Upstream's reported fuel temperature is reconstructible
/// from upstream's own case data by an independent implementation of the same
/// conduction physics, to the accuracy this check can resolve. It does **not**
/// verify the coupled solve, the porous-medium thermal-hydraulics, the axial
/// power shape or the convective closure — all of which live in the 87 K this
/// test does not account for. It is a consistency check on the pebble-internal
/// conduction only, and it is the strongest statement available until the
/// `nuclearSteadyStatePebble` model is ported (tracked under `op-2df1`).
#[test]
fn gfhr_reference_fuel_temperature_is_consistent_with_upstream_pebble_data() {
    let q_pebble = REACTOR_POWER / PEBBLES;
    let q_particle = q_pebble / TRISO_PER_PEBBLE;

    let shell = spherical_shell_temperature_rise(
        watts(q_pebble),
        metres(R_MATRIX),
        metres(R_SHELL),
        conductivity(K_GRAPHITE_SHELL),
    )
    .get::<kelvin_interval>();

    let annulus = heated_annulus_rise(q_pebble);

    let coating = |r_in: f64, r_out: f64, k: f64| {
        spherical_shell_temperature_rise(
            watts(q_particle),
            metres(r_in),
            metres(r_out),
            conductivity(k),
        )
        .get::<kelvin_interval>()
    };
    let coatings = coating(R_FUEL, R_BUFFER, K_BUFFER)
        + coating(R_BUFFER, R_IPYC, K_PYC)
        + coating(R_IPYC, R_SIC, K_SIC)
        + coating(R_SIC, R_OPYC, K_PYC);

    let kernel = solid_sphere_centre_temperature_rise(
        watts(q_particle),
        metres(R_FUEL),
        conductivity(K_FUEL),
    )
    .get::<kelvin_interval>();

    let total = shell + annulus + coatings + kernel;
    let upstream_total_rise = TFMAX_AVG - T_COOLANT_INLET;

    println!(
        "\n=== gFHR pebble conduction, from upstream's own case data ===\n\
         \tpebble power        {q_pebble:.1} W  ({REACTOR_POWER:.3e} W / {PEBBLES:.0} pebbles)\n\
         \tper-particle power  {q_particle:.5} W  (/{TRISO_PER_PEBBLE:.0} TRISO)\n\
         \t--\n\
         \tgraphite shell   1.80 -> 2.00 cm   {shell:7.2} K\n\
         \tfuelled annulus  1.38 -> 1.80 cm   {annulus:7.2} K\n\
         \tTRISO coatings                     {coatings:7.2} K\n\
         \tUO2 kernel centre                  {kernel:7.2} K\n\
         \t                        total      {total:7.2} K\n\
         \t--\n\
         \tupstream Tfmax_avg - T_inlet = {upstream_total_rise:.2} K; \
         this accounts for {:.1} %\n\
         \timplied pebble surface temperature:\n\
         \t\tat Tfmax_min {TFMAX_MIN:.3} K -> {:.1} K\n\
         \t\tat Tfmax_avg {TFMAX_AVG:.3} K -> {:.1} K\n\
         \t\tat Tfmax_max {TFMAX_MAX:.3} K -> {:.1} K\n\
         \t(coolant inlet {T_COOLANT_INLET} K)",
        100.0 * total / upstream_total_rise,
        TFMAX_MIN - total,
        TFMAX_AVG - total,
        TFMAX_MAX - total,
    );

    assert!(
        total > 0.0,
        "the conduction stack produced a non-positive rise"
    );
    assert!(
        total < upstream_total_rise,
        "the pebble-internal rise ({total:.2} K) exceeds upstream's entire \
         inlet-to-peak-fuel rise ({upstream_total_rise:.2} K), which is \
         impossible — the geometry, a conductivity or the power is wrong"
    );
    // The coldest pebble carries less than average power, so its true internal
    // rise is below `total` and its true surface temperature is therefore ABOVE
    // `TFMAX_MIN - total`. The assertion is consequently conservative: if even
    // this under-estimate clears the inlet temperature, the real one does too.
    assert!(
        TFMAX_MIN - total > T_COOLANT_INLET,
        "the implied pebble surface temperature at upstream's MINIMUM peak fuel \
         temperature is {:.1} K, below the {T_COOLANT_INLET} K coolant inlet — a \
         pebble surface cannot be colder than the coldest coolant in the core",
        TFMAX_MIN - total
    );
}

/// **V&V — the spherical-shell conductance agrees with upstream's own
/// `lumped_structure.py` closed form.**
///
/// ## Methodology
///
/// Upstream's gFHR tutorial ships `lumped_structure.py`, which derives the
/// lumped-network conductances for the same pebble. Its graphite-shell term is
///
/// ```text
///   H_shell = (1/(4 pi k_shell) * (1/R_m - 1/R_shell))**-1
/// ```
///
/// This workspace's [`spherical_shell_temperature_rise`] gives
/// `dT = Q/(4 pi k) (1/r_i - 1/r_o)`, so its implied conductance is `Q/dT`,
/// which is the same expression. This test evaluates both on the gFHR numbers
/// and requires agreement to 1e-12 relative — i.e. they are the same formula,
/// not merely close.
///
/// It is a small check, but it is the one place where the two codes state the
/// *same* derived quantity for the *same* geometry, so it is the only strictly
/// like-for-like comparison this case offers today.
///
/// ## Results (measured 2026-09-15)
///
/// `H_shell` = **94.277939 W/K** from both expressions; relative difference
/// **6.03e-16**, i.e. one unit in the last place — the two are the same formula,
/// differing only in floating-point association order.
#[test]
fn the_shell_conductance_matches_upstreams_lumped_structure_formula() {
    // Upstream's expression, transcribed from lumped_structure.py.
    let upstream_h =
        (1.0 / (4.0 * PI * K_GRAPHITE_SHELL) * (1.0 / R_MATRIX - 1.0 / R_SHELL)).recip();

    // This workspace's, as a conductance: H = Q / dT for any Q.
    let q = 1000.0;
    let dt = spherical_shell_temperature_rise(
        watts(q),
        metres(R_MATRIX),
        metres(R_SHELL),
        conductivity(K_GRAPHITE_SHELL),
    )
    .get::<kelvin_interval>();
    let ours_h = q / dt;

    let rel = (ours_h - upstream_h).abs() / upstream_h;
    println!(
        "shell conductance: upstream {upstream_h:.6} W/K, this workspace \
         {ours_h:.6} W/K, relative difference {rel:.3e}"
    );
    assert!(
        rel < 1e-12,
        "the shell conductance differs from upstream's closed form by {rel:.3e}"
    );
}
