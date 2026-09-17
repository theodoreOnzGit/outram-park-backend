//! Code-to-code verification against the **GeN-Foam tutorials' own reference
//! k_eff**, on upstream's own meshes and cross sections (bead `op-2df1`).
//!
//! # What this is
//!
//! Upstream GeN-Foam (gitlab.com/foam-for-nuclear/GeN-Foam, archived at
//! `652b3da` — the commit this port's file headers cite) ships reactor tutorials
//! whose `Alltest` regression scripts carry upstream's **own expected values**,
//! at upstream's own stated tolerance of `0.001` relative:
//!
//! | tutorial | `expectedKeff` |
//! |---|---|
//! | `reactorCases/3D_SmallESFR_NewSolverVerification` | 0.936827 |
//! | `reactorCases/2D_MSFR` | 0.960283 |
//!
//! Each case also ships a complete `constant/neutroRegion/polyMesh` and a
//! `constant/neutroRegion/nuclearData`, so these tests consume upstream's real
//! inputs rather than a reconstruction of them, and compare against a number
//! upstream published for exactly those inputs. That is the definition of a
//! code-to-code check.
//!
//! # The chain
//!
//! ```text
//!   upstream polyMesh  --read_poly_mesh-->    Arc<FvMesh>
//!   upstream cellZones --read_cell_zones-->   zone_of_cell
//!   upstream nuclearData --read_nuclear_data--> NuclearDataInput
//!                                             --> CrossSectionData
//!   ==> DiffusionNeutronics::solve_eigenvalue() ==> k_eff
//! ```
//!
//! The two readers in that chain (`read_nuclear_data`, `read_cell_zones`) were
//! written for this verification: the neutronics solvers and the
//! cross-section layer already existed, but nothing turned an upstream case into
//! their inputs.
//!
//! # What is compared, and what is not
//!
//! **Compared:** the eigenvalue of the reference-state multigroup diffusion
//! problem on upstream's mesh, against upstream's published `expectedKeff`.
//!
//! **NOT compared:** the port has no coupled multi-region driver wired to a
//! case, so it does not *produce* upstream's converged thermal-hydraulic state —
//! it is handed that state (per cell, from upstream's own written fields) and
//! asked for the eigenvalue. What is verified is the neutronics path, given the
//! state; the thermal-hydraulic path that produces the state is verified
//! separately (`genfoam_gfhr_pebble_vs_upstream`, `genfoam_psbt_closures`).
//! Tests that additionally evaluate at the nominal *reference* state report that
//! number too, and state the measured size of the feedback gap rather than
//! hiding it in a loose tolerance.
//!
//! This is therefore a **verification of the neutronics path against an
//! independently-produced reference**, not a reproduction of upstream's coupled
//! result end to end. It is not validation against experiment.
//!
//! # Running
//!
//! These tests **skip** (they do not fail) when the upstream clone is absent,
//! since it is a 39 MB third-party checkout that this repository deliberately
//! does not vendor:
//!
//! ```bash
//! git clone --depth 1 https://gitlab.com/foam-for-nuclear/GeN-Foam.git /workspace/GeN-Foam
//! cargo test --release -p outram-foam-appbuilder-lib --test genfoam_tutorial_keff -- --nocapture
//! ```
//!
//! Override the location with `GENFOAM_UPSTREAM=/path/to/GeN-Foam`.
//!
//! # Results (measured 2026-09-15)
//!
//! Upstream has since been **built and run here** (OpenFOAM v2506 + GeN-Foam
//! `652b3da`), so the comparison is now against runs taken on this machine, each
//! of which first reproduced the tutorial's own `Alltest` reference. Every
//! number below is at upstream's own converged feedback state, with upstream's
//! own boundary conditions.
//!
//! | case | port | upstream | difference |
//! |---|---|---|---|
//! | ESFR, upstream's converged state | 0.937442 | 0.936873 | **+60.7 pcm** |
//! | **MSFR, as the tutorial ships it** | **0.960314** | **0.960312** | **+0.2 pcm** |
//! | MSFR, precursor drift suppressed on both sides | 0.962032 | 0.962019 | +1.4 pcm |
//!
//! The MSFR case was **+787.8 pcm** adrift before 2026-09-15. Bisecting it
//! against upstream — by disabling, one at a time and in upstream itself, the
//! cross-section parametrisation, the precursor drift and the albedo boundary —
//! split it into three independent parts, two of which were port defects (the
//! Laplacian's delta coefficient, 609 pcm; the albedo weight that shared it) and
//! one of which was unported physics (circulating-fuel precursor drift,
//! 178 pcm). See [`msfr_keff_against_upstream_with_per_cell_cross_sections`] for
//! the bisection and
//! [`msfr_keff_against_upstream_with_precursor_drift`] for the drift.
//!
//! The port's leakage bracket — MSFR with a vacuum boundary (0.950415) and with
//! a reflective one (1.075834) — is kept as
//! [`msfr_reference_state_keff_brackets_upstream`]. It is no longer the
//! strongest available statement, now that the `albedoSP3` boundary is
//! implemented and agrees exactly, but both ends are themselves verified against
//! upstream runs with the same boundary imposed (0.950409 and 1.075830), which
//! makes it a check on the operator's two limiting cases.
//!
//! Not covered here: `3D_gFHR` and `1D_PSBT_SC`, whose upstream references are
//! thermal-hydraulic (`Tfmax`, subchannel conditions) rather than `k_eff`, and
//! which need the porous-medium TH path rather than the neutronics one.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use outram_foam_appbuilder_lib::genfoam::neutronics::diffusion::{
    DiffusionNeutronics, DiffusionSettings, PrecursorTransport,
};
use outram_foam_appbuilder_lib::genfoam::neutronics::albedo::AlbedoLinearisation;
use outram_foam_appbuilder_lib::genfoam::neutronics::xs::CrossSectionData;
use outram_foam_appbuilder_lib::io::nuclear_data::read_nuclear_data;
use outram_foam_appbuilder_lib::io::poly_mesh::{read_cell_zones, read_poly_mesh, zone_of_cell};
use outram_foam_basic_lib::fields::boundary::bc::BoundaryCondition;
use outram_foam_basic_lib::ldu_matrix::SolverSettings;
use outram_foam_basic_lib::mesh::{FvMesh, PatchKind};

/// Root of the upstream GeN-Foam checkout, or `None` if it is not present.
fn upstream_root() -> Option<PathBuf> {
    let root = std::env::var("GENFOAM_UPSTREAM")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/workspace/GeN-Foam"));
    root.join("Tutorials").is_dir().then_some(root)
}

/// Copy a case's `constant/<region>` into a scratch directory, decompressing any
/// `.gz` members of the `polyMesh` on the way.
///
/// OpenFOAM writes large mesh files gzipped and reads them back transparently;
/// this port's readers take plain text, so decompressing here is the same step a
/// user takes by hand to inspect a case. Returns `None` if `gzip` is
/// unavailable, so the test skips rather than failing for a missing system tool.
fn stage_region(case: &Path, region: &str, tag: &str) -> Option<PathBuf> {
    let src = case.join("constant").join(region);
    let dst = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("genfoam_{tag}"));
    let _ = std::fs::remove_dir_all(&dst);
    std::fs::create_dir_all(dst.join("polyMesh")).ok()?;

    // The dictionary and its `#include`d siblings.
    for entry in std::fs::read_dir(&src).ok()? {
        let e = entry.ok()?;
        if e.file_type().ok()?.is_file() {
            std::fs::copy(e.path(), dst.join(e.file_name())).ok()?;
        }
    }
    for entry in std::fs::read_dir(src.join("polyMesh")).ok()? {
        let e = entry.ok()?;
        if !e.file_type().ok()?.is_file() {
            continue;
        }
        let name = e.file_name().to_string_lossy().to_string();
        if let Some(stem) = name.strip_suffix(".gz") {
            let out = std::fs::File::create(dst.join("polyMesh").join(stem)).ok()?;
            let ok = Command::new("gzip")
                .arg("-dc")
                .arg(e.path())
                .stdout(out)
                .status()
                .ok()?
                .success();
            if !ok {
                return None;
            }
        } else {
            std::fs::copy(e.path(), dst.join("polyMesh").join(&name)).ok()?;
        }
    }
    Some(dst)
}

