//! Step 8's Monte Carlo (gh:#573): multigroup cross sections per region at
//! each state point, tallied on the Step 7 neutronics mesh.
//!
//! No physics is implemented here; this is composition:
//!
//! 1. **Data** at the state's temperature: the Step 5 path
//!    (`nee_soon::htr10_rmc::data`, `materials`), every material at `T`.
//! 2. **Two k-eigenvalue passes at the same seed** (byte-identical
//!    histories), `outram_mc_libs::physics::transport_csg::run_keff_csg_hybrid_with_progress`
//!    exactly as Step 5 calls it, with the tally of `nee_soon::mgxs`
//!    ([`nee_soon::mgxs::scalar_tally`] / [`nee_soon::mgxs::matrix_tally`])
//!    rebuilt with the **neutronics mesh** as the spatial filter instead of
//!    materials: `[Mesh, Energy]` with `Flux, Total, Absorption, NuFission,
//!    KappaFission`, and `[Mesh, Energy, EnergyOut]` with `ScatterN,
//!    NuFission`. The unstructured mesh filter splits track lengths across
//!    cells (OpenMC's MOAB `bins_crossed`, `outram_mc_libs::tally::mesh_unstructured`).
//!    **In the delta-tracked pebble bed there is no track length**: flux and
//!    reaction rates there come from the tentative-collision estimator
//!    (`w/Σ_maj` and `w·Σ_x/Σ_maj` at every virtual and real collision site,
//!    binned at the site; `KeffSettings::delta_tally_estimator`, default
//!    since gh:#598). ~~Before gh:#598 the bed flux was the real-collision
//!    estimator split along the mesh as if `1/Σ_t` were a track, which
//!    dropped the helium's flux and left every bed Σ ~1/0.61 too large.~~
//!    Fixed; the old numbers are struck through in the V&V records.
//! 3. **Condensation** with [`nee_soon::mgxs::condense`] (one zone per mesh
//!    cell), then [`nee_soon::mgxs::MgxsLibrary::homogenised_subset`] over
//!    each region's cells (flux-weighted, the same as tallying the region
//!    directly), then `rebalanced()` (Σ_t = Σ_a + Σ_s row; the nee_soon
//!    HTR-10 record took the raw condensation from −17 133 to −353 pcm
//!    against MC k_inf with it) and `in_descending_energy()`.
//! 4. **GeN-Foam file** through `nee_soon::genfoam_xs::to_nuclear_data_input`
//!    per state, the states stacked with `xsVariables { TFuel <law>; }`, and
//!    written by `outram_foam_appbuilder_lib::io::nuclear_data::write_nuclear_data`.
//!
//! **σ of a region's constants** is a first-order estimate: per group, the
//! variance of the region's flux and reaction-rate sums is the sum of its
//! cells' variances (cells treated as independent), and the ratio's relative
//! variance is the sum of the two relative variances (their positive
//! correlation ignored). The tally API keeps only per-cell batch moments, so
//! the exact region batch statistics are not available (gh:#595).

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use dhoby_ghaut::workbench::meshes::Region;
use dhoby_ghaut::workbench::mgxs::{MgxsPlan, MgxsSet, RegionXs, StateXs};
use nee_soon::htr10_rmc::core_model::AssembledCore;
use nee_soon::htr10_rmc::data::{
    load_htr10_nuclides_with_progress, Htr10DataConfig, Htr10NuclideLayout, LoadProgress,
};
use nee_soon::htr10_rmc::keff_vs_height::{bed_majorant, fissile_entropy_mesh, fissile_source_box};
use nee_soon::htr10_rmc::materials::{htr10_material_set, Htr10MaterialConfig};
use nee_soon::mgxs::{condense, GroupStructure, MgxsLibrary, MATRIX_SCORES, SCALAR_SCORES};
use outram_blender::unstructured::UnstructuredMesh;
use outram_foam_appbuilder_lib::genfoam::neutronics::xs::input::NuclearDataInput;
use outram_foam_appbuilder_lib::genfoam::neutronics::xs::variables::{VariableLaw, XsVariable};
use outram_mc_libs::physics::keff::{ComputeType, KeffSettings, ThreadCount};
use outram_mc_libs::physics::transport_csg::{run_keff_csg_hybrid_with_progress, GenerationProgress};
use outram_mc_libs::run_diagnostics::RunDiagnostics;
use outram_mc_libs::tally::filter::{EnergyFilter, EnergyOutFilter, FilterKind, MeshFilter};
use outram_mc_libs::tally::mesh::MeshKind;
use outram_mc_libs::tally::tally::{ScoreType, Tally, TallyBin};
use uom::si::f64::ThermodynamicTemperature;
use uom::si::thermodynamic_temperature::kelvin;

