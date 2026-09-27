//! **Defect reproduction: the 273.15 K `(p,h)` -> `(T,p)` boundary is inconsistent.**
//!
//! # What this shows
//!
//! IF97's forward region-1 equation and its backward `T(p,h)` correlation
//! disagree about where the 273.15 K floor is, by ~1.8e-2 K at 4 MPa. Take a
//! `(p, h)` point that sits **exactly on** the floor isotherm by the *forward*
//! equation, invert it with the *backward* equation, and the temperature comes
//! back **below** 273.15 K. Any `(p,h)` entry point that internally performs a
//! `(T,p)` flash on that inverted temperature then hits
//! `region_fwd_eqn_single_phase`'s strict bound and **panics** --
//! [`lambda_ph_eqm`] is one such entry point, via its critical-enhancement term.
//!
//! # Why it is recorded here rather than fixed here
//!
//! The fix belongs in the library and is a maintainer decision, because it has
//! to choose *which* boundary is authoritative: either the backward equation's
//! output is clamped into the forward equation's closed domain at the point of
//! use (cheap, and defensible -- the point was already validated as in-domain by
//! the `(p,h)` guard it entered through), or the `(p,h)` validity guard is
//! tightened so it rejects points whose inverse falls out of the `(T,p)` domain
//! (stricter, and would reject genuinely representable states).
//!
//! This is **not** a physics guard being bolted onto a wrong formulation. The
//! formulation error *is* the disagreement between two representations of one
//! boundary; reconciling them is the fix.
//!
//! # How it was found (2026-09-27)
//!
//! `htgr_sim_v1`'s headless run began panicking with
//! `t,p flashing at eqm out of bounds!` after its opening control-rod insertion
//! was made shallower, which drives more power in and -- with the reactor
//! protection system disarmed by default -- lets the secondary side coast down
//! to the triple point. The panic chain was
//!
//! ```text
//! HtgrPlant::step -> step_with_correctors -> advance_steam_generator
//!   -> NodalisedCounterFlowSteamGenerator::advance_timestep
//!   -> TampinesSteamArray::{advance_timestep, step, correct_thermo}
//!   -> lambda_ph_eqm -> lambda_2_crit_enhancement_term_tp_two_phase_estimate
//!   -> region_fwd_eqn_single_phase  ** panic **
//! ```
//!
//! **The steam generator was not the cause and had not changed.** Its last edit
//! was 2026-08-13; `lambda_ph_eqm`'s single-phase path has handed `(t, p)` to
//! `cp_tp_eqm_single_phase` since before 2026-07-14 (commit `b49686ef3f`, which
//! added the two-phase branch, explicitly left the single-phase path
//! bit-for-bit unchanged). The defect is long-standing and latent; what changed
//! was only that a cell finally reached the floor. This file exists so the next
//! person to meet the panic does not go looking in the steam generator again.
//!
//! Related, and evidence that this class of edge is already known here: the
//! `TampinesSteamArray` constructor nudges its lower **pressure** bound 0.1 %
//! above `p_sat(273.15 K)` for the same reason, and the module doc records two
//! earlier instances (`op-21g.15.7`, and the near-break over-drain).

use tampines_steam_tables::interfaces::functional_programming::ph_flash_eqm::{
    lambda_ph_eqm, t_ph_eqm,
};
use tampines_steam_tables::region_1_subcooled_liquid::h_tp_1;
use uom::si::available_energy::joule_per_kilogram;
use uom::si::f64::{Pressure, ThermodynamicTemperature};
use uom::si::pressure::pascal;
use uom::si::thermodynamic_temperature::kelvin;

/// IF97's lower temperature bound, the triple point.
const IF97_MIN_TEMPERATURE_K: f64 = 273.15;

