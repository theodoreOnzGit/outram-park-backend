//! # V&V: Unit 1's containment on `tampines`'s control-volume network
//! against `sembawang`'s `Containment` (gh:#1097)
//!
//! > Research, education and V&V only. AI draft, unreviewed.
//!
//! Both models take the same `ContainmentSnapshot` each step. The record,
//! with every difference and its cause, is
//! `verification_and_validation/bwr_mark1/containment_network.md`.
//!
//! ## Predictions, written 2026-11-01 before the first run
//!
//! **A. Relief steam into the torus**, 10 kg/s at 2.7 MJ/kg for 600 s at
//! 1 s (the wet well's case, `wet_well_vs_tampines.rs`), the old model with
//! its wetwell atmosphere, `two_sided_equalising`, `outflow_enthalpy` and
//! `gas_cv_t` on and the mechanisms the network does not carry off (no
//! pool layers, no torus shell, the drywell structures by MELCOR's surface
//! in both):
//!
//! 1. Prismatic torus (sembawang's 750 m² pool): `p_ww` above the old
//!    model's by +250 to +450 Pa, from the pool's rise compressing the gas
//!    (the wet well gave +349 Pa); `T_pool` within 2e-3 K.
//! 2. Torus V(z) (711 m² at the level): `p_ww` +250 to +500 Pa (the level
//!    rises 5 % more), `T_pool` within 1e-2 K.
//! 3. No downcomer or vacuum-breaker flow (the torus stays within 1 kPa of
//!    the drywell, below the 3.4 kPa breaker and above the drywell by less
//!    than the submergence); `p_dw` within 200 Pa (the network's drywell
//!    sump, 2 m², humidifies the dry drywell a little).
//! 4. The pressure-work ablation removes most of the `p_ww` difference
//!    (to within ±100 Pa).
//! 5. Every Picard step converges in fewer than 30 iterations.
//!
//! **B. The first hours of the Unit 1 sequence**, the old model's
//! containment snapshots recorded from `Plant::new(Mode::Historical)` and
//! replayed into the network (open loop: the vessel does not see the
//! network's drywell):
//!
//! 6. `p_dw` and `p_ww` within 10 % of the old model while only the relief
//!    valves and the leak drive the containment; the network's `p_ww`
//!    **below** the old model's once the pool heats, because the old model
//!    stratifies the pool (`pool_layers`, its hot top layer setting the
//!    surface's vapour pressure) and the network does not.
//! 7. `T_pool` (the old model's mass mean) within 1 K.
//! 8. A step costs under 5 ms, so the 30 h scenario would take under
//!    10 minutes.
//!
//! ## gh:#1100: sembawang's cavity, FDI and drywell slab coupled
//!
//! Predictions written 2026-11-01 before the first run (the V&V doc carries
//! the replay's, per window):
//!
//! **C. Debris on the drywell floor**, both models from rest, 1 s steps,
//! 1800 s; 50 t UO2 and 10 t Zr at 2600 K arrive at once with 5 MW of decay
//! heat, the vessel at 0.5 MPa; the old model with `mcci`,
//! `cavity_phases`, `fdi_lpme`, `dw_wall_outer`, `condensate_to_floor`.
//!
//! 9. Dry floor: the same `Cavity` code against nearly the same structure
//!    temperature, so the concrete ablated and the H2 and CO made agree
//!    within 2 %, the debris temperature within 5 K; `p_dw` within 3 %
//!    (IF97 against ideal gases; CO and CO2 as their own species on the
//!    network, nitrogen-like moles in the old model: same moles, so the
//!    same pressure to first order).
//! 10. Wet floor (10 kg/s poured for 600 s first, then the debris): the FDI
//!     steam agrees within 5 % (the network's water over the fill against
//!     the old pedestal; the same fall), and the ablated mass within 5 %.
//! 11. With `without_cavity().without_dw_slab()` the network reproduces the
//!     gh:#1097 6 h pins exactly (the new port carries nothing; the
//!     refactors of `lpme` and `dw_slab_step` are arithmetic-preserving).

