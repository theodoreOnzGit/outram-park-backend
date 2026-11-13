//! Fukushima Daiichi Units 2 and 3 accident sequences (gh:#1109): RCIC,
//! HPCI, SRVs, spray, vent and injection from USNRC HRTD ch. 7.1, Table
//! 7.1-2, played out on the plant.
//!
//! > Research, education and V&V only. AI draft, unreviewed.
//!
//! **Methodology.** `Plant::for_unit` with `Mode::Historical` runs each
//! unit's cited sequence (`units23::UNIT2_SEQUENCE`, `UNIT3_SEQUENCE`). The
//! tests check that the sequence plays out (each pump starts, trips or is
//! lost when the model or the table says; DC, SRVs, spray, vent and
//! injection follow the rows), and print the model against every reading
//! the table reports (`units23::OBSERVATIONS`), the check, never an input.
//! These are **not regression pins** (maintainer, gh:#1110: no pins until
//! the model is checked); the printed comparison and its interpretation
//! are recorded in
//! `verification_and_validation/bwr_mark1/units23_sequence.md` with the
//! predictions written before the first run. The full sequences run on the
//! lumped containment (an ablation, for cost); the network default runs a
//! short window.
//!
//! **Results:** see the V&V doc (2026-11-02).

use sembawang::bwr_mark1::turbine_pump::Drive;
use sembawang::bwr_mark1::units23::{hm, observations, Observed};
use sembawang::bwr_mark1::{Frame, Mechanisms, Mode, Plant, Unit, DT_S};

/// One run's frame every `every` steps up to `end_s`, the plant kept.
fn run(unit: Unit, mech: Mechanisms, end_s: f64, every: usize) -> (Plant, Vec<Frame>) {
    let mut plant = Plant::for_unit(unit, Mode::Historical, mech);
    let (mut core, mut cont) = plant.parts();
    let mut frames = Vec::new();
    let steps = (end_s / DT_S).round() as usize;
    for i in 0..steps {
        plant.step_inline(&mut core, &mut cont);
        if let Some(e) = cont.error() {
            panic!("{unit:?}: {e}");
        }
        if (i + 1) % every.max(1) == 0 {
            frames.push(plant.frame());
        }
    }
    (plant, frames)
}

/// The frame nearest `t_s`.
fn at(frames: &[Frame], t_s: f64) -> &Frame {
    frames.iter().min_by(|a, b| (a.t_s - t_s).abs().total_cmp(&(b.t_s - t_s).abs())).unwrap()
}

fn lumped() -> Mechanisms {
    Mechanisms::default().without_network_containment()
}

/// The model against every HRTD reading in the run, printed.
fn print_against_hrtd(unit: Unit, frames: &[Frame], taf_above_baf_m: f64) {
    println!("{unit:?} against HRTD Table 7.1-2 (model at the reading's time):");
    println!("  t h:mm   reading                 HRTD        model");
    for o in observations(unit).filter(|o| o.t_s <= frames.last().unwrap().t_s) {
        let f = at(frames, o.t_s);
        let (name, model) = match o.what {
            Observed::ContainmentPsig => ("containment (drywell)", format!("{:.3} MPa", f.containment.p_dw_pa / 1e6)),
            Observed::DrywellPsig => ("drywell", format!("{:.3} MPa", f.containment.p_dw_pa / 1e6)),
            Observed::TorusPsig => ("torus", format!("{:.3} MPa", f.containment.p_ww_pa / 1e6)),
            Observed::RpvPsig => ("RPV", format!("{:.3} MPa", f.vessel.p_pa / 1e6)),
            Observed::LevelInAboveTaf => ("level above TAF", format!("{:.2} m", f.vessel.level_m - taf_above_baf_m)),
        };
        let unit_s = if o.what == Observed::LevelInAboveTaf { format!("{:.2} m", o.si()) } else { format!("{:.3} MPa", o.si() / 1e6) };
        println!("  {:>2}:{:02}   {name:<22}  {unit_s:>10}  {model:>10}   ({})", (o.t_s / 3600.0) as u32, ((o.t_s % 3600.0) / 60.0) as u32, o.note);
    }
}