/// **V&V: the `(p,h)` -> `T` inverse undershoots the 273.15 K floor.**
///
/// # Methodology
///
/// At `p = 4.000283861286804 MPa` -- the exact pressure the HTGR steam
/// generator's cold side reached when it panicked, carried here verbatim so the
/// reproduction is the live case and not a rounded stand-in:
///
/// 1. Evaluate `h = h_tp_1(273.15 K, p)` with the **forward** region-1
///    equation. By construction this point is *on* the floor isotherm.
/// 2. Invert it with the **backward** equation, `t_ph_eqm(p, h)`.
/// 3. Report the round-trip error, and report (without asserting) whether
///    `lambda_ph_eqm` survives the same point.
///
/// Pass criterion: the inverse is **strictly below** 273.15 K. That is the
/// defect, stated as a falsifiable inequality -- the test fails, correctly, the
/// day the two boundaries are reconciled, and the failure message says so.
///
/// The panic is deliberately **reported, not asserted**: asserting it would
/// make fixing the library break this test for the wrong reason.
///
/// # Results (measured 2026-09-27)
///
/// | Quantity | Value |
/// |---|---|
/// | `p` | 4 000 283.861287 Pa |
/// | `h_tp_1(273.15 K, p)` (forward) | 4 020.912221 J/kg |
/// | `t_ph_eqm(p, h)` (backward) | **273.132322334 K** |
/// | round-trip error | **-1.768e-2 K** |
/// | `lambda_ph_eqm(p, h)` | **panics** (`t,p flashing at eqm out of bounds!`) |
///
/// **Interpretation.** 17.7 mK is far larger than floating-point noise and is
/// the expected size of an IAPWS backward-equation residual near a region
/// boundary -- the backward equations are *correlations fitted* to the forward
/// ones, not exact inverses, and IF97 does not claim they agree at the edge. So
/// the library is not wrong about water; it is wrong about its own domain
/// bookkeeping. Any `(p,h)` point within 17.7 mK of the floor at this pressure
/// is a live panic, which is a ~4 kJ/kg-wide band in enthalpy that a cooling
/// transient passes straight through.
#[test]
fn ph_to_t_inverse_undershoots_the_273_15_floor() {
    let p = Pressure::new::<pascal>(4_000_283.861_286_804);
    let h = h_tp_1(
        ThermodynamicTemperature::new::<kelvin>(IF97_MIN_TEMPERATURE_K),
        p,
    );
    let t_back = t_ph_eqm(p, h).get::<kelvin>();

    // Reported, never asserted -- see the doc comment.
    let lambda = std::panic::catch_unwind(|| lambda_ph_eqm(p, h));

    println!(
        "IF97 273.15 K FLOOR ROUND TRIP\n  \
         p                        = {:.6} Pa\n  \
         h_tp_1(273.15 K, p)      = {:.6} J/kg   (forward region 1)\n  \
         t_ph_eqm(p, h)           = {:.9} K      (backward)\n  \
         round-trip error         = {:+.3e} K\n  \
         lambda_ph_eqm(p, h)      = {}",
        p.get::<pascal>(),
        h.get::<joule_per_kilogram>(),
        t_back,
        t_back - IF97_MIN_TEMPERATURE_K,
        match &lambda {
            Ok(l) => format!("{l:?}"),
            Err(_) => "PANIC (t,p flashing at eqm out of bounds!)".to_string(),
        }
    );

    assert!(
        t_back < IF97_MIN_TEMPERATURE_K,
        "the backward equation no longer undershoots the 273.15 K floor at this \
         pressure: t_ph_eqm returned {t_back} K. If the two boundaries have been \
         reconciled this test has done its job -- delete it, and delete the panic \
         note in its module doc."
    );
}

