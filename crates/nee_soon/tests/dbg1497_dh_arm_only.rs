//! TEMP gh:#1497 repro: DH arm only.
#![allow(unused_imports)]
use petir::mathf::RealMath;
use nee_soon::htr10_mgxs::cells::Htr10Cell;
use nee_soon::htr10_rmc::core_model::mat;
use nee_soon::htr10_rmc::data::{load_htr10_nuclides, Htr10DataConfig, Htr10NuclideLayout};
use nee_soon::htr10_rmc::keff_vs_height::bed_majorant;
use nee_soon::htr10_rmc::materials::{htr10_material_set, Htr10MaterialConfig};
use outram_mc_libs::dh_universe::{DhTreatment, DhUniverse, PebbleParams};
use outram_mc_libs::physics::keff::{ComputeType, KeffSettings, ThreadCount};
use outram_mc_libs::physics::transport_csg::run_keff_csg_hybrid;
use outram_mc_libs::run_diagnostics::RunDiagnostics;
use uom::si::thermodynamic_temperature::kelvin;

fn env_usize(k: &str, d: usize) -> usize {
    std::env::var(k).ok().and_then(|v| v.parse().ok()).unwrap_or(d)
}

/// See the module docs for the methodology.
///
/// # Results
///
/// +102 ± 347 pcm (2026-10-13); see the module docs and
/// `verification_and_validation/htr10_mgxs_cells/README.md`.
#[test]
#[cfg_attr(
    not(feature = "long-tests"),
    ignore = "needs the reference ENDF data (HTR-10 tape set) and runs for minutes"
)]
fn dh_arm_only_1497() {
    let cfg = Htr10DataConfig::default();
    let t_k = cfg.temperature.get::<kelvin>();
    let Ok(layout) = Htr10NuclideLayout::plan(&cfg) else {
        eprintln!("SKIP: HTR-10 nuclear data could not be planned (reference data absent)");
        return;
    };
    let mut diag = RunDiagnostics::new("htr10 single pebble vs DhUniverse");
    let nucs = match load_htr10_nuclides(&cfg, &layout, &mut diag) {
        Ok(n) => n,
        Err(e) => {
            eprintln!("SKIP: HTR-10 nuclear data could not be loaded: {e}");
            return;
        }
    };
    let mats = htr10_material_set(&layout, Htr10MaterialConfig::benchmark_default(t_k));
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
    let settings = KeffSettings {
        n_particles: env_usize("OUTRAM_CELL_PARTICLES", 4_000),
        n_inactive: env_usize("OUTRAM_CELL_INACTIVE", 10),
        n_active: env_usize("OUTRAM_CELL_ACTIVE", 50),
        temperature_k: t_k,
        seed: 20_261_013,
        compute: ComputeType::CpuMultiThread(ThreadCount::Fixed(threads)),
        ..KeffSettings::default()
    };

    let cell = Htr10Cell::single_pebble();
    // DH arm: the same materials in DH's order, helium to the cube's
    // equal-volume sphere.
    let order = [
        mat::KERNEL,
        mat::BUFFER,
        mat::IPYC,
        mat::SIC,
        mat::OPYC,
        mat::GRAPHITE,
        mat::GRAPHITE,
        mat::HELIUM,
    ];
    let r_eq = (3.0 * cell.volume_cm3 / (4.0 * std::f64::consts::PI)).r_cbrt();
    let params = PebbleParams::htr10_li2014()
        .with_materials(order.iter().map(|&m| mats[m].clone()).collect())
        .with_coolant(r_eq);
    let dh = DhUniverse::pebble(params, DhTreatment::DeltaTracking).expect("DH pebble builds");
    let t = std::time::Instant::now();
    let dhk = dh.keff(&nucs, &settings);
    let t_dh = t.elapsed().as_secs_f64();

    println!("DH k = {:.5} +- {:.5} ({t_dh:.0} s)", dhk.k_mean, dhk.k_std);
}