/// What one tutorial's reference-state eigenvalue solve produced.
struct Solved {
    k_eff: f64,
    n_cells: usize,
    groups: usize,
    zones: usize,
    outer_iterations: usize,
    converged: bool,
}

/// Read a per-cell feedback-state fixture: one row per cell, the `xsVariables`
/// in declaration order, `#` comments and a header line skipped.
fn read_cell_state(path: &Path, n_cells: usize) -> Vec<f64> {
    let csv = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("read the cell-state fixture {}: {e}", path.display()));
    let mut cells = Vec::new();
    let mut n_vars = 0usize;
    for line in csv.lines() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        if line
            .split(',')
            .next()
            .is_some_and(|f| f.parse::<f64>().is_err())
        {
            n_vars = line.split(',').count();
            continue;
        }
        for field in line.split(',') {
            cells.push(field.parse::<f64>().expect("a numeric fixture field"));
        }
    }
    assert_eq!(
        cells.len(),
        n_cells * n_vars,
        "fixture should cover every cell"
    );
    cells
}

/// Read upstream's own precursor-transport fields for the MSFR case.
///
/// `alpha` and `diffCoeffPrec` come from
/// `tests/data/genfoam_msfr_precursor_cells.csv`, the face flux `phi` from
/// `tests/data/genfoam_msfr_precursor_phi.csv`. Both are harvested from an
/// upstream run that reproduced the tutorial's own `expectedKeff`; see the
/// files' own headers for provenance.
///
/// The two `wedge` planes are not in the fixture — an axisymmetric symmetry
/// plane carries no flux, and upstream's own values there are machine zero — so
/// they are filled with zeros.
fn read_precursor_transport(mesh: &Arc<FvMesh>, data_dir: &Path) -> PrecursorTransport {
    let cells = std::fs::read_to_string(data_dir.join("genfoam_msfr_precursor_cells.csv"))
        .expect("read the MSFR precursor cell fixture");
    let (mut alpha, mut diffusivity) = (Vec::new(), Vec::new());
    for line in cells.lines() {
        if line.starts_with('#') || line.starts_with("alpha,") || line.trim().is_empty() {
            continue;
        }
        let mut it = line.split(',');
        alpha.push(it.next().unwrap().parse::<f64>().unwrap());
        diffusivity.push(it.next().unwrap().parse::<f64>().unwrap());
    }
    assert_eq!(alpha.len(), mesh.n_cells, "alpha fixture vs mesh");

    let phi_csv = std::fs::read_to_string(data_dir.join("genfoam_msfr_precursor_phi.csv"))
        .expect("read the MSFR precursor phi fixture");
    let mut phi_internal = vec![0.0; mesh.n_internal_faces];
    let mut phi_boundary: Vec<Vec<f64>> = mesh.patches.iter().map(|p| vec![0.0; p.size]).collect();
    let mut seen_internal = 0usize;
    for line in phi_csv.lines() {
        if line.starts_with('#') || line.starts_with("section,") || line.trim().is_empty() {
            continue;
        }
        let mut it = line.split(',');
        let section = it.next().unwrap();
        let index: usize = it.next().unwrap().parse().unwrap();
        let value: f64 = it.next().unwrap().parse().unwrap();
        if section == "internal" {
            phi_internal[index] = value;
            seen_internal += 1;
        } else {
            let p = mesh
                .patches
                .iter()
                .position(|p| p.name == section)
                .unwrap_or_else(|| panic!("mesh has no `{section}` patch"));
            phi_boundary[p][index] = value;
        }
    }
    assert_eq!(
        seen_internal, mesh.n_internal_faces,
        "phi fixture does not cover every internal face"
    );

    PrecursorTransport::new(mesh, alpha, phi_internal, phi_boundary, diffusivity)
}

/// Run the full chain on one staged neutronics region.
///
/// `flux_boundary` is upstream's own condition for the case; both tutorials here
/// set `".*" { type fixedValue; value uniform 0; }`, i.e. zero flux on every
/// patch, so the solve sees exactly the boundary upstream solved.
fn solve_reference_eigenvalue(region: &Path, outer: BoundaryCondition<f64>) -> Solved {
    solve_with_albedo(region, outer, &[])
}

/// As [`solve_reference_eigenvalue`], but additionally imposing an albedo
/// boundary with coefficient `gamma` on each patch named in `albedo_patches`,
/// which overrides `outer` for those patches.
fn solve_with_albedo(
    region: &Path,
    outer: BoundaryCondition<f64>,
    albedo_patches: &[(&str, f64)],
) -> Solved {
    solve_at_state(region, outer, albedo_patches, &[])
}

/// As [`solve_with_albedo`], but evaluating the cross sections at an explicit
/// feedback state given as `(variable name, value)` overrides on the reference
/// state — the mechanism by which upstream's perturbed `states` are used.
fn solve_at_state(
    region: &Path,
    outer: BoundaryCondition<f64>,
    albedo_patches: &[(&str, f64)],
    overrides: &[(&str, f64)],
) -> Solved {
    solve_full(region, outer, albedo_patches, overrides, None, None)
}

/// As [`solve_at_state`], but with `per_cell` giving each cell its own feedback
/// parameters (`n_cells x n_variables`, row-major) — the way GeN-Foam evaluates
/// them. When `Some`, `overrides` is ignored.
fn solve_full(
    region: &Path,
    outer: BoundaryCondition<f64>,
    albedo_patches: &[(&str, f64)],
    overrides: &[(&str, f64)],
    per_cell: Option<&[f64]>,
    drift: Option<&Path>,
) -> Solved {
    let mesh: Arc<FvMesh> =
        read_poly_mesh(&region.join("polyMesh")).expect("read the upstream polyMesh");

    let input = read_nuclear_data(&region.join("nuclearData")).expect("read upstream nuclearData");
    let xs = CrossSectionData::from_input(&input).expect("build CrossSectionData");

    let zones = read_cell_zones(&region.join("polyMesh")).expect("read upstream cellZones");
    let order: Vec<&str> = (0..xs.zone_count())
        .map(|i| xs.zone(i).expect("zone in range").name())
        .collect();
    let zone_map = zone_of_cell(&zones, &order, mesh.n_cells).expect("map cells to zones");

    // The reference state's own parameter values, in xsVariables order — this is
    // the un-fed-back state, see the module docs.
    let reference = &input.states[0];
    let raw: Vec<f64> = input
        .xs_variables
        .iter()
        .map(|v| {
            overrides
                .iter()
                .find(|(n, _)| *n == v.name)
                .map(|(_, x)| *x)
                .unwrap_or(reference.parameters[&v.name])
        })
        .collect();

    // A wedge (or empty) plane is a geometric artefact of a reduced-dimension
    // mesh, not a physical surface: it carries zero net current, so it is
    // zero-gradient regardless of what the real boundary does. Getting this
    // wrong is not subtle — a zero-flux Dirichlet on the two wedge planes of the
    // MSFR mesh turns the axisymmetric symmetry planes into the dominant
    // neutron sink and drives k_eff to 0.153.
    let flux_boundary: Vec<BoundaryCondition<f64>> = mesh
        .patches
        .iter()
        .map(|p| match p.kind {
            PatchKind::Wedge | PatchKind::Empty | PatchKind::Symmetry => {
                BoundaryCondition::ZeroGradient
            }
            _ => outer.clone(),
        })
        .collect();
    let settings = DiffusionSettings {
        k_tolerance: 1e-9,
        flux_tolerance: 1e-7,
        max_outer_iterations: 2000,
        linear: SolverSettings {
            tolerance: 1e-12,
            max_iter: 50_000,
        },
        // GeN-Foam's neutroRegion `fvSchemes` asks for `Gauss linear
        // uncorrected`, i.e. OpenFOAM's `nonOrthDeltaCoeffs`.
        ..DiffusionSettings::default()
    };

    let mut solver = match per_cell {
        None => {
            DiffusionNeutronics::new(mesh.clone(), &xs, &zone_map, &raw, &flux_boundary, settings)
        }
        Some(cells) => DiffusionNeutronics::new_with_cell_parameters(
            mesh.clone(),
            &xs,
            &zone_map,
            cells,
            &flux_boundary,
            settings,
        ),
    }
    .expect("build the diffusion solver");
    for (name, gamma) in albedo_patches {
        let idx = mesh
            .patches
            .iter()
            .position(|p| p.name == *name)
            .unwrap_or_else(|| panic!("mesh has no `{name}` patch"));
        solver.set_albedo_boundary(idx, *gamma, AlbedoLinearisation::CellValue);
    }
    if let Some(data_dir) = drift {
        solver.set_precursor_drift(read_precursor_transport(&mesh, data_dir));
    }

    let report = solver.solve_eigenvalue().expect("eigenvalue solve");

    Solved {
        k_eff: report.k_eff,
        n_cells: mesh.n_cells,
        groups: xs.energy_groups(),
        zones: xs.zone_count(),
        outer_iterations: report.outer_iterations,
        converged: report.converged,
    }
}

