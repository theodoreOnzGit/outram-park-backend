// SPDX-License-Identifier: GPL-3.0
//
// MHTGR end-to-end workflow check of the TRISO-ATOPS fork against Stoyer et
// al. (gh:#413).

//! # V&V (gh:#413): the TRISO-ATOPS fork, end to end, on Stoyer et al.'s MHTGR cases
//!
//! **Source.** B. Stoyer, A. Raichart et al., "TRISO-ATOPS: A Mechanistic
//! Source Term Model for Gas-Cooled Reactors", *Nuclear Technology* (2026),
//! section III (proprietary tier: its values are committed as a cited table in
//! `verification_and_validation/mhtgr_stoyer/`, the PDF is not).
//!
//! ## Methodology
//!
//! Three results for the same inputs, per nuclide and per pool:
//!
//! 1. **the port** (this test): the paper's normal-operation workflow composed
//!    from the fork's public API exactly as upstream's `trisoatops.normal_operation`
//!    composes it -- per node `normal_operation_node` over the 14 x 3 grid of
//!    Table 7, inventories per radial section (Tables 6 / 11) spread evenly over
//!    the 14 axial nodes, parents before daughters with parent pools taken at
//!    the same node, the HPS on, summed and converted to Ci; nuclide selection
//!    with [`ParentDecayPolicy::UpstreamTableDefault`] (upstream's behaviour,
//!    for a code-to-code comparison);
//! 2. **upstream** INL TRISO-ATOPS Python at de374c8, on the same inputs
//!    (`dev/mhtgr_stoyer_upstream.py`, output committed as
//!    `upstream_case_{a,b}*.csv`);
//! 3. **the paper**, Tables 9 and 13 (graphite, circulating, plate-out and HPS
//!    activities) and the *initial* release column of Tables 10 and 14 (=
//!    circulating + `x_liftoff` x plate-out, upstream's t = 0 term).
//!
//! Inputs: Tables 2/3 (Case A) and 12 (Case B) via `constants.csv`; Table 7
//! temperatures (graphite = fuel, as the paper assumes; Case B + 250 K);
//! Tables 6/11 inventories.
//!
//! **Pass criteria.** Port vs upstream: every pool within **1e-9 relative**
//! (the port is a translation; the only differences are the degC/K round
//! trip and summation order). Port vs paper: **reported, not gated** --
//! the question is whether the paper's printed inputs reproduce its printed
//! outputs.
//!
//! ## Final releases (Tables 10 and 14, final column)
//!
//! **Fig. 5** -- four accident temperature curves -- is the maintainer's
//! manual kovan digitisation (`fig05_accident_temperature_c.csv`, provenance
//! in its header and the README). Each curve is applied **uniformly to every
//! node** and run through upstream's `accident_case` path (port:
//! [`port_accident_curve`]; upstream: `dev/mhtgr_stoyer_upstream.py`), and the
//! last total of each run is combined by the paper's **Eq. (29)**, weights
//! 0.05 / 0.2 / 0.25 / 0.5. The paper then reduces the final releases "by an
//! order of magnitude" for 10 %/day building leakage, "applied to releases
//! following the initial breach" (s.III.A.5): **final = initial + (Eq.29 -
//! initial) / 10**, where initial = circulating + x_liftoff x plate-out.
//! Replicated as stated and FLAGGED: it is the paper's post-processing, not
//! TRISO-ATOPS. Two digitisation points at slightly negative time (-0.36 h,
//! -0.54 h) are clamped to t = 0.
//!
//! ## Results (2026-09-29)
//!
//! - **Port vs upstream: worst relative difference 5.6e-12** over every pool
//!   of every nuclide, both cases, both plate-out constants (Case A Eu-155
//!   circulating) -- f64 summation order and the degC/K round trip.
//! - **Upstream (and so the port) vs the paper, with Table 3 AS PRINTED
//!   (`k_plate = 7.50E-05 /s`):** graphite 40/40 within 2 % in both cases,
//!   but only 11/64 circulating, 21/52 plate-out and 10/24 HPS values;
//!   circulating runs ~10x high for long-lived metals, HPS ~5x for halogens.
//! - **With `k_plate = 7.5e-4 /s`** -- upstream's own GUI and manual default
//!   (`trisoatops_gui.py:300`), a diagnostic run, not a new input -- **every**
//!   printed value is reproduced within 2 % (the paper's 3 significant
//!   figures) in both cases: graphite 40/40, circulating 64/64, plate-out
//!   52/52, HPS 24/24, initial release 46/46.
//!
//! - **Accident path, port vs upstream: worst relative difference 2.2e-11**
//!   over every curve's last total, both cases, both plate-out constants.
//! - **Final releases vs the paper: a near-uniform ~0.83** (Case A median
//!   0.830, range 0.80-0.93; Case B median 0.831, range 0.65-1.75 with
//!   Ag-111 1.75 and Cs-134 0.65 the outliers at printed `k_plate`; 0.64-0.90
//!   at 7.5e-4). Nearly every nuclide sits at 0.80-0.93 whatever its
//!   chemistry, so the factor is common to all of them -- the **venting
//!   fraction** (breathing, Eq. 27), which multiplies every fuel and graphite
//!   release and depends on each curve's starting temperature. The rendered
//!   Fig. 5 shows a t = 0 marker on the 5 % curve (~800 degC) and on the 50 %
//!   curve (~200 degC) that the digitisation does not contain (its first
//!   points are 923 degC at 0.32 h and 326 degC at 1.35 h), and its early
//!   points are non-monotonic in places (digitisation noise), which makes
//!   the upstream venting mask gappy. The venting fractions this gives are
//!   0.44 / 0.48 / 0.62 / 0.41 (5 / 20 / 25 / 50 %). **Not corrected here**:
//!   adding points would be a second digitisation; requested from the
//!   maintainer on gh:#413. The plate-out constant does not matter for the
//!   final releases (both runs agree to a few % except Ag-111 and Cs-136).
//!
//! **Interpretation.** The workflow, the fork and the upstream code agree
//! with the published tables once the plate-out constant is the upstream
//! default; the paper's Table 3 entry `7.50E-05` is, on this evidence, a
//! misprint for `7.50E-04` (a factor-of-ten; the paper's own s.II.A.4 basis,
//! 1 % per coolant cycle, is not enough to settle it without the MHTGR loop
//! time). Recorded on gh:#413 for the authors' confirmation; the committed
//! inputs keep Table 3 as printed.