use sembawang::bwr_mark1::containment::{Containment, ContainmentSnapshot};
use sembawang::bwr_mark1::containment_network::{ContainmentNetwork, NetworkConfig, NetworkMechanisms, NetworkResult, TorusShape};
use sembawang::bwr_mark1::params::Params;
use sembawang::bwr_mark1::{Mechanisms, Mode, Plant};

/// `FUKUSHIMA_OLD_ABLATE=pool_layers,fp_decay_follows,mcci,dw_wall_outer`
/// ablates those mechanisms of the recorded plant (the old model), so a
/// remaining difference can be attributed to a `Containment` mechanism:
/// both models then see the ablated plant's snapshots.
fn old_ablations_from_env() -> Mechanisms {
    let mut m = Mechanisms::default();
    for a in std::env::var("FUKUSHIMA_OLD_ABLATE").unwrap_or_default().split(',') {
        m = match a.trim() {
            "pool_layers" => m.without_pool_layers(),
            "fp_decay_follows" => m.without_fp_decay_follows(),
            "mcci" => m.without_mcci(),
            "dw_wall_outer" => m.without_dw_wall_outer(),
            "" => m,
            other => panic!("unknown old-model ablation {other}"),
        };
    }
    m
}

/// `FUKUSHIMA_NETWORK_ABLATE=cavity,fdi,dw_slab` ablates those network
/// mechanisms in case B (for attributing a difference).
fn ablations_from_env() -> NetworkMechanisms {
    let mut m = NetworkMechanisms::default();
    for a in std::env::var("FUKUSHIMA_NETWORK_ABLATE").unwrap_or_default().split(',') {
        m = match a.trim() {
            "cavity" => m.without_cavity(),
            "fdi" => m.without_fdi(),
            "dw_slab" => m.without_dw_slab(),
            "" => m,
            other => panic!("unknown ablation {other}"),
        };
    }
    m
}

/// The physics gh:#1100 couples is ON by default, through the ordinary
/// constructor (root `CLAUDE.md`, "Correct physics is the DEFAULT"), and
/// each `without_*` clears only its own field.
#[test]
fn the_coupled_physics_is_on_by_default() {
    let net = ContainmentNetwork::new(&Params::default()).unwrap();
    let m = net.mechanisms();
    assert!(m.cavity && m.fdi && m.dw_slab, "{m:?}");
    assert_eq!(m, NetworkMechanisms::default());
    let d = NetworkMechanisms::default();
    assert_eq!(d.without_cavity(), NetworkMechanisms { cavity: false, ..d });
    assert_eq!(d.without_fdi(), NetworkMechanisms { fdi: false, ..d });
    assert_eq!(d.without_dw_slab(), NetworkMechanisms { dw_slab: false, ..d });
    assert_eq!(d.without_pressure_work(), NetworkMechanisms { pressure_work: false, ..d });
    assert_eq!(d.without_real_steam(), NetworkMechanisms { real_steam: false, ..d });
    assert_eq!(d.without_real_liquid(), NetworkMechanisms { real_liquid: false, ..d });
    assert_eq!(d.without_level_swell(), NetworkMechanisms { level_swell: false, ..d });
    assert_eq!(d.without_bubble_pass_through(), NetworkMechanisms { bubble_pass_through: false, ..d });
    assert_eq!(d.without_equilibrium_rain_out(), NetworkMechanisms { equilibrium_rain_out: false, ..d });
    assert_eq!(d.without_choking(), NetworkMechanisms { choking: false, ..d });
}

fn debris_snapshot(prm: Params) -> ContainmentSnapshot {
    ContainmentSnapshot {
        mcci: true,
        cavity_phases: true,
        fdi_lpme: true,
        dw_wall_outer: true,
        condensate_to_floor: true,
        vessel_p_pa: 0.5e6,
        ..relief_snapshot(prm)
    }
}

