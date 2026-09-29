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
//! **Not covered here: the FINAL (heat-up) releases** of Tables 10 and 14.
//! They need the four accident temperature curves of **Fig. 5** (pdf p. 16,
//! printed p. 15), which exist only as a plotted figure and must be
//! digitised before a run (requested on gh:#413). The paper's own
//! post-processing of those -- final releases "reduced by an order of
//! magnitude" for 10 %/day building leakage (s.III.A.5) -- will be applied
//! then, flagged.
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

/// The port's normal-operation workflow for one case.
fn port_case(
    case: &str,
    inventory_file: &str,
    delta_t: f64,
    k_plate_override: Option<f64>,
) -> Pools {
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
        for (ax, t_row) in temps.iter().enumerate() {
            for (ring, t) in t_row.iter().enumerate() {
                let node = NodeState {
                    core_temperature: ThermodynamicTemperature::new::<kelvin>(*t),
                    graphite_temperature: ThermodynamicTemperature::new::<kelvin>(*t),
                };
                let inventory = becquerels_from_curies(
                    f(row, &format!("ring{}_ci", ring + 1)) / N_AXIAL as f64,
                );
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
        nodal.insert(s.nuclide.name.to_string(), nodes);
    }
    out
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
