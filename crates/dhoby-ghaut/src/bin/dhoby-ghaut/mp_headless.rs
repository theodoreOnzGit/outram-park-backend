//! `--headless-multiphysics`: Step 9's case run as Step 10 runs it, with no
//! window. Prints the coupling console, the summary against Gao & Shi
//! (2002), writes the r-z fields as CSV and a PNG of the TH mesh coloured by
//! temperature, and saves the recipe with the `step-9` section. With the
//! solved shape ([`headless_spatial`], the default) it also prints the
//! isothermal diffusion `k` against Step 8's Monte Carlo `k` and draws the
//! computed power and `TFuel` on the neutronics mesh.

use std::path::Path;

use dhoby_ghaut::workbench::multiphysics::MultiphysicsSetup;
use dhoby_ghaut::workbench::recipe::{now_rfc3339, Recipe};

use crate::porous_core::{Fields, Summary};

/// The comparison rows: (what, ours, published, note).
pub fn comparison(s: &Summary) -> Vec<(&'static str, f64, f64, &'static str)> {
    let ours = [
        s.vessel_outlet_c,
        s.max_helium_c,
        s.max_kernel_c,
        s.max_surface_c,
        s.dp_kpa,
    ];
    crate::mp_preset::PUBLISHED
        .iter()
        .zip(ours)
        .map(|((what, v, note), o)| (*what, o, *v, *note))
        .collect()
}

/// The comparison with Gao & Shi's **initial** core (the preset recipe's
/// core is the initial loading): (what, ours, published, note).
pub fn initial_comparison(s: &Summary) -> Vec<(&'static str, f64, f64, &'static str)> {
    let ours = [
        s.max_power_density_w_cm3,
        s.mean_fuel_pebble_c,
        s.max_kernel_c,
        s.vessel_outlet_c,
    ];
    crate::mp_preset::PUBLISHED_INITIAL
        .iter()
        .zip(ours)
        .map(|((what, v, note), o)| (*what, o, *v, *note))
        .collect()
}

/// One line per summary quantity (shared with the UI console).
pub fn summary_lines(s: &Summary) -> Vec<String> {
    let mut v = vec![
        format!(
            "{} after {} coupling iterations",
            if s.converged { "CONVERGED" } else { "NOT converged" },
            s.iterations
        ),
        if s.k_eff.is_some() {
            format!(
                "power shape SOLVED (diffusion): node peak/mean {:.4}, max node power density {:.3} W/cm³ (Gao & Shi: 2.57 max, 2.0 mean; the prescribed ablation: 2.57/2.0 = 1.285)",
                s.node_peak_to_mean, s.max_power_density_w_cm3
            )
        } else {
            format!(
                "power shape PRESCRIBED (ablation): peak/mean {:.4} (node average {:.4}), extrapolation length {:.1} cm",
                s.peak_to_mean, s.node_peak_to_mean, s.shape_delta_cm
            )
        },
        format!("bed exit (mixed) {:.1} °C; vessel outlet with bypass {:.1} °C", s.core_exit_c, s.vessel_outlet_c),
        format!("max helium {:.1} °C; max fuel-pebble surface {:.1} °C; max pebble centre {:.1} °C", s.max_helium_c, s.max_surface_c, s.max_centre_c),
        format!("max kernel (peak fuel) {:.1} °C; power-weighted mean fuel pebble {:.1} °C", s.max_kernel_c, s.mean_fuel_pebble_c),
        format!(
            "bed pressure drop {:.3} kPa; superficial velocity at the inlet {:.2} m/s (interstitial {:.2}), largest at a ring exit {:.2} m/s (interstitial {:.2})",
            s.dp_kpa,
            s.inlet_superficial_velocity_m_s,
            s.inlet_superficial_velocity_m_s / s.porosity,
            s.max_exit_superficial_velocity_m_s,
            s.max_exit_superficial_velocity_m_s / s.porosity
        ),
        match s.k_eff {
            Some(k) => format!(
                "k_eff = {k:.5} (diffusion eigenvalue at the converged temperatures; power-weighted fuel-pebble T {:.1} °C)",
                s.t_feedback_c
            ),
            None => format!(
                "feedback T {:.1} °C -> rho {:+.0} pcm -> k = {:.5} relative to a cold-critical reference (LUMPED, not an eigenvalue)",
                s.t_feedback_c, s.rho_pcm, s.k_vs_cold_critical
            ),
        },
        format!("energy balance (heat to helium / power - 1) {:.2e}", s.energy_balance_rel),
        format!(
            "particle Re {:.0}-{:.0}; {} nodes outside Wakao's 15-8500",
            s.re_min, s.re_max, s.wakao_out_of_range
        ),
    ];
    if s.k_eff.is_some() {
        v.push("TENTATIVE comparison with Gao & Shi (2002), INITIAL core (the preset recipe is an initial-core loading; ours is nominal, theirs may include the §4.1 factors):".into());
        for (what, ours, publ, note) in initial_comparison(s) {
            v.push(format!(
                "  {what:<34} ours {ours:8.2}  published {publ:8.2}  diff {:+8.2}  ({note})",
                ours - publ
            ));
        }
    }
    v.push("TENTATIVE comparison with Gao & Shi (2002), equilibrium core, 100 %:".into());
    for (what, ours, publ, note) in comparison(s) {
        v.push(format!(
            "  {what:<34} ours {ours:8.2}  published {publ:8.2}  diff {:+8.2}  ({note})",
            ours - publ
        ));
    }
    v
}