/// Unit 2 over its first 2 h on the lumped containment: RCIC starts at
/// 00:06, trips at L8 on its own, restarts at 00:53; DC is lost at 00:55
/// and RCIC then runs on with no trip, its level held by carry-over.
#[test]
fn unit2_rcic_trips_at_l8_then_runs_uncontrolled_after_dc_is_lost() {
    let (plant, frames) = run(Unit::Two, lumped(), 2.0 * 3600.0, 60);
    let prm = *plant.params();
    let rcic = plant.pumps()[0];
    let trip = rcic.tripped_at_s.expect("RCIC trips at L8 before the restart");
    println!("Unit 2: RCIC's L8 trip at {:.1} min (HRTD 42 min, recorded at p. 7.1-18)", trip / 60.0);
    assert!(trip > hm(0, 6) && trip < hm(0, 53), "{trip}");
    assert!(at(&frames, hm(0, 5)).w_rcic_kg_s == 0.0 && at(&frames, hm(0, 10)).w_rcic_kg_s > 0.0);
    assert!(at(&frames, hm(0, 10)).w_turbine_kg_s > 0.0 && at(&frames, hm(0, 10)).w_hpci_kg_s == 0.0);
    let late = at(&frames, 2.0 * 3600.0);
    assert!(!late.controls.dc_power && rcic.drive == Drive::Injecting && !rcic.lost);
    // No trip after the restart: no DC, no trip logic.
    assert!(rcic.tripped_at_s == Some(trip));
    for f in frames.iter().filter(|f| f.t_s > hm(1, 0)).step_by(10) {
        println!(
            "  {:>5.2} h  p {:.3} MPa  level {:+.2} m TAF  RCIC {:.2} kg/s  turbine {:.3} kg/s  relief {:.1} kg/s  p_dw {:.4} MPa  T_pool {:.2} K",
            f.t_s / 3600.0,
            f.vessel.p_pa / 1e6,
            f.vessel.level_m - prm.active_length_m,
            f.w_rcic_kg_s,
            f.w_turbine_kg_s,
            f.w_relief_kg_s,
            f.containment.p_dw_pa / 1e6,
            f.containment.t_pool_k
        );
    }
    // Unit 2 has no isolation condensers.
    assert!(frames.iter().all(|f| f.q_ic_w == 0.0));
}

/// Unit 3 over its first 3 h on the lumped containment: RCIC trips at L8,
/// restarts throttled at 01:17, and the operators then hold the level
/// between L2 and L8 with DC kept through the station blackout.
#[test]
fn unit3_throttled_rcic_holds_the_level_with_dc_kept() {
    let (plant, frames) = run(Unit::Three, lumped(), 3.0 * 3600.0, 60);
    let prm = *plant.params();
    let rcic = plant.pumps()[0];
    let trip = rcic.tripped_at_s.expect("RCIC trips at L8 before 01:17");
    println!("Unit 3: RCIC's L8 trip at {:.1} min (HRTD 39 min, p. 7.1-18)", trip / 60.0);
    assert!(trip > hm(0, 19) && trip < hm(1, 17), "{trip}");
    assert_eq!(rcic.drive, Drive::Throttled);
    let tail: Vec<f64> = frames.iter().filter(|f| f.t_s > hm(2, 0)).map(|f| f.vessel.level_m - prm.active_length_m).collect();
    let (lo, hi) = tail.iter().fold((f64::MAX, f64::MIN), |(a, b), &x| (a.min(x), b.max(x)));
    println!("Unit 3: level 2–3 h between {lo:+.2} and {hi:+.2} m above TAF (L2 {:.2}, L8 {:.2})", prm.level_l2_above_taf_m, prm.level_l8_above_taf_m);
    assert!(frames.iter().all(|f| f.controls.dc_power));
    assert!(lo > prm.level_l2_above_taf_m - 1.0 && hi < prm.level_l8_above_taf_m + 0.2, "{lo} {hi}");
}

