//! End-to-end **Monte Carlo criticality** demo driven by the outram-blender
//! `sim` backend — the "author geometry → set up → run outram-mc" path the GUI
//! wraps. Offline demonstration only (education / research / V&V).
//!
//! Two runs of the **Godiva** bare-HEU-sphere benchmark (ICSBEP
//! HEU-MET-FAST-001, r = 8.7407 cm, k_eff = 1.0000 ± 0.0010):
//!
//! 1. `SimGeometry::BareSphere` — the validated `run_keff` driver.
//! 2. **Authored**: a uv-sphere primitive built in outram-blender, exported to
//!    outram-mc CSG via `csg_from_mesh`, run with `run_keff_csg`. This exercises
//!    the whole authoring → export → simulate bridge.
//!
//! Run with (the `mc-export` feature pulls in outram-mc-libs):
//! ```text
//! cargo run -p outram-blender --example mc_godiva_keff --features mc-export --release
//! ```
//!
//! # V&V — methodology and results
//!
//! **Methodology.** Godiva bare HEU sphere (r = 8.7407 cm, HEU-MET-FAST-001 atom
//! densities), 3000 histories × [25 inactive + 75 active], multithreaded, judged
//! against the ICSBEP benchmark k_eff = 1.0000 ± 0.0010. Cross sections are the
//! embedded offline LOW tier (`Nuclide::from_core`). Two independent paths:
//! the `run_keff` bare-sphere driver, and the authoring bridge (uv-sphere
//! primitive → `csg_from_mesh` → `run_keff_csg`).
//!
//! **Results (2026-07, embedded LOW-tier data).**
//!
//! | Path | k_eff | Δk vs benchmark |
//! |---|---|---|
//! | `SimGeometry::BareSphere` (`run_keff`) | 1.01153 ± 0.00277 | +1153 pcm |
//! | Authored uv-sphere → CSG (`run_keff_csg`) | 1.01592 ± 0.00259 | +1592 pcm |
//!
//! **Interpretation.** Both paths land near unity, validating the backend
//! pipeline end to end (material building → geometry export → k-eigenvalue run).
//! The ~+1150 pcm bias of the bare path is the embedded LOW-tier data's known
//! offset (no self-shielding; see outram-mc-libs `godiva_keff`), **not** a
//! backend error. The authored-CSG path sits ~440 pcm higher — within
//! expectation for the different source-convergence / leakage treatment of the
//! CSG driver at this history count; the geometry itself (analytic Sphere,
//! r = 8.7407) is exact. This is a *pipeline* validation, not a claim that the
//! embedded data reproduces the benchmark to within its uncertainty.

#[cfg(not(feature = "mc-export"))]
fn main() {
    eprintln!("rebuild with `--features mc-export` to run the Monte Carlo backend demo");
}

#[cfg(feature = "mc-export")]
fn main() {
    use outram_blender::primitives::uv_sphere;
    use outram_blender::sim::{
        csg_from_mesh, ComputeType, KeffSettings, MaterialSpec, McSimSetup, SimGeometry, ThreadCount,
    };

    // Godiva HEU material, atom densities [atoms/barn·cm] (HEU-MET-FAST-001).
    let heu = MaterialSpec {
        name: "Godiva HEU".into(),
        temperature_k: 293.6,
        nuclides: vec![
            ("U234".into(), 4.9184e-4),
            ("U235".into(), 4.4994e-2),
            ("U238".into(), 2.4984e-3),
        ],
    };
    let radius_cm = 8.7407;

    let settings = KeffSettings {
        n_particles: 3000,
        n_inactive: 25,
        n_active: 75,
        compute: ComputeType::CpuMultiThread(ThreadCount::Auto),
        ..Default::default()
    };

    println!("Godiva bare-sphere criticality  (r = {radius_cm} cm)");
    println!("  {} histories/gen × [{} inactive + {} active]\n", settings.n_particles, settings.n_inactive, settings.n_active);

    // ── 1. Bare-sphere driver. ────────────────────────────────────────────────
    let bare = McSimSetup {
        geometry: SimGeometry::BareSphere { radius_cm },
        materials: vec![heu.clone()],
        settings: settings.clone(),
    };
    let r1 = bare.run().expect("bare-sphere run");
    println!("[1] BareSphere driver     k_eff = {:.5} ± {:.5}  ({:+.0} pcm)", r1.k_mean, r1.k_std, (r1.k_mean - 1.0) * 1e5);

    // ── 2. Authored uv-sphere → CSG export → run_keff_csg. ────────────────────
    let sphere_mesh = uv_sphere(32, 16, radius_cm);
    let geometry = csg_from_mesh(&sphere_mesh, 0).expect("sphere exports to CSG");
    let authored = McSimSetup { geometry, materials: vec![heu], settings };
    let r2 = authored.run().expect("authored CSG run");
    println!("[2] Authored uv-sphere→CSG k_eff = {:.5} ± {:.5}  ({:+.0} pcm)", r2.k_mean, r2.k_std, (r2.k_mean - 1.0) * 1e5);

    println!("\n  ICSBEP HEU-MET-FAST-001 benchmark = 1.0000 ± 0.0010");
    println!("  (embedded LOW-tier data carries a small known bias; see outram-mc-libs godiva_keff)");
}
