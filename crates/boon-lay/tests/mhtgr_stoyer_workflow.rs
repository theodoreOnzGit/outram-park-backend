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
//! - **REPORTED (#413 check (b), rule fixed in advance): the dense PCHIP
//!   0.1 h resampling of the digitised Fig. 5**
//!   (`fig05_accident_temperature_c_pchip_0p1h.csv`, made by
//!   `dev/mhtgr_stoyer_fig5_pchip.py`). Vent fractions 5 / 20 / 25 / 50 % are
//!   0.416 / 0.465 / 0.534 / 0.610, against the exact ideal-gas 0.423 / 0.469 /
//!   0.539 / 0.618. **Final / paper median 0.90 in both cases**: Case A
//!   0.84-0.94 (volatiles 0.88, Cs 0.86, Sr 0.84, Ag 0.87, constant-D metals
//!   0.91); Case B median 0.90 with outliers Cs-134 0.67, Kr-85 0.68, Ag-111
//!   1.74 (printed `k_plate`). Per nuclide: `final_release_ratio_sparse_vs_pchip.csv`.
//!   The results below are on the sparse points, kept for comparison:
//! - **Accident path (re-digitised Fig. 5, 2026-09-30T01:50:02Z, kovan
//!   066e691): port vs upstream worst relative difference 3.9e-11.**
//! - **Final releases vs the paper: median 0.88** in both cases, at both
//!   plate-out constants. Case A spans 0.86-1.15: the volatiles (I, Te, Xe)
//!   are at 0.97-0.99, the metals at 0.86-0.94, and the two silvers at
//!   1.12-1.15. Case B spans 0.66-1.79, with Cs-134 0.67, Kr-85 0.76 and
//!   Ag-111 1.79 at the printed plate-out constant. Per nuclide:
//!   `final_release_ratio_before_after.csv`.
//! - **Vent fractions** (5 / 20 / 25 / 50 %) went from 0.44 / 0.48 / 0.62 /
//!   0.41 to **0.55** / 0.48 / 0.62 / **0.52** once the t = 0 markers were
//!   added. The 50 % curve now rises monotonically, so its early dip no
//!   longer gaps the vent mask.
//! - ~~On the 2026-09-29 digitisation: median 0.83 (0.80-0.93 in Case A),
//!   attributed to the missing t = 0 markers~~ -- the re-digitisation moved it
//!   to 0.88, which confirms the direction. The remaining ~12 % sits mainly in
//!   the metals and is not diagnosed.
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
use boon_lay::triso_atops_fork::variant::{PaperReadings, TrisoAtopsVariant};
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

// ANCHOR: setup
/// The model variant and nuclide-selection policy a run uses (gh:#1015).
#[derive(Clone, Copy)]
struct Setup {
    variant: TrisoAtopsVariant,
    policy: ParentDecayPolicy,
}

/// Upstream's code and upstream's parent flag: what every pre-#1015 test runs.
const UPSTREAM: Setup = Setup {
    variant: TrisoAtopsVariant::Upstream,
    policy: ParentDecayPolicy::UpstreamTableDefault,
};
// ANCHOR_END: setup

/// [`port_case`] plus the per-node state the accident path starts from.
fn port_case_nodal(
    case: &str,
    inventory_file: &str,
    delta_t: f64,
    k_plate_override: Option<f64>,
) -> (Pools, Nodal) {
    port_case_nodal_with(case, inventory_file, delta_t, k_plate_override, UPSTREAM)
}

