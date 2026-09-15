//! V&V tests walking the **exact corners** of the IAPWS-IF97 validity
//! envelope through the `(T,p)` region router and the `(p,h)` flash family
//! (bead `op-cv1c`).
//!
//! # Methodology
//!
//! ## What is being checked
//!
//! IAPWS-IF97 is defined on `273.15 K <= T <= 1073.15 K` at
//! `0 < p <= 100 MPa` (Regions 1-4), extended to
//! `1073.15 K <= T <= 2273.15 K` at `0 < p <= 50 MPa` (Region 5). Every
//! bound is **closed** — the envelope corners are valid states, not
//! excluded ones. Until 2026-08-11 the Region-2 and Region-3 arms of
//! [`region_fwd_eqn_single_phase`] closed their pressure ceiling with a
//! half-open `..100e6`, so `p = 100 MPa` *exactly* matched no arm and fell
//! through to the router's `panic!` for every `T` in
//! `(623.15 K, 863.15 K) union (863.15 K, 1073.15 K]`. Because
//! `is_above_isotherm_t_1073_15` (the `(p,h)` validity check) evaluates
//! `h_tp_eqm_single_phase(1073.15 K, p)` internally, that also made *every*
//! `(p,h)` flash at exactly 100 MPa panic regardless of enthalpy. These
//! tests are the regression gate on that fix, and more generally on the
//! whole set of envelope corners, which no previous test covered.
//!
//! ## Reference used for the inclusivity of `p = 100 MPa`
//!
//! The IAPWS-published **backward-equation verification points already in
//! this crate** settle it without needing the release document: IAPWS
//! tabulates `T_3a(p,h)` and `v_3a(p,h)` at exactly `p = 100 MPa`,
//! `h = 2100 kJ/kg` with reference values `T = 733.6163014 K` and
//! `v = 1.676229776e-3 m^3/kg` (asserted in
//! `region_3_single_phase_plus_supercritical_steam/tests/region_3_backward_t_ph.rs::t3a_ph_test3`
//! and `.../region_3_backward_v_ph.rs::v3a_ph_test3`), plus `v_3b` at
//! `100 MPa, 2700 kJ/kg = 2.404234998e-3 m^3/kg`. Those states sit at
//! `T = 733.6 K` and `T = 842.0 K`, i.e. **above** 623.15 K, so the
//! standard itself publishes Region-3 states at exactly 100 MPa above the
//! Region-1 ceiling. Corroborating in-crate: `is_outside_pressure_range`
//! (the `(p,h)` pressure gate) rejects only `p > 100 MPa`, the Region-1
//! router arm already used `..=100e6`, and the Region-3 `v(p,T)` subregion
//! selector uses `(40.0e6..=100.0e6)`. Secondary prose source: the crate's
//! own `lib.rs` and `region_1` module docs ("up to 100 MPa"), themselves
//! following Kretzschmar & Wagner, *International Steam Tables* (2019),
//! the reference this crate's verification tables are taken from.
//!
//! ## Pass criteria
//!
//! 1. **Router** — at `p = 100 MPa` exactly, every temperature from
//!    273.15 K to 1073.15 K returns a region (no panic), and the region
//!    returned is the physically correct one. Region-5 corners
//!    (`T = 1073.15 K` and `T = 2273.15 K` at `p = 50 MPa` exactly) return
//!    `Region5`.
//! 2. **Properties at the corners** — `h`, `v`, `s`, `cp`, `w` are finite
//!    at all ten envelope-corner states, with `v > 0`, `cp > 0`, `w > 0`,
//!    and `h` strictly increasing along the 100 MPa isobar (a consequence
//!    of `cp > 0`). No published table exists at most of these corners, so
//!    for those this group asserts **no-panic plus physical
//!    plausibility**, not table agreement. Two exceptions carry real
//!    references: the `(273.15 K, 100 MPa)` corner (International Steam
//!    Tables 1000 bar / 0 degC row, `max_relative = 1e-4` set by the
//!    table's printed precision) and the whole `(p,h)` group below.
//! 3. **`(p,h)` at exactly 100 MPa** — `t_ph_eqm` and `v_ph_eqm` reproduce
//!    the IAPWS verification values above to `max_relative = 1e-8` (the
//!    tolerance used by the crate's other IF97 table tests), and the
//!    forward/backward round trip `h(T_ref, 100 MPa)` returns 2100 kJ/kg
//!    to `max_relative = 5e-5` (loose because the backward `T(p,h)` and
//!    `v(p,T)` equations are fits, not inverses, of the Region-3 Helmholtz
//!    equation).
//! 4. **Region-1/Region-3 consistency at the corner whose region changed**
//!    — fixing the ceiling moved `(623.15 K, 100 MPa)` from Region 1 to
//!    Region 3 (it was the one point on that isotherm the half-open range
//!    spilled into the Region-1 arm; `(623.15 K, 99.999 MPa)` was already
//!    Region 3). The two regions must agree there to better than 1e-4
//!    relative for that to be harmless.
//!
//! # Results (2026-08-11, `cargo test --release -p tampines-steam-tables
//! --lib envelope_corner -- --nocapture`, crate at v0.2.5)
//!
//! All 4 tests pass (`4 passed; 0 failed`). Measured values:
//!
//! **Router at `p = 100 MPa` exactly** (all previously panicked above
//! 623.15 K except 863.15 K itself):
//!
//! | `T` (K) | region |
//! |---|---|
//! | 273.15 | Region1 |
//! | 300 | Region1 |
//! | 500 | Region1 |
//! | 623.15 | Region3 |
//! | 623.1500001 | Region3 |
//! | 700 | Region3 |
//! | 863.14 | Region3 |
//! | 863.15 | Region2 |
//! | 863.16 | Region2 |
//! | 900 | Region2 |
//! | 1073.15 | Region2 |
//!
//! The 863.15 K row is not an anomaly: the measured B23 boundary pressure
//! is `p_B23(863.15 K) = 1.0000000000e8 Pa` to 11 significant figures and
//! lies a fraction of an ULP **above** `100e6`, so 100 MPa is on/below the
//! B23 line there and belongs to Region 2 — matching the documented
//! convention on `p_boundary_2_3` ("points ON this line belong to region
//! 2"). `p_B23(623.15 K) = 1.6529164253e7 Pa`. Region-5 corners:
//! `(1073.15 K, 50 MPa)`, `(1500 K, 50 MPa)` and `(2273.15 K, 50 MPa)` all
//! return `Region5`.
//!
//! **Properties at the envelope corners** (all finite, all positive where
//! required):
//!
//! | `T` (K) | `p` | region | `h` (J/kg) | `v` (m^3/kg) | `w` (m/s) |
//! |---|---|---|---|---|---|
//! | 273.15 | 100 MPa | Region1 | 9.538596866e4 | 9.566869391e-4 | 1.575525439e3 |
//! | 623.15 | 100 MPa | Region3 | 1.553917461e6 | 1.311736816e-3 | 1.233795138e3 |
//! | 700 | 100 MPa | Region3 | 1.924870816e6 | 1.534186428e-3 | 1.018468751e3 |
//! | 863.15 | 100 MPa | Region2 | 2.812942061e6 | 2.584718496e-3 | 7.696942357e2 |
//! | 1073.15 | 100 MPa | Region2 | 3.715188944e6 | 4.335507653e-3 | 8.209974979e2 |
//! | 273.15 | 611.657 Pa | Region1 | -4.158737331e1 | 1.000206977e-3 | 1.402282321e3 |
//! | 1073.15 | 611.657 Pa | Region5 | 4.160678512e6 | 8.097448015e2 | 7.853163699e2 |
//! | 2273.15 | 611.657 Pa | Region5 | 7.376980263e6 | 1.715206576e3 | 1.115891251e3 |
//! | 1073.15 | 50 MPa | Region5 | 3.926050140e6 | 9.073009644e-3 | 7.771972435e2 |
//! | 2273.15 | 50 MPa | Region5 | 7.365802234e6 | 2.146339965e-2 | 1.147254348e3 |
//!
//! (`s` and `cp` were measured too and are all finite and positive; e.g.
//! `cp(273.15 K, 100 MPa) = 3.905692521e3 J/(kg K)`,
//! `cp(1073.15 K, 100 MPa) = 3.576244769e3 J/(kg K)`. The compressed-liquid
//! corner `v = 9.566869391e-4 m^3/kg` is a measured density of
//! 1045.274017 kg/m^3, the
//! expected ~4.5 % compression of liquid water at 100 MPa and 0 degC;
//! `h = -41.59 J/kg` at the 611.657 Pa / 273.15 K corner is the correct
//! near-zero IF97 reference-state enthalpy.)
//!
//! **`(p,h)` at exactly 100 MPa** — the family that previously panicked
//! for every enthalpy:
//!
//! | quantity | measured | IAPWS reference | rel. dev. |
//! |---|---|---|---|
//! | `t_ph_eqm(100 MPa, 2100 kJ/kg)` | 733.6163014456 K | 733.6163014 K | 6.2e-14 |
//! | `v_ph_eqm(100 MPa, 2100 kJ/kg)` | 1.6762297762e-3 m^3/kg | 1.676229776e-3 | 1.2e-10 |
//! | `v_ph_eqm(100 MPa, 2700 kJ/kg)` | 2.4042349978e-3 m^3/kg | 2.404234998e-3 | 8.3e-11 |
//! | `h_tp_eqm_single_phase(733.6163014 K, 100 MPa)` | 2.0999317803e6 J/kg | 2.1e6 (round trip) | 3.25e-5 |
//!
//! `t_ph_eqm(100 MPa, 2700 kJ/kg) = 842.0460876333 K` (no published
//! reference for `T` at that state in this crate; recorded for the record).
//!
//! **Region-1/Region-3 consistency at `(623.15 K, 100 MPa)`** — the one
//! corner whose region label changed with the fix:
//! `h_tp_1 = 1.5539225034e6 J/kg` vs `h_tp_3 = 1.5539174612e6 J/kg`
//! (relative difference **3.245e-6**), `v_tp_1 = 1.3117600270e-3` vs
//! `v_tp_3 = 1.3117368165e-3 m^3/kg` (**1.769e-5**). Both are inside
//! IF97's own region-boundary consistency band, so the relabelling changes
//! the returned enthalpy at that single point by 3 ppm — and it makes the
//! point *consistent* with its neighbours, since `(623.15 K, 99.999 MPa)`
//! was already Region 3 before the fix while `(623.15 K, 100 MPa)` alone
//! fell through to the Region-1 arm.
//!
//! # Interpretation
//!
//! The router now accepts the whole closed IF97 envelope, the `(p,h)`
//! family works at its stated 100 MPa ceiling and reproduces the IAPWS
//! verification points there to 1e-8, and the only behavioural side effect
//! (the 623.15 K / 100 MPa region relabel) is a 3 ppm enthalpy change that
//! removes an inconsistency rather than creating one.

