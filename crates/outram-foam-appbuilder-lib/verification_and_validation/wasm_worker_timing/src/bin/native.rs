fn main() {
    println!("n_cells {}", foam_wasm_timing::sod_n_cells());
    for dt in [8.0e-6, 4.0e-6, 2.0e-6, 1.0e-6] {
        let mut best = f64::MAX; let mut sum = 0.0;
        for _ in 0..5 { let t = std::time::Instant::now(); sum = foam_wasm_timing::run(dt); best = best.min(t.elapsed().as_secs_f64()); }
        println!("native dt={dt:e} best_of_5={best:.4}s rho_sum={sum:.17e}");
    }
}