/// What the MGXS run reports while it works.
#[derive(Clone, Debug)]
pub enum MgxsProgress {
    /// A stage of state `state` started.
    Stage { state: usize, what: String },
    /// Nuclear-data progress.
    Data(LoadProgress),
    /// A state point finished.
    StateDone(StateXs),
}

/// One Step 8 job.
pub struct MgxsJob {
    pub plan: MgxsPlan,
    pub core: Arc<AssembledCore>,
    /// The Step 7 neutronics mesh (any unit; MC positions are converted).
    pub mesh: Arc<UnstructuredMesh>,
    /// Region of every neutronics cell.
    pub cell_region: Vec<usize>,
    /// The regions (Step 7's map).
    pub regions: Vec<Region>,
    /// Case directory: the file goes to `constant/neutroRegion/nuclearData`.
    pub out_dir: PathBuf,
    /// Generations of the running pass, `(state, pass, generation)`, written
    /// by the transport thread as each finishes and read by the UI every
    /// frame (shared mutable state is `Arc<RwLock<T>>`, the workspace rule).
    pub live: LiveMgxs,
}

/// See [`MgxsJob::live`].
pub type LiveMgxs = Arc<std::sync::RwLock<Vec<(usize, usize, GenerationProgress)>>>;

fn tallies(groups: &GroupStructure, mesh: &Arc<UnstructuredMesh>) -> (Tally, Tally) {
    let n_c = mesh.n_cells();
    let n_g = groups.n_groups();
    let mesh_f = || {
        FilterKind::Mesh(MeshFilter {
            mesh: MeshKind::Unstructured(mesh.clone()),
        })
    };
    let e = || {
        FilterKind::Energy(EnergyFilter {
            bins: groups.edges().to_vec(),
        })
    };
    let scalar = Tally {
        id: 81,
        name: "workbench mgxs scalar".into(),
        filters: vec![mesh_f(), e()],
        scores: SCALAR_SCORES.to_vec(),
        bins: vec![TallyBin::default(); n_c * n_g * SCALAR_SCORES.len()],
    };
    let matrix = Tally {
        id: 82,
        name: "workbench mgxs matrix".into(),
        filters: vec![
            mesh_f(),
            e(),
            FilterKind::EnergyOut(EnergyOutFilter {
                bins: groups.edges().to_vec(),
            }),
        ],
        // The same two scores, same order, as `nee_soon::mgxs::matrix_tally`.
        scores: vec![ScoreType::ScatterN, ScoreType::NuFission],
        bins: vec![TallyBin::default(); n_c * n_g * n_g * MATRIX_SCORES],
    };
    (scalar, matrix)
}

/// Relative σ of a sum over `cells` of scalar score `s` in group `g`, and the
/// sum itself (cells independent; see the module docs).
fn region_sum(t: &Tally, cells: &[usize], n_g: usize, g: usize, s: usize, n: u64) -> (f64, f64) {
    let n_s = SCALAR_SCORES.len();
    let (mut m, mut v) = (0.0, 0.0);
    for &c in cells {
        let b = &t.bins[(c * n_g + g) * n_s + s];
        let mu = b.mean(n);
        let sd = mu * b.rel_std_dev(n);
        m += mu;
        if sd.is_finite() {
            v += sd * sd;
        }
    }
    (m, if m > 0.0 { v.sqrt() / m } else { 0.0 })
}