/// Case C: `wet` pours 10 kg/s onto the floor for 600 s before the
/// debris arrives. Returns both models' last results.
fn run_debris(wet: bool) -> (NetworkResult, sembawang::bwr_mark1::containment::ContainmentResult, f64) {
    let prm = Params::default();
    let base = ContainmentSnapshot { srv_steam_kg_s: 0.0, ..debris_snapshot(prm) };
    let mut old = Containment::new(&prm);
    let mut net = ContainmentNetwork::new(&prm).unwrap();
    let pour = ContainmentSnapshot { pedestal_inflow_kg_s: 10.0, ..base.clone() };
    let mut worst_p: f64 = 0.0;
    if wet {
        for _ in 0..600 {
            old.step(&pour);
            net.step(&pour).unwrap();
        }
    }
    let debris = ContainmentSnapshot { ex_vessel_w: 5.0e6, cav_uo2_kg: 50_000.0, cav_zr_kg: 10_000.0, cav_debris_k: 2600.0, ..base };
    let (mut r_net, mut r_old) = (NetworkResult::default(), Default::default());
    for _ in 0..1800 {
        r_old = old.step(&debris);
        r_net = net.step(&debris).unwrap();
        worst_p = worst_p.max(((r_net.p_dw_pa - r_old.p_dw_pa) / r_old.p_dw_pa).abs());
    }
    assert_eq!(net.cavity().ablated_kg, r_net.cav_ablated_kg);
    (r_net, r_old, worst_p)
}

/// **C (predictions 9 and 10).** Results (2026-11-01): both held. Dry:
/// ablated 1258.3 / 1258.3 kg, H2 8.251 / 8.251 kg, CO 12.012 / 12.012 kg,
/// debris 2774.9 / 2774.9 K, worst |Δ`p_dw`| 0.03 %. Wet: FDI steam 27.7 /
/// 27.6 kg, ablated 1240.3 / 1240.3 kg; worst |Δ`p_dw`| 3.46 % (not
/// predicted, not asserted: the network's sump boils from saturation, the
/// old pedestal at its own temperature, and the 6000 kg poured displaces
/// the drywell gas).
#[test]
fn debris_on_the_drywell_floor_against_the_old_containment() {
    for wet in [false, true] {
        let (n, o, worst_p) = run_debris(wet);
        println!(
            "C wet={wet}: ablated {:.1} / {:.1} kg, H2 {:.3} / {:.3} kg, CO {:.3} / {:.3} kg, debris {:.1} / {:.1} K, FDI steam {:.1} / {:.1} kg, p_dw {:.0} / {:.0} Pa (worst {:.2} %), T_dw {:.1} / {:.1} K, wall {:.1} K, slab {:.2} K, sump {:.0} kg",
            n.cav_ablated_kg,
            o.cav_ablated_kg,
            n.cav_h2_kg,
            o.cav_h2_kg,
            n.cav_co_kg,
            o.cav_co_kg,
            n.cav_debris_k,
            o.cav_debris_k,
            n.fdi_steam_kg,
            o.fdi_steam_kg,
            n.p_dw_pa,
            o.p_dw_pa,
            100.0 * worst_p,
            n.t_dw_k,
            o.t_dw_k,
            n.dw_wall_k,
            n.dw_slab_inner_k,
            n.dw_sump_kg
        );
        assert!(n.cav_ablated_kg > 0.0 && n.dw_co_kg + n.dw_co2_kg > 0.0, "C wet={wet}: the cavity did nothing");
        let rel = |a: f64, b: f64| ((a - b) / b).abs();
        if wet {
            assert!(n.fdi_steam_kg > 0.0, "C wet: no FDI steam");
            assert!(rel(n.fdi_steam_kg, o.fdi_steam_kg) < 0.05, "C10 FDI {} {}", n.fdi_steam_kg, o.fdi_steam_kg);
            assert!(rel(n.cav_ablated_kg, o.cav_ablated_kg) < 0.05, "C10 ablated {} {}", n.cav_ablated_kg, o.cav_ablated_kg);
        } else {
            assert!(rel(n.cav_ablated_kg, o.cav_ablated_kg) < 0.02, "C9 ablated {} {}", n.cav_ablated_kg, o.cav_ablated_kg);
            assert!(rel(n.cav_h2_kg, o.cav_h2_kg) < 0.02 && rel(n.cav_co_kg, o.cav_co_kg) < 0.02, "C9 gases");
            assert!((n.cav_debris_k - o.cav_debris_k).abs() < 5.0, "C9 debris");
            assert!(worst_p < 0.03, "C9 p_dw {worst_p}");
        }
    }
}


