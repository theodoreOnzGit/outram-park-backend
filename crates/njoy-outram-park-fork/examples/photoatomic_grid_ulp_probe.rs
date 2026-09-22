//! Which of `sigfig` and `terp1` carries a 1-ulp disagreement with NJOY?
//!
//! A Type-2 (binary) comparison is the only one that can see a single-ulp
//! difference — the Type-1 text rounds it away at 12 digits. When one appears
//! in an ESZG cross-section word, two things could be responsible: the energy
//! the lookup was made at (`sigfig`, whose `10**ipwr` is a compiler-dependent
//! integer power) or the interpolation itself (`terp1`).
//!
//! This separates them. For every ESZG word that differs, it re-evaluates the
//! column with the grid energy nudged by one ulp each way. If a nudge
//! reproduces NJOY's bits, the disagreement is in the **grid**; if no nudge
//! does, it is in the **interpolation**.
//!
//! ```text
//! cargo run --release -p njoy-outram-park-fork \
//!     --example photoatomic_grid_ulp_probe -- <tape.endf> <mat> <njoy-type2.ace>
//! ```

use njoy_outram_park_fork::acer::photoatomic::{photoatomic_ace, PhotoatomicOptions};
use njoy_outram_park_fork::acer::read::AceFileType;
use njoy_outram_park_fork::endf::gety1::Gety1;
use njoy_outram_park_fork::endf::records::SectionCursor;
use njoy_outram_park_fork::endf::tape::Tape;

fn column_with_grid(tape: &Tape, mat: i32, mt: i32, grid: &[f64]) -> Vec<f64> {
    let sec = tape.section(mat, 23, mt).expect("MF=23 section");
    let mut cur = SectionCursor::new(&sec.rows);
    cur.read_cont().expect("head");
    let t = cur.read_tab1().expect("tab1");
    let mut g = Gety1::new(&t);
    grid.iter().map(|&e| g.get(e).y).collect()
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 3 {
        eprintln!("usage: photoatomic_grid_ulp_probe <tape.endf> <mat> <njoy-type2.ace>");
        std::process::exit(2);
    }
    let tape = Tape::read_file(std::path::Path::new(&args[0])).expect("read tape");
    let mat: i32 = args[1].parse().expect("mat");
    let want = std::fs::read(&args[2]).expect("read NJOY type-2 ACE");

    let opts = PhotoatomicOptions::default();
    let built = photoatomic_ace(&tape, mat, &opts, None).expect("build");
    let nes = built.nes;
    let grid = built.grid_ev.clone();
    let table = built.into_raw(&opts, AceFileType::Type2Binary);

    // Column k of ESZG maps to an MF=23 MT; column 0 is the energy itself.
    let mts = [0i32, 504, 502, 522, 516];
    let mut grid_blamed = 0usize;
    let mut interp_blamed = 0usize;
    let mut shown = 0usize;
    for k in 1..5usize {
        let recomputed = |g: &[f64]| column_with_grid(&tape, mat, mts[k], g);
        let base = recomputed(&grid);
        for i in 0..nes {
            let o = 512 + 16 * (k * nes + i);
            if o + 8 > want.len() {
                continue;
            }
            let njoy = f64::from_le_bytes(want[o..o + 8].try_into().unwrap());
            if njoy == table.xss[k * nes + i] {
                continue;
            }
            // Nudge this one energy and see whether the column word follows.
            let mut hit = None;
            for dir in [1i64, -1] {
                let mut g2 = grid.clone();
                g2[i] = f64::from_bits((grid[i].to_bits() as i64 + dir) as u64);
                if recomputed(&g2)[i] == njoy {
                    hit = Some(dir);
                    break;
                }
            }
            match hit {
                Some(_) => grid_blamed += 1,
                None => interp_blamed += 1,
            }
            if shown < 10 {
                shown += 1;
                println!(
                    "  MT={} i={i} E={:.17e}  mine {:.17e} njoy {:.17e}  base {:.17e}  -> {}",
                    mts[k],
                    grid[i],
                    table.xss[k * nes + i],
                    njoy,
                    base[i],
                    match hit {
                        Some(1) => "grid is 1 ulp LOW",
                        Some(-1) => "grid is 1 ulp HIGH",
                        _ => "NOT the grid -- interpolation",
                    }
                );
            }
        }
    }
    println!("differing ESZG words: {} blamed on the grid, {} on the interpolation",
        grid_blamed, interp_blamed);
}