/// Report one case, and return the relative difference from upstream in pcm.
fn report(name: &str, s: &Solved, expected: f64) -> f64 {
    let pcm = 1.0e5 * (s.k_eff - expected) / expected;
    println!(
        "\n=== {name} ===\n\
         \tmesh:        {} cells, {} energy groups, {} cross-section zones\n\
         \tk_eff (port, reference state): {:.6}\n\
         \tk_eff (upstream Alltest):      {expected:.6}\n\
         \tdifference:  {:+.6} ({pcm:+.1} pcm, {:+.4} % relative)\n\
         \touter power iterations: {} (converged: {})",
        s.n_cells,
        s.groups,
        s.zones,
        s.k_eff,
        s.k_eff - expected,
        100.0 * (s.k_eff - expected) / expected,
        s.outer_iterations,
        s.converged,
    );
    pcm
}

/// Skip with a clear reason rather than failing when upstream is absent.
macro_rules! require_upstream {
    () => {
        match upstream_root() {
            Some(r) => r,
            None => {
                println!(
                    "SKIP: upstream GeN-Foam not found. Clone it with\n  \
                     git clone --depth 1 \
                     https://gitlab.com/foam-for-nuclear/GeN-Foam.git /workspace/GeN-Foam\n\
                     or set GENFOAM_UPSTREAM."
                );
                return;
            }
        }
    };
}

/// **V&V — ESFR: reference-state k_eff against upstream's `expectedKeff`.**
///
/// ## Methodology
///
/// `Tutorials/reactorCases/3D_SmallESFR_NewSolverVerification/newSolver` — a
/// one-energy-group, eight-precursor model of a small European Sodium Fast
/// Reactor, on upstream's own 3-D `neutroRegion` mesh. Cross sections come from
/// upstream's `nuclearData`, whose reference state is
/// `TFuel 900 K, TClad 668 K, rhoCool 860 kg/m3, axExp 0, radExp 0`, with the
/// zone data reached through five `#include`d `XS…` files. Flux boundary
/// condition: `fixedValue 0` on every patch, as upstream's `0/neutroRegion/
/// defaultFlux` sets. Multigroup diffusion, outer power iteration to
/// `k_tolerance = 1e-9`.
///
/// Upstream reference: `expectedKeff = 0.936827` at 0.001 relative tolerance.
///
/// Pass criterion here is deliberately looser than upstream's own 0.001: see the
/// module docs — upstream's number comes from the *coupled* run, this one from
/// the reference state, so a feedback-sized difference is expected and is
/// reported rather than tuned away. The test asserts the solve converged and
/// that the eigenvalue is within 5 % of upstream, which is tight enough to catch
/// a wrong mesh, a mis-parsed cross section, a mis-assigned zone or a broken
/// power iteration, and loose enough not to claim the feedback gap is an error.
///
/// ## Results (measured 2026-09-15)
///
/// | quantity | value |
/// |---|---|
/// | mesh (upstream's own) | 23 322 cells, 1 energy group, 10 cross-section zones |
/// | `k_eff` — port, reference state | **0.944987** |
/// | `k_eff` — upstream `expectedKeff` | 0.936827 |
/// | difference | **+871.1 pcm** (+0.871 %) |
/// | outer power iterations | 64, converged |
///
/// **Interpretation, and why the sign matters.** Upstream's number is produced
/// by a fully coupled run — `regionSolvers { fluidRegion onePhase;
/// neutroRegion diffusionNeutronics; thermoMechanicalRegion
/// extendedThermoMechanics; }` — read out of `200/uniform/reactorState` at the
/// converged steady state, so its cross sections are evaluated at fed-back
/// `TFuel`/`TClad`/`rhoCool`/`axExp`/`radExp`, not at the nominal reference
/// state this test uses.
///
/// In a sodium fast reactor every one of those feedbacks is negative at power:
/// fuel heating (Doppler), coolant density loss, and axial/radial expansion all
/// remove reactivity. The coupled `k_eff` must therefore sit **below** the
/// reference-state one, and it does: 0.936827 < 0.944987. The port is on the
/// correct side by a magnitude — 871 pcm — that is ordinary for an SFR's total
/// power-defect.
///
/// That is a consistency argument, not a measurement of the feedback: this test
/// cannot prove the 871 pcm is *all* feedback, because the port has no coupled
/// driver to evaluate the fed-back state with. Closing that gap — running the
/// perturbed states through the parametrisation and comparing the reactivity
/// coefficients themselves — is the next step, and is what would turn this from
/// "consistent" into "verified". Tracked under `op-2df1`.
#[test]
fn esfr_reference_state_keff_against_upstream() {
    let root = require_upstream!();
    let case = root.join("Tutorials/reactorCases/3D_SmallESFR_NewSolverVerification/newSolver");
    let Some(region) = stage_region(&case, "neutroRegion", "esfr") else {
        println!("SKIP: could not stage the ESFR case (is `gzip` available?)");
        return;
    };

    // Upstream's `0/neutroRegion/defaultFlux` sets `".*" { type fixedValue;
    // value uniform 0; }` — zero flux on every patch, and the mesh has no wedge
    // or empty planes. So this is upstream's own boundary condition exactly.
    let s = solve_reference_eigenvalue(&region, BoundaryCondition::FixedValue(0.0));
    let expected = 0.936827;
    let pcm = report("ESFR (3D_SmallESFR_NewSolverVerification)", &s, expected);

    assert!(s.converged, "the power iteration did not converge");
    assert!(
        pcm.abs() < 5000.0,
        "reference-state k_eff is {:+.1} pcm from upstream's {expected}, far beyond \
         what temperature feedback explains — suspect the mesh, the cross sections, \
         the zone map or the power iteration",
        pcm
    );
}