/// **Does `lambda_ph_eqm` actually work in Region 5?** Its doc comment says no.
///
/// # Why this is asked
///
/// `lambda_ph_eqm`'s doc reads *"Valid over the same `(p,h)` range as the rest of
/// this module (Regions 1-4; **Region 5 is unsupported**)"*. That matters to any
/// caller whose steam can get hot: `TampinesSteamArray::correct_thermo` calls
/// `t_ph_eqm`, `v_ph_eqm`, `mu_ph_eqm`, `lambda_ph_eqm` and `cp_ph_eqm` on every
/// cell, every substep, so if one of the five stops at 1073.15 K while the others
/// reach 2273.15 K, the useful envelope is the smallest of them.
///
/// # Methodology
///
/// Pick a point squarely inside Region 5 — **1500 K at 1 MPa**, comfortably above
/// the 1073.15 K Region 2/5 boundary and below the 2273.15 K top — get its
/// enthalpy from the **forward** Region 5 equation, and ask the `(p,h)` family for
/// the properties. Report which succeed and which panic. No assertion is made on
/// `lambda` before measuring; the point is to find out.
///
/// # Results (measured 2026-09-27) — the doc comment is STALE
///
/// | Call | Outcome at 1500 K, 1 MPa |
/// |---|---|
/// | `h_tp_eqm_single_phase` (forward) | ok |
/// | `t_ph_eqm` | ok, recovers ~1500 K |
/// | `lambda_ph_eqm` | see this test's stdout |
///
/// `lambda_ph_eqm`'s three internal dependencies **all gained Region 5 arms**:
/// `cp_tp_eqm_single_phase -> cp_tp_5`, `cv_tp_eqm_single_phase -> cv_tp_5` and
/// `kappa_t_tp_eqm -> kappa_t_tp_5`, and `region_fwd_eqn_single_phase` has a
/// `Region5` arm. The "Region 5 is unsupported" sentence predates commit
/// `2ab91fefc3` ("(p,h), (p,s) and (h,s) now cover Region 5") and was never
/// updated.
///
/// # A SEPARATE caveat that survives whatever this test prints
///
/// **Running is not the same as being valid.** IAPWS R15-11, the thermal
/// conductivity release, is stated for `273.16 K <= T <= 1173.15 K`. Region 5
/// runs to **2273.15 K**, so above ~1173 K a returned `lambda` is an
/// **extrapolation of the correlation**, not a tabulated value — whether or not
/// the code refuses. That is a physics limit, not a dispatch limit, and no amount
/// of region plumbing fixes it. A caller relying on `lambda` above 1173.15 K
/// should know it is extrapolating.
#[test]
fn does_lambda_ph_eqm_work_in_region_5() {
    use tampines_steam_tables::interfaces::functional_programming::pt_flash_eqm::h_tp_eqm_single_phase;

    let t_r5 = ThermodynamicTemperature::new::<kelvin>(1500.0);
    let p = Pressure::new::<pascal>(1.0e6);

    let h = std::panic::catch_unwind(|| h_tp_eqm_single_phase(t_r5, p));
    let h = match h {
        Ok(h) => h,
        Err(_) => {
            println!("REGION 5 (1500 K, 1 MPa): the FORWARD h(T,p) itself panicked");
            panic!("cannot set up the test point");
        }
    };
    let t_back = std::panic::catch_unwind(|| t_ph_eqm(p, h));
    let lambda = std::panic::catch_unwind(|| lambda_ph_eqm(p, h));

    println!(
        "REGION 5 PROPERTY DISPATCH (T = 1500 K, p = 1 MPa)\n  \
         h_tp_eqm_single_phase = {:.3} kJ/kg  (forward, Region 5)\n  \
         t_ph_eqm(p, h)        = {}\n  \
         lambda_ph_eqm(p, h)   = {}\n  \
         NOTE: IAPWS R15-11 states lambda only to 1173.15 K, so a value here is \
         an EXTRAPOLATION of the correlation even when the code returns one.",
        h.get::<joule_per_kilogram>() / 1.0e3,
        match &t_back {
            Ok(t) => format!("{:.3} K", t.get::<kelvin>()),
            Err(_) => "PANIC".to_string(),
        },
        match &lambda {
            Ok(l) => format!("{l:?}"),
            Err(_) => "PANIC (doc comment would be correct)".to_string(),
        },
    );

    assert!(
        t_back.is_ok(),
        "the (p,h) backward equation must reach Region 5 -- commit 2ab91fefc3 added it"
    );
}

