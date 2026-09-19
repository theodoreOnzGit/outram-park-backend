//! Exhaustive code-to-code verification of the Shannon-entropy diagnostic
//! against OpenMC.
//!
//! # What is being verified
//!
//! [`RegularMesh::shannon_entropy`] and [`RegularMesh::count_sites`] are ports
//! of `shannon_entropy()` (`openmc/src/eigenvalue.cpp:587-616`) and
//! `RegularMesh::count_sites` (`openmc/src/mesh.cpp:1677`), which in turn lean
//! on `RegularMesh::get_index_in_direction` (`:1580`) and
//! `StructuredMesh::get_bin_from_indices` (`:1134`).
//!
//! # Why the oracle is a SHARED BANK, not a shared run
//!
//! Entropy is a functional of the fission bank, and the bank is a functional of
//! the whole transport. Running both codes and comparing their entropy
//! trajectories would conflate a defect in this 20-line diagnostic with every
//! difference in cross sections, sampling and RNG between the two codes — and
//! it could not be made exact at any statistics.
//!
//! So the bank is held fixed. OpenMC's **own final source bank** — all 5000
//! sites, position and weight, at full double precision — is committed, and
//! both implementations are run on it. Any disagreement is then attributable
//! to the entropy code and nothing else.
//!
//! The oracle value itself is an independent transcription of the C++,
//! branch-for-branch, written in Python in the committed deck
//! (`openmc_inputs/entropy_reference.py`). Two independent transcriptions of
//! the same upstream, agreeing to machine precision on 5000 real fission
//! sites, is what makes this a verification rather than a demonstration.
//!
//! # Reference
//!
//! OpenMC **0.16.1-dev25** (commit `d7d3284a1`), ENDF/B-VIII.0, bare Godiva
//! (ICSBEP HEU-MET-FAST-001), seed 20260917, 5000 particles x 40 batches with
//! 10 inactive, on a 5x5x5 entropy mesh spanning the sphere's bounding box
//! `[-8.7407, 8.7407]^3` cm. Deck committed at
//! `verification_and_validation/shannon_entropy/openmc_inputs/`.
//!
//! The mesh is deliberately sized to the sphere rather than loosely around it,
//! so fission sites land close to the mesh faces — the branch of
//! `get_index_in_direction` where this port was found to be wrong.
//!
//! # Results (2026-09-17)
//!
//! | Quantity | This port | Oracle | Agreement |
//! |---|---|---|---|
//! | `H` on OpenMC's final source bank | see `entropy_matches_the_openmc_bank_oracle_exactly` | 6.177382859923244 bits | exact to 1e-15 relative |
//! | per-bin weights, all 125 bins | — | `entropy_bin_counts.csv` | exact |
//! | k (context only, not gated here) | — | 0.999535 +/- 0.001961 | — |
//!
//! OpenMC's own last-generation entropy is **6.1780190209**, which is close to
//! but NOT equal to the oracle, **by construction**: upstream takes entropy on
//! the fission bank *before* synchronisation, while the statepoint source bank
//! is *after* it. They are different samples of the same converged
//! distribution, so that number is reported for context and is deliberately
//! not asserted on.

use outram_mc_libs::geometry::position::{Direction, Position};
use outram_mc_libs::particle::bank::BankSite;
use outram_mc_libs::tally::mesh::RegularMesh;

const BANK_CSV: &str =
    include_str!("../verification_and_validation/shannon_entropy/entropy_source_bank.csv");
const ORACLE_CSV: &str =
    include_str!("../verification_and_validation/shannon_entropy/entropy_oracle.csv");
const COUNTS_CSV: &str =
    include_str!("../verification_and_validation/shannon_entropy/entropy_bin_counts.csv");

/// Look one scalar out of the key/value oracle.
fn oracle(key: &str) -> f64 {
    for line in ORACLE_CSV.lines().skip(1) {
        let mut it = line.split(',');
        if it.next() == Some(key) {
            return it
                .next()
                .expect("oracle row has a value")
                .trim()
                .parse()
                .expect("oracle parses");
        }
    }
    panic!("oracle key {key} not found");
}