/// **V&V — MSFR: reference-state k_eff against upstream's `expectedKeff`.**
///
/// ## Methodology
///
/// `Tutorials/reactorCases/2D_MSFR/rootCase` — a six-energy-group,
/// eight-precursor model of the Molten Salt Fast Reactor, on upstream's own 2-D
/// axisymmetric `neutroRegion` mesh. Reference state `TFuel 900 K,
/// rhoCool 4125 kg/m3`; the zone data is inline rather than `#include`d. Same
/// boundary treatment, solver settings and pass criterion as the ESFR case
/// above, and the same caveat about reference state vs upstream's coupled run.
///
/// Upstream reference: `expectedKeff = 0.960283` at 0.001 relative tolerance.
///
/// ## Why this case is bracketed rather than compared directly
///
/// Unlike ESFR, the MSFR tutorial does **not** use a zero-flux boundary. Its
/// `0/neutroRegion/defaultFlux` sets `wedge` on the two axisymmetric planes and
/// `albedoSP3` with `gamma 0.1` on `topwall`, `bottomwall`, `reflector` and
/// `hx`. The port has no `albedoSP3` boundary condition — it is explicitly
/// deferred in `genfoam::neutronics::sp3` — so the case cannot be posed
/// faithfully today, and inventing an approximation to it would produce a
/// confident number with nothing behind it.
///
/// What *can* be stated rigorously is a bound. Vacuum (`fixedValue 0`) absorbs
/// everything that reaches the boundary and so leaks strictly more than any
/// albedo; total reflection (`zeroGradient`) leaks strictly less. The true
/// albedo answer lies between them, so if upstream's value falls inside the
/// bracket the port is consistent with upstream up to the one physics piece it
/// is missing — and if it falls outside, no albedo could reconcile them and the
/// fault is elsewhere. That is what this test asserts.
///
/// ## Results (measured 2026-09-15)
///
/// | boundary | `k_eff` | vs upstream |
/// |---|---|---|
/// | vacuum (`fixedValue 0`, lower bound) | **0.958444** | −191.5 pcm |
/// | upstream `expectedKeff` | 0.960283 | — |
/// | total reflection (`zeroGradient`, upper bound) | **1.075834** | +12 033 pcm |
///
/// Mesh: 8 595 cells, 6 energy groups, 4 cross-section zones. Bracket width
/// 0.117 (12 225 pcm); upstream's value sits inside it, 1.6 % of the way up from
/// the vacuum bound — i.e. the real MSFR boundary is only slightly less leaky
/// than vacuum, which is what `gamma = 0.1` should mean.
///
/// **A wrong answer this test caught.** Posed with `fixedValue 0` on *every*
/// patch — including the two `wedge` planes — the same solve returns
/// `k_eff = 0.153242`, −84 042 pcm. A wedge plane is a geometric artefact of an
/// axisymmetric mesh, not a surface, and a zero-flux Dirichlet there makes the
/// symmetry planes the dominant neutron sink. The fix is in
/// `solve_reference_eigenvalue`: patch *kind* decides the boundary, not a
/// blanket default.
#[test]
fn msfr_reference_state_keff_brackets_upstream() {
    let root = require_upstream!();
    let case = root.join("Tutorials/reactorCases/2D_MSFR/rootCase");
    let Some(region) = stage_region(&case, "neutroRegion", "msfr") else {
        println!("SKIP: could not stage the MSFR case (is `gzip` available?)");
        return;
    };

    let expected = 0.960283;

    // Lower bound: vacuum. Every non-wedge patch is a perfect absorber, so this
    // leaks more than the real albedo boundary and k_eff must come out low.
    let vacuum = solve_reference_eigenvalue(&region, BoundaryCondition::FixedValue(0.0));
    let vacuum_pcm = report("MSFR (2D_MSFR) — vacuum bound", &vacuum, expected);

    // Upper bound: total reflection. Zero current on every patch, no leakage at
    // all, so k_eff must come out high.
    let reflective = solve_reference_eigenvalue(&region, BoundaryCondition::ZeroGradient);
    let reflective_pcm = report("MSFR (2D_MSFR) — reflective bound", &reflective, expected);

    println!(
        "\nMSFR bracket: {:.6} (vacuum) <= k_upstream {expected:.6} <= {:.6} (reflective)\n\
         \twidth of the bracket: {:.6} ({:.0} pcm) — the albedo boundary\n\
         \tupstream actually uses (`albedoSP3`, gamma = 0.1) lies inside it.",
        vacuum.k_eff,
        reflective.k_eff,
        reflective.k_eff - vacuum.k_eff,
        1.0e5 * (reflective.k_eff - vacuum.k_eff) / expected,
    );

    assert!(
        vacuum.converged && reflective.converged,
        "a solve did not converge"
    );
    assert!(
        vacuum.k_eff < reflective.k_eff,
        "the vacuum bound ({:.6}) must be below the reflective bound ({:.6}); \
         if it is not, the two boundary conditions are not doing what they say",
        vacuum.k_eff,
        reflective.k_eff
    );
    assert!(
        vacuum.k_eff < expected && expected < reflective.k_eff,
        "upstream's k_eff = {expected} falls OUTSIDE the port's leakage bracket \
         [{:.6}, {:.6}] ({vacuum_pcm:+.0} pcm, {reflective_pcm:+.0} pcm). No choice of \
         albedo can reconcile the two, so the discrepancy is in the mesh, the \
         cross sections, the zone map or the power iteration — not in the \
         boundary condition.",
        vacuum.k_eff,
        reflective.k_eff
    );
}

/// **V&V — MSFR: the `albedoSP3` boundary condition against upstream, run here,
/// with every other difference switched off.**
///
/// ## Methodology
///
/// The boundary condition is the only thing under test, so everything that could
/// confound it is removed from **both** sides rather than argued about:
///
/// | confounder | how it is removed |
/// |---|---|
/// | cross-section feedback | upstream's `xsVariables` dictionary emptied, so every zone uses its `reference` data verbatim; the port is evaluated at the reference state, which its RBF reproduces exactly |
/// | circulating-fuel precursor drift | upstream's `controlDict` set to `liquidFuel false`, which is the `chi_eff` collapse the port implements |
/// | thermal-hydraulic coupling | both runs start from the tutorial's own converged `t = 165 s` fields |
///
/// What is left is one multigroup diffusion eigenvalue problem, posed on the
/// same mesh with the same numbers, differing only in whose boundary condition
/// implementation evaluates it. Reproduce the upstream side with:
///
/// ```bash
/// cp -r $GENFOAM/Tutorials/reactorCases/2D_MSFR/steadyStateEN/165  case/0
/// cp -r $GENFOAM/Tutorials/reactorCases/2D_MSFR/steadyStateEN/{constant,system} case/
/// # system/controlDict:            liquidFuel false;  startTime 0;  endTime 200;
/// # constant/neutroRegion/nuclearData:  xsVariables { }
/// GeN-Foam            # -> 0.973858 in 200/uniform/reactorState
/// ```
///
/// The boundary conditions themselves are upstream's own, read off
/// `0/neutroRegion/defaultFlux`:
///
/// | patch | upstream | here |
/// |---|---|---|
/// | `front`, `back` | `wedge` | zero gradient (symmetry planes) |
/// | `topwall`, `bottomwall`, `reflector` | `albedoSP3`, `gamma 0.1` | albedo, `gamma = 0.1` |
/// | `hx` | `albedoSP3`, **`gamma 0.5`** | albedo, `gamma = 0.5` |
///
/// The `hx` patch is vacuum-valued, not `0.1` like the other three — worth
/// calling out because assuming a single `gamma` for the whole boundary is the
/// obvious mistake, and one this test made before the dictionary was read in
/// full. [`AlbedoLinearisation::CellValue`] is used because upstream's
/// `gradientInternalCoeffs()` returns `-gamma/D` against the *cell* value, with
/// no face-value closure.
///
/// ## Results (measured 2026-09-15)
///
/// | boundary condition | port | upstream, run here | difference |
/// |---|---|---|---|
/// | `albedoSP3`, per-patch gamma | **0.973858** | **0.973858** | **0.0 pcm** |
/// | `fixedValue 0` (vacuum) | 0.950415 | 0.950409 | +0.6 pcm |
/// | `zeroGradient` (reflective) | 1.075834 | 1.075830 | +0.4 pcm |
///
/// All three agree to better than 1 pcm, across a 12 500 pcm span of leakage.
/// That is what makes this a verification of the *operator* and not of one
/// tuned case: a coefficient error would have to be invisible at both limits
/// and in between.
///
/// ## The defect this found
///
/// It did not always agree. The albedo weight is a `mixed` fraction
/// `w = gamma_albedo * delta / D`, which the Laplacian multiplies by
/// `D |Sf| / delta_laplacian` — so the diagonal contribution is
/// `gamma_albedo |Sf| * delta / delta_laplacian`, and it equals upstream's
/// `gamma_albedo |Sf|` **only when those two deltas are the same length**.
/// While `fvm::laplacian` divided by `|d|` and the albedo weight also used
/// `|d|`, they cancelled by accident. Fixing the Laplacian to OpenFOAM's
/// `nonOrthDeltaCoeffs` broke the cancellation and cost -185 pcm until the
/// albedo was made to ask the Laplacian for its delta
/// ([`DeltaCoeff::boundary_delta`]) instead of recomputing one.
///
/// The general lesson, worth more than the number: two places computing "the
/// distance to the face" independently is a latent bug even when they agree,
/// because nothing makes them keep agreeing.
#[test]
fn msfr_keff_with_upstreams_albedo_boundary() {
    let root = require_upstream!();
    let case = root.join("Tutorials/reactorCases/2D_MSFR/rootCase");
    let Some(region) = stage_region(&case, "neutroRegion", "msfr_albedo") else {
        println!("SKIP: could not stage the MSFR case (is `gzip` available?)");
        return;
    };

    // Upstream GeN-Foam, built and run here, with the parametrisation and the
    // precursor drift switched off so only the boundary condition differs.
    // See the doc comment above for the exact case set-up.
    const UPSTREAM_REF_STATE: f64 = 0.973858;

    // Per-patch gamma, read off upstream's own 0/neutroRegion/defaultFlux.
    let s = solve_with_albedo(
        &region,
        BoundaryCondition::ZeroGradient,
        &[
            ("topwall", 0.1),
            ("bottomwall", 0.1),
            ("reflector", 0.1),
            ("hx", 0.5),
        ],
    );

    let pcm = report(
        "MSFR (2D_MSFR) — upstream's albedo boundary",
        &s,
        UPSTREAM_REF_STATE,
    );

    assert!(s.converged, "the power iteration did not converge");
    assert!(
        pcm.abs() < 5.0,
        "the port's albedoSP3 boundary gives k_eff = {:.6} where upstream gives \
         {UPSTREAM_REF_STATE:.6} on the identical problem ({pcm:+.2} pcm). With \
         the parametrisation and the precursor drift switched off on both sides, \
         nothing but the boundary condition can account for a difference.",
        s.k_eff
    );
}