/// The whole of both sequences on the lumped containment, every HRTD
/// reading printed against the model (the check; results in the V&V doc),
/// and each sequence row seen to act: Unit 2's RCIC lost at 70:39, its SRV
/// held open at 75:14, seawater from 77:08; Unit 3's HPCI from 21:47 to
/// 35:56 with DC lost then, its spray from 40:53 to 42:39, its vent closed
/// from 59:14 to 64:14. Not a pin: nothing here asserts a pressure.
#[test]
fn both_sequences_play_out_against_the_hrtd_readings() {
    let (p2, f2) = run(Unit::Two, lumped(), Unit::Two.scenario_end_s(), 60);
    let taf2 = p2.params().active_length_m;
    print_against_hrtd(Unit::Two, &f2, taf2);
    assert!(p2.pumps()[0].lost && at(&f2, hm(70, 30)).w_rcic_kg_s > 0.0 && at(&f2, hm(70, 50)).w_rcic_kg_s == 0.0);
    assert!(at(&f2, hm(75, 0)).w_injection_kg_s == 0.0 && at(&f2, hm(77, 20)).controls.injection);
    assert!(at(&f2, hm(44, 0)).controls.vent_ordered == false && at(&f2, hm(44, 30)).controls.vent_ordered);
    let (p3, f3) = run(Unit::Three, lumped(), Unit::Three.scenario_end_s(), 60);
    let taf3 = p3.params().active_length_m;
    print_against_hrtd(Unit::Three, &f3, taf3);
    assert!(p3.pumps()[0].lost && p3.pumps()[1].lost);
    assert!(at(&f3, hm(21, 40)).w_hpci_kg_s == 0.0 && at(&f3, hm(35, 50)).controls.dc_power && !at(&f3, hm(36, 0)).controls.dc_power);
    assert!(at(&f3, hm(40, 40)).w_spray_kg_s == 0.0 && at(&f3, hm(42, 30)).w_spray_kg_s > 0.0 && at(&f3, hm(42, 50)).w_spray_kg_s == 0.0);
    assert!(!at(&f3, hm(60, 0)).controls.vent_ordered && at(&f3, hm(64, 30)).controls.vent_ordered);
    // The spray's share (the `without_containment_spray` ablation): Unit 3
    // to 43 h with the spray's water landing unchanged.
    let (_, g3) = run(Unit::Three, lumped().without_containment_spray(), hm(43, 0), 60);
    for t in [hm(40, 53), hm(41, 30), hm(41, 55), hm(42, 24)] {
        println!(
            "Unit 3 at {:.2} h: drywell {:.4} MPa with the spray's exchange, {:.4} MPa without",
            t / 3600.0,
            at(&f3, t).containment.p_dw_pa / 1e6,
            at(&g3, t).containment.p_dw_pa / 1e6
        );
    }
    for (name, f) in [("Unit 2", &f2), ("Unit 3", &f3)] {
        let peak = f.iter().map(|x| x.containment.p_dw_pa).fold(0.0, f64::max);
        let vent = f.iter().find(|x| x.containment.vent_flowing).map(|x| x.t_s / 3600.0);
        let core = f.iter().find(|x| x.core.peak_clad_k > 1500.0).map(|x| x.t_s / 3600.0);
        println!("{name}: peak drywell {:.3} MPa; vent first flowing {vent:?} h; peak clad above 1500 K from {core:?} h; H2 {:.0} kg", peak / 1e6, f.last().unwrap().core.h2_total_kg);
    }
}

/// The network default (gh:#1099) runs Unit 2 with `NetworkConfig::unit2()`
/// for 20 min of plant time: it builds, steps without failing, and the
/// RCIC's exhaust and the relief valves' steam reach its torus.
#[test]
fn the_network_runs_unit2_for_twenty_minutes() {
    let (plant, frames) = run(Unit::Two, Mechanisms::default(), 20.0 * 60.0, 60);
    assert!(plant.mechanisms.network_containment && plant.containment_choice().unit == Unit::Two);
    let last = frames.last().unwrap();
    println!(
        "network, Unit 2 at 20 min: p_dw {:.4} MPa, p_ww {:.4} MPa, T_pool {:.2} K, RCIC {:.2} kg/s",
        last.containment.p_dw_pa / 1e6,
        last.containment.p_ww_pa / 1e6,
        last.containment.t_pool_k,
        last.w_rcic_kg_s
    );
    assert!(frames.iter().any(|f| f.w_rcic_kg_s > 0.0 && f.w_turbine_kg_s > 0.0));
    assert!(last.containment.t_pool_k > plant.params().initial_pool_k);
}

/// The network default (gh:#1099) over each unit's first HRTD readings:
/// Unit 2 to 9 h (the 08:39 containment and RPV readings), Unit 3 to 22 h
/// (21:22, 21:47), printed beside the lumped model's. Ignored: 44 min of
/// wall time on 2026-11-02 (Unit 2 456 s, Unit 3 2195 s, the lumped
/// comparisons the rest), and not a pin (the network's geometry awaits the
/// human check of gh:#1110). Results in `units23_sequence.md`.
#[test]
#[ignore]
fn the_network_over_the_first_hrtd_readings() {
    for (unit, end) in [(Unit::Two, hm(9, 0)), (Unit::Three, hm(22, 0))] {
        let start = std::time::Instant::now();
        let (p, f) = run(unit, Mechanisms::default(), end, 60);
        println!("{unit:?} on the network to {:.0} h: {:.0} s of wall time", end / 3600.0, start.elapsed().as_secs_f64());
        print_against_hrtd(unit, &f, p.params().active_length_m);
        let (q, g) = run(unit, lumped(), end, 60);
        println!("{unit:?} on the lumped containment, for comparison:");
        print_against_hrtd(unit, &g, q.params().active_length_m);
        for t in [hm(1, 0), hm(4, 0), hm(8, 39), end] {
            let (a, b) = (at(&f, t), at(&g, t));
            println!("  {:>5.2} h  drywell network {:.4} / lumped {:.4} MPa; pool {:.2} / {:.2} K", t / 3600.0, a.containment.p_dw_pa / 1e6, b.containment.p_dw_pa / 1e6, a.containment.t_pool_k, b.containment.t_pool_k);
        }
    }
}
