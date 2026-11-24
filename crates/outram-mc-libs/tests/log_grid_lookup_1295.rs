//! **The union-grid lookup is the per-reaction search, to the bit** (gh:#1295).
//!
//! # Methodology
//!
//! `SpeedTier::Fast` finds a HIGH-tier nuclide's energy once on its union grid
//! (OpenMC's logarithmic index, `material::log_grid`) and reads every section
//! there; `SpeedTier::Standard` keeps the per-reaction `partition_point`, the
//! oracle. The two twins of one nuclide (one processing, `with_speed`) are
//! compared at the energies where an index error would show: **every 7th
//! union-grid energy exactly, the doubles either side of it, the midpoint to the
//! next point, and the two ends of the grid and beyond**. Compared: every
//! `MicroXS` field, `total_at_energy`, and 16 `sample_inelastic` draws with the
//! seed that is left. Pass: all bit-identical. Data: ENDF/B-VIII.0 U-238 and
//! U-235 from `reference-data/endf/`, RECONR/BROADR at 0.001, 293.6 K.
//!
//! `tests/speed_tier_fast_is_exact.rs` keeps its log-spaced probes (U-238,
//! U-234, F-19, H-1 with S(a,b), nuclide selection); this adds the grid points
//! themselves, where a search's tie-breaking decides the interval.
//!
//! # Results
//!
//! 2026-11-24 (`develop` 2637457c0 plus this change): identical at every probe.
//! U-238: union grid 155,207 points, 88,696 probes, 2,394,792 values compared;
//! U-235: 76,027 points, 43,448 probes, 1,173,096 values.
#![cfg(feature = "std")] // reads ENDF tapes (gh:#1605)

use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::reference_data::reference_endf;
use outram_mc_libs::material::nuclide::{MicroXS, Nuclide};
use outram_mc_libs::material::speed::SpeedTier;

const TEMP_K: f64 = 293.6;

fn bits(x: &MicroXS) -> [u64; 10] {
    [
        x.total.to_bits(),
        x.elastic.to_bits(),
        x.fission.to_bits(),
        x.absorption.to_bits(),
        x.inelastic.to_bits(),
        x.n2n.to_bits(),
        x.n3n.to_bits(),
        x.mt5.to_bits(),
        x.other.to_bits(),
        x.nu_fission.to_bits(),
    ]
}

fn check(file: &str, name: &str) {
    let Some(path) = reference_endf(file) else {
        eprintln!("SKIP {name}: {file} absent");
        return;
    };
    let tape = Tape::read_file(&path).expect("tape");
    let mat = tape.materials()[0];
    let processed = Nuclide::process_evaluation(&tape, mat, TEMP_K, 1e-3, 1e-3).expect("process");
    let mut grid: Vec<f64> = processed.recon.sections.iter().flat_map(|s| s.pairs.iter().map(|p| p.0)).collect();
    grid.sort_by(f64::total_cmp);
    grid.dedup();
    let fast = Nuclide::from_processed(&tape, mat, name, TEMP_K, processed).expect("nuclide");
    assert_eq!(fast.speed(), SpeedTier::Fast, "{name}: the ordinary constructor is the Fast tier");
    let std = fast.clone().with_speed(SpeedTier::Standard);

    let mut probes = vec![grid[0] * 0.5, grid[0], grid[grid.len() - 1], grid[grid.len() - 1] * 2.0];
    for i in (0..grid.len() - 1).step_by(7) {
        let g = grid[i];
        probes.extend([g, f64::from_bits(g.to_bits() - 1), f64::from_bits(g.to_bits() + 1), 0.5 * (g + grid[i + 1])]);
    }
    let mut compared = 0usize;
    for &e in &probes {
        let (a, b) = (fast.xs_at_energy(e, TEMP_K), std.xs_at_energy(e, TEMP_K));
        assert_eq!(bits(&a), bits(&b), "{name}: xs_at_energy differs at {e:e} eV: fast {a:?}, standard {b:?}");
        let (ta, tb) = (fast.total_at_energy(e, TEMP_K), std.total_at_energy(e, TEMP_K));
        assert_eq!(ta.to_bits(), tb.to_bits(), "{name}: total_at_energy differs at {e:e} eV");
        let (mut sa, mut sb) = (0x2545_F491_4F6C_DD1D_u64 ^ e.to_bits(), 0x2545_F491_4F6C_DD1D_u64 ^ e.to_bits());
        for _ in 0..16 {
            let (ia, ib) = (fast.sample_inelastic(e, &mut sa), std.sample_inelastic(e, &mut sb));
            assert_eq!(format!("{ia:?}"), format!("{ib:?}"), "{name}: sample_inelastic differs at {e:e} eV");
        }
        assert_eq!(sa, sb, "{name}: sample_inelastic used a different number of draws at {e:e} eV");
        compared += 11 + 16;
    }
    println!("{name}: union grid {} points, {} probes, {compared} values compared, all identical", grid.len(), probes.len());
}

#[test]
fn union_grid_lookup_is_the_per_reaction_search_on_u238_and_u235() {
    check("n-092_U_238-ENDF8.0.endf", "U238");
    check("n-092_U_235-ENDF8.0.endf", "U235");
}