/// The fields as CSV, one row per node.
pub fn fields_csv(f: &Fields) -> String {
    let mut out = String::from("ring,node,r_in_m,r_out_m,z_top_m,z_bottom_m,q_w_m3,t_helium_k,t_surface_k,t_centre_k,t_kernel_k,t_pebble_avg_k,reynolds\n");
    let dz = f.height_m / f.n_z as f64;
    for i in 0..f.n_r {
        let r_in = if i == 0 { 0.0 } else { f.ring_outer_m[i - 1] };
        for j in 0..f.n_z {
            let k = i * f.n_z + j;
            out.push_str(&format!(
                "{i},{j},{r_in:.5},{:.5},{:.5},{:.5},{:.6e},{:.3},{:.3},{:.3},{:.3},{:.3},{:.1}\n",
                f.ring_outer_m[i],
                j as f64 * dz,
                (j + 1) as f64 * dz,
                f.q_w_m3[k],
                f.t_helium_k[k],
                f.t_surface_k[k],
                f.t_centre_k[k],
                f.t_kernel_k[k],
                f.t_pebble_avg_k[k],
                f.reynolds[k]
            ));
        }
    }
    out
}

/// A one-line, reproducible CSV of the summary (pinned by the fixture).
pub fn summary_csv(s: &Summary) -> String {
    format!(
        "converged,iterations,core_exit_c,vessel_outlet_c,max_helium_c,max_surface_c,max_kernel_c,dp_kpa,t_feedback_c,rho_pcm,k_vs_cold_critical\n\
         {},{},{:.1},{:.1},{:.1},{:.1},{:.1},{:.3},{:.1},{:.0},{:.4}",
        s.converged,
        s.iterations,
        s.core_exit_c,
        s.vessel_outlet_c,
        s.max_helium_c,
        s.max_surface_c,
        s.max_kernel_c,
        s.dp_kpa,
        s.t_feedback_c,
        s.rho_pcm,
        s.k_vs_cold_critical
    )
}

/// One console line of a solved-shape iteration (shared with the UI).
pub fn spatial_line(r: &crate::porous_core::IterationReport) -> String {
    let sp = r.spatial.as_ref();
    format!(
        "{:5}  {:9.2e}  {:9.4}  {:9.2e}  {:8.5}  {:8.1e}  {:12.2}  {:13.2}  {:5.3}/{:5.3}/{:5.3}  {:6.4}  {:4}  {:5}  {:5.1}",
        r.iteration,
        r.flow_residual,
        r.temperature_change_k,
        sp.map_or(f64::NAN, |s| s.power_change),
        sp.map_or(f64::NAN, |s| s.k_eff),
        sp.map_or(f64::NAN, |s| s.k_change),
        r.max_helium_k - 273.15,
        r.max_kernel_k - 273.15,
        sp.map_or(f64::NAN, |s| s.split.bed),
        sp.map_or(f64::NAN, |s| s.split.cavity),
        sp.map_or(f64::NAN, |s| s.split.outside),
        sp.map_or(f64::NAN, |s| s.rescale),
        sp.map_or(0, |s| s.outer_iterations),
        sp.map_or(0, |s| s.extrapolated_cells),
        sp.map_or(0.0, |s| s.neutronics_s),
    )
}