/// OpenMC's final source bank, as bank sites. Direction and energy are not
/// used by the entropy path and are filled with placeholders.
fn openmc_bank() -> Vec<BankSite> {
    BANK_CSV
        .lines()
        .skip(1)
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let v: Vec<f64> = l
                .split(',')
                .map(|s| s.trim().parse().expect("bank parses"))
                .collect();
            assert_eq!(v.len(), 4, "bank row is x,y,z,wgt");
            BankSite {
                r: Position::new(v[0], v[1], v[2]),
                u: Direction::new(0.0, 0.0, 1.0),
                e: 1.0e6,
                wgt: v[3],
                seed: 0,
            }
        })
        .collect()
}

fn reference_mesh() -> RegularMesh {
    RegularMesh {
        lower_left: [oracle("ll_x"), oracle("ll_y"), oracle("ll_z")],
        upper_right: [oracle("ur_x"), oracle("ur_y"), oracle("ur_z")],
        dimension: [
            oracle("dim_x") as usize,
            oracle("dim_y") as usize,
            oracle("dim_z") as usize,
        ],
    }
}

/// **The headline check.** `H` on OpenMC's own 5000-site fission source bank
/// must equal the independently transcribed C++ result to machine precision.
///
/// Pass criterion: relative difference below 1e-14. That is a numerical-
/// reproducibility bound, not a physics tolerance — the two implementations
/// sum the same 125 terms in the same order, so anything larger means the
/// algorithms differ, not that the arithmetic drifted.
#[test]
fn entropy_matches_the_openmc_bank_oracle_exactly() {
    let mesh = reference_mesh();
    let bank = openmc_bank();
    assert_eq!(
        bank.len(),
        oracle("n_sites") as usize,
        "bank size matches the oracle"
    );

    let h = mesh
        .shannon_entropy(&bank)
        .expect("a 5000-site bank has an entropy");
    let expected = oracle("transcribed_entropy_on_final_source_bank");
    let rel = (h - expected).abs() / expected.abs();

    println!(
        "H(ours) = {h:.15} bits\nH(oracle) = {expected:.15} bits\nrelative difference = {rel:.3e}\n\
         OpenMC's own last-generation entropy = {:.10} (different bank by construction)\n\
         ceiling log2(n_bins) = {:.6}",
        oracle("openmc_entropy_last_generation"),
        oracle("max_possible_entropy_log2_nbins"),
    );

    assert!(
        rel < 1.0e-14,
        "Shannon entropy disagrees with the transcribed OpenMC algorithm on an \
         IDENTICAL bank: ours {h:.15}, oracle {expected:.15}, relative {rel:.3e}. \
         The bank is shared, so this cannot be a transport or sampling difference \
         -- the entropy or the mesh binning is wrong."
    );
}

/// The scalar above could pass with compensating binning errors, so the
/// per-bin weights are gated too: all 125 bins, exactly.
///
/// This is what actually verifies `get_index_in_direction` and
/// `get_bin_from_indices` — the bin ORDER (x fastest, then y, then z) as well
/// as the membership. A transposed bin order leaves `H` completely unchanged,
/// because entropy is invariant under permutation of the bins, so the scalar
/// test above is blind to it. This one is not.
#[test]
fn per_bin_weights_match_openmc_bin_for_bin() {
    let mesh = reference_mesh();
    let (counts, outside) = mesh.count_sites(&openmc_bank());

    let expected: Vec<f64> = COUNTS_CSV
        .lines()
        .skip(1)
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            l.split(',')
                .nth(1)
                .expect("counts row")
                .trim()
                .parse()
                .expect("count parses")
        })
        .collect();

    assert_eq!(counts.len(), expected.len(), "bin count matches");
    assert_eq!(
        outside,
        oracle("any_site_outside_mesh") != 0.0,
        "the outside-mesh flag must agree with OpenMC's"
    );

    let mut worst = 0.0_f64;
    let mut worst_bin = 0usize;
    for (i, (ours, theirs)) in counts.iter().zip(expected.iter()).enumerate() {
        let d = (ours - theirs).abs();
        if d > worst {
            worst = d;
            worst_bin = i;
        }
    }
    println!("125 bins compared; worst absolute weight difference {worst:.3e} at bin {worst_bin}");
    assert!(
        worst < 1.0e-12,
        "per-bin weights differ from OpenMC: worst {worst:.3e} at bin {worst_bin} \
         (ours {}, theirs {}). Entropy is permutation invariant, so a bin-ORDER \
         error hides from the scalar check and shows up only here.",
        counts[worst_bin],
        expected[worst_bin]
    );
}

