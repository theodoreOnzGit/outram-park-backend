//! DTFR verification against an NJOY2016 `dtfr` run in CLAW mode
//! (`dtfr.f90`, `iedit = 1`).
//!
//! Oracle: upstream NJOY2016 (`ac5adf5`, gfortran 13.3.0) `groupr` on the
//! U-238 293.6 K PENDF of the GROUPR golden deck with the elastic matrix added
//! (`6 2/`), `moder` to binary, then `dtfr -24 25 0 0 / 0 0 1 / 1 29 / 0 0 /
//! 'u238' 9237 1 293.6 /` — `reference-data/dtfr/u238-29g-claw.njoy-input`;
//! the GENDF it consumed is committed alongside
//! (`u238-ENDF8.0-293.6K-29g-iwt3-6sigz-mf6.gendf`) and the DTF output is
//! `u238-29g-claw.dtf`: a 48-column edit table and the `l=0 n-n table
//! (32x29)` — per DTF group, absorption, nu*sigma_f, total, then the 29-wide
//! scatter band (`dtfr.f90:838-890`).
//!
//! The crate's `build_neutron_table` assembles exactly the n-n table's
//! content from MF=3/MT=1 and MF=6/MT=2 (total at `iptotl`, absorption =
//! total - scatter at `iptotl-2`, the band from `iptotl+1`), so the
//! comparison is that 32 x 29 block; nu*sigma_f is zero on both sides (the
//! GENDF carries no nubar) and the 48 edit columns are outside the crate's
//! minimal reader.
//!
//! **Prediction** (stated before measuring): every one of the 928 values
//! agrees with NJOY's 6 printed figures — both sides add the same GENDF
//! numbers; a band-packing offset would misplace whole diagonals, a wrong
//! group ordering would reverse the table.

use njoy_outram_park_fork::dtfr::gendf::build_neutron_table;
use njoy_outram_park_fork::dtfr::input::DtfrInput;
use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::reference_data::reference_file_or_skip;

const MAT: i32 = 9237;
const NG: i32 = 29;
/// `1PE12.5`: 6 significant figures.
const TOL: f64 = 2e-5;
const ABS_FLOOR: f64 = 1e-9;

/// Parse the `l=0 n-n table` block of a CLAW file into `[group][position]`.
fn njoy_nn_table(text: &str, ng: usize) -> (usize, Vec<Vec<f64>>) {
    let mut lines = text.lines();
    let header = lines
        .find(|l| l.contains("n-n table"))
        .expect("n-n table header");
    // "u238   l=0 n-n table ( 32x 29)"
    let dims = header.split('(').nth(1).unwrap();
    let itabl: usize = dims.split('x').next().unwrap().trim().parse().unwrap();
    let mut values = Vec::with_capacity(itabl * ng);
    for line in lines {
        if values.len() >= itabl * ng {
            break;
        }
        for k in 0..6 {
            let field = &line[12 * k..12 * (k + 1)];
            values.push(field.trim().parse::<f64>().expect("1PE12.5 field"));
        }
    }
    // The final line is zero-filled to six fields (dtfr.f90:844-849).
    values.truncate(itabl * ng);
    assert_eq!(values.len(), itabl * ng, "table size");
    let table = values.chunks(itabl).map(|c| c.to_vec()).collect();
    (itabl, table)
}

#[test]
fn dtfr_u238_claw_nn_table_matches_njoy() {
    let tag = "dtfr-u238-claw";
    let Some(gendf) = reference_file_or_skip(
        "dtfr",
        "u238-ENDF8.0-293.6K-29g-iwt3-6sigz-mf6.gendf",
        tag,
    ) else {
        return;
    };
    let Some(dtf) = reference_file_or_skip("dtfr", "u238-29g-claw.dtf", tag) else {
        return;
    };
    let text = std::fs::read_to_string(dtf).unwrap();
    let (itabl_njoy, njoy) = njoy_nn_table(&text, NG as usize);

    let tape = Tape::read_file(&gendf).expect("GENDF parses");
    let deck = DtfrInput::claw_defaults(1, NG);
    let iptotl = deck.neutron.iptotl;
    let table = build_neutron_table(&tape, MAT, &deck.neutron, 1).expect("build_neutron_table");
    // CLAW n-n table = internal positions iptotl-2 ..= itabl (dtfr.f90:868-890).
    let itabl_ours = (deck.neutron.itabl - (iptotl - 2) + 1) as usize;
    assert_eq!(itabl_ours, itabl_njoy, "n-n table length");

    let mut worst = (0.0f64, 0, 0, 0.0, 0.0);
    let mut nonzero = 0usize;
    for g in 1..=NG {
        for p in 1..=itabl_njoy as i32 {
            let want = njoy[(g - 1) as usize][(p - 1) as usize];
            let got = table.get(iptotl - 2 + p - 1, g);
            if want != 0.0 {
                nonzero += 1;
            }
            let scale = got.abs().max(want.abs());
            if scale <= ABS_FLOOR {
                continue;
            }
            let rel = (got - want).abs() / scale;
            if rel > worst.0 {
                worst = (rel, g, p, got, want);
            }
        }
    }
    println!(
        "[{tag}] {}x{} n-n table, {nonzero} non-zero entries, worst {:.3e} at group {} position {} (crate {:.6e}, njoy {:.6e})",
        itabl_njoy, NG, worst.0, worst.1, worst.2, worst.3, worst.4
    );
    assert!(nonzero > 100, "oracle table carries data ({nonzero} non-zero)");
    assert!(
        worst.0 < TOL,
        "{tag}: worst {:.3e} at group {} position {} (crate {:.6e}, njoy {:.6e})",
        worst.0,
        worst.1,
        worst.2,
        worst.3,
        worst.4
    );
}