fn relief_snapshot(prm: Params) -> ContainmentSnapshot {
    ContainmentSnapshot {
        dt_s: 1.0,
        srv_steam_kg_s: 10.0,
        srv_h: 2.7e6,
        gas_cv_t: true,
        ww_atmosphere: true,
        dw_wall_hs: true,
        two_sided_equalising: true,
        outflow_enthalpy: true,
        prm,
        ..Default::default()
    }
}

fn run_relief(cfg: NetworkConfig, mech: NetworkMechanisms) -> (NetworkResult, f64, f64, f64, usize) {
    let prm = Params::default();
    let snap = relief_snapshot(prm);
    let mut old = Containment::new(&prm);
    let mut net = ContainmentNetwork::with_config(&prm, cfg, mech).unwrap();
    let mut worst_t: f64 = 0.0;
    let mut most_iter = 0;
    let mut r_old = old.step(&ContainmentSnapshot { srv_steam_kg_s: 0.0, ..snap.clone() });
    let mut r_net = net.step(&ContainmentSnapshot { srv_steam_kg_s: 0.0, ..snap.clone() }).unwrap();
    let (dp0, dpdw0) = (r_net.p_ww_pa - r_old.p_ww_pa, r_net.p_dw_pa - r_old.p_dw_pa);
    println!("at rest: p_ww {:.2} vs {:.2} Pa, p_dw {:.2} vs {:.2}", r_net.p_ww_pa, r_old.p_ww_pa, r_net.p_dw_pa, r_old.p_dw_pa);
    let _ = (dp0, dpdw0);
    for _ in 0..600 {
        r_old = old.step(&snap);
        r_net = net.step(&snap).unwrap();
        worst_t = worst_t.max((r_net.t_pool_k - r_old.t_pool_k).abs());
        most_iter = most_iter.max(r_net.iterations);
    }
    (r_net.clone(), r_net.p_ww_pa - r_old.p_ww_pa, r_net.p_dw_pa - r_old.p_dw_pa, worst_t, most_iter)
}