/// Analytic limit: a source spread perfectly evenly over `n` bins has entropy
/// exactly `log2(n)`. This pins the LOGARITHM BASE, which no comparison
/// against a plateau can do — using `ln` instead of `log2` rescales every
/// value by `ln 2` and still looks like a converging curve.
#[test]
fn a_uniform_source_reaches_the_log2_ceiling() {
    let mesh = RegularMesh {
        lower_left: [0.0, 0.0, 0.0],
        upper_right: [4.0, 4.0, 4.0],
        dimension: [4, 4, 4],
    };
    // One unit-weight site at the centre of every one of the 64 cells.
    let mut bank = Vec::new();
    for iz in 0..4 {
        for iy in 0..4 {
            for ix in 0..4 {
                bank.push(BankSite {
                    r: Position::new(ix as f64 + 0.5, iy as f64 + 0.5, iz as f64 + 0.5),
                    u: Direction::new(0.0, 0.0, 1.0),
                    e: 1.0,
                    wgt: 1.0,
                    seed: 0,
                });
            }
        }
    }
    let h = mesh
        .shannon_entropy(&bank)
        .expect("uniform bank has an entropy");
    let ceiling = (mesh.n_bins() as f64).log2();
    println!("uniform H = {h:.15}, log2(64) = {ceiling:.15}");
    assert!(
        (h - ceiling).abs() < 1.0e-14,
        "a uniform source must sit exactly at log2(n_bins) = {ceiling}, got {h}. \
         A mismatch of a factor ln 2 = 0.693 means the logarithm base is wrong."
    );
}

/// Analytic limit: every site in one bin is a delta distribution, entropy 0.
#[test]
fn a_fully_concentrated_source_has_zero_entropy() {
    let mesh = RegularMesh {
        lower_left: [0.0, 0.0, 0.0],
        upper_right: [4.0, 4.0, 4.0],
        dimension: [4, 4, 4],
    };
    let bank: Vec<BankSite> = (0..100)
        .map(|_| BankSite {
            r: Position::new(0.5, 0.5, 0.5),
            u: Direction::new(0.0, 0.0, 1.0),
            e: 1.0,
            wgt: 1.0,
            seed: 0,
        })
        .collect();
    let h = mesh
        .shannon_entropy(&bank)
        .expect("concentrated bank has an entropy");
    assert_eq!(h, 0.0, "a delta source has exactly zero entropy, got {h}");
}

/// Two equally weighted bins is exactly **one bit**. Together with the
/// `log2(n)` ceiling above this fixes the base from two directions.
#[test]
fn an_even_two_bin_split_is_exactly_one_bit() {
    let mesh = RegularMesh {
        lower_left: [0.0, 0.0, 0.0],
        upper_right: [2.0, 1.0, 1.0],
        dimension: [2, 1, 1],
    };
    let bank = vec![
        BankSite {
            r: Position::new(0.5, 0.5, 0.5),
            u: Direction::new(0.0, 0.0, 1.0),
            e: 1.0,
            wgt: 1.0,
            seed: 0,
        },
        BankSite {
            r: Position::new(1.5, 0.5, 0.5),
            u: Direction::new(0.0, 0.0, 1.0),
            e: 1.0,
            wgt: 1.0,
            seed: 0,
        },
    ];
    let h = mesh
        .shannon_entropy(&bank)
        .expect("two-site bank has an entropy");
    assert!(
        (h - 1.0).abs() < 1.0e-15,
        "an even two-way split is 1 bit, got {h}"
    );
}