/// Diagnostic: the MSFR reactivity coefficients implied by upstream's own
/// perturbed cross-section states, and what temperature shift the gap between
/// the port's reference-state `k_eff` and upstream's coupled one would need.
#[test]
fn msfr_reactivity_coefficients_from_upstreams_perturbed_states() {
    let root = require_upstream!();
    let case = root.join("Tutorials/reactorCases/2D_MSFR/rootCase");
    let Some(region) = stage_region(&case, "neutroRegion", "msfr_coeff") else {
        println!("SKIP");
        return;
    };
    let albedo = [
        ("topwall", 0.1),
        ("bottomwall", 0.1),
        ("reflector", 0.1),
        ("hx", 0.5),
    ];
    let run = |ov: &[(&str, f64)]| {
        solve_at_state(&region, BoundaryCondition::ZeroGradient, &albedo, ov).k_eff
    };

    let k_ref = run(&[]);
    let k_hot = run(&[("TFuel", 1500.0)]);
    let k_light = run(&[("rhoCool", 3419.0)]);
    let k_both = run(&[("TFuel", 1500.0), ("rhoCool", 3419.0)]);

    let pcm = |a: f64, b: f64| 1.0e5 * (a - b) / (a * b);
    println!(
        "\nMSFR reactivity coefficients from upstream's own perturbed states:\n\
         \tk(reference: TFuel 900 K, rhoCool 4125)   = {k_ref:.6}\n\
         \tk(TFuel 1500 K)                           = {k_hot:.6}  \
         ({:+.1} pcm over 600 K -> {:+.4} pcm/K)\n\
         \tk(rhoCool 3419 kg/m3)                     = {k_light:.6}  \
         ({:+.1} pcm over -706 kg/m3 -> {:+.4} pcm per kg/m3)\n\
         \tk(both perturbed)                         = {k_both:.6}\n\
         \tupstream coupled expectedKeff             = 0.960283",
        pcm(k_hot, k_ref),
        pcm(k_hot, k_ref) / 600.0,
        pcm(k_light, k_ref),
        pcm(k_light, k_ref) / -706.0,
    );
    // Upstream's coupled value must lie between the port's unfed-back reference
    // state and its fully-perturbed one: the case's own perturbed states bound
    // how far negative feedback can carry k, and upstream's converged state is
    // somewhere inside that span. Both ends come from upstream's own data, so
    // nothing here is fitted to the answer.
    assert!(
        k_both < 0.960283 && 0.960283 < k_ref,
        "upstream's coupled k_eff 0.960283 is outside the span the case's own \
         perturbed states allow, [{k_both:.6}, {k_ref:.6}] — the feedback \
         parametrisation or the reference-state solve is wrong"
    );
}

/// **V&V — MSFR: direct `k_eff` comparison against upstream GeN-Foam, built and
/// run, at upstream's own converged feedback state.**
///
/// ## What makes this a true like-for-like comparison
///
/// The earlier MSFR tests compared the port at the *nominal reference* state
/// against upstream's *coupled* result, leaving a residual that was measured but
/// not closed. Upstream GeN-Foam has since been **built from source and run
/// here** — OpenFOAM v2506 (ESI, tag `OpenFOAM-v2506`) plus GeN-Foam `652b3da` —
/// so its converged state is now known rather than inferred.
///
/// The run reproduced upstream's own published reference, which is what
/// qualifies it as a reference run at all:
///
/// | | |
/// |---|---|
/// | `keff` from `steadyStateEN/165/uniform/reactorState` | **0.96031** |
/// | `expectedKeff` in the tutorial's `Alltest` | 0.960283 |
/// | difference | **+2.8 pcm** |
/// | total power | 2.00016e7 W against a 2e7 target |
///
/// Its converged cross-section state, read from
/// `steadyStateEN/165/neutroRegion/` over all 8595 cells:
///
/// | field | mean | min | max | reference state |
/// |---|---|---|---|---|
/// | `TFuel` | **1037.46 K** | 991.63 | 1188.43 | 900 |
/// | `rhoCool` | **4011.60 kg/m³** | 3887.04 | 4049.41 | 4125 |
///
/// This test evaluates the port's cross sections at that measured state, with
/// upstream's own per-patch albedo boundaries, and compares against upstream's
/// own `k_eff` from the same run.
///
/// ## The one approximation, stated
///
/// Upstream evaluates the cross sections **per cell** at each cell's local
/// `TFuel`/`rhoCool`; `DiffusionNeutronics` takes a single global parameter
/// vector, so this uses the volume-mean state. Because the `TFuel`
/// parametrisation is logarithmic and the field spans 991–1188 K, the mean of
/// the cross sections is not the cross section at the mean, and the difference
/// between them is the irreducible part of whatever residual remains. Closing it
/// needs per-cell parametrisation in the port, not a better reference.
///
/// ## Results (measured 2026-09-15)
///
/// See the printed report.
#[test]
fn msfr_keff_against_upstream_run_at_its_converged_state() {
    let root = require_upstream!();
    let case = root.join("Tutorials/reactorCases/2D_MSFR/rootCase");
    let Some(region) = stage_region(&case, "neutroRegion", "msfr_converged") else {
        println!("SKIP: could not stage the MSFR case (is `gzip` available?)");
        return;
    };

    // Upstream's own converged volume means, from the run described above.
    const UPSTREAM_TFUEL: f64 = 1037.4605;
    const UPSTREAM_RHOCOOL: f64 = 4011.5951;
    // Upstream's own k_eff from that run (its Alltest expects 0.960283).
    const UPSTREAM_KEFF: f64 = 0.96031;

    let albedo = [
        ("topwall", 0.1),
        ("bottomwall", 0.1),
        ("reflector", 0.1),
        ("hx", 0.5),
    ];

    let at_reference = solve_at_state(&region, BoundaryCondition::ZeroGradient, &albedo, &[]);
    let at_converged = solve_at_state(
        &region,
        BoundaryCondition::ZeroGradient,
        &albedo,
        &[("TFuel", UPSTREAM_TFUEL), ("rhoCool", UPSTREAM_RHOCOOL)],
    );

    let pcm = |a: f64, b: f64| 1.0e5 * (a - b) / b;
    let ref_pcm = pcm(at_reference.k_eff, UPSTREAM_KEFF);
    let conv_pcm = pcm(at_converged.k_eff, UPSTREAM_KEFF);

    println!(
        "\n=== MSFR: port vs upstream GeN-Foam, both run here ===\n\
         \tupstream k_eff (built + run, v2506 / 652b3da) = {UPSTREAM_KEFF:.6}\n\
         \t  (its own Alltest expects 0.960283 -> +2.8 pcm, so the run is sound)\n\
         \tupstream converged state: TFuel {UPSTREAM_TFUEL:.2} K, \
         rhoCool {UPSTREAM_RHOCOOL:.2} kg/m3\n\
         \t--\n\
         \tport @ reference state (900 K, 4125)   k = {:.6}  ({ref_pcm:+.1} pcm)\n\
         \tport @ upstream's converged state      k = {:.6}  ({conv_pcm:+.1} pcm)\n\
         \t--\n\
         \tevaluating at the right state closed {:.0} pcm of the gap",
        at_reference.k_eff,
        at_converged.k_eff,
        ref_pcm.abs() - conv_pcm.abs(),
    );

    assert!(
        at_converged.converged,
        "the power iteration did not converge"
    );
    assert!(
        conv_pcm.abs() < ref_pcm.abs(),
        "evaluating at upstream's converged state ({:+.1} pcm) should be closer \
         to upstream than the nominal reference state ({:+.1} pcm) — if it is \
         not, the cross-section parametrisation is not reproducing the feedback",
        conv_pcm,
        ref_pcm
    );
}