/// **A (predictions 1–5). Results (2026-11-01):** prismatic `p_ww`
/// +341.1 Pa, `p_dw` +6.4 Pa, worst `T_pool` 2.87e-3 K (prediction 1:
/// pressure held, temperature **MISSED**, from the IF97 liquid: 4.3e-5 K
/// with constant `c_p`); torus +328.8 Pa, 2.89e-3 K (prediction 2 held,
/// though the reasoning was wrong: the compression is set by the water's
/// volume, not the area); no downcomer or breaker flow (3 held); without
/// the pressure work +280.7 Pa (4 **MISSED**: the pool's rise compresses
/// the gas geometrically, the pressure work is only its heating); at most
/// 2 Picard iterations (5 held). At rest the torus reads −8.3 Pa (IF97
/// steam against sembawang's ideal steam at the same mass).
#[test]
fn relief_steam_into_the_torus_against_the_old_containment() {
    let prismatic = NetworkConfig { torus_shape: TorusShape::Prismatic, ..NetworkConfig::unit1() };
    let on = NetworkMechanisms::default();
    let (r, dp, dp_dw, dt, it) = run_relief(prismatic, on);
    println!(
        "A1 prismatic: dp_ww {dp:+.1} Pa, dp_dw {dp_dw:+.1} Pa, worst dT_pool {dt:.2e} K, level {:.4} m, downcomer {:.3e} kg/s, VB {:.3e} kg/s, iterations <= {it}",
        r.pool_level_m, r.downcomer_kg_s, r.vacuum_breaker_kg_s
    );
    let (r2, dp2, dp_dw2, dt2, it2) = run_relief(NetworkConfig::unit1(), on);
    println!("A2 torus: dp_ww {dp2:+.1} Pa, dp_dw {dp_dw2:+.1} Pa, worst dT_pool {dt2:.2e} K, level {:.4} m, iterations <= {it2}", r2.pool_level_m);
    let (_, dp3, _, dt3, _) = run_relief(prismatic, NetworkMechanisms { pressure_work: false, ..on });
    println!("A4 prismatic without pressure work: dp_ww {dp3:+.1} Pa, worst dT_pool {dt3:.2e} K");
    let (_, dp4, _, dt4, _) = run_relief(prismatic, NetworkMechanisms { real_steam: false, real_liquid: false, ..on });
    println!("A-extra prismatic with sembawang's ideal steam and constant c_p: dp_ww {dp4:+.1} Pa, worst dT_pool {dt4:.2e} K");
    let (_, dp6, _, dt6, _) = run_relief(prismatic, NetworkMechanisms { real_liquid: false, ..on });
    println!("A-extra prismatic with sembawang's constant c_p only: dp_ww {dp6:+.1} Pa, worst dT_pool {dt6:.2e} K");
    let (_, dp5, _, _, _) = run_relief(prismatic, NetworkMechanisms { level_swell: false, ..on });
    println!("A-extra prismatic without level swell: dp_ww {dp5:+.1} Pa");
    assert!(dp > 250.0 && dp < 450.0, "A1 {dp}");
    // Prediction 1's 2e-3 K MISSED (2.87e-3 K): the network's IF97 liquid
    // (the wet well ran with sembawang's constant c_p); with constant c_p
    // the worst is 4.2e-5 K. Pinned after the run, labelled so:
    assert!((dt - 2.87e-3).abs() < 0.1e-3, "A1 {dt}");
    assert!(dt4 < 1e-4 && dt6 < 1e-4, "A1 ablated {dt4} {dt6}");
    assert!(dp2 > 250.0 && dp2 < 500.0, "A2 {dp2}");
    assert!(dt2 < 1e-2, "A2 {dt2}");
    assert!(r.downcomer_kg_s.abs() < 1e-9 && r.vacuum_breaker_kg_s.abs() < 1e-9, "A3 {r:?}");
    assert!(dp_dw.abs() < 200.0 && dp_dw2.abs() < 200.0, "A3 {dp_dw} {dp_dw2}");
    // Prediction 4 MISSED (+280.7 Pa without the pressure work): the pool's
    // rise compresses the gas geometrically whatever the energy term; the
    // pressure work is only its heating. Pinned after the run, labelled so:
    assert!((dp3 - 280.7).abs() < 5.0, "A4 {dp3}");
    assert!(it < 30 && it2 < 30, "A5 {it} {it2}");
}