/// **The defect this port was found to have.** `get_index_in_direction`
/// special-cases both faces:
///
/// ```text
/// if (r <= lower_left_[i])  return r == lower_left_[i] ? 1 : 0;
/// if (r >= upper_right_[i]) return r == upper_right_[i] ? shape_[i] : shape_[i] + 1;
/// ```
///
/// so a point lying **exactly** on either face is INSIDE. This port previously
/// computed a bare `ceil((r - lower_left)/width)`, which gives `ceil(0.0) = 0`
/// on the lower face and rejected it as outside. The upper face was already
/// correct. Gated here in every axis and in both directions so the asymmetry
/// cannot come back on one axis only.
#[test]
fn both_mesh_faces_are_inside_exactly_as_upstream() {
    let mesh = RegularMesh {
        lower_left: [-2.0, -3.0, -4.0],
        upper_right: [2.0, 3.0, 4.0],
        dimension: [4, 6, 8],
    };
    let ll = mesh.lower_left;
    let ur = mesh.upper_right;

    // Exactly on the lower corner -> first cell, bin 0.
    assert_eq!(
        mesh.get_bin(Position::new(ll[0], ll[1], ll[2])),
        Some(0),
        "a point exactly on the lower corner is in cell 0 (upstream returns index 1)"
    );
    // Exactly on the upper corner -> last cell.
    assert_eq!(
        mesh.get_bin(Position::new(ur[0], ur[1], ur[2])),
        Some(mesh.n_bins() - 1),
        "a point exactly on the upper corner is in the last cell"
    );

    // One axis at a time, exactly on each face, interior elsewhere.
    for axis in 0..3 {
        for on_lower in [true, false] {
            let mut p = [0.0, 0.0, 0.0];
            p[axis] = if on_lower { ll[axis] } else { ur[axis] };
            assert!(
                mesh.get_bin(Position::new(p[0], p[1], p[2])).is_some(),
                "axis {axis}: a point exactly on the {} face must be INSIDE",
                if on_lower { "lower" } else { "upper" }
            );
        }
    }

    // Strictly outside, by one ulp, must be rejected on every axis.
    for axis in 0..3 {
        let mut lo = [0.0, 0.0, 0.0];
        lo[axis] = f64::from_bits(ll[axis].to_bits() + 1); // one ulp more negative
        assert_eq!(
            mesh.get_bin(Position::new(lo[0], lo[1], lo[2])),
            None,
            "axis {axis}: one ulp below the lower face is outside"
        );
        let mut hi = [0.0, 0.0, 0.0];
        hi[axis] = f64::from_bits(ur[axis].to_bits() + 1); // one ulp above
        assert_eq!(
            mesh.get_bin(Position::new(hi[0], hi[1], hi[2])),
            None,
            "axis {axis}: one ulp above the upper face is outside"
        );
    }
}

/// Bin ordering is x fastest, then y, then z — `bin = (iz*ny + iy)*nx + ix`,
/// matching `get_bin_from_indices` (`src/mesh.cpp:1134`). Asserted directly,
/// because entropy cannot see it (see `per_bin_weights_match_openmc_bin_for_bin`).
#[test]
fn bin_ordering_is_x_fastest_then_y_then_z() {
    let (nx, ny, nz) = (2usize, 3usize, 5usize);
    let mesh = RegularMesh {
        lower_left: [0.0, 0.0, 0.0],
        upper_right: [nx as f64, ny as f64, nz as f64],
        dimension: [nx, ny, nz],
    };
    for iz in 0..nz {
        for iy in 0..ny {
            for ix in 0..nx {
                let p = Position::new(ix as f64 + 0.5, iy as f64 + 0.5, iz as f64 + 0.5);
                assert_eq!(
                    mesh.get_bin(p),
                    Some((iz * ny + iy) * nx + ix),
                    "cell ({ix},{iy},{iz}) must map to (iz*ny + iy)*nx + ix"
                );
            }
        }
    }
}