use approx::assert_relative_eq;
use uom::si::available_energy::{joule_per_kilogram, kilojoule_per_kilogram};
use uom::si::f64::*;
use uom::si::pressure::{megapascal, pascal};
use uom::si::specific_heat_capacity::joule_per_kilogram_kelvin;
use uom::si::specific_volume::cubic_meter_per_kilogram;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::velocity::meter_per_second;

use super::{
    FwdEqnRegion, cp_tp_eqm_single_phase, h_tp_eqm_single_phase, region_fwd_eqn_single_phase,
    s_tp_eqm_single_phase, v_tp_eqm_single_phase, w_tp_eqm_single_phase,
};
use crate::interfaces::functional_programming::ph_flash_eqm::{t_ph_eqm, v_ph_eqm};
use crate::region_3_single_phase_plus_supercritical_steam::p_boundary_2_3;

/// Test helper: temperature in kelvin.
fn t_k(t: f64) -> ThermodynamicTemperature {
    ThermodynamicTemperature::new::<kelvin>(t)
}

/// Methodology 1 / pass criterion 1: the `(T,p)` region router must return
/// a region — not panic — at the **exact** IF97 pressure ceilings
/// (100 MPa for Regions 1-3, 50 MPa for Region 5), and the region returned
/// must be the physically correct one. Every 100 MPa row above 623.15 K
/// except 863.15 K panicked before the `op-cv1c` fix.
///
/// Results (2026-08-11): see the module-doc table — all 11 temperatures at
/// 100 MPa and all three Region-5 corners return the expected region;
/// `p_B23(863.15 K) = 1.0000000000e8 Pa`,
/// `p_B23(623.15 K) = 1.6529164253e7 Pa`.
#[test]
fn region_router_accepts_the_exact_100_mpa_pressure_ceiling() {
    let p100 = Pressure::new::<megapascal>(100.0);

    // The B23 boundary pressures that decide the 623.15 K and 863.15 K
    // rows below — printed so a reader can see why 863.15 K is Region 2.
    let p_b23_863 = p_boundary_2_3(t_k(863.15)).get::<pascal>();
    let p_b23_623 = p_boundary_2_3(t_k(623.15)).get::<pascal>();
    println!("p_B23(863.15 K) = {:.10e} Pa", p_b23_863);
    println!("p_B23(623.15 K) = {:.10e} Pa", p_b23_623);
    assert!(
        p_b23_863 >= 100.0e6,
        "B23 line must reach 100 MPa at 863.15 K"
    );
    assert!(p_b23_623 < 100.0e6);

    let expected_at_100_mpa = [
        (273.15_f64, FwdEqnRegion::Region1),
        (300.0, FwdEqnRegion::Region1),
        (500.0, FwdEqnRegion::Region1),
        (623.15, FwdEqnRegion::Region3),
        (623.150_000_1, FwdEqnRegion::Region3),
        (700.0, FwdEqnRegion::Region3),
        (863.14, FwdEqnRegion::Region3),
        // p_B23(863.15 K) sits a hair above 100e6, so 100 MPa is on/below
        // the B23 line here and belongs to Region 2 by the convention
        // documented on `p_boundary_2_3`.
        (863.15, FwdEqnRegion::Region2),
        (863.16, FwdEqnRegion::Region2),
        (900.0, FwdEqnRegion::Region2),
        (1073.15, FwdEqnRegion::Region2),
    ];
    for (temp, expected) in expected_at_100_mpa {
        let region = region_fwd_eqn_single_phase(t_k(temp), p100);
        println!("T = {:>12} K, p = 100 MPa -> {:?}", temp, region);
        assert_eq!(
            region, expected,
            "wrong region at T = {temp} K, p = 100 MPa (exact IF97 ceiling)"
        );
    }

    // Region 5 corners: its own ceiling is 50 MPa, also inclusive.
    let p50 = Pressure::new::<megapascal>(50.0);
    for temp in [1073.15_f64, 1500.0, 2273.15] {
        let region = region_fwd_eqn_single_phase(t_k(temp), p50);
        println!("T = {:>12} K, p = 50 MPa -> {:?}", temp, region);
        assert_eq!(
            region,
            FwdEqnRegion::Region5,
            "wrong region at T = {temp} K, p = 50 MPa"
        );
    }

    // Low-pressure corner of the envelope, just above p_sat(273.15 K).
    let p_min = Pressure::new::<pascal>(611.657);
    assert_eq!(
        region_fwd_eqn_single_phase(t_k(273.15), p_min),
        FwdEqnRegion::Region1
    );
    assert_eq!(
        region_fwd_eqn_single_phase(t_k(1073.15), p_min),
        FwdEqnRegion::Region5
    );
    assert_eq!(
        region_fwd_eqn_single_phase(t_k(2273.15), p_min),
        FwdEqnRegion::Region5
    );
}