/// Run every state point, write the GeN-Foam file, return the set.
pub fn run(job: &MgxsJob, post: &mut impl FnMut(MgxsProgress)) -> Result<MgxsSet, String> {
    let plan = &job.plan;
    let groups = GroupStructure::new(plan.groups.edges_ev()).map_err(|e| format!("{e:?}"))?;
    let n_g = groups.n_groups();
    let core = job.core.as_ref();
    // Regions present on the neutronics mesh, in region order.
    let present: Vec<usize> = (0..job.regions.len())
        .filter(|g| job.cell_region.contains(g))
        .collect();
    let cells_of: Vec<Vec<usize>> = present
        .iter()
        .map(|g| {
            (0..job.cell_region.len())
                .filter(|&c| job.cell_region[c] == *g)
                .collect()
        })
        .collect();
    let vols = outram_blender::unstructured::overlap::cell_volumes_cm3(&job.mesh);
    let cell_names: Vec<String> = (0..job.mesh.n_cells()).map(|c| format!("c{c}")).collect();
    let mut temps = plan.temperatures_k.clone();
    if temps.is_empty() {
        return Err("no state points".into());
    }
    // The first listed is the reference; the rest follow in ascending order.
    let reference = temps[0];
    temps.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mut states: Vec<StateXs> = Vec::new();
    let mut inputs: Vec<(f64, NuclearDataInput)> = Vec::new();
    let mut notes: Vec<String> = Vec::new();
    for (si, &t_k) in temps.iter().enumerate() {
        post(MgxsProgress::Stage {
            state: si,
            what: format!("nuclear data at {t_k} K"),
        });
        let t0 = Instant::now();
        let cfg = Htr10DataConfig {
            temperature: ThermodynamicTemperature::new::<kelvin>(t_k),
            ..Htr10DataConfig::default()
        };
        let layout = Htr10NuclideLayout::plan(&cfg)
            .map_err(|e| format!("nuclide plan refused at {t_k} K: {e}"))?;
        let mut diag = RunDiagnostics::new("dhoby-ghaut mgxs");
        let nuclides = load_htr10_nuclides_with_progress(&cfg, &layout, &mut diag, |p| {
            post(MgxsProgress::Data(p))
        })
        .map_err(|e| format!("nuclear data at {t_k} K: {e}"))?;
        let materials = htr10_material_set(&layout, Htr10MaterialConfig::benchmark_default(t_k));
        for n in &layout.notes {
            if !notes.contains(n) {
                notes.push(n.clone());
            }
        }
        let data_s = t0.elapsed().as_secs_f64();
        let estimator_note = format!(
            "delta-tracked bed tallied with the {:?} estimator (gh:#598)",
            KeffSettings::default().delta_tally_estimator
        );
        if !notes.contains(&estimator_note) {
            notes.push(estimator_note);
        }
        let settings = KeffSettings {
            n_particles: plan.particles,
            n_inactive: plan.inactive,
            n_active: plan.active,
            temperature_k: t_k,
            seed: plan.seed,
            compute: ComputeType::CpuMultiThread(ThreadCount::Fixed(plan.threads.max(1))),
            ..KeffSettings::default()
        };
        let majorant = bed_majorant(&materials, &nuclides);
        let entropy = fissile_entropy_mesh(core);
        let (mut scalar, mut matrix) = tallies(&groups, &job.mesh);
        let t1 = Instant::now();
        let mut ks = [0.0; 2];
        let mut kstd = 0.0;
        let mut histories = 0;
        for (pass, tally) in [&mut scalar, &mut matrix].into_iter().enumerate() {
            post(MgxsProgress::Stage {
                state: si,
                what: format!(
                    "{} pass at {t_k} K",
                    if pass == 0 {
                        "reaction-rate"
                    } else {
                        "scattering-matrix"
                    }
                ),
            });
            let live = job.live.clone();
            let res = run_keff_csg_hybrid_with_progress(
                &core.geometry,
                &materials,
                &nuclides,
                std::slice::from_ref(&majorant),
                Some(&entropy),
                fissile_source_box(core),
                &settings,
                Some(tally),
                move |g| {
                    if let Ok(mut l) = live.write() {
                        l.push((si, pass, g));
                    }
                },
            );
            ks[pass] = res.k_mean;
            kstd = res.k_std;
            histories = res.histories;
        }
        if (ks[0] - ks[1]).abs() > 1e-12 {
            notes.push(format!(
                "at {t_k} K the two passes gave k = {:.6} and {:.6}: their histories differ, so the flux of the first is not the exact denominator of the second",
                ks[0], ks[1]
            ));
        }
        let transport_s = t1.elapsed().as_secs_f64();
        let n = plan.active as u64;
        let per_cell =
            condense(&groups, &cell_names, &scalar, &matrix, n).map_err(|e| format!("{e:?}"))?;
        let mut zones = Vec::with_capacity(present.len());
        for (k, &g) in present.iter().enumerate() {
            let one = per_cell.homogenised_subset(&cells_of[k], job.regions[g].id.clone());
            zones.extend(one.zones);
        }
        let lib = MgxsLibrary {
            groups: groups.clone(),
            zones,
        }
        .rebalanced()
        .in_descending_energy();
        // σ, ascending then reversed to match.
        let regions: Vec<RegionXs> = present
            .iter()
            .enumerate()
            .map(|(k, &g)| {
                let cells = &cells_of[k];
                let rel = |s: usize| -> Vec<f64> {
                    let v: Vec<f64> = (0..n_g)
                        .map(|gg| {
                            let (_, rf) = region_sum(&scalar, cells, n_g, gg, 0, n);
                            let (_, rx) = region_sum(&scalar, cells, n_g, gg, s, n);
                            if s == 0 {
                                rf
                            } else {
                                (rf * rf + rx * rx).sqrt()
                            }
                        })
                        .collect();
                    v.into_iter().rev().collect()
                };
                let z = &lib.zones[k];
                RegionXs {
                    region: job.regions[g].id.clone(),
                    volume_cm3: cells.iter().map(|&c| vols[c]).sum(),
                    flux: z.flux.clone(),
                    flux_rel_sigma: rel(0),
                    total: z.total.clone(),
                    absorption: z.absorption.clone(),
                    nu_fission: z.nu_fission.clone(),
                    kappa_fission: z.kappa_fission.clone(),
                    chi: z.chi.clone(),
                    scatter: z.scatter.clone(),
                    rel_sigma_total: rel(1),
                    rel_sigma_absorption: rel(2),
                    rel_sigma_nu_fission: rel(3),
                }
            })
            .collect();
        let input =
            nee_soon::genfoam_xs::to_nuclear_data_input(&lib).map_err(|e| format!("{e:?}"))?;
        inputs.push((t_k, input));
        let st = StateXs {
            temperature_k: t_k,
            k: ks[0],
            k_sigma: kstd,
            histories,
            data_s,
            transport_s,
            regions,
        };
        post(MgxsProgress::StateDone(st.clone()));
        states.push(st);
    }
    let nd = stack_states(&inputs, reference, plan.law);
    let header = format!(
        "Written by the Dhoby Ghaut workbench, Step 8 (gh:#573), {}.\n\
         TENTATIVE: research, education and V&V only. Monte Carlo MGXS of the HTR-10\n\
         preset (nee_soon::htr10_rmc), {} groups, {} histories x {} active generations per pass,\n\
         per region of the Step 7 neutronics mesh. Each state point puts EVERY material at\n\
         TFuel (isothermal core); no delayed-neutron data (precGroups 0, gh:#595).\n\
         Zones are the neutronics polyMesh cellZones. Units: MKSA (1/m).",
        dhoby_ghaut::workbench::recipe::now_rfc3339(),
        n_g,
        plan.particles,
        plan.active
    );
    let path = write_nuclear_data_checked(&nd, &header, &job.out_dir)?;
    let mut edges = groups.edges().to_vec();
    edges.reverse();
    Ok(MgxsSet {
        edges_ev_desc: edges,
        law: plan.law,
        states,
        nuclear_data_path: Some(path.display().to_string()),
        notes,
    })
}