/// `count_sites` accumulates statistical **weight**, not a site count
/// (`cnt(mesh_bin) += site.wgt` upstream). The two coincide under analog
/// fission with unit weights, so a count-based implementation passes every
/// casual test and then diverges silently under weight windows or implicit
/// capture. Two sites of unequal weight in different bins make them differ.
#[test]
fn count_sites_accumulates_weight_not_hits() {
    let mesh = RegularMesh {
        lower_left: [0.0, 0.0, 0.0],
        upper_right: [2.0, 1.0, 1.0],
        dimension: [2, 1, 1],
    };
    let bank = vec![
        BankSite {
            r: Position::new(0.5, 0.5, 0.5),
            u: Direction::new(0.0, 0.0, 1.0),
            e: 1.0,
            wgt: 3.0,
            seed: 0,
        },
        BankSite {
            r: Position::new(1.5, 0.5, 0.5),
            u: Direction::new(0.0, 0.0, 1.0),
            e: 1.0,
            wgt: 1.0,
            seed: 0,
        },
    ];
    let (counts, outside) = mesh.count_sites(&bank);
    assert!(!outside);
    assert_eq!(counts, vec![3.0, 1.0], "weights, not one hit per site");

    // H for a 3:1 split = -(0.75 log2 0.75 + 0.25 log2 0.25) = 0.811278...
    let h = mesh.shannon_entropy(&bank).expect("entropy exists");
    let expect = -(0.75f64 * 0.75f64.log2() + 0.25f64 * 0.25f64.log2());
    assert!(
        (h - expect).abs() < 1.0e-15,
        "weighted entropy must be {expect}, got {h}; a hit-counting implementation \
         would return exactly 1.0 here"
    );
}

/// A bank with nothing inside the mesh has no distribution to take an entropy
/// of. Upstream divides unconditionally and yields NaN; this port returns
/// `None` so a NaN cannot propagate into a convergence plot.
#[test]
fn an_empty_or_fully_outside_bank_has_no_entropy() {
    let mesh = RegularMesh {
        lower_left: [0.0, 0.0, 0.0],
        upper_right: [1.0, 1.0, 1.0],
        dimension: [2, 2, 2],
    };
    assert_eq!(
        mesh.shannon_entropy(&[]),
        None,
        "an empty bank has no entropy"
    );

    let far = vec![BankSite {
        r: Position::new(100.0, 100.0, 100.0),
        u: Direction::new(0.0, 0.0, 1.0),
        e: 1.0,
        wgt: 1.0,
        seed: 0,
    }];
    assert_eq!(
        mesh.shannon_entropy(&far),
        None,
        "a bank entirely outside the mesh has no entropy, and must not be NaN"
    );
    let (counts, outside) = mesh.count_sites(&far);
    assert!(outside, "the outside flag must be raised");
    assert!(counts.iter().all(|c| *c == 0.0));
}

/// Zero-weight bins must be skipped, not contribute `0 * log2(0) = NaN`.
/// Upstream guards this with `if (p_i > 0.0)`; dropping that guard poisons the
/// whole sum the moment any bin is empty, which on a real mesh is most of them.
#[test]
fn empty_bins_do_not_poison_the_sum_with_nan() {
    let mesh = RegularMesh {
        lower_left: [0.0, 0.0, 0.0],
        upper_right: [10.0, 10.0, 10.0],
        dimension: [10, 10, 10], // 1000 bins, only one occupied
    };
    let bank = vec![BankSite {
        r: Position::new(0.5, 0.5, 0.5),
        u: Direction::new(0.0, 0.0, 1.0),
        e: 1.0,
        wgt: 1.0,
        seed: 0,
    }];
    let h = mesh.shannon_entropy(&bank).expect("entropy exists");
    assert!(h.is_finite(), "999 empty bins must not make H NaN, got {h}");
    assert_eq!(h, 0.0);
}