/// Methodology 2 / pass criterion 2: every `(T,p)` single-phase property
/// function must return a finite, physically plausible value at each
/// corner of the IF97 envelope — `T_min = 273.15 K`, `T_max = 1073.15 K`
/// (and the Region-5 `T_max = 2273.15 K`), `p_max = 100 MPa`
/// (50 MPa in Region 5) and a near-triple-point `p_min = 611.657 Pa` — and
/// the test must run to completion without panicking.
///
/// No published table covers most of these corners, so for those this test
/// asserts **no-panic plus physical plausibility** (finite values, `v > 0`,
/// `cp > 0`, `w > 0`, `h` strictly increasing along the 100 MPa isobar)
/// rather than agreement with published numbers. **One corner is an
/// exception and is checked against a reference**: `(273.15 K, 100 MPa)`
/// is the first row of the International Steam Tables 1000 bar table
/// carried by this crate (`v = 0.000956687 m^3/kg`, `h = 95.386 kJ/kg`,
/// `s = -0.008582 kJ/(kg K)`, `cp = 3.9057 kJ/(kg K)`, `w = 1575.5 m/s`),
/// asserted to `max_relative = 1e-4` — a tolerance set by the table's
/// printed precision, not by solver accuracy.
///
/// Results (2026-08-11): all ten corners finite; see the module-doc table.
/// The referenced corner matches: `v = 9.566869391e-4 m^3/kg` (ref
/// 9.56687e-4, 6.4e-8 relative), `h = 9.538596866e4 J/kg` (ref 9.5386e4,
/// 3.3e-7), `s = -8.582287093 J/(kg K)` (ref -8.582, 3.3e-5),
/// `cp = 3.905692521e3 J/(kg K)` (ref 3.9057e3, 1.9e-6),
/// `w = 1.575525439e3 m/s` (ref 1575.5, 1.6e-5); density
/// 1045.274017 kg/m^3. Other spot values:
/// `h(1073.15 K, 100 MPa) = 3.715188944e6 J/kg`;
/// `h(2273.15 K, 50 MPa) = 7.365802234e6 J/kg`.
#[test]
fn envelope_corner_states_return_finite_physical_properties() {
    let p100 = Pressure::new::<megapascal>(100.0);
    let p50 = Pressure::new::<megapascal>(50.0);
    let p_min = Pressure::new::<pascal>(611.657);

    let corners = [
        (273.15_f64, p100),
        (623.15, p100),
        (700.0, p100),
        (863.15, p100),
        (1073.15, p100),
        (273.15, p_min),
        (1073.15, p_min),
        (2273.15, p_min),
        (1073.15, p50),
        (2273.15, p50),
    ];

    for (temp, p) in corners {
        let t = t_k(temp);
        let h = h_tp_eqm_single_phase(t, p).get::<joule_per_kilogram>();
        let v = v_tp_eqm_single_phase(t, p).get::<cubic_meter_per_kilogram>();
        let s = s_tp_eqm_single_phase(t, p).get::<joule_per_kilogram_kelvin>();
        let cp = cp_tp_eqm_single_phase(t, p).get::<joule_per_kilogram_kelvin>();
        let w = w_tp_eqm_single_phase(t, p).get::<meter_per_second>();
        println!(
            "T={:>9} K p={:>14.4} Pa region={:?} h={:.9e} v={:.9e} s={:.9e} cp={:.9e} w={:.9e}",
            temp,
            p.get::<pascal>(),
            region_fwd_eqn_single_phase(t, p),
            h,
            v,
            s,
            cp,
            w
        );
        assert!(h.is_finite(), "h not finite at ({temp} K, {p:?})");
        assert!(s.is_finite(), "s not finite at ({temp} K, {p:?})");
        assert!(
            v.is_finite() && v > 0.0,
            "v not positive-finite at ({temp} K, {p:?})"
        );
        assert!(
            cp.is_finite() && cp > 0.0,
            "cp not positive-finite at ({temp} K, {p:?})"
        );
        assert!(
            w.is_finite() && w > 0.0,
            "w not positive-finite at ({temp} K, {p:?})"
        );
    }

    // The (273.15 K, 100 MPa) corner DOES have a published reference: the
    // International Steam Tables row `[1000.000 bar, 0.000 degC,
    // v = 0.000956687, h = 95.386, s = -0.008582, cp = 3.9057,
    // w = 1575.5]` (h, s, cp in kJ units), which sits commented out as the
    // first row of the 1000 bar table in
    // `tests_and_examples/pt_flash_steam_table/single_phase_table_240_bar_to_1000_bar.rs`.
    // Assert against it directly; 1e-4 relative is set by the table's own
    // printed precision (4-7 significant figures), not by solver accuracy.
    let t_cold = t_k(273.15);
    assert_relative_eq!(
        v_tp_eqm_single_phase(t_cold, p100).get::<cubic_meter_per_kilogram>(),
        0.000_956_687,
        max_relative = 1e-4
    );
    assert_relative_eq!(
        h_tp_eqm_single_phase(t_cold, p100).get::<joule_per_kilogram>(),
        95.386e3,
        max_relative = 1e-4
    );
    assert_relative_eq!(
        s_tp_eqm_single_phase(t_cold, p100).get::<joule_per_kilogram_kelvin>(),
        -0.008_582e3,
        max_relative = 1e-4
    );
    assert_relative_eq!(
        cp_tp_eqm_single_phase(t_cold, p100).get::<joule_per_kilogram_kelvin>(),
        3.9057e3,
        max_relative = 1e-4
    );
    assert_relative_eq!(
        w_tp_eqm_single_phase(t_cold, p100).get::<meter_per_second>(),
        1575.5,
        max_relative = 1e-4
    );
    let rho_cold = 1.0 / v_tp_eqm_single_phase(t_cold, p100).get::<cubic_meter_per_kilogram>();
    println!("rho(273.15 K, 100 MPa) = {:.6} kg/m^3", rho_cold);
    assert!(
        (1000.0..1200.0).contains(&rho_cold),
        "compressed-liquid density {rho_cold} kg/m^3 out of plausible range"
    );

    // cp > 0 implies h rises monotonically along an isobar.
    let mut h_prev = f64::NEG_INFINITY;
    for temp in [273.15_f64, 623.15, 700.0, 863.15, 1073.15] {
        let h = h_tp_eqm_single_phase(t_k(temp), p100).get::<joule_per_kilogram>();
        assert!(
            h > h_prev,
            "enthalpy not monotonic along the 100 MPa isobar at T = {temp} K"
        );
        h_prev = h;
    }
}

