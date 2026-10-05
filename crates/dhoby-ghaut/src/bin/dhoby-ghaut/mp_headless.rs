//! `--headless-multiphysics`: Step 9's case run as Step 10 runs it, with no
//! window. Prints the coupling console, the summary against Gao & Shi
//! (2002), writes the r-z fields as CSV and a PNG of the TH mesh coloured by
//! temperature, and saves the recipe with the `step-9` section.

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

/// One line per summary quantity (shared with the UI console).
pub fn summary_lines(s: &Summary) -> Vec<String> {
    let mut v = vec![
        format!(
            "{} after {} coupling iterations",
            if s.converged { "CONVERGED" } else { "NOT converged" },
            s.iterations
        ),
        format!("power shape: peak/mean {:.4}, extrapolation length {:.1} cm", s.peak_to_mean, s.shape_delta_cm),
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
        format!(
            "feedback T {:.1} °C -> rho {:+.0} pcm -> k = {:.5} relative to a cold-critical reference (LUMPED, not an eigenvalue)",
            s.t_feedback_c, s.rho_pcm, s.k_vs_cold_critical
        ),
        format!("energy balance (heat to helium / power - 1) {:.2e}", s.energy_balance_rel),
        format!(
            "particle Re {:.0}-{:.0}; {} nodes outside Wakao's 15-8500",
            s.re_min, s.re_max, s.wakao_out_of_range
        ),
    ];
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

    fn preset() -> dhoby_ghaut::workbench::multiphysics::MultiphysicsSetup {
        crate::mp_preset::htr10(300.15)
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