/// **Does the `(p,s)` flash reach Region 5, and does the *checked* facade?**
///
/// # Why this test exists
///
/// `interfaces::checked::ps`'s `try_t_ps_eqm` doc said the flash covers
/// "Regions 1-4; Region 5 is not implemented for this flash path and lies
/// outside the entropy window", and the same module's panic-trace table claimed
/// `ps_flash_region` "has no code path that returns `Region5`". Commit
/// `2ab91fefc3` ("(p,h), (p,s) and (h,s) now cover Region 5", 2026-09-14)
/// replaced both `todo!("region 5 ps flash not implemented")` arms with a
/// dispatch to the in-house Chebyshev `t_ps_5` correlation. The doc was never
/// updated. This test settles which half of the sentence survived.
///
/// # Methodology
///
/// Build a Region 5 state from the **IAPWS forward** equations, so the test
/// point's provenance is the standard and not the thing under test: take
/// `T = 1500 K`, `p = 1 MPa` (inside Region 5's 1073.15-2273.15 K band and well
/// under its 50 MPa ceiling) and evaluate `s_tp_eqm_single_phase(T, p)`. Then
/// call, under `catch_unwind` so a panic is data rather than a crash:
///
/// 1. `ps_flash_region(p, s)` — which region the router assigns;
/// 2. `t_ps_eqm(p, s)` — the **unchecked** backward path;
/// 3. `try_t_ps_eqm(p, s)` — the **checked** facade.
///
/// Pass criterion: the unchecked path must return, and must return the
/// temperature it was built from to better than 1e-4 relative — that is the
/// round-trip against the forward equation, the only defensible reference,
/// there being no published IAPWS backward equation for Region 5.
///
/// # Results — measured 2026-09-27
///
/// `cargo test --release -j 3 -p tampines-steam-tables --test
/// boundary_273_15_repro -- --nocapture`:
///
/// - `s_tp_eqm_single_phase(1500 K, 1 MPa) = 9.33359 kJ/(kg K)`
/// - `ps_flash_region` returns **`Region5`** — so the "no code path returns
///   Region5" claim is stale.
/// - `t_ps_eqm(p, s) = 1500.0000 K`, `|dT/T| = 6.0e-9` — the unchecked path
///   reaches Region 5 and round-trips.
/// - `try_t_ps_eqm(p, s)` returns **`Err(OutOfRange { quantity: "specific
///   entropy", value: 9333.5885, min: -0.0884, max: 8502.3497, unit:
///   "J/(kg K)" })`** — the checked facade's envelope still stops at the
///   1073.15 K isotherm (`max` is `s(1073.15 K, 1 MPa)`), so "lies outside the
///   entropy window" is **still true** for the checked path.
///
/// # Interpretation, and the R15-11 distinction
///
/// The two halves of the old sentence had opposite fates: "not implemented for
/// this flash path" is **false** (it is implemented), "lies outside the entropy
/// window" is **true** (the facade refuses it). A reader who needs Region 5
/// through `(p,s)` must call the unchecked `t_ps_eqm`, accepting that it panics
/// rather than erring outside the envelope.
///
/// And, exactly as for `lambda_ph_eqm` above: a dispatch that *succeeds* is not
/// the same as a value that is *valid*. The Region 5 temperature here comes from
/// this crate's own Chebyshev fit, not from IAPWS-IF97 — IAPWS publishes no
/// Region 5 backward equation at all. The number is an in-house correlation
/// verified only against the forward equations it inverts.
#[test]
fn does_the_ps_flash_reach_region_5_and_does_the_checked_facade() {
    use tampines_steam_tables::interfaces::checked::ps::try_t_ps_eqm;
    use tampines_steam_tables::interfaces::functional_programming::ps_flash_eqm::{
        ps_flash_region, t_ps_eqm,
    };
    use tampines_steam_tables::interfaces::functional_programming::pt_flash_eqm::s_tp_eqm_single_phase;
    use uom::si::specific_heat_capacity::joule_per_kilogram_kelvin;

    let t_r5 = ThermodynamicTemperature::new::<kelvin>(1500.0);
    let p = Pressure::new::<pascal>(1.0e6);

    let s = s_tp_eqm_single_phase(t_r5, p);
    let region = std::panic::catch_unwind(|| ps_flash_region(p, s));
    let t_back = std::panic::catch_unwind(|| t_ps_eqm(p, s));
    let checked = std::panic::catch_unwind(|| try_t_ps_eqm(p, s));

    println!(
        "REGION 5 (p,s) DISPATCH (T = 1500 K, p = 1 MPa)\n  \
         s_tp_eqm_single_phase = {:.5} kJ/(kg K)  (forward, Region 5)\n  \
         ps_flash_region(p, s) = {}\n  \
         t_ps_eqm(p, s)        = {}\n  \
         try_t_ps_eqm(p, s)    = {}\n  \
         NOTE: IAPWS publishes NO Region 5 backward equation. Any temperature \
         here is this crate's own Chebyshev fit (`t_ps_5`), verified only \
         against the forward equations it inverts.",
        s.get::<joule_per_kilogram_kelvin>() / 1.0e3,
        match &region {
            Ok(r) => format!("{r:?}"),
            Err(_) => "PANIC".to_string(),
        },
        match &t_back {
            Ok(t) => format!(
                "{:.4} K  (|dT/T| = {:.1e})",
                t.get::<kelvin>(),
                (t.get::<kelvin>() - 1500.0).abs() / 1500.0
            ),
            Err(_) => "PANIC".to_string(),
        },
        match &checked {
            Ok(Ok(t)) => format!("Ok({:.4} K)", t.get::<kelvin>()),
            Ok(Err(e)) => format!("Err({e:?})"),
            Err(_) => "PANIC".to_string(),
        },
    );

    let t_back = t_back.expect("the (p,s) backward path must reach Region 5 -- commit 2ab91fefc3");
    let rel = (t_back.get::<kelvin>() - 1500.0).abs() / 1500.0;
    assert!(
        rel < 1.0e-4,
        "the Region 5 (p,s) inversion must round-trip the forward equation: got {} K, |dT/T| = {rel:.3e}",
        t_back.get::<kelvin>()
    );
}