/// Methodology 3 / pass criterion 3: the `(p,h)` flash family must work at
/// exactly `p = 100 MPa` — the state that panicked for *every* enthalpy
/// before the `op-cv1c` fix, because the internal 1073.15 K-isotherm bound
/// helper evaluates `h_tp_eqm_single_phase(1073.15 K, p)` there — and must
/// reproduce the IAPWS backward-equation verification points at that
/// pressure to `max_relative = 1e-8`.
///
/// Reference values (IAPWS-IF97 supplementary backward equations, as
/// already asserted by this crate's Region-3 backward tests):
/// `T_3a(100 MPa, 2100 kJ/kg) = 733.6163014 K`,
/// `v_3a(100 MPa, 2100 kJ/kg) = 1.676229776e-3 m^3/kg`,
/// `v_3b(100 MPa, 2700 kJ/kg) = 2.404234998e-3 m^3/kg`.
///
/// Results (2026-08-11): `t_ph_eqm = 733.6163014456 K` (6.2e-14 relative),
/// `v_ph_eqm = 1.6762297762e-3 m^3/kg` (1.2e-10),
/// `v_ph_eqm(2700 kJ/kg) = 2.4042349978e-3 m^3/kg` (8.3e-11), and the
/// forward round trip `h_tp_eqm_single_phase(733.6163014 K, 100 MPa) =
/// 2.0999317803e6 J/kg` vs the 2.1e6 J/kg input (3.25e-5 relative — the
/// expected size of the backward-equation fit residual, hence the looser
/// 5e-5 criterion). `t_ph_eqm(100 MPa, 2700 kJ/kg) = 842.0460876333 K`.
#[test]
fn ph_flash_at_exactly_100_mpa_matches_iapws_verification_points() {
    let p100 = Pressure::new::<megapascal>(100.0);

    let h_3a = AvailableEnergy::new::<kilojoule_per_kilogram>(2100.0);
    let t_measured = t_ph_eqm(p100, h_3a).get::<kelvin>();
    let v_measured = v_ph_eqm(p100, h_3a).get::<cubic_meter_per_kilogram>();
    println!("t_ph_eqm(100 MPa, 2100 kJ/kg) = {:.10} K", t_measured);
    println!("v_ph_eqm(100 MPa, 2100 kJ/kg) = {:.10e} m3/kg", v_measured);
    assert_relative_eq!(t_measured, 7.336163014e2, max_relative = 1e-8);
    assert_relative_eq!(v_measured, 1.676229776e-3, max_relative = 1e-8);

    let h_3b = AvailableEnergy::new::<kilojoule_per_kilogram>(2700.0);
    let t_3b = t_ph_eqm(p100, h_3b).get::<kelvin>();
    let v_3b = v_ph_eqm(p100, h_3b).get::<cubic_meter_per_kilogram>();
    println!("t_ph_eqm(100 MPa, 2700 kJ/kg) = {:.10} K", t_3b);
    println!("v_ph_eqm(100 MPa, 2700 kJ/kg) = {:.10e} m3/kg", v_3b);
    assert_relative_eq!(v_3b, 2.404_234_998e-3, max_relative = 1e-8);
    assert!(t_3b.is_finite() && t_3b > 733.0 && t_3b < 1073.15);

    // Forward/backward round trip through the previously panicking
    // (T,p) point: h(T_ref, 100 MPa) must come back to 2100 kJ/kg within
    // the backward-equation fit residual.
    let h_round = h_tp_eqm_single_phase(t_k(7.336163014e2), p100).get::<joule_per_kilogram>();
    println!(
        "h_tp_eqm_single_phase(733.6163014 K, 100 MPa) = {:.10e} J/kg",
        h_round
    );
    assert_relative_eq!(h_round, 2.1e6, max_relative = 5e-5);
}