use std::collections::HashMap;
use std::path::Path;

use boon_lay::triso_atops_fork::activities::{becquerels_from_curies, FailureFractions};
use boon_lay::triso_atops_fork::normal_operation::{
    normal_operation_node, NodalActivities, NodeState, ParentPools, PlantConstants,
};
use boon_lay::triso_atops_fork::run_selection::{select_nuclides, ParentDecayPolicy};
use uom::si::f64::{Frequency, Length, ThermodynamicTemperature, Time};
use uom::si::frequency::hertz;
use uom::si::length::meter;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;

const DIR: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/verification_and_validation/mhtgr_stoyer"
);
/// Upstream's 365-day year (`convert_time("yr")`).
const YEAR_S: f64 = 365.0 * 24.0 * 3600.0;
const N_AXIAL: usize = 14;
const N_RADIAL: usize = 3;

/// A cited CSV as header -> column, one map per row, `#` lines skipped.
fn table(name: &str) -> Vec<HashMap<String, String>> {
    let text = std::fs::read_to_string(Path::new(DIR).join(name)).expect(name);
    let mut lines = text.lines().filter(|l| !l.starts_with('#'));
    let header: Vec<String> = lines
        .next()
        .unwrap()
        .split(',')
        .map(str::to_string)
        .collect();
    lines
        .map(|l| {
            header
                .iter()
                .cloned()
                .zip(l.split(',').map(str::to_string))
                .collect()
        })
        .collect()
}

