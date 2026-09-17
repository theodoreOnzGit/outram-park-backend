//! Can the GNN surrogate reproduce DEM — and how much faster is it?
//!
//! ```bash
//! cargo run --release -p outram-park-fork-liggghts \
//!     --features gnn-burn --example gnn_vs_dem_surrogate
//! ```
//!
//! Needs `reference-data/liggghts/` checked out. Requires the `gnn-burn`
//! feature, which is **off by default** because it pulls a tensor stack.
//!
//! # The experiment
//!
//! **Small scale, where DEM is cheap enough to be ground truth.** Take the
//! settled 354-pebble bed, give every pebble the same small downward velocity
//! kick, and let it relax. That is a deliberate choice of regime: it keeps the
//! contact network essentially fixed (displacements stay far below a pebble
//! radius) while the velocities evolve non-trivially, which is the only regime
//! where the comparison is both fair and meaningful — see "The structural
//! catch" below.
//!
//! The network is trained to map each node's velocity at step `t` to its
//! velocity at `t+1`, on the DEM trajectory, over the contact graph built once
//! at `t = 0`. It is then rolled out autoregressively from `v(0)` and compared
//! against the DEM trajectory it never saw.
//!
//! **Large scale.** The same forward pass on the full HTR-10 bed (27 564
//! pebbles), timed against a DEM step.
//!
//! # Baselines, which are the point
//!
//! A relaxing bed's velocities decay toward zero, so a surrogate that predicts
//! "zero" or "no change" scores well while having learned nothing. Any claim
//! that the GNN matches DEM is worthless unless it beats those, so both are
//! measured alongside it:
//!
//! - **persistence** — predict `v(t+1) = v(t)`;
//! - **zero** — predict `v(t+1) = 0`.
//!
//! # Results (measured 2026-09-17)
//!
//! Machine: Intel Xeon @ 2.80 GHz, 4 cores, 15 GB RAM; `--release`. Network:
//! 32 hidden, 2 layers, 2 message-passing steps, 400 epochs, Adam at 0.005, on
//! a single 120-step trajectory. `burn`'s `ndarray` CPU backend.
//!
//! **Training fits; the rollout does not.** Loss fell `1.299e-1 -> 1.883e-7`,
//! seven orders. Then, against the DEM trajectory it never saw:
//!
//! | horizon | GNN | persistence | GNN / persistence |
//! |---|---|---|---|
//! | 1 step | `2.48e-4` | `1.86e-4` | **1.33** |
//! | 5 | `1.10e-3` | `7.76e-4` | 1.41 |
//! | 20 | `5.73e-3` | `2.64e-3` | 2.17 |
//! | 60 | `8.81e-2` | `5.61e-3` | 15.7 |
//! | 120 | `1.68e0` | `8.14e-3` | **206** |
//!
//! Over the full 120 steps the signal's own RMS is `2.594e-2 m/s`; the GNN is
//! at **6476 % of signal**, persistence at 31 %, predict-zero at 100 %. **The
//! surrogate beats neither baseline.**
//!
//! The horizon sweep is the load-bearing part. At one step the GNN is in the
//! right ballpark, so the plumbing is sound and this is not a wiring bug — it
//! is super-exponential compounding, the rollout feeding on its own error,
//! which this module's own upstream docs warn about. Under-reaching is *not*
//! the cause: `reach_bound` reports 1 hop required against 4 configured.
//!
//! **And it is slower, at both scales.**
//!
//! | | GNN | DEM | |
//! |---|---|---|---|
//! | 354 pebbles | 2.98 ms/step | 0.43 ms/step | 7x slower |
//! | HTR-10, 27 564 | 0.181 s/forward | 0.055 s/step | 3.3x slower |
//!
//! # What this does and does not show
//!
//! It does **not** show that GNN surrogates cannot work for DEM. Three things
//! about this configuration matter more than the method:
//!
//! - the network is minimal and untuned, trained on **one** trajectory with
//!   **no noise injection and no training-time rollout** — precisely the
//!   devices MeshGraphNet uses to stabilise autoregressive rollouts. Their
//!   absence makes divergence expected. The accuracy result is plausibly
//!   fixable; it is simply not fixed here.
//! - it runs on `burn`'s **ndarray CPU backend**. Surrogates earn their speed
//!   on GPUs; against an O(N) cell-list DEM step on four CPU cores there is no
//!   headroom to win. The speed result is a property of this backend.
//! - the regime is the mild one the fixed-graph constraint forces (below).
//!
//! What it does show is that **as configured, on this backend, the surrogate is
//! both less accurate and slower than simply running the DEM** — so nothing in
//! this crate should be replaced by it on the strength of the graph bridge
//! existing.
//!
//! # The structural catch, stated before the numbers
//!
//! `raffles::gnn::training::train` takes **one fixed graph** for every sample.
//! A DEM contact network is dynamic, so the two only line up where the topology
//! barely changes. That is the regime this example uses — and it is also the
//! regime where the dynamics are mildest. A pour, a collapse or a discharge,
//! where a surrogate would actually earn its keep, breaks the fixed-graph
//! assumption and is **not** covered here.

