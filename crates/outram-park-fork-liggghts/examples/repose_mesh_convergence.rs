//! **Does mesh resolution explain the 2.65 deg angle-of-repose gap against LIGGGHTS?**
//!
//! `bn` — filed 2026-09-18. NEW WORK, no upstream counterpart.
//!
//! # The claim being tested
//!
//! `docs/verification-and-validation.md` records the lifting-cylinder case as
//! the **one discordant measurement** in an otherwise near-exact port: this
//! crate gives `12.78 deg` against LIGGGHTS' `15.43 deg`, where the
//! primitive-wall cases agree to 1–3 ulp and the 27,554-pebble settled bed
//! agrees to four decimals.
//!
//! That record states a cause, and states it **before the run rather than
//! after** — which is what makes it a hypothesis and not a rationalisation:
//!
//! > LIGGGHTS' `TriMesh` resolves a particle touching several facets at a
//! > shared edge, while this crate's `MeshWall` takes only the **single
//! > nearest facet**.
//!
//! It has never been measured. This example measures it.
//!
//! # Why the case is saturated, and the number that shows it
//!
//! The reference cylinder is `r = 0.05 m` tessellated into **80 circumferential
//! segments**, so each facet is `2*pi*r/80 = 3.927 mm` wide. The pebbles are
//! `10 mm` across. **A wall-pressed pebble therefore spans ~2.55 facets**, so
//! essentially *every* wall contact in this case straddles at least one edge —
//! precisely where single-nearest-facet is wrong. The approximation is not
//! being lightly exercised here; it is being exercised everywhere at once.
//!
//! # The prediction, recorded BEFORE measuring
//!
//! If single-facet resolution is the cause, the error must scale with **how
//! many edges a pebble straddles**, which is set by tessellation:
//!
//! - **Refining** the mesh (more, narrower facets) means MORE straddled edges,
//!   so more missed contacts, less support, and a **FLATTER heap — a LOWER
//!   angle**.
//! - **Coarsening** it toward `facet width ~ pebble diameter` means most
//!   contacts land on a single facet, where the approximation is exact, so the
//!   angle should **RISE toward LIGGGHTS' 15.43 deg**.
//! - LIGGGHTS, which resolves multi-facet contact, should be comparatively
//!   **mesh-insensitive**. (Not measured here — this example runs this crate
//!   only. Establishing that half needs upstream re-run at each resolution.)
//!
//! So the signature of the stated cause is a **monotonic rise in angle as the
//! mesh is coarsened**. A flat sweep REFUTES it, and would mean the gap lives
//! somewhere else — the contact law, the rolling model, or the heap-geometry
//! measurement itself (note the apex also differs, by 6.2 %).
//!
//! # What this cannot show
//!
//! Agreement at some resolution would **not** make the port correct: it would
//! mean the port is correct only where its approximation happens to hold.
//! Neither code is validated against experiment — real granular materials
//! repose at 25–35 deg and BOTH codes sit near 13–15 deg, so this whole family
//! of numbers is a code-to-code comparison, not a physical result.
//!
//! # Running
//!
//! ```bash
//! cargo run --release -p outram-park-fork-liggghts --example repose_mesh_convergence -- <stl-path>
//! ```
//!
//! One resolution per invocation, so the sweep parallelises across processes.
//! Each run integrates 1.1 M steps over 656 pebbles (~38 min measured).

use outram_park_fork_liggghts::boundary::Boundary;
use outram_park_fork_liggghts::granular::{GranularContactModel, GranularMaterial, RollingModel};
use outram_park_fork_liggghts::granular_system::GranularSystem;
use outram_park_fork_liggghts::mesh_wall::{MeshWall, MovingBoundary, WallGeometry};
use outram_park_fork_liggghts::particle::{Particle, Vec3};
use uom::si::f64::{Length, Mass, ThermodynamicTemperature};
use uom::si::{length::meter, mass::kilogram, thermodynamic_temperature::kelvin};

const R_P: f64 = 0.005;
const RHO: f64 = 2500.0;
const DT: f64 = 5.0e-6;
const LIFT_SPEED: f64 = 0.02;

fn repo_data(name: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../reference-data/liggghts")
        .join(name)
}