/// `FUKUSHIMA_OLD_REPLAY=no_fp_heat;no_pool_layers;no_fp_heat,no_pool_layers`:
/// each `;`-separated variant replays the recorded snapshots, open loop,
/// into a fresh old `Containment` with those terms removed from the
/// snapshot (`no_fp_heat`: the `q_*_w` decay heat of the fission products
/// the containment holds; `no_pool_layers`: one well-mixed pool), so that
/// a network−old difference is attributed to one `Containment` term with
/// the plant's drive held fixed. Returns `(name, [(p_dw, p_ww, T_pool,
/// T_dw)])` per variant; the empty variant is the plain replay, which must
/// reproduce the recorded results exactly.
fn old_replays(snaps: &[ContainmentSnapshot]) -> Vec<(String, Vec<(f64, f64, f64, f64)>)> {
    let spec = std::env::var("FUKUSHIMA_OLD_REPLAY").unwrap_or_default();
    if spec.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    for v in std::iter::once("").chain(spec.split(';')) {
        let flags: Vec<&str> = v.split(',').map(str::trim).filter(|x| !x.is_empty()).collect();
        for f in &flags {
            assert!(["no_fp_heat", "no_pool_layers"].contains(f), "unknown replay term {f}");
        }
        let mut c = Containment::new(&snaps[0].prm);
        let rows = snaps
            .iter()
            .map(|s| {
                let mut s = s.clone();
                if flags.contains(&"no_fp_heat") {
                    s.q_noble_w = 0.0;
                    s.q_alkali_w = 0.0;
                    s.q_halogen_w = 0.0;
                    s.q_chalcogen_w = 0.0;
                }
                if flags.contains(&"no_pool_layers") {
                    s.pool_layers = false;
                }
                let r = c.step(&s);
                (r.p_dw_pa, r.p_ww_pa, r.t_pool_k, r.t_dw_k)
            })
            .collect();
        out.push((if v.is_empty() { "plain".to_string() } else { v.to_string() }, rows));
    }
    out
}

/// The old model's containment snapshots over `steps` steps of the
/// historical sequence, and its results.
fn record(steps: usize) -> (Vec<ContainmentSnapshot>, Vec<sembawang::bwr_mark1::containment::ContainmentResult>) {
    let mut plant = Plant::with_mechanisms(Mode::Historical, old_ablations_from_env());
    let (mut core, mut cont) = plant.parts();
    let (mut snaps, mut results) = (Vec::with_capacity(steps), Vec::with_capacity(steps));
    for _ in 0..steps {
        let s = plant.begin();
        let c = core.step(&s.core);
        let k = cont.step(&s.containment);
        snaps.push(s.containment.clone());
        results.push(k.clone());
        plant.finish(c, k);
    }
    (snaps, results)
}