#[cfg(not(feature = "gnn-burn"))]
fn main() {
    eprintln!(
        "this example needs the `gnn-burn` feature:\n  cargo run --release -p \
         outram-park-fork-liggghts --features gnn-burn --example gnn_vs_dem_surrogate"
    );
}

#[cfg(feature = "gnn-burn")]
fn main() {
    run::main();
}

#[cfg(feature = "gnn-burn")]
mod run {
    use outram_park_fork_liggghts::boundary::Boundary;
    use outram_park_fork_liggghts::gnn_bridge;
    use outram_park_fork_liggghts::granular::{GranularContactModel, GranularMaterial, RollingModel};
    use outram_park_fork_liggghts::granular_system::GranularSystem;
    use outram_park_fork_liggghts::particle::{Particle, Vec3};
    use raffles::gnn::mpnn::MessagePassingNet;
    use raffles::gnn::training::{self, MpnnTrainingConfig};
    use uom::si::f64::{Length, Mass, ThermodynamicTemperature};
    use uom::si::{length::meter, mass::kilogram, thermodynamic_temperature::kelvin};

    // `Autodiff<B>` is itself a `Backend`, so the same network trains and rolls
    // out — no separate inference type is needed. f32 matches raffles' own
    // backend choice.
    type Backend = burn::backend::Autodiff<burn::backend::NdArray<f32>>;

    fn load(name: &str, radius: f64, density: f64) -> Option<Vec<Particle>> {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../reference-data/liggghts")
            .join(name);
        let text = std::fs::read_to_string(path).ok()?;
        let mass = density * 4.0 / 3.0 * std::f64::consts::PI * radius.powi(3);
        let mut rows: Vec<(usize, Particle)> = text
            .lines()
            .skip(1)
            .filter(|l| !l.trim().is_empty())
            .map(|l| {
                let f: Vec<f64> = l.split(',').map(|v| v.parse().unwrap()).collect();
                (
                    f[0] as usize,
                    Particle::new(
                        Vec3::new(f[1], f[2], f[3]),
                        Vec3::new(f[4], f[5], f[6]),
                        Vec3::zero(),
                        Mass::new::<kilogram>(mass),
                        Length::new::<meter>(radius),
                        ThermodynamicTemperature::new::<kelvin>(300.0),
                    )
                    .unwrap(),
                )
            })
            .collect();
        rows.sort_by_key(|r| r.0);
        Some(rows.into_iter().map(|r| r.1).collect())
    }

    /// RMS over every node and step of a trajectory difference `[m/s]`.
    fn rms(a: &[Vec<Vec<f64>>], b: &[Vec<Vec<f64>>]) -> f64 {
        let mut acc = 0.0;
        let mut n = 0usize;
        for (fa, fb) in a.iter().zip(b.iter()) {
            for (na, nb) in fa.iter().zip(fb.iter()) {
                for (x, y) in na.iter().zip(nb.iter()) {
                    acc += (x - y) * (x - y);
                    n += 1;
                }
            }
        }
        (acc / n as f64).sqrt()
    }