fn f(row: &HashMap<String, String>, key: &str) -> f64 {
    row[key]
        .parse()
        .unwrap_or_else(|_| panic!("{key}: {}", row[key]))
}

/// Per nuclide `[graphite, circulating, plate-out, HPS, initial release]` in Ci.
type Pools = HashMap<String, [f64; 5]>;

/// Per nuclide, per node (axial-major, `ax * N_RADIAL + ring`): the port's
/// normal-operation pools (effective atoms) and the node inventory \[Ci\].
type Nodal = HashMap<String, Vec<(NodalActivities, f64)>>;

/// The port's normal-operation workflow for one case.
fn port_case(
    case: &str,
    inventory_file: &str,
    delta_t: f64,
    k_plate_override: Option<f64>,
) -> Pools {
    port_case_nodal(case, inventory_file, delta_t, k_plate_override).0
}

/// [`port_case`] plus the per-node state the accident path starts from.
fn port_case_nodal(
    case: &str,
    inventory_file: &str,
    delta_t: f64,
    k_plate_override: Option<f64>,
) -> (Pools, Nodal) {
    let consts: HashMap<String, HashMap<String, String>> = table("constants.csv")
        .into_iter()
        .map(|r| (r["key"].clone(), r))
        .collect();
    let c = |k: &str| -> f64 {
        if k == "k_plate" {
            if let Some(kp) = k_plate_override {
                return kp;
            }
        }
        f(&consts[k], &format!("case_{case}"))
    };
    let plant = PlantConstants {
        k_plate: Frequency::new::<hertz>(c("k_plate")),
        k_clean: Frequency::new::<hertz>(c("k_clean")),
        graphite_thickness: Length::new::<meter>(c("a_graph")),
        grain_size: Length::new::<meter>(c("a_grain")),
        sic_thickness: Length::new::<meter>(c("a_SiC")),
        kernel_radius: Length::new::<meter>(c("r_kernel")),
        run_time: Time::new::<second>(c("run_time") * YEAR_S),
        irradiation_time: Time::new::<second>(c("irradiation_time") * YEAR_S),
    };
    let fractions = FailureFractions {
        heavy_metal: c("f_hm"),
        sic: c("f_sic"),
        incremental: c("f_inc"),
        incremental_sic: c("f_inc_sic"),
    };
    let temps: Vec<[f64; N_RADIAL]> = table("table07_core_temperature_k.csv")
        .iter()
        .map(|r| std::array::from_fn(|i| f(r, &format!("ring{}_k", i + 1)) + delta_t))
        .collect();
    assert_eq!(temps.len(), N_AXIAL);
    let inventories = table(inventory_file);
    let names: Vec<&str> = inventories.iter().map(|r| r["nuclide"].as_str()).collect();
    let (selected, skipped) = select_nuclides(
        &names,
        plant.irradiation_time,
        None,
        ParentDecayPolicy::UpstreamTableDefault,
    );
    assert!(skipped.is_empty(), "{skipped:?}");

    let mut nodal: HashMap<String, Vec<NodalActivities>> = HashMap::new();
    let mut per_node = Nodal::new();
    let mut out = Pools::new();
    for s in &selected {
        let row = inventories
            .iter()
            .find(|r| r["nuclide"] == s.nuclide.name)
            .expect("inventory row");
        let parent = s
            .parent_decay
            .then(|| nodal.get(s.nuclide.parents[0]))
            .flatten()
            .cloned();
        let mut nodes = Vec::with_capacity(N_AXIAL * N_RADIAL);
        let mut inv_ci = Vec::with_capacity(N_AXIAL * N_RADIAL);
        for (ax, t_row) in temps.iter().enumerate() {
            for (ring, t) in t_row.iter().enumerate() {
                let node = NodeState {
                    core_temperature: ThermodynamicTemperature::new::<kelvin>(*t),
                    graphite_temperature: ThermodynamicTemperature::new::<kelvin>(*t),
                };
                let node_ci = f(row, &format!("ring{}_ci", ring + 1)) / N_AXIAL as f64;
                inv_ci.push(node_ci);
                let inventory = becquerels_from_curies(node_ci);
                let pools = parent.as_ref().map_or(ParentPools::none(), |p| {
                    p[ax * N_RADIAL + ring].parent_pools()
                });
                nodes.push(normal_operation_node(
                    &s.nuclide,
                    s.short_lived,
                    inventory,
                    fractions,
                    plant,
                    node,
                    true,
                    pools,
                ));
            }
        }
        let lam = s.nuclide.decay_constant();
        let sum = |g: fn(&NodalActivities) -> f64| -> f64 {
            nodes
                .iter()
                .map(|n| n.to_curies(lam))
                .map(|c| {
                    g(&NodalActivities {
                        release_rate: c.release_rate,
                        source_rate: c.source_rate,
                        graphite_activity: c.graphite_activity,
                        circulating_activity: c.circulating_activity,
                        plate_out_activity: c.plate_out_activity,
                        clean_up_activity: c.clean_up_activity,
                    })
                })
                .sum()
        };
        let g = sum(|n| n.graphite_activity);
        let circ = sum(|n| n.circulating_activity);
        let p = sum(|n| n.plate_out_activity);
        let hps = sum(|n| n.clean_up_activity);
        out.insert(
            s.nuclide.name.to_string(),
            [g, circ, p, hps, circ + c("x_liftoff") * p],
        );
        per_node.insert(
            s.nuclide.name.to_string(),
            nodes.iter().copied().zip(inv_ci).collect(),
        );
        nodal.insert(s.nuclide.name.to_string(), nodes);
    }
    (out, per_node)
}