/// Stack one single-state `nuclearData` per temperature into one file with
/// `xsVariables { TFuel <law>; }`. GeN-Foam's `reference` state is the one
/// at `reference` \[K\] (the first temperature the user listed); the others
/// follow in the order given.
pub fn stack_states(
    inputs: &[(f64, NuclearDataInput)],
    reference: f64,
    law: dhoby_ghaut::workbench::mgxs::InterpLaw,
) -> NuclearDataInput {
    let ri = inputs.iter().position(|x| x.0 == reference).unwrap_or(0);
    let law = match law {
        dhoby_ghaut::workbench::mgxs::InterpLaw::LnT => VariableLaw::Log,
        dhoby_ghaut::workbench::mgxs::InterpLaw::SqrtT => VariableLaw::Sqrt,
        dhoby_ghaut::workbench::mgxs::InterpLaw::Linear => VariableLaw::Linear,
    };
    let mut nd = inputs[ri].1.clone();
    nd.xs_variables = vec![XsVariable::new("TFuel", law)];
    nd.states.clear();
    let mut order: Vec<usize> = vec![ri];
    order.extend((0..inputs.len()).filter(|&i| i != ri));
    for i in order {
        let (t_k, inp) = &inputs[i];
        let mut s = inp.states[0].clone();
        s.name = if i == ri {
            "reference".into()
        } else {
            format!("T{:.0}K", t_k)
        };
        s.parameters.clear();
        s.parameters.insert("TFuel".into(), *t_k);
        nd.states.push(s);
    }
    nd
}

