//! Native driver: the same C-ABI sequence `run_worker.mjs` calls in the
//! browser Worker, timed with `Instant`. `main` glue (CLI); its logic is in
//! the library, which the crate's tests reach.
//!
//!   native ogrid N NR NZ DT MAX_STEPS [NU]
//!   native cfmesh CELL_SIZE N_LAYERS N_SEG DT MAX_STEPS [NU]
//!   native mesh-only cfmesh CELL_SIZE N_LAYERS N_SEG   (cell count, mesh time)
use foam_wasm_pipe::*;
use std::time::Instant;

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    if a[0] == "notes" {
        let (m, r) = mesh_cfmesh(a[1].parse().expect("cell size"), a[2].parse().expect("layers"), a[3].parse().expect("n_seg"));
        let patches: Vec<(String, usize)> = m.patches().iter().map(|p| (p.name.clone(), p.faces.len())).collect();
        println!("cells={} notes={:?} patches={patches:?} non_star={}", m.n_cells(), r.stage_notes, m.n_non_star_cells());
        return;
    }
    let mesh_only = a[0] == "mesh-only";
    let a = if mesh_only { a[1..].to_vec() } else { a };
    let f = |i: usize| a[i].parse::<f64>().expect("number");
    let t0 = Instant::now();
    let (cells, rest) = match a[0].as_str() {
        "ogrid" => (pipe_mesh_ogrid(f(1) as u32, f(2) as u32, f(3) as u32), 4),
        _ => (pipe_mesh_cfmesh(f(1), f(2) as u32, f(3) as u32), 4),
    };
    let t_mesh = t0.elapsed().as_secs_f64();
    let t1 = Instant::now();
    pipe_to_fv();
    let t_fv = t1.elapsed().as_secs_f64();
    println!(
        "route={} cells={cells} mesh_s={t_mesh:.4} to_fv_s={t_fv:.4} non_ortho_max={:.2} non_ortho_mean={:.2} skew_max={:.3} neg={} vol={:.6e} aspect_max={:.2}",
        a[0],
        pipe_quality(0),
        pipe_quality(4),
        pipe_quality(1),
        pipe_quality(2),
        pipe_quality(3),
        pipe_quality(5)
    );
    if mesh_only {
        return;
    }
    let dt = f(rest);
    let max_steps = f(rest + 1) as usize;
    match a.get(rest + 2) {
        Some(nu) => pipe_case_new_nu(dt, nu.parse().expect("nu")),
        None => pipe_case_new(dt),
    };
    let mut best = f64::INFINITY;
    let mut total = 0.0;
    let mut steps = 0;
    let (mut hu, mut hp) = (Vec::new(), Vec::new());
    println!("step,t_s,step_wall_s,steady_metric,uc15_over_2U,uc17_over_2U,dpdz_over_hp,qout_over_qin,max_cont_err,p_inlet");
    for k in 1..=max_steps {
        let ts = Instant::now();
        let r = pipe_steps(1);
        let w = ts.elapsed().as_secs_f64();
        best = best.min(w);
        hu.push((k as f64 * dt, pipe_diag(0)));
        hp.push((k as f64 * dt, pipe_diag(5)));
        total += w;
        steps = k;
        if k % 10 == 0 || k <= 3 || r < 1e-3 {
            println!(
                "{k},{:.3},{w:.4},{r:.3e},{:.5},{:.5},{:.5},{:.6},{:.2e},{:.4e}",
                k as f64 * dt,
                pipe_diag(0),
                pipe_diag(4),
                pipe_diag(1),
                pipe_diag(2),
                pipe_diag(3),
                pipe_diag(5)
            );
        }
        if r < 1e-3 {
            break;
        }
    }
    let t1u = settling_time(&hu, 0.01);
    let t1p = settling_time(&hp, 0.01);
    let t1 = t1u.max(t1p);
    let k1 = (t1 / dt).round() as usize;
    println!(
        "summary steps={steps} t_phys_s={:.2} solve_wall_s={total:.3} best_step_s={best:.4} mean_step_s={:.4} settle1pct_t_s={t1:.2} (uc {t1u:.2}, dp {t1p:.2}) settle1pct_steps={k1} uc15={:.5} uc17={:.5} dpdz_over_hp={:.5} dp_total={:.4e}",
        steps as f64 * dt,
        total / steps as f64,
        pipe_diag(0),
        pipe_diag(4),
        pipe_diag(1),
        pipe_diag(5)
    );
}
