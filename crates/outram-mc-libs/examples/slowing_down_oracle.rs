//! Exploratory driver for `physics::slowing_down` — see
//! `tests/ring_rpt_hunt_lessons.rs` for the regression form.
use njoy_outram_park_fork::reference_data::reference_endf;
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::physics::slowing_down::*;
use std::time::Instant;

const TEMP: f64 = 293.6;

fn main() {
    let e_top: f64 = std::env::var("E_TOP")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(10.0e3);
    let e_bot: f64 = std::env::var("E_BOT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1.0);
    let hist: usize = std::env::var("HIST")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(100_000);

    let t0 = Instant::now();
    let u238 = load("U238", "n-092_U_238.endf");
    let c12 = load("C12", "n-006_C_012-ENDF8.0.endf");
    eprintln!("data ready in {:.1} s", t0.elapsed().as_secs_f64());
    let nuclides = vec![c12, u238];

    let band = SlowingDownBand { e_top, e_bot };
    let mut grid = nuclides[1].native_energy_grid(e_bot * 0.9, e_top * 1.1);
    let g2 = nuclides[0].native_energy_grid(e_bot * 0.9, e_top * 1.1);
    grid.extend_from_slice(&g2);
    grid.sort_by(|a, b| a.partial_cmp(b).unwrap());
    grid.dedup_by(|a, b| (*a - *b).abs() <= 1e-12 * b.abs());
    eprintln!("union grid: {} points over {e_bot}–{e_top} eV", grid.len());

    let sigma_p_c = 4.7392_f64; // C-12 potential scattering, b
    println!(
        "\n{:>10}  {:>9}  {:>11}  {:>11}  {:>11}  {:>9}",
        "sigma_b/b", "N_C/N_U8", "det p_esc", "MC iso", "MC aniso", "MC prod"
    );
    for &sigma_b in &[10.0_f64, 30.0, 100.0, 300.0, 1000.0, 10000.0] {
        let ratio = sigma_b / sigma_p_c;
        let mix = vec![
            MixComponent {
                nuclide_idx: 0,
                atom_density: ratio * 1.0e-3,
            },
            MixComponent {
                nuclide_idx: 1,
                atom_density: 1.0e-3,
            },
        ];
        let t = Instant::now();
        let det = solve_deterministic(&nuclides, &mix, band, TEMP, &grid);
        let t_det = t.elapsed().as_secs_f64();

        let mc = |k: ScatterKernel| {
            InfiniteMediumMc {
                histories: hist,
                seed: 0xABCD_0001,
                kernel: k,
                max_collisions: 200_000,
            }
            .run(&nuclides, &mix, band, TEMP)
        };
        let iso = mc(ScatterKernel::IsotropicCmAtRest);
        let ani = mc(ScatterKernel::AnisotropicCmAtRest);
        let pro = mc(ScatterKernel::Production);
        let se = InfiniteMediumMc {
            histories: hist,
            ..Default::default()
        }
        .stderr_of(iso.escaped);
        println!(
            "{sigma_b:>10.0}  {ratio:>9.1}  {:>11.5}  {:>11.5}  {:>11.5}  {:>11.5}   (det {:.1} s, 1σ {:.5})",
            det.escaped, iso.escaped, ani.escaped, pro.escaped, t_det, se
        );
        println!(
            "{:>10}  {:>9}  {:>11}  {:>+11.2}  {:>+11.2}  {:>+11.2}   % diff vs deterministic",
            "",
            "",
            "",
            100.0 * (iso.escaped / det.escaped - 1.0),
            100.0 * (ani.escaped / det.escaped - 1.0),
            100.0 * (pro.escaped / det.escaped - 1.0)
        );
    }
}

fn load(name: &str, file: &str) -> Nuclide {
    let p = reference_endf(file).unwrap_or_else(|| panic!("missing {file}"));
    Nuclide::from_endf_file(&p, name, TEMP, 1.0e-3).expect("reconstruct")
}