/// **Two Region 5 boundary claims, checked rather than repeated.**
///
/// # Why this test exists
///
/// Two docs in this crate make falsifiable statements about the Region 5 edge
/// that nothing asserted:
///
/// 1. `interfaces::checked::mod`'s module doc and `try_t_ph_eqm`'s doc say a
///    Region 5 `(p,h)` state "falls out of the `h` upper bound here", i.e. the
///    **checked** facade refuses it even though the unchecked `t_ph_eqm` now
///    reaches Region 5 (see `does_lambda_ph_eqm_work_in_region_5`).
/// 2. `crates/tampines-steam-tables/CLAUDE.md` says *"R5 boundary: results above
///    2273 K are extrapolations, not IF97. The library returns `OutOfRange` by
///    default."*
///
/// # Methodology
///
/// Same Region 5 point as the tests above: `T = 1500 K`, `p = 1 MPa`, with `h`
/// from the IAPWS **forward** equation. Then:
///
/// - call `try_t_ph_eqm(p, h)` and record the variant returned;
/// - build a state above Region 5's 2273.15 K ceiling — `h` at 1 MPa scaled to
///   about 2.5x the 1500 K enthalpy, which lies above the 2273.15 K isotherm —
///   and call the **unchecked** `t_ph_eqm` under `catch_unwind`, recording
///   whether the library errs or **panics**.
///
/// Pass criterion: claim 1 holds (the checked facade returns
/// `Err(OutOfRange { quantity: "specific enthalpy", .. })`). Claim 2 is
/// **reported, not asserted**, because "returns `OutOfRange`" is a statement
/// about a doc, and asserting a panic would make fixing the library fail this
/// test for the wrong reason.
///
/// # Results — measured 2026-09-27
///
/// - `try_t_ph_eqm(1 MPa, 5218.863 kJ/kg)` → `Err(OutOfRange { quantity:
///   "specific enthalpy", .. })`. **Claim 1 is still true**: the checked facade's
///   window stops at the 1073.15 K isotherm while the unchecked path reaches
///   2273.15 K, so the two differ by the whole of Region 5. That is a real
///   asymmetry, not a defect in either — but a caller who needs Region 5 must
///   leave the checked facade and accept panics.
/// - The above-2273.15 K unchecked call **PANICS** (see printed output). So
///   `CLAUDE.md`'s *"the library returns `OutOfRange` by default"* is true only
///   of the `interfaces::checked::*` facade, never of the unchecked functional
///   API, which is what the rest of that document is about.
#[test]
fn the_checked_ph_facade_still_refuses_region_5_and_the_2273_k_ceiling_panics() {
    use tampines_steam_tables::interfaces::checked::try_t_ph_eqm;
    use tampines_steam_tables::interfaces::functional_programming::pt_flash_eqm::h_tp_eqm_single_phase;

    let t_r5 = ThermodynamicTemperature::new::<kelvin>(1500.0);
    let p = Pressure::new::<pascal>(1.0e6);
    let h = h_tp_eqm_single_phase(t_r5, p);

    let checked = try_t_ph_eqm(p, h);

    // Comfortably above the 2273.15 K isotherm at this pressure.
    let h_too_hot = h * 2.5;
    let over_ceiling = std::panic::catch_unwind(|| t_ph_eqm(p, h_too_hot));

    println!(
        "REGION 5 EDGE CLAIMS (p = 1 MPa)\n  \
         h_tp_eqm_single_phase(1500 K, p) = {:.3} kJ/kg\n  \
         try_t_ph_eqm(p, h)               = {}\n  \
         t_ph_eqm(p, 2.5*h) [> 2273.15 K] = {}\n  \
         CLAUDE.md's \"the library returns OutOfRange by default\" above 2273 K \
         describes interfaces::checked only, NOT the unchecked functional API.",
        h.get::<joule_per_kilogram>() / 1.0e3,
        match &checked {
            Ok(t) => format!("Ok({:.4} K)", t.get::<kelvin>()),
            Err(e) => format!("Err({e:?})"),
        },
        match &over_ceiling {
            Ok(t) => format!("Ok({:.4} K)", t.get::<kelvin>()),
            Err(_) => "PANIC".to_string(),
        },
    );

    assert!(
        checked.is_err(),
        "the checked (p,h) facade must still refuse a Region 5 state -- its \
         enthalpy window stops at the 1073.15 K isotherm. Got {checked:?}"
    );
}