/// **V&V — ESFR: direct `k_eff` comparison against upstream GeN-Foam, built and
/// run, at upstream's own converged feedback state.**
///
/// ## The reference run
///
/// Upstream GeN-Foam (OpenFOAM v2506 + GeN-Foam `652b3da`, both built here) was
/// run on `3D_SmallESFR_NewSolverVerification/legacySolver`, four-way parallel,
/// to `t = 200`. It reproduced the tutorial's own published value:
///
/// | | |
/// |---|---|
/// | `keff` from `processor0/200/uniform/reactorState` | **0.936873** |
/// | `expectedKeff` in the tutorial's `Alltest` | 0.936827 |
/// | difference | **+49 pcm**, inside upstream's own 0.001 (936 pcm) tolerance |
///
/// Only `legacySolver` was run. Its `newSolver` sibling selects
/// `extendedThermoMechanics`, which cannot be built from any available source —
/// GeN-Foam pins an offbeat commit that upstream force-pushed away, and every
/// surviving offbeat with the API it needs predates OpenFOAM v2506. The
/// tutorial checks **both** solvers against the same `expectedKeff`, so the
/// reference value is unaffected.
///
/// ## The converged state
///
/// Read from the reconstructed `200/neutroRegion/` fields. `TFuel` and `TClad`
/// are zero in the non-fuelled zones (reflector, plena, diagrid), so the means
/// below are over the **6540 fuelled cells**, not all 23322 — a plain mean would
/// be meaningless:
///
/// | variable | mean | min | max | reference |
/// |---|---|---|---|---|
/// | `TFuel` | **1398.87 K** | 667.99 | 1948.85 | 900 |
/// | `TClad` | **771.64 K** | 667.99 | 902.46 | 668 |
/// | `rhoCool` | **860** (uniform) | — | — | 860 |
/// | `axExp` | **0.007711** | — | — | 0 |
/// | `radExp` | **0.001268** | — | — | 0 |
///
/// `rhoCool` came back exactly at its reference value, so the coolant-density
/// feedback contributes nothing here; the gap is fuel and cladding temperature
/// plus the two expansions.
///
/// ## Results (measured 2026-09-15)
///
/// See the printed report. The same per-cell-versus-global caveat applies as in
/// [`msfr_keff_against_upstream_run_at_its_converged_state`]: upstream evaluates
/// cross sections per cell, the port takes one global parameter vector, and
/// `TFuel` spans 668–1949 K under a logarithmic parametrisation.
#[test]
fn esfr_keff_against_upstream_run_at_its_converged_state() {
    let root = require_upstream!();
    let case = root.join("Tutorials/reactorCases/3D_SmallESFR_NewSolverVerification/newSolver");
    let Some(region) = stage_region(&case, "neutroRegion", "esfr_converged") else {
        println!("SKIP: could not stage the ESFR case (is `gzip` available?)");
        return;
    };

    // Upstream's own converged means over its fuelled cells, from the run above.
    const UP_TFUEL: f64 = 1398.866;
    const UP_TCLAD: f64 = 771.640;
    const UP_RHOCOOL: f64 = 860.0;
    const UP_AXEXP: f64 = 0.007711;
    const UP_RADEXP: f64 = 0.001268;
    /// Upstream's own k_eff from that run (its Alltest expects 0.936827).
    const UPSTREAM_KEFF: f64 = 0.936873;

    let vacuum = BoundaryCondition::FixedValue(0.0);
    let at_reference = solve_at_state(&region, vacuum.clone(), &[], &[]);
    let at_converged = solve_at_state(
        &region,
        vacuum,
        &[],
        &[
            ("TFuel", UP_TFUEL),
            ("TClad", UP_TCLAD),
            ("rhoCool", UP_RHOCOOL),
            ("axExp", UP_AXEXP),
            ("radExp", UP_RADEXP),
        ],
    );

    let pcm = |a: f64, b: f64| 1.0e5 * (a - b) / b;
    let ref_pcm = pcm(at_reference.k_eff, UPSTREAM_KEFF);
    let conv_pcm = pcm(at_converged.k_eff, UPSTREAM_KEFF);

    println!(
        "\n=== ESFR: port vs upstream GeN-Foam, both run here ===\n\
         \tupstream k_eff (built + run, v2506 / 652b3da) = {UPSTREAM_KEFF:.6}\n\
         \t  (its own Alltest expects 0.936827 -> +49 pcm, so the run is sound)\n\
         \tupstream converged state (over 6540 fuelled cells):\n\
         \t  TFuel {UP_TFUEL:.1} K, TClad {UP_TCLAD:.1} K, rhoCool {UP_RHOCOOL:.0}, \
         axExp {UP_AXEXP:.6}, radExp {UP_RADEXP:.6}\n\
         \t--\n\
         \tport @ reference state             k = {:.6}  ({ref_pcm:+.1} pcm)\n\
         \tport @ upstream's converged state  k = {:.6}  ({conv_pcm:+.1} pcm)\n\
         \t--\n\
         \tevaluating at the right state closed {:.0} pcm of the gap",
        at_reference.k_eff,
        at_converged.k_eff,
        ref_pcm.abs() - conv_pcm.abs(),
    );

    assert!(
        at_converged.converged,
        "the power iteration did not converge"
    );
    assert!(
        conv_pcm.abs() < ref_pcm.abs(),
        "evaluating at upstream's converged state ({conv_pcm:+.1} pcm) should be \
         closer to upstream than the nominal reference state ({ref_pcm:+.1} pcm)"
    );
    // The verification claim: at the same state, the port agrees with upstream
    // inside upstream's OWN stated tolerance for this tutorial (0.001 relative,
    // i.e. 100 pcm). This is the assertion that makes ESFR a direct code-to-code
    // verification rather than a characterisation.
    assert!(
        conv_pcm.abs() < 100.0,
        "port and upstream differ by {conv_pcm:+.1} pcm at the same state, \
         outside upstream's own 0.001 relative tolerance"
    );
}