/// Write `nd` as `<case>/constant/neutroRegion/nuclearData` and read it back
/// through the solver's reader and `CrossSectionData::from_input`, so a file
/// the multiphysics step cannot read is an error here and not there.
pub fn write_nuclear_data_checked(
    nd: &NuclearDataInput,
    header: &str,
    case_dir: &Path,
) -> Result<PathBuf, String> {
    let text = outram_foam_appbuilder_lib::io::nuclear_data::write_nuclear_data(nd, header);
    let dir = case_dir.join("constant").join("neutroRegion");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join("nuclearData");
    std::fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))?;
    let back = outram_foam_appbuilder_lib::io::nuclear_data::read_nuclear_data(&path)
        .map_err(|e| e.to_string())?;
    outram_foam_appbuilder_lib::genfoam::neutronics::xs::CrossSectionData::from_input(&back)
        .map_err(|e| format!("the written nuclearData does not build CrossSectionData: {e:?}"))?;
    Ok(path)
}

/// The run as CSV rows (one per state, region, group), for the headless mode.
pub fn csv(set: &MgxsSet) -> String {
    let mut s = String::from(
        "temperature_k,k,k_sigma,region,group,e_hi_ev,e_lo_ev,flux,flux_rel_sigma,sigma_t_per_cm,rel_sigma_t,sigma_a_per_cm,rel_sigma_a,nu_sigma_f_per_cm,rel_sigma_nu_f,chi,sigma_s_gg_per_cm,d_cm\n",
    );
    for st in &set.states {
        for r in &st.regions {
            for g in 0..set.n_groups() {
                s.push_str(&format!(
                    "{},{:.5},{:.5},{},{},{:.4e},{:.4e},{:.5e},{:.3e},{:.5e},{:.3e},{:.5e},{:.3e},{:.5e},{:.3e},{:.4},{:.5e},{:.5e}\n",
                    st.temperature_k,
                    st.k,
                    st.k_sigma,
                    r.region,
                    g,
                    set.edges_ev_desc[g],
                    set.edges_ev_desc[g + 1],
                    r.flux[g],
                    r.flux_rel_sigma[g],
                    r.total[g],
                    r.rel_sigma_total[g],
                    r.absorption[g],
                    r.rel_sigma_absorption[g],
                    r.nu_fission[g],
                    r.rel_sigma_nu_fission[g],
                    r.chi[g],
                    r.scatter[g][g],
                    r.diffusion(g)
                ));
            }
        }
    }
    s
}

/// Write the CSV beside the case, and the set itself as `mgxs_set.toml` (the
/// hand-off a later `--headless-multiphysics --case` reads).
pub fn write_csv(set: &MgxsSet, dir: &Path) -> Result<PathBuf, String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let p = dir.join("mgxs.csv");
    std::fs::write(&p, csv(set)).map_err(|e| e.to_string())?;
    let t = dir.join("mgxs_set.toml");
    std::fs::write(&t, set.to_toml()?).map_err(|e| e.to_string())?;
    Ok(p)
}