/// The header of [`spatial_line`].
pub const SPATIAL_HEADER: &str = " iter  flow res.  max dT[K]  power res.  k_eff     |dk|      T_he,max [C]  T_kern,max [C]  P bed/cav/out      rescale  outers  extrap  t_n[s]";

/// Read Step 8's set from a `--headless-mgxs` case folder (`mgxs_set.toml`);
/// if the `nuclearData` path it records is not there (run from elsewhere),
/// use the case's own `constant/neutroRegion/nuclearData`.
///
/// # Errors
///
/// A missing or unreadable file.
pub fn load_mgxs(case: &Path) -> Result<dhoby_ghaut::workbench::mgxs::MgxsSet, String> {
    let p = case.join("mgxs_set.toml");
    let text = std::fs::read_to_string(&p).map_err(|e| format!("{}: {e} (run --headless-mgxs --out {} first)", p.display(), case.display()))?;
    let mut set = dhoby_ghaut::workbench::mgxs::MgxsSet::from_toml(&text)
        .map_err(|e| format!("{}: {e}", p.display()))?;
    let here = case.join("constant").join("neutroRegion").join("nuclearData");
    let recorded_ok = set
        .nuclear_data_path
        .as_ref()
        .is_some_and(|q| Path::new(q).exists());
    if !recorded_ok && here.exists() {
        set.nuclear_data_path = Some(here.display().to_string());
    }
    Ok(set)
}

/// **Diagnostic only** (`--diagnostic-bed-sigma-scale F`, the gh:#591 V&V
/// record): a copy of Step 8's `nuclearData` with every bed region's
/// macroscopic constants multiplied by `f` (`D` divided by it), written into
/// `out/diagnostic_nuclear_data/` and pointed at. It tests the hypothesis
/// that Step 8's collision-estimator flux in the delta-tracked bed misses the
/// helium voids (flux low by the filling fraction, so every bed `Σ` high by
/// its inverse). Never the default; the fix belongs in Step 8.
///
/// **SUPERSEDED 2026-10-06 (gh:#598).** The hypothesis held, and the cause
/// was fixed in the Monte Carlo tally (the delta-tracked bed is now scored
/// with the tentative-collision estimator, binned at the site). Step 8 data
/// written after that fix already carry the helium's flux, so applying this
/// scale to them double-corrects. It is kept only to reproduce the gh:#591
/// record on the pre-fix data, and the run prints a warning.
///
/// # Errors
///
/// A file failure.
pub fn diagnostic_bed_scale(
    mgxs: &mut dhoby_ghaut::workbench::mgxs::MgxsSet,
    meshes: &dhoby_ghaut::workbench::meshes::MeshSet,
    f: f64,
    out: &Path,
) -> Result<Vec<String>, String> {
    use outram_foam_appbuilder_lib::io::nuclear_data::{read_nuclear_data, write_nuclear_data};
    let src = mgxs.nuclear_data_path.clone().ok_or("no nuclearData")?;
    let mut nd = read_nuclear_data(Path::new(&src)).map_err(|e| e.to_string())?;
    let regions = &meshes.plan.regions;
    let bed: Vec<String> = regions
        .regions
        .iter()
        .enumerate()
        .filter(|(g, _)| regions.is_bed(*g))
        .map(|(_, r)| r.id.clone())
        .collect();
    for st in &mut nd.states {
        for z in &mut st.zones {
            if !bed.contains(&z.name) {
                continue;
            }
            for v in [&mut z.nu_sigma_eff, &mut z.sigma_pow, &mut z.sigma_removal] {
                v.iter_mut().for_each(|x| *x *= f);
            }
            z.d.iter_mut().for_each(|x| *x /= f);
            for m in &mut z.scattering {
                for row in m {
                    row.iter_mut().for_each(|x| *x *= f);
                }
            }
        }
    }
    let dir = out.join("diagnostic_nuclear_data");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let p = dir.join("nuclearData");
    let header = format!(
        "DIAGNOSTIC COPY (gh:#591 V&V), NOT Step 8's data: zones {bed:?} scaled by f = {f} (Sigma x f, D / f)\nto test whether Step 8's bed flux misses the helium voids. Source: {src}"
    );
    std::fs::write(&p, write_nuclear_data(&nd, &header)).map_err(|e| e.to_string())?;
    mgxs.nuclear_data_path = Some(p.display().to_string());
    Ok(bed)
}