/// **B (predictions 6–8).** The first `FUKUSHIMA_NETWORK_HOURS` hours (6
/// by default) of the historical sequence. The results below are gh:#1097's
/// mapping, reproduced exactly by `FUKUSHIMA_NETWORK_ABLATE=cavity,dw_slab`
/// (prediction 11, held); the default's (gh:#1100) are pinned below and
/// tabled in the V&V doc. **Results (2026-11-01):** all
/// three MISSED. To 4.5 h within 4.4 % (the network higher, by the pool's
/// displacement of the torus gas, not lower by stratification as
/// predicted); one step of +60 % in `p_dw` at 4.956 h; 6 h worst |dp_ww|
/// 9.78 %, |dT_pool| 1.32 K; 3.08 ms a step over 6 h and 7.78 ms over the
/// whole 30 h (840 s, sub-steps from 4.9 h). The 30 h table is in the V&V
/// doc.
#[test]
fn the_first_hours_of_unit_1_against_the_old_containment() {
    let hours: f64 = std::env::var("FUKUSHIMA_NETWORK_HOURS").ok().and_then(|x| x.parse().ok()).unwrap_or(6.0);
    let steps = (hours * 3600.0) as usize;
    let (snaps, old) = record(steps);
    let replays = old_replays(&snaps);
    if let Some((_, plain)) = replays.first() {
        // The open-loop replay is the recorded old model, exactly.
        for (o, r) in old.iter().zip(plain) {
            assert!(o.p_dw_pa == r.0 && o.p_ww_pa == r.1 && o.t_pool_k == r.2, "the plain replay departs from the recording");
        }
    }
    let prm = snaps[0].prm;
    let mech = ablations_from_env();
    let mut net = ContainmentNetwork::with_config(&prm, NetworkConfig::unit1(), mech).unwrap();
    let start = std::time::Instant::now();
    let mut worst = (0.0_f64, 0.0_f64, 0.0_f64);
    let mut most_iter = 0;
    let mut worst_at = (0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
    // `FUKUSHIMA_NETWORK_TRACE=a,b` prints every step between hours a and b.
    let trace: Option<(f64, f64)> = std::env::var("FUKUSHIMA_NETWORK_TRACE").ok().and_then(|x| {
        let v: Vec<f64> = x.split(',').filter_map(|y| y.parse().ok()).collect();
        (v.len() == 2).then(|| (v[0], v[1]))
    });
    for (i, (s, o)) in snaps.iter().zip(&old).enumerate() {
        let r = match net.step(s) {
            Ok(r) => r,
            Err(e) => panic!("network failed at {:.3} h: {e}", i as f64 / 3600.0),
        };
        most_iter = most_iter.max(r.iterations);
        let h = i as f64 / 3600.0;
        if trace.is_some_and(|(a, b)| h >= a && h <= b) {
            println!(
                "trace {h:.5} h: p_dw {:.0}/{:.0} p_ww {:.0}/{:.0} leak {:.1} kg/s h {:.3e} dc {:+.2} vb {:+.2} it {} sub {}",
                r.p_dw_pa, o.p_dw_pa, r.p_ww_pa, o.p_ww_pa, s.leak_steam_kg_s, s.leak_h, r.downcomer_kg_s, r.vacuum_breaker_kg_s, r.iterations, r.substeps
            );
        }
        let rel_dw = (r.p_dw_pa - o.p_dw_pa) / o.p_dw_pa;
        let rel_ww = (r.p_ww_pa - o.p_ww_pa) / o.p_ww_pa;
        if rel_dw.abs() > worst.0 {
            worst_at = (i as f64 / 3600.0, r.p_dw_pa, o.p_dw_pa, s.leak_steam_kg_s, s.ex_vessel_w, r.downcomer_kg_s, r.vacuum_breaker_kg_s);
        }
        worst.0 = worst.0.max(rel_dw.abs());
        worst.1 = worst.1.max(rel_ww.abs());
        worst.2 = worst.2.max((r.t_pool_k - o.t_pool_k).abs());
        if (i + 1) % 1800 == 0 {
            println!(
                "t {:5.2} h | p_dw {:8.0} / {:8.0} Pa ({:+.2} %) | p_ww {:8.0} / {:8.0} ({:+.2} %) | T_dw {:6.1} / {:6.1} K | T_pool {:6.2} / {:6.2} (top {:6.2}) | srv {:5.2} leak {:5.3} kg/s | dc {:+.3} vb {:+.3} kg/s | level {:.3} m | it {} sub {}",
                (i + 1) as f64 / 3600.0,
                r.p_dw_pa,
                o.p_dw_pa,
                100.0 * rel_dw,
                r.p_ww_pa,
                o.p_ww_pa,
                100.0 * rel_ww,
                r.t_dw_k,
                o.t_dw_k,
                r.t_pool_k,
                o.t_pool_k,
                o.t_pool_top_k,
                s.srv_steam_kg_s,
                s.leak_steam_kg_s,
                r.downcomer_kg_s,
                r.vacuum_breaker_kg_s,
                r.pool_level_m,
                r.iterations,
                r.substeps
            );
            println!(
                "      cavity: ablated {:.0} / {:.0} kg, H2 {:.1} / {:.1} kg, CO {:.1} / {:.1} kg, debris {:.0} / {:.0} K, FDI {:.0} / {:.0} kg | wall {:.1} slab {:.1} K | sump {:.0} kg | dw H2 {:.1} / {:.1} kg | flange {} / {} | rb H2 {:.4} / {:.4}",
                r.cav_ablated_kg,
                o.cav_ablated_kg,
                r.cav_h2_kg,
                o.cav_h2_kg,
                r.cav_co_kg,
                o.cav_co_kg,
                r.cav_debris_k,
                o.cav_debris_k,
                r.fdi_steam_kg,
                o.fdi_steam_kg,
                r.dw_wall_k,
                r.dw_slab_inner_k,
                r.dw_sump_kg,
                r.dw_h2_kg,
                o.dw_h2_kg,
                r.flange_leaking,
                o.flange_leaking,
                r.rb_h2_fraction,
                o.rb_h2_fraction
            );
            for (name, rows) in replays.iter().skip(1) {
                let (pd, pw, tp, td) = rows[i];
                println!(
                    "      replay old [{name}]: p_dw {:+.2} % p_ww {:+.2} % (old {:.0} / {:.0} Pa) | T_pool {:.2} / {:.2} K | T_dw {:.1} / {:.1} K",
                    100.0 * (r.p_dw_pa - pd) / pd,
                    100.0 * (r.p_ww_pa - pw) / pw,
                    pd,
                    pw,
                    r.t_pool_k,
                    tp,
                    r.t_dw_k,
                    td
                );
            }
        }
    }
    let el = start.elapsed().as_secs_f64();
    println!(
        "B: {hours} h in {el:.1} s ({:.3} ms a step; 30 h would take {:.0} s); worst |dp_dw| {:.2} %, |dp_ww| {:.2} %, |dT_pool| {:.2} K; iterations <= {most_iter}",
        1e3 * el / steps as f64,
        el / steps as f64 * 108_000.0,
        100.0 * worst.0,
        100.0 * worst.1,
        worst.2
    );
    println!("B: worst p_dw at {:.4} h: {:.0} / {:.0} Pa, leak {:.3} kg/s, ex-vessel {:.3e} W, dc {:+.3} vb {:+.3} kg/s", worst_at.0, worst_at.1, worst_at.2, worst_at.3, worst_at.4, worst_at.5, worst_at.6);
    // Predictions 6 and 7 MISSED (2026-11-01; causes in the V&V doc): over
    // 6 h the worst |dp_dw| is 60.39 % (one step at 4.956 h, the vessel's
    // 2149 kg/s blowdown into the drywell meeting downcomers whose
    // equalising limit is taken from the start of the step), |dp_ww|
    // 9.78 %, |dT_pool| 1.32 K. Pinned after the run, labelled so, for the
    // default 6 h only:
    println!("B: ablations {mech:?}");
    let gh1097 = NetworkMechanisms::default().without_cavity().without_dw_slab();
    let old_default = old_ablations_from_env() == Mechanisms::default();
    if (hours - 6.0).abs() < 1e-9 && mech == gh1097 && old_default {
        assert!((worst.0 - 0.6039).abs() < 0.005 && (worst.1 - 0.0978).abs() < 0.005, "B pins: {worst:?}");
        assert!((worst.2 - 1.32).abs() < 0.05, "B pins: {worst:?}");
    }
    // gh:#1100 (2026-11-01): with the cavity, FDI and the drywell slab
    // coupled (the default), the 6 h worsts moved 60.39 -> 56.29 % (the
    // slab cools the structures before the blowdown), 9.78 -> 13.65 %
    // (the network's torus is now LOW after the debris lands, by the old
    // model's hot surface layer; V&V doc) and 1.32 -> 1.86 K. Pinned after
    // the run, labelled so, at the same widths as the gh:#1097 pins:
    if (hours - 6.0).abs() < 1e-9 && mech == NetworkMechanisms::default() && old_default {
        assert!((worst.0 - 0.5629).abs() < 0.005 && (worst.1 - 0.1365).abs() < 0.005, "B pins (gh:#1100): {worst:?}");
        assert!((worst.2 - 1.86).abs() < 0.05, "B pins (gh:#1100): {worst:?}");
    }
}