/// [`port_case_nodal`] under any [`Setup`].
fn port_case_nodal_with(
    case: &str,
    inventory_file: &str,
    delta_t: f64,
    k_plate_override: Option<f64>,
    setup: Setup,
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
    // ANCHOR: plant_constants
    let plant = PlantConstants::new(
        Frequency::new::<hertz>(c("k_plate")),
        Frequency::new::<hertz>(c("k_clean")),
        Length::new::<meter>(c("a_graph")),
        Length::new::<meter>(c("a_grain")),
        Length::new::<meter>(c("a_SiC")),
        Length::new::<meter>(c("r_kernel")),
        Time::new::<second>(c("run_time") * YEAR_S),
        Time::new::<second>(c("irradiation_time") * YEAR_S),
    )
    .with_variant(setup.variant);
    // ANCHOR_END: plant_constants
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
    // ANCHOR: select_nuclides
    let (selected, skipped) = select_nuclides(
        &names,
        plant.irradiation_time,
        None,
        setup.policy,
    );
    assert!(skipped.is_empty(), "{skipped:?}");
    // ANCHOR_END: select_nuclides

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
fn fig5(file: &str) -> Vec<(String, Vec<(f64, f64)>)> {
    let mut curves: Vec<(String, Vec<(f64, f64)>)> = Vec::new();
    for r in table(file) {
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

/// Upstream `accident_case`'s totals \[Ci\] per nuclide for one Fig. 5
/// curve applied uniformly to every node, as `(hours, series)` over the
/// samples upstream keeps (its vent-mask truncation); callers take the last
/// value for the final release (#413 check (c) uses the whole series), composed from the port's public
/// accident functions in upstream's order -- including upstream's truncation
/// of the temperature field to the FIRST `n_keep` samples when the venting
/// mask is gappy (reproduced, as the code-to-code suite does).
fn port_accident_curve(
    names: &[String],
    nodal: &Nodal,
    curve: &[(f64, f64)],
    c: &dyn Fn(&str) -> f64,
) -> HashMap<String, (Vec<f64>, Vec<f64>)> {
    port_accident_curve_with(names, nodal, curve, c, TrisoAtopsVariant::Upstream)
}

/// [`port_accident_curve`] under any model variant (gh:#1015). Every node
/// carries the same temperature history, so each time sample's release
/// fractions are computed once and applied to every node: the values, and
/// the order they are summed in, are those of the per-node loop this
/// replaced, so upstream results are unchanged.
fn port_accident_curve_with(
    names: &[String],
    nodal: &Nodal,
    curve: &[(f64, f64)],
    c: &dyn Fn(&str) -> f64,
    variant: TrisoAtopsVariant,
) -> HashMap<String, (Vec<f64>, Vec<f64>)> {
    use boon_lay::triso_atops_fork::accident::{
        accident_release_curies, atoms_to_curies, coolant_release, mean_temperature_rate,
        release_activity_for, AccidentFractions, NormalOperationNode, ReleaseMaterial as AccMat,
    };
    use boon_lay::triso_atops_fork::diffusion::{
        integrate_diffusion_over_time_for, DiffusionMaterial,
    };
    use boon_lay::triso_atops_fork::release_models::{
        release_fraction_transient_for, ReleaseMaterial,
    };
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
    let a_grain = Length::new::<meter>(c("a_grain"));
    let t_irrad = Time::new::<second>(c("irradiation_time") * YEAR_S);
    let t_run = Time::new::<second>(c("run_time") * YEAR_S);
    let mut out = HashMap::new();
    for nuc in selected {
        // ANCHOR: accident_release_fractions
        let group = nuc.element_group_for(variant);
        let volatile = matches!(group, ElementGroup::NobleGas | ElementGroup::Halogen);
        let lam = nuc.decay_constant().get::<hertz>();
        let int_k = integrate_diffusion_over_time_for(
            variant,
            nuc.z,
            &vent_times,
            &history,
            DiffusionMaterial::Kernel,
        );
        let int_g = (!volatile).then(|| {
            integrate_diffusion_over_time_for(
                variant,
                nuc.z,
                &vent_times,
                &history,
                DiffusionMaterial::Graphite,
            )
        });
        let rf_k: Vec<f64> = (0..n_keep)
            .map(|t| {
                release_fraction_transient_for(
                    variant,
                    nuc.z,
                    group,
                    int_k[t],
                    r_kernel,
                    Some(a_sic),
                    ReleaseMaterial::Kernel,
                    a_grain,
                )
                .get::<ratio>()
            })
            .collect();
        let rf_g: Option<Vec<f64>> = int_g.as_ref().map(|ig| {
            (0..n_keep)
                .map(|t| {
                    release_fraction_transient_for(
                        variant,
                        nuc.z,
                        group,
                        ig[t],
                        a_graph,
                        None,
                        ReleaseMaterial::Graphite,
                        a_grain,
                    )
                    .get::<ratio>()
                })
                .collect()
        });
        // ANCHOR_END: accident_release_fractions
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
                // ANCHOR: accident_release_activity
                k_sum[t] += atoms_to_curies(
                    release_activity_for(
                        variant,
                        group,
                        fractions,
                        node,
                        rf_k[t],
                        true,
                        AccMat::Kernel,
                        true,
                        nuc.z,
                        t_irrad,
                        t_run,
                    ),
                    lam,
                );
                // ANCHOR_END: accident_release_activity
                if let Some(rg) = &rf_g {
                    g_sum[t] += atoms_to_curies(
                        release_activity_for(
                            variant,
                            group,
                            fractions,
                            node,
                            rg[t],
                            true,
                            AccMat::Graphite,
                            true,
                            nuc.z,
                            t_irrad,
                            t_run,
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
        let hours: Vec<f64> = curve[..n_keep].iter().map(|(t, _)| *t).collect();
        out.insert(nuc.name.to_string(), (hours, series));
    }
    out
}

/// `numpy.interp` at `t` with the value held beyond the last sample.
fn interp_hold(h: &[f64], v: &[f64], t: f64) -> f64 {
    let t = t.min(h[h.len() - 1]);
    let k = h.partition_point(|x| *x <= t).clamp(1, h.len() - 1);
    let (h0, h1) = (h[k - 1], h[k]);
    if h1 == h0 {
        v[k]
    } else {
        v[k - 1] + (v[k] - v[k - 1]) * (t - h0) / (h1 - h0)
    }
}

/// **#413 check (c), part 2 (DIAGNOSTIC): the port's cumulative release vs
/// time equals upstream's**, for the six nuclides of the paper's Figs. 6/7,
/// both cases, dense PCHIP Fig. 5, Eq. (29) at every whole hour 0-140 h
/// (each curve held after upstream's vent-mask truncation).
///
/// Methodology: upstream series from `dev/mhtgr_stoyer_fig67_overlay.py`
/// (`upstream_case_{a,b}_accident_pchip_series.csv`); port series from
/// [`port_accident_curve`]; gate 1e-9 relative (code-to-code). The overlay
/// against the paper's digitised Figs. 6/7 is in
/// `fig67_overlay_ours_vs_paper.csv` and the README; it is reported, not
/// gated, and changes nothing in the reported #413 result.
///
/// Results (2026-09-30): pass; worst relative difference printed.
#[test]
fn accident_series_port_matches_upstream_for_figs_6_and_7() {
    let nucs = ["Kr-85", "Xe-133", "I-131", "Sr-90", "Cs-137", "Ag-110m"];
    let weights = [("5", 0.05), ("20", 0.2), ("25", 0.25), ("50", 0.5)];
    let mut worst = 0.0f64;
    for (case, inv, dt) in CASES {
        let c = constants(case, None);
        let (_, nodal) = port_case_nodal(case, inv, dt, None);
        let names: Vec<String> = table(inv).iter().map(|r| r["nuclide"].clone()).collect();
        let curves: Vec<(String, HashMap<String, (Vec<f64>, Vec<f64>)>)> =
            fig5("fig05_accident_temperature_c_pchip_0p1h.csv")
                .into_iter()
                .map(|(pct, curve)| (pct, port_accident_curve(&names, &nodal, &curve, &c)))
                .collect();
        for row in table(&format!("upstream_case_{case}_accident_pchip_series.csv")) {
            let t = f(&row, "time_h");
            for n in nucs {
                let port: f64 = curves
                    .iter()
                    .map(|(pct, m)| {
                        let w = weights.iter().find(|(p, _)| p == pct).unwrap().1;
                        let (h, v) = &m[n];
                        w * interp_hold(h, v, t)
                    })
                    .sum();
                worst = worst.max(rel(port, f(&row, &format!("{n}_eq29"))));
            }
        }
    }
    println!("port vs upstream release-vs-time series: worst relative difference {worst:.3e}");
    assert!(worst < 1e-9, "{worst:e}");
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
    for (curve_file, tag, label) in [
        (
            "fig05_accident_temperature_c.csv",
            "",
            "sparse digitised points",
        ),
        (
            "fig05_accident_temperature_c_pchip_0p1h.csv",
            "_pchip",
            "PCHIP 0.1 h (REPORTED)",
        ),
    ] {
        for ((case, inv, dt), paper) in CASES.into_iter().zip(papers) {
            for (kp, suffix) in [(None, ""), (Some(7.5e-4), "_diagnostic_kplate_7p5e-4")] {
                let c = constants(case, kp);
                let (pools, nodal) = port_case_nodal(case, inv, dt, kp);
                let names: Vec<String> = table(inv).iter().map(|r| r["nuclide"].clone()).collect();
                let up: HashMap<String, HashMap<String, String>> =
                    table(&format!("upstream_case_{case}{suffix}_accident{tag}.csv"))
                        .into_iter()
                        .map(|r| (r["nuclide"].clone(), r))
                        .collect();
                let mut eq29: HashMap<String, f64> = HashMap::new();
                for (pct, curve) in fig5(curve_file) {
                    let port: HashMap<String, f64> =
                        port_accident_curve(&names, &nodal, &curve, &c)
                            .into_iter()
                            .map(|(n, (_, s))| (n, *s.last().unwrap()))
                            .collect();
                    for (n, v) in &port {
                        let u = f(&up[n], &format!("total_{pct}"));
                        let r = rel(*v, u);
                        if r > worst.0 {
                            worst = (
                                r,
                                format!(
                                    "{label} case {case}{suffix} {pct}% {n}: port {v} upstream {u}"
                                ),
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
                "[{label}] case {}{suffix}: final / paper: median {:.3}, range [{:.3}, {:.3}], within 10 % {}/{}",
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
    }
    println!(
        "accident port vs upstream, worst relative difference {:.3e} ({})",
        worst.0, worst.1
    );
    assert!(worst.0 < 1e-9, "{worst:?}");
}

// ---------------------------------------------------------------------------
// gh:#1015: the `atops2026paper` variant against the paper's printed releases.
// Method, the candidate readings (ranked before any run), predictions and the
// full results: crates/boon-lay/docs/triso-atops-2026-paper-reproduction.md.
// The `// ANCHOR: name` / `// ANCHOR_END: name` comments in this file mark
// the code the deep dive `crates/boon-lay/docs/lessons/src/atops2026-paper.md`
// pulls in with mdBook's {{#include}} at build time: move them with the code.
// ---------------------------------------------------------------------------

// ANCHOR: paper_variant
/// The paper variant with one reading changed from the as-written default.
fn paper_with(change: impl FnOnce(&mut PaperReadings)) -> TrisoAtopsVariant {
    let mut r = PaperReadings::AS_WRITTEN;
    change(&mut r);
    TrisoAtopsVariant::Atops2026Paper(r)
}

/// The paper's reading of parent coupling (A11): short-lived parents only.
const PAPER_POLICY: ParentDecayPolicy = ParentDecayPolicy::ShortLivedParentOnly;
// ANCHOR_END: paper_variant

// ANCHOR: configurations
/// The candidate list of the reproduction doc, s.2, in that order: the
/// as-written paper variant, then each ambiguity moved to its next candidate
/// with every other reading kept as written, then upstream.
fn configurations() -> Vec<(&'static str, Setup)> {
    use boon_lay::triso_atops_fork::variant::{
        AccidentBoothRadius, AccidentInventory, BirthRateTime, BoothRadius,
        LongLivedVolatileRelease, LowTemperatureLaw, OtherMetalRelease, SeriesEvaluation,
        SilverAccidentInventory, YttriumGroup,
    };
    let p = |v| Setup {
        variant: v,
        policy: PAPER_POLICY,
    };
    vec![
        ("atops2026paper (as written)", p(TrisoAtopsVariant::ATOPS2026PAPER)),
        ("A1(2) radius: RMS of diameters", p(paper_with(|r| r.booth_radius = BoothRadius::RmsOfDiameters))),
        ("A1(3) radius: upstream geometric mean", p(paper_with(|r| r.booth_radius = BoothRadius::UpstreamGeometricMean))),
        ("A2(2) accident radius: kernel r", p(paper_with(|r| r.accident_booth_radius = AccidentBoothRadius::KernelRadius))),
        ("A3(2) other metals: R/B 1e-5", p(paper_with(|r| r.other_metals = OtherMetalRelease::UpstreamFixedRb))),
        ("A4(2) long-lived volatiles: Eq. (6a)", p(paper_with(|r| r.long_lived_volatiles = LongLivedVolatileRelease::UpstreamEmpirical))),
        ("A5(2) birth time: t_irrad", p(paper_with(|r| r.birth_rate_time = BirthRateTime::IrradiationTime))),
        ("A6(2) accident inventory: upstream", p(paper_with(|r| r.accident_inventory = AccidentInventory::UpstreamUnnormalised))),
        ("A7(2) series: truncated", p(paper_with(|r| r.series = SeriesEvaluation::UpstreamTruncated))),
        ("A8(2) clamps: upstream's set", p(paper_with(|r| r.low_temperature = LowTemperatureLaw::UpstreamPartialClamp))),
        ("A9(2) yttrium: other", p(paper_with(|r| r.yttrium = YttriumGroup::UpstreamOther))),
        ("A10(2) silver accident: failed only", p(paper_with(|r| r.silver_accident_inventory = SilverAccidentInventory::FailedParticles))),
        ("A11(2) parents: upstream table flag", Setup { variant: TrisoAtopsVariant::ATOPS2026PAPER, policy: ParentDecayPolicy::UpstreamTableDefault }),
        ("A1(3)+A3(2)+A4(2)+A5(2) (radius, other, volatiles, birth time as code)", p(paper_with(|r| {
            r.booth_radius = BoothRadius::UpstreamGeometricMean;
            r.other_metals = OtherMetalRelease::UpstreamFixedRb;
            r.long_lived_volatiles = LongLivedVolatileRelease::UpstreamEmpirical;
            r.birth_rate_time = BirthRateTime::IrradiationTime;
        }))),
        ("upstream (de374c8)", UPSTREAM),
    ]
}
// ANCHOR_END: configurations

/// One case's score against the paper: per pool `(within 2 %, compared)`
/// for graphite, circulating, plate-out, HPS and the initial release; the
/// final release as `(within 10 %, compared, median ours/paper)`; and the
/// per-nuclide initial and final ratios ours/paper.
struct CaseScore {
    pools: [(usize, usize); 5],
    finals: (usize, usize, f64),
    initial_ratio: HashMap<String, f64>,
    final_ratio: HashMap<String, f64>,
}

/// Run one case under `setup` and score it against Tables 9/10 (Case A) or
/// 13/14 (Case B). Finals: the dense PCHIP Fig. 5, Eq. (29), and the paper's
/// building-leakage post-processing, exactly as the #413 test does.
fn score_case(case: &str, inv: &str, dt: f64, kp: Option<f64>, setup: Setup) -> CaseScore {
    let (t_norm, t_acc) = if case == "a" {
        ("table09_case_a_normal_operation_ci.csv", "table10_case_a_accident_release_ci.csv")
    } else {
        ("table13_case_b_normal_operation_ci.csv", "table14_case_b_accident_release_ci.csv")
    };
    let (pools, nodal) = port_case_nodal_with(case, inv, dt, kp, setup);
    let mut score = [(0usize, 0usize); 5];
    for (k, col) in ["graphite_ci", "circulating_ci", "plateout_ci", "hps_ci"].iter().enumerate() {
        for r in table(t_norm) {
            let want = f(&r, col);
            if want > 0.0 {
                score[k].1 += 1;
                score[k].0 += usize::from(rel(pools[&r["nuclide"]][k], want) < 0.02);
            }
        }
    }
    let mut initial_ratio = HashMap::new();
    for r in table(t_acc) {
        let want = f(&r, "initial_release_ci");
        if want > 0.0 {
            score[4].1 += 1;
            score[4].0 += usize::from(rel(pools[&r["nuclide"]][4], want) < 0.02);
            initial_ratio.insert(r["nuclide"].clone(), pools[&r["nuclide"]][4] / want);
        }
    }
    let c = constants(case, kp);
    let names: Vec<String> = table(inv).iter().map(|r| r["nuclide"].clone()).collect();
    let weights = [("5", 0.05), ("20", 0.2), ("25", 0.25), ("50", 0.5)];
    let mut eq29: HashMap<String, f64> = HashMap::new();
    for (pct, curve) in fig5("fig05_accident_temperature_c_pchip_0p1h.csv") {
        let w = weights.iter().find(|(p, _)| *p == pct).unwrap().1;
        for (n, (_, s)) in port_accident_curve_with(&names, &nodal, &curve, &c, setup.variant) {
            *eq29.entry(n).or_insert(0.0) += w * s.last().unwrap();
        }
    }
    let mut final_ratio = HashMap::new();
    for r in table(t_acc) {
        let n = &r["nuclide"];
        let initial = pools[n][4];
        let ours = initial + (eq29[n] - initial) / 10.0;
        final_ratio.insert(n.clone(), ours / f(&r, "final_release_ci_as_printed"));
    }
    let mut sorted: Vec<f64> = final_ratio.values().copied().collect();
    sorted.sort_by(f64::total_cmp);
    let within = sorted.iter().filter(|x| (0.9..1.1).contains(*x)).count();
    CaseScore {
        pools: score,
        finals: (within, sorted.len(), sorted[sorted.len() / 2]),
        initial_ratio,
        final_ratio,
    }
}

// ANCHOR: table19
/// Table 19's complete-failure release fractions under `setup`:
/// `(nuclide, ours, paper)`. One node at 1250 °C (core and graphite, p. 21),
/// `t_irrad = t_run = 3 yr` (p. 21, "run for 3 years"; A13), `f_HM = 1` and
/// every other fraction 0 (Table 17), no HPS (A13), Table 3 geometry,
/// Table 16 inventories (the parent coupling of I-133 to Xe-133 needs them).
/// Release fraction = (circulating + plate-out) / inventory, which for
/// `k_clean = 0` is independent of `k_plate`.
fn table19(setup: Setup) -> Vec<(String, f64, f64)> {
    let rows = table("table19_complete_failure_release_fraction.csv");
    let names: Vec<&str> = rows.iter().map(|r| r["nuclide"].as_str()).collect();
    let three_years = Time::new::<second>(3.0 * YEAR_S);
    let plant = PlantConstants::new(
        Frequency::new::<hertz>(7.5e-4),
        Frequency::new::<hertz>(0.0),
        Length::new::<meter>(4.5e-3),
        Length::new::<meter>(1.0e-5),
        Length::new::<meter>(3.5e-5),
        Length::new::<meter>(2.13e-4),
        three_years,
        three_years,
    )
    .with_variant(setup.variant);
    let fractions = FailureFractions {
        heavy_metal: 1.0,
        sic: 0.0,
        incremental: 0.0,
        incremental_sic: 0.0,
    };
    let t = ThermodynamicTemperature::new::<kelvin>(1250.0 + 273.15);
    let node = NodeState {
        core_temperature: t,
        graphite_temperature: t,
    };
    let (selected, skipped) = select_nuclides(&names, three_years, None, setup.policy);
    assert!(skipped.is_empty(), "{skipped:?}");
    let mut done: HashMap<String, NodalActivities> = HashMap::new();
    let mut out = Vec::new();
    for s in &selected {
        let row = rows.iter().find(|r| r["nuclide"] == s.nuclide.name).unwrap();
        let inv_ci = f(row, "inventory_ci");
        let parent = s
            .parent_decay
            .then(|| done.get(s.nuclide.parents[0]))
            .flatten()
            .map_or(ParentPools::none(), NodalActivities::parent_pools);
        let n = normal_operation_node(
            &s.nuclide,
            s.short_lived,
            becquerels_from_curies(inv_ci),
            fractions,
            plant,
            node,
            false,
            parent,
        );
        let ci = n.to_curies(s.nuclide.decay_constant());
        out.push((
            s.nuclide.name.to_string(),
            (ci.circulating_activity + ci.plate_out_activity) / inv_ci,
            f(row, "release_fraction"),
        ));
        done.insert(s.nuclide.name.to_string(), n);
    }
    out
}
// ANCHOR_END: table19

// ANCHOR: bit_for_bit
/// **gh:#1015: `Atops2026Paper(PaperReadings::UPSTREAM_CODE)` is `Upstream`,
/// bit for bit**, on both MHTGR cases: every normal-operation pool of every
/// nuclide, and every accident total on the sparse Fig. 5 points.
///
/// This pins that the paper variant's switches are the only thing between
/// the two: with every reading set to the code's, nothing else differs.
/// Result (2026-10-22): pass, exact equality.
#[test]
fn paper_variant_with_upstream_readings_is_upstream_bit_for_bit() {
    let all_code = Setup {
        variant: TrisoAtopsVariant::Atops2026Paper(PaperReadings::UPSTREAM_CODE),
        policy: ParentDecayPolicy::UpstreamTableDefault,
    };
    for (case, inv, dt) in CASES {
        let (p_up, n_up) = port_case_nodal_with(case, inv, dt, None, UPSTREAM);
        let (p_pa, n_pa) = port_case_nodal_with(case, inv, dt, None, all_code);
        assert_eq!(p_up, p_pa, "case {case} pools");
        let c = constants(case, None);
        let names: Vec<String> = table(inv).iter().map(|r| r["nuclide"].clone()).collect();
        for (_, curve) in fig5("fig05_accident_temperature_c.csv") {
            let a = port_accident_curve_with(&names, &n_up, &curve, &c, UPSTREAM.variant);
            let b = port_accident_curve_with(&names, &n_pa, &curve, &c, all_code.variant);
            assert_eq!(a, b, "case {case} accident");
        }
    }
}
// ANCHOR_END: bit_for_bit

/// **gh:#1015: the `atops2026paper` variant against the paper's own printed
/// releases, with the sensitivity to every ambiguity.** Reported, not gated.
///
/// ## Methodology
///
/// The paper's three worked cases, with inputs exactly as printed:
///
/// - **Case A** (350 MWth) and **Case B** (600 MWth), normal operation:
///   Tables 9 / 13 (pp. 17-18, 22-23), the four pools of every nuclide, and
///   the initial release of Tables 10 / 14 (pp. 18, 23). A value counts as
///   reproduced within 2 %, the paper's three significant figures. Accident
///   finals: the final column of Tables 10 / 14 from the dense PCHIP
///   resampling of the maintainer's Fig. 5 digitisation, Eq. (29) and the
///   paper's building-leakage reduction (as the #413 test does). Counted
///   within 10 %, which allows for the digitisation (#413 measured a 0.88-0.90
///   median for upstream). Both plate-out constants are run (A12).
/// - **Table 19** (p. 25): the complete-failure release fractions at
///   1250 °C, see [`table19`].
///
/// Configurations, in the order written before any run (the reproduction
/// doc, s.2): the as-written paper variant; each ambiguity moved to its next
/// candidate with the rest as written; a combination of the four
/// code-readings the predictions single out; and upstream.
///
/// ## Results (2026-10-22, 52 s)
///
/// Full tables, the ranked candidates and the decisions are in
/// `crates/boon-lay/docs/triso-atops-2026-paper-reproduction.md`. This test
/// prints them; the run record is
/// `verification_and_validation/mhtgr_stoyer/atops2026paper_reproduction_2026-10-22.txt`.
/// The headline, at `k_plate = 7.5e-4` (printed `7.5e-5` in brackets):
///
/// | | A: G | A: C | A: P | A: HPS | A: initial | A: final ±10 % (median) | T19 |
/// |---|---|---|---|---|---|---|---|
/// | `atops2026paper` | 0/40 | 26/64 (10) | 15/52 (1) | 23/24 (9) | 13/46 (3) | 3/46 (1.21) | 6/8 |
/// | `upstream` | 40/40 | 64/64 (11) | 52/52 (21) | 24/24 (10) | 46/46 (16) | 24/46 (0.90) | 8/8 |
///
/// Case B is the same to within a count or two. Table 19: `upstream` is
/// within 0.3 % on all eight values; `atops2026paper` misses Kr-85 (+62.8 %,
/// Eq. (6c) instead of (6a)) and Sr-90 (−56.1 %, the 150.8 µm radius).
/// Predictions (+60 %, −55 %) hit.
///
/// **Interpretation.** The paper's printed numbers were computed by the
/// upstream code, not by the equations as the paper writes them. No single
/// candidate reading on the pre-written list reproduces them; only the full
/// set of code readings does, and that is `Upstream`.
#[test]
fn atops2026paper_against_the_papers_printed_releases() {
    let header = "| configuration | k_plate | A: G | A: C | A: P | A: HPS | A: initial | A: final ±10 % (median) | B: G | B: C | B: P | B: HPS | B: initial | B: final ±10 % (median) | T19 ±2 % |";
    println!("{header}");
    println!("|{}", "---|".repeat(header.matches('|').count() - 1));
    let key = [
        "Kr-85", "Xe-133", "I-131", "Te-132", "Sr-90", "Y-91", "Cs-137", "Ag-110m", "Zr-95",
        "Ce-144",
    ];
    let mut movement: Vec<(String, String)> = Vec::new();
    for (label, setup) in configurations() {
        let t19 = table19(setup);
        let t19_ok = t19.iter().filter(|(_, o, p)| rel(*o, *p) < 0.02).count();
        for (kp, kp_label) in [(None, "7.5e-5"), (Some(7.5e-4), "7.5e-4")] {
            let mut line = format!("| {label} | {kp_label} |");
            let mut a_score = None;
            for (case, inv, dt) in CASES {
                let s = score_case(case, inv, dt, kp, setup);
                for (ok, n) in s.pools {
                    line += &format!(" {ok}/{n} |");
                }
                line += &format!(" {}/{} ({:.2}) |", s.finals.0, s.finals.1, s.finals.2);
                if case == "a" {
                    a_score = Some(s);
                }
            }
            line += &format!(" {t19_ok}/{} |", t19.len());
            println!("{line}");
            if kp.is_some() {
                let s = a_score.unwrap();
                let cells: Vec<String> = key
                    .iter()
                    .map(|n| {
                        let i = s.initial_ratio.get(*n).map_or("-".into(), |x| format!("{x:.3}"));
                        let fi = s.final_ratio.get(*n).map_or("-".into(), |x| format!("{x:.3}"));
                        format!("{i} / {fi}")
                    })
                    .collect();
                movement.push((label.to_string(), cells.join(" | ")));
            }
        }
    }
    println!();
    println!("Case A, k_plate 7.5e-4: ours/paper, initial / final release");
    println!("| configuration | {} |", key.join(" | "));
    println!("|---|{}", "---|".repeat(key.len()));
    for (label, cells) in &movement {
        println!("| {label} | {cells} |");
    }
    println!();
    println!("Table 19 (p. 25), complete-failure release fraction at 1250 degC");
    println!("| nuclide | paper | upstream | rel. diff | atops2026paper | rel. diff |");
    println!("|---|---|---|---|---|---|");
    // ANCHOR: table19_comparison
    let up = table19(UPSTREAM);
    let pa = table19(Setup {
        variant: TrisoAtopsVariant::ATOPS2026PAPER,
        policy: PAPER_POLICY,
    });
    for ((n, u, p), (_, a, _)) in up.iter().zip(&pa) {
        println!(
            "| {n} | {p:.3e} | {u:.3e} | {:+.1} % | {a:.3e} | {:+.1} % |",
            100.0 * (u / p - 1.0),
            100.0 * (a / p - 1.0)
        );
    }
    // ANCHOR_END: table19_comparison
}
