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
//! **NOT compared:** upstream's `expectedKeff` is produced by its full coupled
//! `steadyStateEN` run — a thermal-hydraulic steady state first, then the
//! neutronics eigenvalue with cross sections evaluated at the *converged* field
//! temperatures and densities, not at the nominal reference state. These tests
//! evaluate at the reference state, because the port has no coupled
//! multi-region driver wired to a case. The gap between the two is a physical
//! feedback effect, not a numerical error, and each test states the measured
//! size of that gap rather than hiding it in a loose tolerance.
//!
//! This is therefore a **verification of the neutronics path against an
//! independently-produced reference**, not a reproduction of upstream's coupled
//! result. It is not validation against experiment.
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
//! | case | port | upstream | difference |
//! |---|---|---|---|
//! | ESFR, reference state, upstream's own zero-flux boundary | 0.944987 | 0.936827 | **+871 pcm**, correct sign for negative SFR feedback |
//! | MSFR, vacuum lower bound | 0.958444 | 0.960283 | −191 pcm |
//! | MSFR, reflective upper bound | 1.075834 | 0.960283 | +12 033 pcm |
//!
//! Upstream's MSFR value falls **inside** the port's leakage bracket, which is
//! the strongest statement available while the `albedoSP3` boundary the case
//! actually uses is unimplemented. Full methodology, interpretation and the
//! wrong answer that the wedge-patch treatment fixed are in each test's own doc
//! comment.
//!
//! Not covered here: `3D_gFHR` and `1D_PSBT_SC`, whose upstream references are
//! thermal-hydraulic (`Tfmax`, subchannel conditions) rather than `k_eff`, and
//! which need the porous-medium TH path rather than the neutronics one.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use outram_foam_appbuilder_lib::genfoam::neutronics::diffusion::{
    DiffusionNeutronics, DiffusionSettings,
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
    };

    let mut solver =
        DiffusionNeutronics::new(mesh.clone(), &xs, &zone_map, &raw, &flux_boundary, settings)
            .expect("build the diffusion solver");
    for (name, gamma) in albedo_patches {
        let idx = mesh
            .patches
            .iter()
            .position(|p| p.name == *name)
            .unwrap_or_else(|| panic!("mesh has no `{name}` patch"));
        solver.set_albedo_boundary(idx, *gamma, AlbedoLinearisation::CellValue);
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

/// **V&V — MSFR: `k_eff` with upstream's own albedo boundary conditions.**
///
/// ## Methodology
///
/// The same mesh, cross sections and reference state as
/// [`msfr_reference_state_keff_brackets_upstream`], now posed with the boundary
/// conditions upstream's `0/neutroRegion/defaultFlux` actually specifies:
///
/// | patch | upstream | here |
/// |---|---|---|
/// | `front`, `back` | `wedge` | zero gradient (symmetry planes) |
/// | `topwall`, `bottomwall`, `reflector` | `albedoSP3`, `gamma 0.1` | albedo, `gamma = 0.1` |
/// | `hx` | `albedoSP3`, **`gamma 0.5`** | albedo, `gamma = 0.5` |
///
/// The `hx` patch is vacuum-valued, not 0.1 like the other three — worth calling
/// out because assuming a single `gamma` for the whole boundary is the obvious
/// mistake, and one this test made before the dictionary was read in full.
///
/// The albedo condition is a port of upstream's `albedoSP3` verified against
/// upstream's own coefficient functions, using
/// [`AlbedoLinearisation::CellValue`] — upstream's `gradientInternalCoeffs()`
/// returns `-gamma/D` against the *cell* value, with no face-value closure. See
/// `genfoam::neutronics::albedo`.
///
/// ## What this does and does not settle
///
/// The boundary treatment is now upstream's, so the **spatial neutronics** —
/// mesh, cross sections, zone map, diffusion operator, boundary conditions,
/// power iteration — is posed identically. What remains different is the
/// **state at which the cross sections are evaluated**: upstream's
/// `expectedKeff` comes from its coupled `steadyStateEN` stage, where the salt
/// has heated under 20 MW against a heat exchanger held at 900 K, so its cross
/// sections sit at the converged `TFuel`/`rhoCool`; this test evaluates at the
/// nominal reference state (`TFuel 900 K`, `rhoCool 4125 kg/m3`), because the
/// port has no coupled thermal-hydraulic driver to produce the fed-back state.
///
/// **That is a real gap and this test does not paper over it.** Two things are
/// asserted, both from upstream's own data and neither fitted to the answer:
///
/// 1. **Sign.** The MSFR's feedbacks are negative, so upstream's coupled value
///    must fall *below* the reference-state value. If the port came out below
///    upstream, no amount of feedback could explain it.
/// 2. **Magnitude.** The gap must be within the span the case's own perturbed
///    states can produce — see
///    [`msfr_reactivity_coefficients_from_upstreams_perturbed_states`], which
///    bounds it at `k(TFuel 1500 K, rhoCool 3419) = 0.923777`.
///
/// Closing the remaining gap to a direct equality needs the coupled TH solve
/// (pump, buoyancy, turbulence, the `fixedTemperature` heat exchanger) and the
/// circulating-fuel precursor drift the README mentions. Tracked under
/// `op-2df1`.
///
/// ## Results (measured 2026-09-15)
///
/// | quantity | value |
/// |---|---|
/// | `k_eff`, port, reference state, upstream's boundaries | **0.979508** |
/// | `k_eff`, upstream coupled `expectedKeff` | 0.960283 |
/// | difference | **+2002 pcm** |
/// | port at `TFuel 1500 K` | 0.960741 |
/// | port at `TFuel 1500 K` + `rhoCool 3419` | 0.923777 |
///
/// The measured feedback coefficients are **−3.32 pcm/K** in `TFuel` and
/// **+5.28 pcm per kg/m3** in `rhoCool`. A +2002 pcm gap is therefore what a
/// core running some 600 K above the 900 K cold leg produces, or a smaller
/// temperature rise combined with the density drop that accompanies it — both
/// physically ordinary for this case, and neither verifiable from here.
///
/// For scale: swapping the boundary from a blanket `fixedValue 0` to upstream's
/// actual albedo moved `k_eff` by **+2106 pcm** (0.958444 to 0.979508), so the
/// boundary condition ported here is worth about as much as the entire
/// remaining discrepancy.
#[test]
fn msfr_keff_with_upstreams_albedo_boundary() {
    let root = require_upstream!();
    let case = root.join("Tutorials/reactorCases/2D_MSFR/rootCase");
    let Some(region) = stage_region(&case, "neutroRegion", "msfr_albedo") else {
        println!("SKIP: could not stage the MSFR case (is `gzip` available?)");
        return;
    };

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

    let expected = 0.960283;
    let pcm = report("MSFR (2D_MSFR) — upstream's albedo boundary", &s, expected);

    assert!(s.converged, "the power iteration did not converge");
    assert!(
        pcm > 0.0,
        "the reference-state k_eff ({:.6}) is BELOW upstream's coupled value \
         ({expected}). The MSFR's feedbacks are negative, so no fed-back state \
         can raise k above the reference one — the discrepancy cannot be \
         feedback and is a defect somewhere in the neutronics.",
        s.k_eff
    );
    // The fully-perturbed state of the case's own data is the floor; upstream's
    // converged state lies between it and the reference state.
    assert!(
        s.k_eff > expected && expected > 0.923777,
        "upstream's k_eff is outside the feedback span [0.923777, {:.6}] the \
         case's own perturbed states allow",
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