/// Load an `id,x,y,z,vx,vy,vz` state file, sorted by id.
///
/// **Column 0 is the particle ID, not `x`.** Reading `f[0..3]` as the position
/// yields `(id, x, y)` with ids running to 656, which scatters the pebbles
/// metres apart. That does not crash and does not error: the run completes
/// ~8x faster (no contacts to resolve) and reports an empty heap as
/// "profile too sparse". It was caught only because the REFERENCE mesh was
/// run as a control and failed to reproduce the recorded 12.78 deg — a sweep
/// without that control would have read five sparse profiles as a physics
/// result. Sorting by id matches `tests/angle_of_repose.rs` so the initial
/// state is identical.
fn load_state(path: &std::path::Path) -> Option<Vec<(Vec3, Vec3)>> {
    let text = std::fs::read_to_string(path).ok()?;
    let mut rows: Vec<(usize, Vec3, Vec3)> = text
        .lines()
        .skip(1)
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let f: Vec<f64> = l
                .split(',')
                .map(|v| v.trim().parse().expect("numeric csv"))
                .collect();
            (
                f[0] as usize,
                Vec3::new(f[1], f[2], f[3]),
                Vec3::new(f[4], f[5], f[6]),
            )
        })
        .collect();
    rows.sort_by_key(|r| r.0);
    (!rows.is_empty()).then(|| rows.into_iter().map(|r| (r.1, r.2)).collect())
}

/// Angle of repose by least squares on the radial height profile.
///
/// Identical binning and fit to `tests/angle_of_repose.rs` so the numbers are
/// directly comparable to the recorded `12.78 deg`; duplicated rather than
/// shared because a test is not a library.
fn repose_angle(centres: &[Vec3], min_count: usize) -> Option<(f64, f64, f64)> {
    use std::collections::HashMap;
    let w = 2.0 * R_P;
    let mut bins: HashMap<i64, (f64, usize)> = HashMap::new();
    for c in centres {
        let b = ((c.x * c.x + c.y * c.y).sqrt() / w) as i64;
        let e = bins.entry(b).or_insert((f64::MIN, 0));
        e.0 = e.0.max(c.z + R_P);
        e.1 += 1;
    }
    let mut prof: Vec<(f64, f64, usize)> = bins
        .into_iter()
        .map(|(b, (h, n))| ((b as f64 + 0.5) * w, h, n))
        .collect();
    prof.sort_by(|a, b| a.0.partial_cmp(&b.0).expect("finite radius"));
    let pts: Vec<(f64, f64)> = prof
        .iter()
        .filter(|(_, h, n)| *n >= min_count && *h > 2.5 * R_P)
        .map(|(r, h, _)| (*r, *h))
        .collect();
    if pts.len() < 3 {
        return None;
    }
    let n = pts.len() as f64;
    let sx: f64 = pts.iter().map(|p| p.0).sum();
    let sy: f64 = pts.iter().map(|p| p.1).sum();
    let sxx: f64 = pts.iter().map(|p| p.0 * p.0).sum();
    let sxy: f64 = pts.iter().map(|p| p.0 * p.1).sum();
    let slope = (n * sxy - sx * sy) / (n * sxx - sx * sx);
    let apex = prof.first().map(|p| p.1).unwrap_or(0.0);
    let outer = pts.last().expect("non-empty").0;
    Some((slope.abs().atan().to_degrees(), apex, outer))
}