fn upstream(file: &str) -> Pools {
    table(file)
        .into_iter()
        .map(|r| {
            (
                r["nuclide"].clone(),
                [
                    f(&r, "graphite_ci"),
                    f(&r, "circulating_ci"),
                    f(&r, "plateout_ci"),
                    f(&r, "hps_ci"),
                    f(&r, "initial_release_ci"),
                ],
            )
        })
        .collect()
}

fn rel(a: f64, b: f64) -> f64 {
    if a == b {
        0.0
    } else {
        (a - b).abs() / a.abs().max(b.abs())
    }
}

const CASES: [(&str, &str, f64); 2] = [
    ("a", "table06_case_a_inventory_ci.csv", 0.0),
    ("b", "table11_case_b_inventory_ci.csv", 250.0),
];

/// **Port vs upstream, both cases, both plate-out constants: within 1e-9.**
#[test]
fn port_matches_upstream_on_both_mhtgr_cases() {
    let mut worst = (0.0f64, String::new());
    for (case, inv, dt) in CASES {
        for (kp, suffix) in [(None, ""), (Some(7.5e-4), "_diagnostic_kplate_7p5e-4")] {
            let port = port_case(case, inv, dt, kp);
            let up = upstream(&format!("upstream_case_{case}{suffix}.csv"));
            assert_eq!(port.len(), up.len(), "case {case}{suffix}: nuclide count");
            for (name, u) in &up {
                let p = port
                    .get(name)
                    .unwrap_or_else(|| panic!("{name} missing from the port"));
                for k in 0..5 {
                    let r = rel(p[k], u[k]);
                    if r > worst.0 {
                        worst = (
                            r,
                            format!(
                                "case {case}{suffix} {name} column {k}: port {} upstream {}",
                                p[k], u[k]
                            ),
                        );
                    }
                }
            }
        }
    }
    println!(
        "port vs upstream, worst relative difference {:.3e} ({})",
        worst.0, worst.1
    );
    assert!(worst.0 < 1e-9, "{worst:?}");
}