/// **V&V — MSFR: `k_eff` against upstream, with the cross sections evaluated
/// per cell as upstream evaluates them.**
///
/// ## What this closes
///
/// [`msfr_keff_against_upstream_run_at_its_converged_state`] compared the port
/// at upstream's volume-**mean** `TFuel`/`rhoCool`, and named the residual: the
/// `TFuel` parametrisation is logarithmic over a 991–1188 K field, so the mean
/// of the cross sections is not the cross section at the mean. That was the
/// stated approximation, and it is now removed —
/// [`DiffusionNeutronics::new_with_cell_parameters`] evaluates each cell at its
/// own state, which is what GeN-Foam does.
///
/// The per-cell state is upstream's own, committed as
/// `tests/data/genfoam_msfr_cell_state.csv` (8595 cells, `TFuel` and `rhoCool`
/// in `xsVariables` order) from the run that reproduced the tutorial's
/// `expectedKeff` to +2.8 pcm.
///
/// ## The like-for-like reference is upstream's **drift-off** run
///
/// The MSFR tutorial sets `liquidFuel true`, so upstream transports the delayed
/// precursors — it solves an advection-diffusion equation for each precursor
/// group (`neutronics/include/precEq.H`) and feeds the resulting
/// `delayedNeutroSource` into the flux equation. This port instead collapses to
/// `chi_eff = chi_p (1 - beta) + chi_d beta`, which is **exactly** upstream's
/// `liquidFuel false` branch, so that is the run this test is judged against.
///
/// Both upstream runs were taken here, from the tutorial's own converged state
/// at t = 165 s, changing nothing but that one switch:
///
/// | upstream configuration | `k_eff` |
/// |---|---|
/// | `liquidFuel true` (as shipped) | **0.960312** |
/// | `liquidFuel false` (what this test compares against) | **0.962019** |
/// | worth of precursor drift | **-177.8 pcm** |
///
/// Drift has since been ported
/// ([`msfr_keff_against_upstream_with_precursor_drift`], +0.2 pcm against the
/// as-shipped value), but this test deliberately leaves it **off**: with drift
/// suppressed on both sides it isolates the diffusion operator, the albedo
/// boundaries and the cross-section parametrisation from the transport model, so
/// a regression in either half lands on a different test. The +179 pcm gap to
/// the as-shipped value that it reports is therefore expected, and is asserted
/// to equal the measured worth of drift rather than being waved away.
///
/// ## Results (measured 2026-09-15)
///
/// | | port | upstream | difference |
/// |---|---|---|---|
/// | per-cell XS, drift off | **0.962032** | 0.962019 | **+1.4 pcm** |
/// | per-cell XS, vs as-shipped | 0.962032 | 0.960312 | +179.1 pcm (= the drift) |
///
/// +1.4 pcm on a 6-group, 8595-cell, 4-zone eigenvalue problem with per-cell
/// feedback and four albedo patches is a code-to-code agreement, not a
/// characterisation.
///
/// ## What this number cost, and the two defects it exposed
///
/// It was **+787.8 pcm** before 2026-09-15. Bisecting it against upstream — by
/// disabling, one at a time, the cross-section parametrisation (empty
/// `xsVariables`), the precursor drift (`liquidFuel false`) and the albedo
/// boundary (`fixedValue 0` / `zeroGradient`) in upstream itself — split it into
/// three independent parts and found two real port defects:
///
/// | part | worth | cause |
/// |---|---|---|
/// | operator | 609 pcm | `fvm::laplacian` divided by `\|d\|` where OpenFOAM's `uncorrected` scheme divides by `n . d` (`nonOrthDeltaCoeffs`). Fixed: [`DeltaCoeff::NonOrthogonal`]. |
/// | albedo weight | (inside the above) | the Robin weight used a different distance from the Laplacian, so `gamma_albedo` no longer cancelled to `gamma \|Sf\|`. Fixed by sharing one delta. |
/// | precursor drift | 178 pcm | unported physics; since ported — [`msfr_keff_against_upstream_with_precursor_drift`]. |
/// | parametrisation | 14 pcm | already correct; the RBF port moves `k_eff` by -1201 pcm against upstream's -1215 pcm. |
///
/// The bisection is reproducible: each row is an upstream run with one switch
/// flipped, not an inference.
#[test]
fn msfr_keff_against_upstream_with_per_cell_cross_sections() {
    let root = require_upstream!();
    let case = root.join("Tutorials/reactorCases/2D_MSFR/rootCase");
    let Some(region) = stage_region(&case, "neutroRegion", "msfr_percell") else {
        println!("SKIP: could not stage the MSFR case (is `gzip` available?)");
        return;
    };

    // Upstream, run here from the tutorial's own converged t = 165 s state.
    // `AS_SHIPPED` is `liquidFuel true`, i.e. with circulating-fuel precursor
    // drift; `DRIFT_OFF` is the same case with that one switch flipped, which is
    // the model this port implements (see the doc comment above).
    const UPSTREAM_AS_SHIPPED: f64 = 0.960312;
    const UPSTREAM_DRIFT_OFF: f64 = 0.962019;
    const UP_TFUEL_MEAN: f64 = 1037.4605;
    const UP_RHOCOOL_MEAN: f64 = 4011.5951;

    // Upstream's per-cell state, in xsVariables order (TFuel, rhoCool).
    let csv = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/genfoam_msfr_cell_state.csv"),
    )
    .expect("read the MSFR per-cell state fixture");
    let mut cells = Vec::new();
    for line in csv.lines() {
        if line.starts_with('#') || line.starts_with("TFuel,") || line.trim().is_empty() {
            continue;
        }
        let mut it = line.split(',');
        cells.push(it.next().unwrap().parse::<f64>().unwrap());
        cells.push(it.next().unwrap().parse::<f64>().unwrap());
    }
    assert_eq!(cells.len(), 8595 * 2, "fixture should cover every cell");

    let albedo = [
        ("topwall", 0.1),
        ("bottomwall", 0.1),
        ("reflector", 0.1),
        ("hx", 0.5),
    ];
    let at_mean = solve_full(
        &region,
        BoundaryCondition::ZeroGradient,
        &albedo,
        &[("TFuel", UP_TFUEL_MEAN), ("rhoCool", UP_RHOCOOL_MEAN)],
        None,
        None,
    );
    let per_cell = solve_full(
        &region,
        BoundaryCondition::ZeroGradient,
        &albedo,
        &[],
        Some(&cells),
        None,
    );

    let pcm = |a: f64, b: f64| 1.0e5 * (a - b) / b;
    let mean_pcm = pcm(at_mean.k_eff, UPSTREAM_DRIFT_OFF);
    let cell_pcm = pcm(per_cell.k_eff, UPSTREAM_DRIFT_OFF);
    let shipped_pcm = pcm(per_cell.k_eff, UPSTREAM_AS_SHIPPED);
    let drift_worth = pcm(UPSTREAM_AS_SHIPPED, UPSTREAM_DRIFT_OFF);

    println!(
        "\n=== MSFR: per-cell cross sections vs upstream GeN-Foam run ===\n\
         \tupstream, liquidFuel false (what the port models) = {UPSTREAM_DRIFT_OFF:.6}\n\
         \tupstream, liquidFuel true  (as shipped)           = {UPSTREAM_AS_SHIPPED:.6}\n\
         \t  -> precursor drift is worth {drift_worth:+.1} pcm (op-exma)\n\
         \t--\n\
         \tport @ upstream's MEAN state      k = {:.6}  ({mean_pcm:+.1} pcm vs drift-off)\n\
         \tport @ upstream's PER-CELL state  k = {:.6}  ({cell_pcm:+.1} pcm vs drift-off)\n\
         \t  per-cell evaluation closed a further {:.0} pcm\n\
         \t--\n\
         \tport vs the as-shipped tutorial value: {shipped_pcm:+.1} pcm,\n\
         \t  which is the missing drift and nothing else.",
        at_mean.k_eff,
        per_cell.k_eff,
        mean_pcm.abs() - cell_pcm.abs(),
    );

    assert!(per_cell.converged, "the power iteration did not converge");
    assert!(
        cell_pcm.abs() < mean_pcm.abs(),
        "evaluating per cell ({cell_pcm:+.1} pcm) should beat evaluating at the \
         mean ({mean_pcm:+.1} pcm) — the logarithmic TFuel law makes the mean of \
         the cross sections differ from the cross section at the mean"
    );
    // The verification claim. Judged against upstream's `liquidFuel false` run,
    // which is the model this port implements, at upstream's own per-cell state
    // and with upstream's own albedo patches. 25 pcm is a quarter of upstream's
    // own 0.001 relative tutorial tolerance.
    assert!(
        cell_pcm.abs() < 25.0,
        "port and upstream differ by {cell_pcm:+.1} pcm at the same state and the \
         same physics (upstream with liquidFuel false, k = {UPSTREAM_DRIFT_OFF:.6})"
    );
    // And the gap to the as-shipped tutorial value is accounted for: it is the
    // precursor drift, to within the tolerance above. If this ever fails, either
    // drift landed (op-exma) — update both constants — or a second, unexplained
    // term has appeared.
    assert!(
        (shipped_pcm + drift_worth).abs() < 25.0,
        "the gap to upstream's as-shipped value ({shipped_pcm:+.1} pcm) no longer \
         equals the measured worth of precursor drift ({:+.1} pcm); something \
         other than drift is now in play",
        -drift_worth
    );
}