/// Methodology 4 / pass criterion 4: closing the 100 MPa ceiling moves the
/// single point `(623.15 K, 100 MPa)` from the Region-1 arm to the
/// Region-3 arm (it was the only pressure on that isotherm that the
/// half-open range spilled into Region 1 — `(623.15 K, 99.999 MPa)` was
/// already Region 3). That relabelling is only harmless if the two region
/// equations agree there, so this test measures the disagreement directly
/// and requires it to stay below `1e-4` relative in both `h` and `v`.
///
/// Results (2026-08-11): `h_tp_1 = 1.5539225034e6 J/kg` vs
/// `h_tp_3 = 1.5539174612e6 J/kg`, relative difference **3.245e-6**;
/// `v_tp_1 = 1.3117600270e-3` vs `v_tp_3 = 1.3117368165e-3 m^3/kg`,
/// relative difference **1.769e-5**. Both well inside the criterion, and
/// inside IF97's own region-boundary consistency band — so the enthalpy
/// returned at that corner shifts by about 3 ppm and the corner now agrees
/// with its immediate pressure neighbours instead of disagreeing with them.
#[test]
fn region_1_and_region_3_agree_at_the_623_15_k_100_mpa_corner() {
    use crate::region_1_subcooled_liquid::{h_tp_1, v_tp_1};
    use crate::region_3_single_phase_plus_supercritical_steam::{h_tp_3, v_tp_3};

    let p100 = Pressure::new::<megapascal>(100.0);
    let t = t_k(623.15);

    let h1 = h_tp_1(t, p100).get::<joule_per_kilogram>();
    let h3 = h_tp_3(t, p100).get::<joule_per_kilogram>();
    let v1 = v_tp_1(t, p100).get::<cubic_meter_per_kilogram>();
    let v3 = v_tp_3(t, p100).get::<cubic_meter_per_kilogram>();
    let h_rel = (h3 - h1).abs() / h1;
    let v_rel = (v3 - v1).abs() / v1;
    println!("h_tp_1 = {h1:.10e} J/kg, h_tp_3 = {h3:.10e} J/kg, rel diff = {h_rel:.3e}");
    println!("v_tp_1 = {v1:.10e} m3/kg, v_tp_3 = {v3:.10e} m3/kg, rel diff = {v_rel:.3e}");
    assert!(
        h_rel < 1e-4,
        "Region 1/3 enthalpy mismatch {h_rel:.3e} at the 623.15 K corner"
    );
    assert!(
        v_rel < 1e-4,
        "Region 1/3 specific-volume mismatch {v_rel:.3e} at the 623.15 K corner"
    );

    // The corner now matches its immediate pressure neighbour.
    assert_eq!(
        region_fwd_eqn_single_phase(t, p100),
        region_fwd_eqn_single_phase(t, Pressure::new::<megapascal>(99.999)),
        "100 MPa must land in the same region as 99.999 MPa on the 623.15 K isotherm"
    );
}