/// **Port vs the paper, reported per pool** (not gated; see the module doc).
#[test]
fn port_against_the_paper_tables_is_reported() {
    let papers = [
        (
            "a",
            "table09_case_a_normal_operation_ci.csv",
            "table10_case_a_accident_release_ci.csv",
        ),
        (
            "b",
            "table13_case_b_normal_operation_ci.csv",
            "table14_case_b_accident_release_ci.csv",
        ),
    ];
    for ((case, inv, dt), (_, t_norm, t_acc)) in CASES.into_iter().zip(papers) {
        for (kp, label) in [
            (None, "Table 3 as printed (k_plate 7.5e-5)"),
            (Some(7.5e-4), "diagnostic k_plate 7.5e-4 (upstream default)"),
        ] {
            let port = port_case(case, inv, dt, kp);
            let mut line = format!("case {} [{label}]:", case.to_uppercase());
            for (k, col) in ["graphite_ci", "circulating_ci", "plateout_ci", "hps_ci"]
                .iter()
                .enumerate()
            {
                let (mut n, mut ok) = (0, 0);
                for r in table(t_norm) {
                    let want = f(&r, col);
                    if want > 0.0 {
                        n += 1;
                        ok += usize::from(rel(port[&r["nuclide"]][k], want) < 0.02);
                    }
                }
                line += &format!(" {col} {ok}/{n};");
            }
            let (mut n, mut ok) = (0, 0);
            for r in table(t_acc) {
                let want = f(&r, "initial_release_ci");
                if want > 0.0 {
                    n += 1;
                    ok += usize::from(rel(port[&r["nuclide"]][4], want) < 0.02);
                }
            }
            println!("{line} initial release {ok}/{n} (within 2 %)");
        }
    }
}

/// The Fig. 5 curves (the maintainer's digitisation), negative-time noise
/// clamped to t = 0, keyed by the core percentage.
fn fig5() -> Vec<(String, Vec<(f64, f64)>)> {
    let mut curves: Vec<(String, Vec<(f64, f64)>)> = Vec::new();
    for r in table("fig05_accident_temperature_c.csv") {
        let p = r["core_fraction_percent"].clone();
        let pt = (f(&r, "time_h").max(0.0), f(&r, "temperature_c"));
        match curves.iter_mut().find(|(q, _)| *q == p) {
            Some((_, v)) => v.push(pt),
            None => curves.push((p, vec![pt])),
        }
    }
    curves
}

/// Case constants as a lookup, with the diagnostic `k_plate` override.
fn constants(case: &str, k_plate_override: Option<f64>) -> impl Fn(&str) -> f64 {
    let consts: HashMap<String, HashMap<String, String>> = table("constants.csv")
        .into_iter()
        .map(|r| (r["key"].clone(), r))
        .collect();
    let case = case.to_string();
    move |k: &str| {
        if k == "k_plate" {
            if let Some(kp) = k_plate_override {
                return kp;
            }
        }
        f(&consts[k], &format!("case_{case}"))
    }
}