fn main() {
    let stl_path = match std::env::args().nth(1) {
        Some(p) => std::path::PathBuf::from(p),
        None => {
            eprintln!("usage: repose_mesh_convergence <cylinder.stl>");
            return;
        }
    };
    let Ok(stl) = std::fs::read_to_string(&stl_path) else {
        eprintln!("cannot read {}", stl_path.display());
        return;
    };
    let n_facets = stl.matches("facet normal").count();
    let Some(init) = load_state(&repo_data("lift_init.csv")) else {
        eprintln!("skipping: reference-data/liggghts/lift_init.csv not present");
        return;
    };
    // Guard the load: every pebble must start inside the 0.05 m cylinder.
    // This is the check that would have caught the id-as-x defect immediately
    // instead of after five 5-minute runs.
    let r_max = init
        .iter()
        .map(|(x, _)| (x.x * x.x + x.y * x.y).sqrt())
        .fold(0.0_f64, f64::max);
    assert!(
        r_max < 0.06,
        "initial state is not inside the cylinder (max radius {r_max:.3} m) -- \
         check the CSV column order; column 0 is the particle ID, not x"
    );
    let mesh = MeshWall::from_ascii_stl(&stl).expect("valid cylinder stl");

    let mass = RHO * 4.0 / 3.0 * std::f64::consts::PI * R_P.powi(3);
    let particles: Vec<Particle> = init
        .iter()
        .map(|(x, v)| {
            Particle::new(
                *x,
                *v,
                Vec3::zero(),
                Mass::new::<kilogram>(mass),
                Length::new::<meter>(R_P),
                ThermodynamicTemperature::new::<kelvin>(300.0),
            )
            .expect("valid pebble")
        })
        .collect();

    // Identical to tests/angle_of_repose.rs: same material, same lift, same
    // rest. ONLY the mesh differs, which is what makes the sweep a controlled
    // experiment rather than four separate runs.
    let material = GranularMaterial::new(5.0e6, 0.3, 0.5, 0.5).expect("valid material");
    let model = GranularContactModel::hertz_history(material)
        .with_rolling(RollingModel::cdt(0.1).expect("valid mu_r"));
    let floor = Boundary::wall(Vec3::zero(), Vec3::new(0.0, 0.0, 1.0)).expect("valid floor");
    let cylinder = MovingBoundary::new(
        WallGeometry::Mesh(mesh),
        Vec3::new(0.0, 0.0, LIFT_SPEED),
        Vec3::zero(),
        Vec3::zero(),
    );
    let mut sys = GranularSystem::new(
        particles,
        vec![floor],
        model,
        Vec3::new(0.0, 0.0, -9.81),
        DT,
    )
    .expect("valid system")
    .with_moving_walls(vec![cylinder]);

    let t0 = std::time::Instant::now();
    sys.run(800_000); // lift 4.0 s
    sys.moving_walls_mut()[0].velocity = Vec3::zero();
    sys.run(300_000); // rest 1.5 s
    let secs = t0.elapsed().as_secs_f64();

    let ours: Vec<Vec3> = sys.particles().iter().map(|p| p.position).collect();

    // Dump the final heap so any re-analysis -- a different `min_count`, a
    // different flank definition -- can be done WITHOUT re-integrating 1.1 M
    // steps. Re-running to change a post-processing parameter is how a sweep
    // silently becomes five separate experiments.
    if let Ok(dir) = std::env::var("REPOSE_DUMP_DIR") {
        let stem = stl_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("heap");
        let mut csv = String::from("x,y,z\n");
        for p in &ours {
            csv.push_str(&format!("{:.9},{:.9},{:.9}\n", p.x, p.y, p.z));
        }
        let out = std::path::Path::new(&dir).join(format!("heap_{stem}.csv"));
        if let Err(e) = std::fs::write(&out, csv) {
            eprintln!("warning: could not write {}: {e}", out.display());
        }
    }

    let segments = n_facets / 16; // 8 axial bands x 2 triangles per segment
    let facet_mm = if segments > 0 {
        2.0 * std::f64::consts::PI * 0.05 / segments as f64 * 1000.0
    } else {
        f64::NAN
    };

    // `min_count = 8` is what `tests/angle_of_repose.rs` uses, so it is the ONLY
    // one comparable to the committed 12.78 deg. `min_count = 3` admits sparse
    // outer annuli that the test excludes; both are printed because the
    // difference between them is a post-processing choice, not physics, and
    // conflating the two is what made an earlier run look like a discrepancy.
    if let Some((d8, a8, o8)) = repose_angle(&ours, 8) {
        println!(
            "RESULT8 stl={} segments={segments} angle_deg={d8:.3} apex_m={a8:.5} outer_m={o8:.5}",
            stl_path.file_name().and_then(|s| s.to_str()).unwrap_or("?"),
        );
    } else {
        println!("RESULT8 stl=? angle_deg=NONE");
    }
    match repose_angle(&ours, 3) {
        Some((deg, apex, outer)) => {
            println!(
                "RESULT stl={} facets={n_facets} segments={segments} \
                 facet_mm={facet_mm:.3} spans={:.2} angle_deg={deg:.3} apex_m={apex:.5} \
                 outer_m={outer:.5} n={} secs={secs:.1}",
                stl_path.file_name().and_then(|s| s.to_str()).unwrap_or("?"),
                10.0 / facet_mm,
                ours.len(),
            );
        }
        None => println!(
            "RESULT stl={} facets={n_facets} angle_deg=NONE (profile too sparse) secs={secs:.1}",
            stl_path.file_name().and_then(|s| s.to_str()).unwrap_or("?")
        ),
    }
}