/// The solved-shape run (gh:#591): the isothermal `k` comparison, then the
/// Picard coupling, then the files and images. `only_k` stops after the
/// comparison (for the mesh and boundary studies).
///
/// # Errors
///
/// A solver or file failure.
pub fn headless_spatial(
    recipe: &Recipe,
    setup: &MultiphysicsSetup,
    built: crate::meshing::BuiltMeshes,
    mgxs: dhoby_ghaut::workbench::mgxs::MgxsSet,
    boundary: crate::spatial::NeutronBoundary,
    out: &Path,
    only_k: bool,
) -> Result<(), String> {
    use dhoby_ghaut::workbench::meshes::MeshRole;
    use outram_blender::csg::plot::PlotBasis;
    let t0 = std::time::Instant::now();
    println!("Step 10 coupled run, power SOLVED by diffusion (TENTATIVE; see the element list)");
    for e in &setup.elements {
        println!("  [{}] {}: {}", e.status.badge(), e.name, e.note);
    }
    let n_mesh = built.meshes[MeshRole::Neutronics.index()].clone();
    let th_mesh = built.meshes[MeshRole::ThermalHydraulics.index()].clone();
    let inputs = dhoby_ghaut::workbench::multiphysics::MultiphysicsInputs {
        meshes: built.set,
        mgxs,
    };
    inputs.check_for_neutronics()?;
    let (setup, moved) = crate::mp_preset::on_built_core(setup.clone(), &inputs.meshes.domain, recipe);
    let setup = &setup;
    for m in &moved {
        println!("Step 9 adjusted: {m}");
    }
    let nm = inputs.meshes.mesh(MeshRole::Neutronics).expect("checked");
    let tm = inputs.meshes.mesh(MeshRole::ThermalHydraulics).expect("checked");
    println!(
        "meshes: neutronics {} cells ({} cm), TH {} cells ({} cm); {} groups, state points {:?} K, law {}",
        nm.cells,
        inputs.meshes.plan.roles[0].cell_size_cm,
        tm.cells,
        inputs.meshes.plan.roles[1].cell_size_cm,
        inputs.mgxs.n_groups(),
        inputs.mgxs.states.iter().map(|s| s.temperature_k).collect::<Vec<_>>(),
        inputs.mgxs.law.keyword()
    );
    println!("isothermal k (every neutronics cell at the state temperature), boundary: {}", boundary.label());
    println!("   T [K]   k diffusion   k Monte Carlo (Step 8)       diff [pcm]     P/A diffusion  P/A Monte Carlo");
    let ks = crate::spatial::isothermal_k(&inputs, boundary)?;
    let mut krows = String::from("temperature_k,k_diffusion,k_mc,k_mc_sigma,diff_pcm,kinf_system_diffusion,kinf_system_mc\n");
    let mut regrows = String::from("temperature_k,region,production_share_diffusion,production_share_mc,absorption_share_diffusion,absorption_share_mc,flux_g_diffusion,flux_g_mc\n");
    for i in &ks {
        let d = (i.k_diffusion - i.k_mc) * 1e5;
        println!(
            "  {:7.2}   {:.5}       {:.5} ± {:.5}      {d:+8.0} ({:+.1} σ)   {:.5}        {:.5}",
            i.t_k, i.k_diffusion, i.k_mc, i.k_mc_sigma, d / (i.k_mc_sigma * 1e5), i.kinf_system_diffusion, i.kinf_system_mc
        );
        krows.push_str(&format!(
            "{},{:.6},{:.6},{:.6},{d:.0},{:.6},{:.6}\n",
            i.t_k, i.k_diffusion, i.k_mc, i.k_mc_sigma, i.kinf_system_diffusion, i.kinf_system_mc
        ));
        for (a, b) in i.diffusion.iter().zip(&i.mc) {
            regrows.push_str(&format!(
                "{},{},{:.5},{:.5},{:.5},{:.5},{},{}\n",
                i.t_k,
                a.region,
                a.production,
                b.production,
                a.absorption,
                b.absorption,
                a.flux.iter().map(|f| format!("{f:.4e}")).collect::<Vec<_>>().join(" "),
                b.flux.iter().map(|f| format!("{f:.4e}")).collect::<Vec<_>>().join(" ")
            ));
        }
    }
    if let Some(i) = ks.first() {
        println!("region balance at {} K (shares of production / absorption; flux per group scaled to the MC production):", i.t_k);
        println!("  region               prod D   prod MC   abs D    abs MC   flux D (fast, ...)          flux MC");
        for (a, b) in i.diffusion.iter().zip(&i.mc) {
            println!(
                "  {:<20} {:7.4}  {:7.4}   {:7.4}  {:7.4}   {:<26} {}",
                a.region,
                a.production,
                b.production,
                a.absorption,
                b.absorption,
                a.flux.iter().map(|f| format!("{f:.3e}")).collect::<Vec<_>>().join(" "),
                b.flux.iter().map(|f| format!("{f:.3e}")).collect::<Vec<_>>().join(" ")
            );
        }
    }
    std::fs::create_dir_all(out).map_err(|e| e.to_string())?;
    std::fs::write(out.join("isothermal_k.csv"), krows).map_err(|e| e.to_string())?;
    std::fs::write(out.join("isothermal_region_balance.csv"), regrows).map_err(|e| e.to_string())?;
    // The cold isothermal shape on its own (no TH): what the first coupling
    // iteration hands the march, and the diffusion shape at the reference
    // state, drawn on the mesh.
    {
        let t_ref = inputs.mgxs.states[0].temperature_k;
        let mut neut = crate::spatial::Neutronics::new(&inputs, boundary)?;
        let core = crate::porous_core::PorousCore::new(setup)?;
        let mut notes = Vec::new();
        let tr = crate::spatial::Transfer::new(&inputs, &th_mesh, &core, &mut notes)?;
        let p_w = setup.neutronics.thermal_power_mw * 1e6;
        let sol = neut.solve(&vec![t_ref; neut.mesh().n_cells], p_w)?;
        let (raw, split, _bed) = tr.power_to_nodes_raw(&inputs, &sol.q_w_m3, p_w)?;
        let q: Vec<f64> = raw
            .iter()
            .enumerate()
            .map(|(k, p)| p * p_w / raw.iter().sum::<f64>() / core.node_volume_m3(k))
            .collect();
        let mean = p_w / (0..q.len()).map(|k| core.node_volume_m3(k)).sum::<f64>();
        let (kmax, qmax) = q
            .iter()
            .copied()
            .enumerate()
            .fold((0, 0.0), |a, (k, v)| if v > a.1 { (k, v) } else { a });
        let nz = core.fields().n_z;
        let qn_max = sol.q_w_m3.iter().copied().fold(0.0, f64::max);
        println!(
            "isothermal {t_ref} K shape at {:.1} MW: node peak/mean {:.3} (max {:.3} W/cm³ at ring {} node {} of {}), neutronics-cell peak {:.3} W/cm³; split bed/cavity/outside {:.3}/{:.3}/{:.3}",
            p_w * 1e-6,
            qmax / mean,
            qmax * 1e-6,
            kmax / nz,
            kmax % nz,
            nz,
            qn_max * 1e-6,
            split.bed,
            split.cavity,
            split.outside
        );
        let img = crate::spatial_draw::draw_field(
            &n_mesh,
            &sol.q_w_m3.iter().map(|q| q * 1e-6).collect::<Vec<_>>(),
            PlotBasis::Xz,
            [0.0; 3],
            &format!("ISOTHERMAL {t_ref} K DIFFUSION POWER DENSITY (NO FEEDBACK), X-Z"),
            "W/CM3",
            1e-9,
            "NO FISSION POWER",
            900,
        )?;
        img.write_png(&out.join("neutronics_power_isothermal_xz.png"))
            .map_err(|e| e.to_string())?;
    }
    if only_k {
        return Ok(());
    }
    println!("{SPATIAL_HEADER}");
    let (s, f, sf) = crate::spatial::solve(
        setup,
        &inputs,
        &th_mesh,
        boundary,
        || false,
        |notes| {
            for n in notes {
                println!("note: {n}");
            }
        },
        |r, _| println!("{}", spatial_line(r)),
    )?;
    for l in summary_lines(&s) {
        println!("{l}");
    }
    println!("wall clock {:.1} s", t0.elapsed().as_secs_f64());
    // The ablation on the same core: the prescribed J0 x cosine (2.57/2.0)
    // with the lumped feedback, everything else unchanged.
    let abl = crate::mp_preset::prescribed(setup.clone());
    let (sa, fa) = run(&abl, true)?;
    println!("ABLATION on the same core (--prescribed-power): power shape and k prescribed");
    println!("  quantity                         solved     prescribed");
    for (what, a, b) in [
        ("node peak/mean power density", s.node_peak_to_mean, sa.node_peak_to_mean),
        ("max node power density [W/cm3]", s.max_power_density_w_cm3, sa.max_power_density_w_cm3),
        ("max kernel [C]", s.max_kernel_c, sa.max_kernel_c),
        ("max fuel-pebble surface [C]", s.max_surface_c, sa.max_surface_c),
        ("max helium [C]", s.max_helium_c, sa.max_helium_c),
        ("vessel outlet [C]", s.vessel_outlet_c, sa.vessel_outlet_c),
        ("bed pressure drop [kPa]", s.dp_kpa, sa.dp_kpa),
        ("power-weighted mean fuel pebble [C]", s.mean_fuel_pebble_c, sa.mean_fuel_pebble_c),
    ] {
        println!("  {what:<34} {a:9.3}  {b:9.3}");
    }
    std::fs::write(out.join("ablation_prescribed_fields.csv"), fields_csv(&fa)).map_err(|e| e.to_string())?;
    crate::coupled_ui::write_field_png(
        &fa,
        crate::coupled_ui::FieldKind::Power,
        &out.join("ablation_prescribed_rz_power_density.png"),
    )?;
    let fp = out.join("multiphysics_fields.csv");
    std::fs::write(&fp, fields_csv(&f)).map_err(|e| e.to_string())?;
    let sp = out.join("multiphysics_summary.csv");
    let base = summary_csv(&s);
    let mut lines = base.lines();
    let head = lines.next().unwrap_or_default();
    let row = lines.next().unwrap_or_default();
    std::fs::write(
        &sp,
        format!(
            "{head},node_peak_to_mean,max_power_density_w_cm3,k_eff\n{row},{:.4},{:.4},{:.6}\n",
            s.node_peak_to_mean,
            s.max_power_density_w_cm3,
            s.k_eff.unwrap_or(f64::NAN)
        ),
    )
    .map_err(|e| e.to_string())?;
    let mut csv = String::from("cell,x_m,y_m,z_m,q_w_m3,t_fuel_k\n");
    for c in 0..n_mesh.n_cells() {
        let p = n_mesh.cell_centre(c);
        csv.push_str(&format!(
            "{c},{:.5},{:.5},{:.5},{:.6e},{:.3}\n",
            p[0], p[1], p[2], sf.q_n_w_m3[c], sf.t_n_k[c]
        ));
    }
    std::fs::write(out.join("neutronics_mesh_fields.csv"), csv).map_err(|e| e.to_string())?;
    use crate::coupled_ui::FieldKind;
    for (kind, name) in [
        (FieldKind::Power, "power_density"),
        (FieldKind::Helium, "helium_temperature"),
        (FieldKind::Kernel, "kernel_temperature"),
    ] {
        crate::coupled_ui::write_field_png(&f, kind, &out.join(format!("multiphysics_rz_{name}.png")))?;
    }
    let d = &inputs.meshes.domain;
    let zmid = 0.5 * (d.conus_top + d.bed_top);
    let q_wcm3: Vec<f64> = sf.q_n_w_m3.iter().map(|q| q * 1e-6).collect();
    for (basis, origin, tag, what) in [
        (PlotBasis::Xz, [0.0, 0.0, 0.0], "xz", "X-Z THROUGH THE AXIS".to_string()),
        (PlotBasis::Xy, [0.0, 0.0, zmid], "xy", format!("X-Y AT Z = {zmid:.1} CM (BED MID-HEIGHT)")),
    ] {
        let img = crate::spatial_draw::draw_field(
            &n_mesh,
            &q_wcm3,
            basis,
            origin,
            &format!("STEP 10 COMPUTED POWER DENSITY ON THE NEUTRONICS MESH, {what}"),
            "W/CM3",
            1e-9,
            "NO FISSION POWER",
            900,
        )?;
        img.write_png(&out.join(format!("neutronics_power_{tag}.png"))).map_err(|e| e.to_string())?;
    }
    let img = crate::spatial_draw::draw_field(
        &n_mesh,
        &sf.t_n_k.iter().map(|t| t - 273.15).collect::<Vec<_>>(),
        PlotBasis::Xz,
        [0.0; 3],
        "STEP 10 TFUEL SEEN BY THE CROSS SECTIONS, X-Z THROUGH THE AXIS",
        "C",
        f64::MIN,
        "",
        900,
    )?;
    img.write_png(&out.join("neutronics_tfuel_xz.png")).map_err(|e| e.to_string())?;
    let md = recipe
        .to_markdown(&now_rfc3339())
        .map_err(|e| e.to_string())?;
    let md = setup
        .write_into(&md, &now_rfc3339())
        .map_err(|e| e.to_string())?;
    std::fs::write(out.join("recipe_with_step9.md"), md).map_err(|e| e.to_string())?;
    println!("wrote the fields, summary, isothermal_k.csv, neutronics_mesh_fields.csv and the PNGs into {}", out.display());
    Ok(())
}