/// **V&V — MSFR: `k_eff` against upstream *as the tutorial ships*, with
/// circulating-fuel precursor drift.**
///
/// ## What this closes
///
/// [`msfr_keff_against_upstream_with_per_cell_cross_sections`] matched upstream
/// run with `liquidFuel false` to +1.4 pcm, and named the one remaining
/// difference: the tutorial actually runs with `liquidFuel true`, so upstream
/// transports its delayed-neutron precursors and the port did not. That was
/// worth a measured −177.8 pcm. This test turns drift on and compares against
/// the value the tutorial ships.
///
/// ## Methodology
///
/// Everything is upstream's own, from the run that reproduced the tutorial's
/// `expectedKeff` (0.960283) to +3.0 pcm:
///
/// | input | source |
/// |---|---|
/// | mesh, cross sections, zone map | `Tutorials/reactorCases/2D_MSFR/rootCase` |
/// | per-cell `TFuel` / `rhoCool` | `tests/data/genfoam_msfr_cell_state.csv` |
/// | `alpha`, `diffCoeffPrec` | `tests/data/genfoam_msfr_precursor_cells.csv` |
/// | face flux `phi = fvc::flux(U alpha)` | `tests/data/genfoam_msfr_precursor_phi.csv` |
/// | boundary conditions | `0/neutroRegion/defaultFlux` (`albedoSP3`, 0.1 / 0.5) |
///
/// The last three are `NO_WRITE` in upstream. Harvesting them needed those five
/// `IOobject`s flipped to `AUTO_WRITE` in the upstream clone — a diagnostic
/// change with no physics in it, and one verified as such: the harvest run
/// returned `keff = 0.960312`, identical to the same case built without it.
///
/// What the port still supplies itself is the whole neutronics: the diffusion
/// operator, the albedo boundaries, the cross-section parametrisation, the
/// precursor transport equation and the power iteration. It is handed the
/// thermal-hydraulic *state* because it has no coupled multi-region driver;
/// it is not handed any part of the answer.
///
/// ## Results (measured 2026-09-15)
///
/// | | `k_eff` | vs upstream as shipped |
/// |---|---|---|
/// | upstream, `liquidFuel true` (as shipped) | 0.960312 | — |
/// | port, drift **off** | 0.962032 | +179.1 pcm |
/// | **port, drift on** | see the printed report | |
///
/// The assertion is 25 pcm, a quarter of upstream's own 0.001 relative
/// tolerance for this tutorial.
///
/// ## What this is not
///
/// Verification, not validation. Both codes are being compared to each other on
/// the same idealised 2-D axisymmetric model; neither is being compared to a
/// measurement. The MSFR tutorial is itself a demonstration case, not a
/// benchmark with a published reference `k_eff`.
#[test]
fn msfr_keff_against_upstream_with_precursor_drift() {
    let root = require_upstream!();
    let case = root.join("Tutorials/reactorCases/2D_MSFR/rootCase");
    let Some(region) = stage_region(&case, "neutroRegion", "msfr_drift") else {
        println!("SKIP: could not stage the MSFR case (is `gzip` available?)");
        return;
    };
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data");

    // Upstream, run here. `AS_SHIPPED` is the tutorial's own configuration.
    const UPSTREAM_AS_SHIPPED: f64 = 0.960312;
    const UPSTREAM_DRIFT_OFF: f64 = 0.962019;

    let cells = read_cell_state(&data.join("genfoam_msfr_cell_state.csv"), 8595);
    let albedo = [
        ("topwall", 0.1),
        ("bottomwall", 0.1),
        ("reflector", 0.1),
        ("hx", 0.5),
    ];

    let without = solve_full(
        &region,
        BoundaryCondition::ZeroGradient,
        &albedo,
        &[],
        Some(&cells),
        None,
    );
    let with = solve_full(
        &region,
        BoundaryCondition::ZeroGradient,
        &albedo,
        &[],
        Some(&cells),
        Some(&data),
    );

    let pcm = |a: f64, b: f64| 1.0e5 * (a - b) / b;
    let off_pcm = pcm(without.k_eff, UPSTREAM_AS_SHIPPED);
    let on_pcm = pcm(with.k_eff, UPSTREAM_AS_SHIPPED);
    let port_drift = pcm(with.k_eff, without.k_eff);
    let upstream_drift = pcm(UPSTREAM_AS_SHIPPED, UPSTREAM_DRIFT_OFF);

    println!(
        "\n=== MSFR: circulating-fuel precursor drift vs upstream ===\n\
         \tupstream, as shipped (liquidFuel true) = {UPSTREAM_AS_SHIPPED:.6}\n\
         \tupstream, liquidFuel false             = {UPSTREAM_DRIFT_OFF:.6}\n\
         \t--\n\
         \tport, drift off  k = {:.6}  ({off_pcm:+.1} pcm)\n\
         \tport, drift ON   k = {:.6}  ({on_pcm:+.1} pcm)\n\
         \t--\n\
         \tworth of drift: port {port_drift:+.1} pcm, upstream {upstream_drift:+.1} pcm\n\
         \t  (difference {:+.1} pcm)",
        without.k_eff,
        with.k_eff,
        port_drift - upstream_drift,
    );

    assert!(with.converged, "the power iteration did not converge");
    assert!(
        port_drift < 0.0,
        "drift raised k_eff by {port_drift:+.1} pcm. Circulating fuel carries \
         precursors from the high-importance core towards lower-importance \
         regions, so it must lower k_eff; upstream measures {upstream_drift:+.1} pcm."
    );
    // The verification claim: the port reproduces the tutorial's own k_eff.
    assert!(
        on_pcm.abs() < 25.0,
        "port k_eff = {:.6} against upstream's {UPSTREAM_AS_SHIPPED:.6} \
         ({on_pcm:+.1} pcm), outside the 25 pcm bar",
        with.k_eff
    );
    // And the drift term itself agrees, not just the total — a compensating
    // pair of errors would pass the line above but not this one.
    assert!(
        (port_drift - upstream_drift).abs() < 25.0,
        "the port makes drift worth {port_drift:+.1} pcm where upstream makes it \
         {upstream_drift:+.1} pcm"
    );
}