/// Upstream `accident_case`'s last total \[Ci\] per nuclide for one Fig. 5
/// curve applied uniformly to every node, composed from the port's public
/// accident functions in upstream's order -- including upstream's truncation
/// of the temperature field to the FIRST `n_keep` samples when the venting
/// mask is gappy (reproduced, as the code-to-code suite does).
fn port_accident_curve(
    names: &[String],
    nodal: &Nodal,
    curve: &[(f64, f64)],
    c: &dyn Fn(&str) -> f64,
) -> HashMap<String, f64> {
    use boon_lay::triso_atops_fork::accident::{
        accident_release_curies, atoms_to_curies, coolant_release, mean_temperature_rate,
        release_activity, AccidentFractions, NormalOperationNode, ReleaseMaterial as AccMat,
    };
    use boon_lay::triso_atops_fork::diffusion::{integrate_diffusion_over_time, DiffusionMaterial};
    use boon_lay::triso_atops_fork::release_models::{release_fraction_transient, ReleaseMaterial};
    use boon_lay::triso_atops_fork::run_selection::select_nuclides_accident;
    use boon_lay::triso_atops_fork::ElementGroup;
    use uom::si::f64::Pressure;
    use uom::si::pressure::kilopascal;
    use uom::si::ratio::ratio;
    use uom::si::thermodynamic_temperature::degree_celsius;

    let times: Vec<Time> = curve
        .iter()
        .map(|(t, _)| Time::new::<second>(t * 3600.0))
        .collect();
    let temps: Vec<ThermodynamicTemperature> = curve
        .iter()
        .map(|(_, t)| ThermodynamicTemperature::new::<degree_celsius>(*t))
        .collect();
    let nodes: Vec<Vec<ThermodynamicTemperature>> = vec![temps.clone(); N_AXIAL * N_RADIAL];
    let rate = mean_temperature_rate(&times, &nodes);
    let (vent, vent_times) =
        coolant_release(&times, &rate, &temps, Pressure::new::<kilopascal>(101.325));
    let n_keep = vent.len();
    let history: Vec<ThermodynamicTemperature> = temps[..n_keep].to_vec();
    let t_end = times[times.len() - 1];
    let refs: Vec<&str> = names.iter().map(String::as_str).collect();
    let (selected, _) = select_nuclides_accident(&refs, t_end, None);
    let fractions = AccidentFractions {
        heavy_metal: c("f_hm"),
        sic: c("f_sic"),
        incremental: c("f_inc"),
        incremental_sic: c("f_inc_sic"),
        incremental_accident: c("f_inc_acc"),
        incremental_sic_accident: c("f_inc_sic_acc"),
    };
    let r_kernel = Length::new::<meter>(c("r_kernel"));
    let a_sic = Length::new::<meter>(c("a_SiC"));
    let a_graph = Length::new::<meter>(c("a_graph"));
    let mut out = HashMap::new();
    for nuc in selected {
        let group = nuc.element_group();
        let volatile = matches!(group, ElementGroup::NobleGas | ElementGroup::Halogen);
        let lam = nuc.decay_constant().get::<hertz>();
        let int_k =
            integrate_diffusion_over_time(nuc.z, &vent_times, &history, DiffusionMaterial::Kernel);
        let int_g = (!volatile).then(|| {
            integrate_diffusion_over_time(nuc.z, &vent_times, &history, DiffusionMaterial::Graphite)
        });
        let (mut k_sum, mut g_sum) = (vec![0.0; n_keep], vec![0.0; n_keep]);
        let (mut circ, mut plate) = (0.0, 0.0);
        for (act, inv_ci) in &nodal[nuc.name] {
            let node = NormalOperationNode {
                kernel_inventory_atoms: inv_ci / lam * 3.7e10,
                release_rate: act.release_rate,
                source_rate: act.source_rate,
                graphite_activity: act.graphite_activity,
                circulating_activity: act.circulating_activity,
                plate_out_activity: act.plate_out_activity,
                clean_up_activity: act.clean_up_activity,
            };
            circ += act.circulating_activity;
            plate += act.plate_out_activity;
            for t in 0..n_keep {
                let rf_k = release_fraction_transient(
                    nuc.z,
                    group,
                    int_k[t],
                    r_kernel,
                    Some(a_sic),
                    ReleaseMaterial::Kernel,
                )
                .get::<ratio>();
                k_sum[t] += atoms_to_curies(
                    release_activity(
                        group,
                        fractions,
                        node,
                        rf_k,
                        true,
                        AccMat::Kernel,
                        true,
                        nuc.z,
                    ),
                    lam,
                );
                if let Some(ig) = &int_g {
                    let rf_g = release_fraction_transient(
                        nuc.z,
                        group,
                        ig[t],
                        a_graph,
                        None,
                        ReleaseMaterial::Graphite,
                    )
                    .get::<ratio>();
                    g_sum[t] += atoms_to_curies(
                        release_activity(
                            group,
                            fractions,
                            node,
                            rf_g,
                            true,
                            AccMat::Graphite,
                            true,
                            nuc.z,
                        ),
                        lam,
                    );
                }
            }
        }
        let series = accident_release_curies(
            &k_sum,
            &g_sum,
            &vent,
            atoms_to_curies(circ, lam),
            atoms_to_curies(plate, lam),
            c("x_liftoff"),
        );
        out.insert(nuc.name.to_string(), *series.last().unwrap());
    }
    out
}