/// Run the case, printing the console as it goes.
///
/// # Errors
///
/// A solver failure.
pub fn run(setup: &MultiphysicsSetup, quiet: bool) -> Result<(Summary, Fields), String> {
    if !quiet {
        println!(" iter  flow resid   max dT [K]   T_he,max [C]  T_kernel,max [C]   rho [pcm]   k(cold-crit)");
    }
    crate::porous_core::solve(
        setup,
        || false,
        |r, _| {
            if !quiet {
                println!(
                    "{:5}  {:10.3e}  {:11.4}  {:12.2}  {:16.2}  {:10.1}   {:.5}",
                    r.iteration,
                    r.flow_residual,
                    r.temperature_change_k,
                    r.max_helium_k - 273.15,
                    r.max_kernel_k - 273.15,
                    r.rho_pcm,
                    r.k_vs_cold_critical
                );
            }
        },
    )
}

/// `--headless-multiphysics`: run, print, and write the outputs to `out`.
///
/// # Errors
///
/// A solver or file failure.
pub fn headless(recipe: &Recipe, setup: &MultiphysicsSetup, out: &Path) -> Result<(), String> {
    println!(
        "Step 10 coupled run (TENTATIVE; see the element list for what is and is not modelled)"
    );
    for e in &setup.elements {
        println!("  [{}] {}: {}", e.status.badge(), e.name, e.note);
    }
    let (s, f) = run(setup, false)?;
    for l in summary_lines(&s) {
        println!("{l}");
    }
    std::fs::create_dir_all(out).map_err(|e| e.to_string())?;
    let fp = out.join("multiphysics_fields.csv");
    std::fs::write(&fp, fields_csv(&f)).map_err(|e| e.to_string())?;
    let sp = out.join("multiphysics_summary.csv");
    std::fs::write(&sp, summary_csv(&s)).map_err(|e| e.to_string())?;
    use crate::coupled_ui::FieldKind;
    let mut pngs = Vec::new();
    for (kind, name) in [
        (FieldKind::Power, "power_density"),
        (FieldKind::Helium, "helium_temperature"),
        (FieldKind::Kernel, "kernel_temperature"),
    ] {
        let png = out.join(format!("multiphysics_rz_{name}.png"));
        crate::coupled_ui::write_field_png(&f, kind, &png)?;
        pngs.push(png.display().to_string());
    }
    let png = pngs.join(", ");
    let md = recipe
        .to_markdown(&now_rfc3339())
        .map_err(|e| e.to_string())?;
    let md = setup
        .write_into(&md, &now_rfc3339())
        .map_err(|e| e.to_string())?;
    let rp = out.join("recipe_with_step9.md");
    std::fs::write(&rp, md).map_err(|e| e.to_string())?;
    println!(
        "wrote {}, {}, {} and {}",
        fp.display(),
        sp.display(),
        png,
        rp.display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use dhoby_ghaut::workbench::multiphysics::PowerShape;
    use tampines::gas_phase::properties::helium_state;
    use uom::si::f64::{Pressure, ThermodynamicTemperature};
    use uom::si::pressure::pascal;
    use uom::si::specific_heat_capacity::joule_per_kilogram_kelvin;
    use uom::si::thermodynamic_temperature::kelvin;

    /// The prescribed-shape ablation (no Steps 7-8 needed): the fixture and
    /// the march's verification tests run on it.
    fn preset() -> dhoby_ghaut::workbench::multiphysics::MultiphysicsSetup {
        crate::mp_preset::prescribed(crate::mp_preset::htr10(300.15))
    }

    /// **Verification, energy.** One ring, uniform power: the bed exit
    /// temperature from the `(p, h)` march must equal an independent
    /// estimate `T_in + P / (m c_p)` with `c_p` from the `(T, p)` route at
    /// the mean temperature. Helium is a near-ideal monatomic gas at 3 MPa,
    /// so the two agree to well under 1 K. Measured 2026-10-05: see the
    /// assertion message on failure; the tolerance is 0.5 K.
    #[test]
    fn exit_temperature_matches_an_independent_cp_balance() {
        let mut s = preset();
        s.foam.radial_rings = 1;
        s.foam.axial_nodes = 10;
        s.neutronics.shape = PowerShape::Uniform;
        let (sum, _) = super::run(&s, true).expect("run");
        let p = Pressure::new::<pascal>(s.foam.outlet_pressure_mpa * 1e6);
        let t_in = s.foam.inlet_temperature_c + 273.15;
        let t_mean = 0.5 * (t_in + sum.core_exit_c + 273.15);
        let cp = helium_state(ThermodynamicTemperature::new::<kelvin>(t_mean), p)
            .expect("he")
            .specific_heat_cp
            .get::<joule_per_kilogram_kelvin>();
        let want =
            t_in + s.neutronics.thermal_power_mw * 1e6 / (s.foam.core_mass_flow_kg_s * cp) - 273.15;
        assert!(
            (sum.core_exit_c - want).abs() < 0.5,
            "march {} °C vs cp balance {want} °C",
            sum.core_exit_c
        );
        assert!(sum.energy_balance_rel.abs() < 1e-9);
    }

    /// **Verification, coupling loop and shape.** The HTR-10 case converges;
    /// at convergence every ring sees the same plenum-to-plenum pressure
    /// drop (the condition the loop enforces), the hottest ring is the
    /// central one and carries the least flow per unit area, and the shape
    /// reproduces the published peak/mean it was built for.
    #[test]
    fn htr10_case_converges_with_equal_ring_pressure_drops() {
        let s = preset();
        let mut core = crate::porous_core::PorousCore::new(&s).expect("setup");
        let total: f64 = core.node_power_w().iter().sum();
        assert!((total / 1e7 - 1.0).abs() < 1e-12);
        let mut last = None;
        for _ in 0..s.coupling.max_iterations {
            let r = core.iterate().expect("iterate");
            let done = core.converged(&r);
            last = Some(r);
            if done {
                break;
            }
        }
        let r = last.expect("ran");
        assert!(
            core.converged(&r),
            "residual {} dT {}",
            r.flow_residual,
            r.temperature_change_k
        );
        assert!(r.flow_residual < 1e-4);
        let f = &r.ring_flow_kg_s;
        assert!(
            f[0] < f[f.len() - 1],
            "centre ring should carry less flow: {f:?}"
        );
        let sum = core.summary(true).expect("summary");
        assert!(
            (sum.peak_to_mean - 2.57 / 2.0).abs() < 1e-9,
            "{}",
            sum.peak_to_mean
        );
    }

    /// The headless summary of the HTR-10 preset matches the committed
    /// fixture. A change to the solver, the correlations it calls or the
    /// prefill shows up here; regenerate with `--headless-multiphysics`
    /// only after looking at why the numbers moved, and record the move.
    #[test]
    fn the_headless_multiphysics_summary_matches_the_committed_fixture() {
        let (sum, _) = super::run(&preset(), true).expect("run");
        let got = super::summary_csv(&sum);
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/dhoby_ghaut_multiphysics.csv");
        let want = std::fs::read_to_string(&path).expect("fixture");
        assert_eq!(
            got.trim(),
            want.trim(),
            "regenerate {} only after reviewing the move",
            path.display()
        );
    }

    /// The recipe the workbench saves carries Step 9 as a kovan artifact
    /// that both readers accept: the whole recipe still loads, and Step 9
    /// comes back unchanged.
    #[test]
    fn a_saved_recipe_carries_step_9_and_still_loads() {
        let r = crate::preset::htr10();
        let s = preset();
        let md = r.to_markdown("2026-10-05T00:00:00Z").expect("recipe");
        let md = s.write_into(&md, "2026-10-05T00:00:00Z").expect("step 9");
        let back =
            dhoby_ghaut::workbench::recipe::Recipe::from_markdown(&md).expect("recipe loads");
        assert_eq!(back, r);
        let s2 = dhoby_ghaut::workbench::multiphysics::MultiphysicsSetup::from_recipe_markdown(&md)
            .expect("present")
            .expect("parses");
        assert_eq!(s2, s);
    }
}