    pub fn main() {
        const R: f64 = 0.005;
        const RHO: f64 = 2500.0;
        const DT: f64 = 1.0e-5;
        const STEPS: usize = 120;
        const KICK: f64 = -0.05;

        let Some(mut ps) = load("pebble_bed_settled.csv", R, RHO) else {
            eprintln!("skipping: reference-data/liggghts/ not present");
            return;
        };
        for p in &mut ps {
            p.velocity = Vec3::new(0.0, 0.0, KICK);
        }
        println!(
            "small-scale bed: {} pebbles, dt = {DT:.0e} s, {STEPS} steps",
            ps.len()
        );

        let material = GranularMaterial::new(5.0e6, 0.3, 0.5, 0.3).unwrap();
        let model = GranularContactModel::hertz_history(material)
            .with_rolling(RollingModel::cdt(0.1).unwrap());
        let bounds = vec![
            Boundary::cylinder(Vec3::zero(), Vec3::new(0.0, 0.0, 1.0), 0.030).unwrap(),
            Boundary::wall(Vec3::zero(), Vec3::new(0.0, 0.0, 1.0)).unwrap(),
        ];

        // --- DEM ground truth -------------------------------------------------
        let mut sys =
            GranularSystem::new(ps.clone(), bounds, model, Vec3::new(0.0, 0.0, -9.81), DT).unwrap();
        let graph = gnn_bridge::contact_graph(sys.particles(), 0.0).unwrap();
        println!(
            "contact graph: {} nodes, {} directed edges",
            graph.node_count(),
            graph.edge_count()
        );

        let snap = |s: &GranularSystem| -> Vec<Vec<f64>> {
            s.particles()
                .iter()
                .map(|p| vec![p.velocity.x, p.velocity.y, p.velocity.z])
                .collect()
        };
        let t_dem = std::time::Instant::now();
        let mut truth = vec![snap(&sys)];
        for _ in 0..STEPS {
            sys.step();
            truth.push(snap(&sys));
        }
        let dem_secs = t_dem.elapsed().as_secs_f64();
        println!(
            "DEM: {STEPS} steps in {dem_secs:.4} s ({:.4} ms/step)",
            1e3 * dem_secs / STEPS as f64
        );

        // --- train ------------------------------------------------------------
        // TARGETS ARE INCREMENTS, not absolute states: `train`/`rollout` are
        // written for a model that predicts the change over one step and
        // `rollout` does the addition itself (raffles' training module docs,
        // "Predicting the increment, not the state"). Handing it absolute
        // states would make the rollout add each state to itself.
        let inputs: Vec<Vec<Vec<f64>>> = truth[..STEPS].to_vec();
        let targets: Vec<Vec<Vec<f64>>> = truth
            .windows(2)
            .map(|w| {
                w[0].iter()
                    .zip(w[1].iter())
                    .map(|(a, b)| a.iter().zip(b.iter()).map(|(x, y)| y - x).collect())
                    .collect()
            })
            .collect();
        let device = Default::default();
        let net: MessagePassingNet<Backend> = MessagePassingNet::new(3, 3, 32, 2, 2, &device);
        let config = MpnnTrainingConfig {
            epochs: 400,
            learning_rate: 0.005,
        };
        let t_train = std::time::Instant::now();
        let (trained, report) =
            training::train(net, &graph, &inputs, &targets, &config, &device).unwrap();
        println!(
            "trained {} epochs in {:.1} s: loss {:.3e} -> {:.3e}",
            config.epochs,
            t_train.elapsed().as_secs_f64(),
            report.initial_loss(),
            report.final_loss()
        );

        // --- rollout vs DEM, against the baselines ----------------------------
        let t_roll = std::time::Instant::now();
        let predicted = training::rollout(&trained, &graph, &truth[0], STEPS, &device).unwrap();
        let roll_secs = t_roll.elapsed().as_secs_f64();

        let persistence: Vec<Vec<Vec<f64>>> = (0..=STEPS).map(|_| truth[0].clone()).collect();
        let zeros: Vec<Vec<Vec<f64>>> = (0..=STEPS)
            .map(|_| vec![vec![0.0; 3]; truth[0].len()])
            .collect();

        // Error as a function of rollout length. A one-step prediction can be
        // accurate while a long rollout diverges, because the rollout feeds the
        // network its own errors (raffles' own docs say exactly this). Reporting
        // only the 120-step number would confuse "the surrogate is inaccurate"
        // with "the plumbing is wrong" -- if even 1 step is far off, the fault is
        // mine, not the method's.
        println!("\n--- rollout error vs horizon (separates instability from a bug) ---");
        for h in [1usize, 5, 20, 60, STEPS] {
            let p = training::rollout(&trained, &graph, &truth[0], h, &device).unwrap();
            let e = rms(&p, &truth[..=h]);
            let base = rms(&vec![truth[0].clone(); h + 1], &truth[..=h]);
            println!(
                "  {h:>4} steps: GNN {e:.4e}   persistence {base:.4e}   GNN/persistence {:.2}",
                e / base
            );
        }

        let e_gnn = rms(&predicted, &truth);
        let e_pers = rms(&persistence, &truth);
        let e_zero = rms(&zeros, &truth);
        let scale = rms(&truth, &zeros); // RMS of the signal itself

        println!("\n--- small scale: rollout error vs DEM over {STEPS} steps ---");
        println!("signal RMS                 {scale:.4e} m/s");
        println!(
            "GNN surrogate              {e_gnn:.4e} m/s   ({:.1} % of signal)",
            100.0 * e_gnn / scale
        );
        println!(
            "baseline: persistence      {e_pers:.4e} m/s   ({:.1} %)",
            100.0 * e_pers / scale
        );
        println!(
            "baseline: predict zero     {e_zero:.4e} m/s   ({:.1} %)",
            100.0 * e_zero / scale
        );
        println!(
            "GNN beats persistence: {}   GNN beats zero: {}",
            e_gnn < e_pers,
            e_gnn < e_zero
        );
        println!(
            "rollout {STEPS} steps in {roll_secs:.4} s ({:.4} ms/step) vs DEM {:.4} ms/step -> {:.2}x",
            1e3 * roll_secs / STEPS as f64,
            1e3 * dem_secs / STEPS as f64,
            dem_secs / roll_secs
        );

        // --- large scale: forward-pass cost on the HTR-10 bed -----------------
        let Some(big) = load("htr10_settled.csv", 0.03, 1730.0) else {
            println!("\n(large scale skipped: htr10_settled.csv not present)");
            return;
        };
        println!("\n--- large scale: HTR-10, {} pebbles ---", big.len());
        let t = std::time::Instant::now();
        let big_graph = gnn_bridge::contact_graph(&big, 0.0).unwrap();
        println!(
            "contact_graph: {:.3} s ({} edges)",
            t.elapsed().as_secs_f64(),
            big_graph.edge_count()
        );
        let big_net: MessagePassingNet<Backend> = MessagePassingNet::new(3, 3, 32, 2, 2, &device);
        let initial: Vec<Vec<f64>> = big
            .iter()
            .map(|p| vec![p.velocity.x, p.velocity.y, p.velocity.z])
            .collect();
        let t = std::time::Instant::now();
        let _ = training::rollout(&big_net, &big_graph, &initial, 1, &device).unwrap();
        println!(
            "one GNN forward pass on 27 564 nodes: {:.4} s",
            t.elapsed().as_secs_f64()
        );
        println!(
            "NOTE: this is SPEED ONLY. The large-scale network is untrained, so \
             nothing here says the surrogate is accurate at this scale."
        );
    }
}