/// **The accident (final) path: port vs upstream per Fig. 5 curve, and the
/// Eq. (29) combination vs the paper's Tables 10/14 final column.**
///
/// Methodology: in the module doc's "Final releases" section. Port vs
/// upstream is gated at 1e-9 relative on every curve's last total (the
/// code-to-code tolerance). Against the paper, reported per nuclide, not gated.
#[test]
fn accident_final_releases_port_upstream_and_paper() {
    let papers = [
        "table10_case_a_accident_release_ci.csv",
        "table14_case_b_accident_release_ci.csv",
    ];
    let weights = [("5", 0.05), ("20", 0.2), ("25", 0.25), ("50", 0.5)];
    let mut worst = (0.0f64, String::new());
    for ((case, inv, dt), paper) in CASES.into_iter().zip(papers) {
        for (kp, suffix) in [(None, ""), (Some(7.5e-4), "_diagnostic_kplate_7p5e-4")] {
            let c = constants(case, kp);
            let (pools, nodal) = port_case_nodal(case, inv, dt, kp);
            let names: Vec<String> = table(inv).iter().map(|r| r["nuclide"].clone()).collect();
            let up: HashMap<String, HashMap<String, String>> =
                table(&format!("upstream_case_{case}{suffix}_accident.csv"))
                    .into_iter()
                    .map(|r| (r["nuclide"].clone(), r))
                    .collect();
            let mut eq29: HashMap<String, f64> = HashMap::new();
            for (pct, curve) in fig5() {
                let port = port_accident_curve(&names, &nodal, &curve, &c);
                for (n, v) in &port {
                    let u = f(&up[n], &format!("total_{pct}"));
                    let r = rel(*v, u);
                    if r > worst.0 {
                        worst = (
                            r,
                            format!("case {case}{suffix} {pct}% {n}: port {v} upstream {u}"),
                        );
                    }
                    let w = weights.iter().find(|(p, _)| *p == pct).unwrap().1;
                    *eq29.entry(n.clone()).or_insert(0.0) += w * v;
                }
            }
            let mut ratios = Vec::new();
            for r in table(paper) {
                let n = &r["nuclide"];
                let initial = pools[n][4];
                let final_as_paper = initial + (eq29[n] - initial) / 10.0;
                ratios.push((
                    n.clone(),
                    final_as_paper / f(&r, "final_release_ci_as_printed"),
                ));
            }
            let mut sorted: Vec<f64> = ratios.iter().map(|(_, x)| *x).collect();
            sorted.sort_by(f64::total_cmp);
            println!(
                "case {}{suffix}: final / paper: median {:.3}, range [{:.3}, {:.3}], within 10 % {}/{}",
                case.to_uppercase(),
                sorted[sorted.len() / 2],
                sorted[0],
                sorted[sorted.len() - 1],
                sorted.iter().filter(|x| (0.9..1.1).contains(*x)).count(),
                sorted.len()
            );
            println!(
                "   {}",
                ratios
                    .iter()
                    .map(|(n, x)| format!("{n} {x:.3}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
    }
    println!(
        "accident port vs upstream, worst relative difference {:.3e} ({})",
        worst.0, worst.1
    );
    assert!(worst.0 < 1e-9, "{worst:?}");
}
