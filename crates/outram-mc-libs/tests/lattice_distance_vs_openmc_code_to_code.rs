// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors.

//! `RectLattice` and `HexLattice` `distance`, `get_indices` and tile centres
//! against OpenMC's `src/lattice.cpp`, compiled and run (gh:#1386 prio-1 row 3,
//! epic gh:#1336). Research, education and V&V only.
//!
//! # Methodology
//!
//! `verification_and_validation/upstream_openmc_surfaces/run.sh` takes
//! `RectLattice::{distance, get_indices, get_local_position}` and
//! `HexLattice::{distance, get_indices, get_local_position}` from OpenMC
//! `8f79d9d02483c90e4134f03a9df4b75ce6b062bf` (MIT) **verbatim** (SHA-256-checked
//! line ranges), together with `isclose` (`math_functions.cpp:1015`) and
//! `coincident` (`geometry.h:68`), stubs the class declarations (S3 in the V&V
//! README) and builds with `g++ -O2 -ffp-contract=off`. Instances:
//!
//! - rect 0: 2-D, 3x3, pitch 2; rect 1: 3-D, 2x3x2, pitch 1.2/1.2/1.0;
//!   **rect 2: 3-D, 3x3x3, pitch 1e5 and rect 3: 2-D, 4x4, pitch 1e6** (the
//!   large-pitch cases of upstream #3853);
//! - hex 0-3: orientation y and x, 2-D and 3-D, pitch 1.8 / 4.0, off-centre
//!   centres; hex 4-5: orientation y and x, 4 rings, pitch 1e4 (large pitch).
//!
//! Inputs: `distance` on a grid of tile-local origins (inside, on edges, on
//! corners, random) x 12 directions (axis-aligned, diagonal, a near-diagonal
//! `1, 1+1e-13`, random); `get_indices` on global points on and off tile
//! boundaries (exactly on a boundary, one ulp-ish inside, outside the lattice, on
//! hex edge mid-points and vertices, random) x 6 to 8 directions; hex tile
//! centres; and a 12-crossing walk per lattice and ray (indices, `distance`,
//! `r += d u`), all as hex bits. The Rust side runs the same inputs through the
//! port.
//!
//! The port's `HexLattice::distance` takes a **tile-local** position and rebuilds
//! the lattice-frame one by adding the tile centre; the driver passes upstream the
//! lattice-frame position `r_local + centre` (centre = `-get_local_position(0, i)`,
//! exact), which is what that rebuild computes. The walk test feeds upstream's
//! `geometry.cpp:459-467` convention (lattice-frame `r` as is), so any rounding
//! introduced by the port's local-frame calling convention shows up there.
//!
//! Upstream's "infinite" is `DBL_MAX`, the port's `f64::INFINITY`: counted as
//! agreeing misses.
//!
//! # Prediction (before the first run)
//!
//! Rect `get_indices` and the hex routines were translated closely, so bit-identity
//! was expected there. Rect `distance` was expected to **differ in the translation
//! vector** for large pitch: the port predates upstream #3853 and reconstructs the
//! crossing position (`|r + u d - edge| < 1e-12`) where upstream compares the axis
//! distances (`isclose`). The distance value itself should be bit-identical.
//!
//! # Results
//!
//! Before the fix of this change (rect `distance` ported from `8f79d9d0`): see the
//! V&V README for the counts and the after-fix numbers.

use outram_mc_libs::geometry::lattice::{HexLattice, HexOrientation, RectLattice};
use outram_mc_libs::geometry::position::{Direction, Position};

const REFERENCE: &str =
    include_str!("../verification_and_validation/upstream_openmc_surfaces/reference.txt");
const OPENMC_INFTY: f64 = f64::MAX;

fn hex(tok: &str) -> f64 {
    f64::from_bits(u64::from_str_radix(tok, 16).unwrap_or_else(|_| panic!("bad hex {tok}")))
}
fn int(tok: &str) -> i32 {
    tok.parse().unwrap()
}
fn toks(line: &str) -> Vec<&str> {
    line.split_whitespace().collect()
}

fn rect(inst: usize) -> RectLattice {
    for line in REFERENCE.lines() {
        let t = toks(line);
        if t.first() == Some(&"LR") && t[1].parse::<usize>().unwrap() == inst {
            let n = [int(t[3]) as usize, int(t[4]) as usize, int(t[5]) as usize];
            return RectLattice {
                id: inst as i32,
                n,
                lower_left: Position::new(hex(t[6]), hex(t[7]), hex(t[8])),
                pitch: [hex(t[9]), hex(t[10]), hex(t[11])],
                universes: vec![0; n[0] * n[1] * n[2]],
                outer: None,
            };
        }
    }
    panic!("no rect lattice {inst}");
}

fn hexl(inst: usize) -> HexLattice {
    for line in REFERENCE.lines() {
        let t = toks(line);
        if t.first() == Some(&"LH") && t[1].parse::<usize>().unwrap() == inst {
            let nr = int(t[3]) as usize;
            let na = int(t[4]) as usize;
            let w = 2 * nr - 1;
            return HexLattice {
                id: inst as i32,
                orientation: if t[2] == "0" { HexOrientation::Y } else { HexOrientation::X },
                n_rings: nr,
                n_axial: na,
                center: Position::new(hex(t[5]), hex(t[6]), hex(t[7])),
                pitch: [hex(t[8]), hex(t[9])],
                universes: vec![0; w * w * na],
                outer: None,
            };
        }
    }
    panic!("no hex lattice {inst}");
}

#[derive(Default, Debug)]
struct Tally {
    total: usize,
    value_identical: usize,
    both_infinite: usize,
    trans_identical: usize,
    worst_rel: f64,
    bad: Vec<String>,
}

impl Tally {
    fn dist(&mut self, got: f64, want: f64) -> bool {
        if got.to_bits() == want.to_bits() {
            self.value_identical += 1;
            true
        } else if want == OPENMC_INFTY && got == f64::INFINITY {
            self.both_infinite += 1;
            true
        } else {
            if want.is_finite() && want != 0.0 && got.is_finite() {
                let rel = ((got - want) / want).abs();
                if rel > self.worst_rel {
                    self.worst_rel = rel;
                }
            }
            false
        }
    }
    fn report(&self, what: &str) {
        println!(
            "{what}: {} cases; value bit-identical {} (+{} both-infinite); second quantity identical {}; worst rel {:.3e}",
            self.total, self.value_identical, self.both_infinite, self.trans_identical, self.worst_rel
        );
        for b in self.bad.iter().take(6) {
            println!("  e.g. {b}");
        }
    }
    fn assert_ok(&self, what: &str) {
        self.report(what);
        assert!(self.total > 0, "no cases for {what}");
        assert_eq!(self.value_identical + self.both_infinite, self.total, "{what}: value");
        assert_eq!(self.trans_identical, self.total, "{what}: translation / indices");
    }
}

fn pos(t: &[&str], i: usize) -> Position {
    Position::new(hex(t[i]), hex(t[i + 1]), hex(t[i + 2]))
}
fn dir(t: &[&str], i: usize) -> Direction {
    Direction::new(hex(t[i]), hex(t[i + 1]), hex(t[i + 2]))
}

fn rect_distance(insts: &[usize]) -> Tally {
    let mut tally = Tally::default();
    let lats: Vec<(usize, RectLattice)> = insts.iter().map(|&i| (i, rect(i))).collect();
    for line in REFERENCE.lines() {
        let t = toks(line);
        if t.first() != Some(&"RD") {
            continue;
        }
        let inst: usize = t[1].parse().unwrap();
        let Some((_, lat)) = lats.iter().find(|(i, _)| *i == inst) else { continue };
        let (r, u) = (pos(&t, 2), dir(&t, 5));
        let (d, tr) = lat.distance(r, u);
        let want_tr = [int(t[10]), int(t[11]), int(t[12])];
        tally.total += 1;
        let ok_d = tally.dist(d, hex(t[9]));
        if tr == want_tr {
            tally.trans_identical += 1;
        }
        if (!ok_d || tr != want_tr) && tally.bad.len() < 6 {
            tally.bad.push(format!(
                "rect {inst} r=({:e},{:e},{:e}) u=({:e},{:e},{:e}): d {d:e} vs {:e}, trans {tr:?} vs {want_tr:?}",
                r.x, r.y, r.z, u.u, u.v, u.w, hex(t[9])
            ));
        }
    }
    tally
}

fn indices(kind: &str, insts: &[usize]) -> Tally {
    let mut tally = Tally::default();
    let rl: Vec<(usize, RectLattice)> =
        if kind == "RI" { insts.iter().map(|&i| (i, rect(i))).collect() } else { vec![] };
    let hl: Vec<(usize, HexLattice)> =
        if kind == "HI" { insts.iter().map(|&i| (i, hexl(i))).collect() } else { vec![] };
    for line in REFERENCE.lines() {
        let t = toks(line);
        if t.first() != Some(&kind) {
            continue;
        }
        let inst: usize = t[1].parse().unwrap();
        let (r, u) = (pos(&t, 2), dir(&t, 5));
        let got = if kind == "RI" {
            let Some((_, l)) = rl.iter().find(|(i, _)| *i == inst) else { continue };
            l.get_indices(r, u)
        } else {
            let Some((_, l)) = hl.iter().find(|(i, _)| *i == inst) else { continue };
            l.get_indices(r, u)
        };
        let want = [int(t[9]), int(t[10]), int(t[11])];
        tally.total += 1;
        tally.value_identical += 1; // no floating value here
        if got == want {
            tally.trans_identical += 1;
        } else if tally.bad.len() < 6 {
            tally.bad.push(format!(
                "{kind} {inst} r=({:e},{:e},{:e}) u=({:e},{:e},{:e}): {got:?} vs {want:?}",
                r.x, r.y, r.z, u.u, u.v, u.w
            ));
        }
    }
    tally
}

fn hex_distance(insts: &[usize]) -> Tally {
    let mut tally = Tally::default();
    let lats: Vec<(usize, HexLattice)> = insts.iter().map(|&i| (i, hexl(i))).collect();
    for line in REFERENCE.lines() {
        let t = toks(line);
        if t.first() != Some(&"HD") {
            continue;
        }
        let inst: usize = t[1].parse().unwrap();
        let Some((_, lat)) = lats.iter().find(|(i, _)| *i == inst) else { continue };
        let (r, u) = (pos(&t, 2), dir(&t, 5));
        let i = [int(t[8]), int(t[9]), int(t[10])];
        let (d, tr) = lat.distance(r, u, i);
        let mut want_tr = [int(t[13]), int(t[14]), int(t[15])];
        tally.total += 1;
        let ok_d = tally.dist(d, hex(t[12]));
        if hex(t[12]) == OPENMC_INFTY {
            // Upstream leaves `lattice_trans` uninitialised when no boundary is
            // hit (UB, garbage in the reference): nothing to compare.
            want_tr = tr;
        }
        if tr == want_tr {
            tally.trans_identical += 1;
        }
        if (!ok_d || tr != want_tr) && tally.bad.len() < 6 {
            tally.bad.push(format!(
                "hex {inst} tile {i:?} r=({:e},{:e},{:e}) u=({:e},{:e},{:e}): d {d:e} vs {:e}, trans {tr:?} vs {want_tr:?}",
                r.x, r.y, r.z, u.u, u.v, u.w, hex(t[12])
            ));
        }
    }
    tally
}

#[test]
fn rect_distance_small_pitch_matches_openmc() {
    rect_distance(&[0, 1]).assert_ok("RectLattice::distance, pitch ~1-2 cm");
}

/// Upstream #3853: for pitch 1e5 / 1e6 cm upstream decides the crossed boundary
/// from the axis distances. Known gap closed in this change (gh:#1386).
#[test]
fn rect_distance_large_pitch_matches_openmc() {
    rect_distance(&[2, 3]).assert_ok("RectLattice::distance, pitch 1e5-1e6 cm");
}

#[test]
fn rect_get_indices_matches_openmc() {
    indices("RI", &[0, 1, 2, 3]).assert_ok("RectLattice::get_indices");
}

#[test]
fn hex_distance_small_pitch_matches_openmc() {
    hex_distance(&[0, 1, 2, 3]).assert_ok("HexLattice::distance, pitch 1.8 cm");
}

#[test]
fn hex_distance_large_pitch_matches_openmc() {
    hex_distance(&[4, 5]).assert_ok("HexLattice::distance, pitch 1e4 cm");
}

#[test]
fn hex_get_indices_matches_openmc() {
    indices("HI", &[0, 1, 2, 3, 4, 5]).assert_ok("HexLattice::get_indices");
}

#[test]
fn hex_tile_centre_matches_openmc() {
    let mut tally = Tally::default();
    let lats: Vec<HexLattice> = (0..6).map(hexl).collect();
    for line in REFERENCE.lines() {
        let t = toks(line);
        if t.first() != Some(&"HO") {
            continue;
        }
        let lat = &lats[t[1].parse::<usize>().unwrap()];
        let i = [int(t[2]), int(t[3]), int(t[4])];
        let c = lat.tile_center(i);
        let want = pos(&t, 6);
        tally.total += 1;
        // The driver gets the centre as `-(0 - centre)`, so a zero comes out as -0:
        // compare by value (0.0 == -0.0), which is bit-equal for every non-zero.
        let same = c.x == want.x && c.y == want.y && (lat.n_axial == 1 || c.z == want.z);
        if same {
            tally.value_identical += 1;
            tally.trans_identical += 1;
        } else if tally.bad.len() < 6 {
            tally.bad.push(format!("tile {i:?}: ({:e},{:e},{:e}) vs ({:e},{:e},{:e})", c.x, c.y, c.z, want.x, want.y, want.z));
        }
    }
    tally.assert_ok("HexLattice tile centre");
}

/// Integration test: walk five rays through each lattice for 12 crossings
/// (`get_indices`, `get_local_position`, `distance`, `r += d u`, `i += trans`), as
/// OpenMC's transport does one lattice level, and compare every step's indices,
/// distance and position bit by bit.
fn walks(kind: &str, insts: &[usize]) -> Tally {
    let mut tally = Tally::default();
    // Re-run the walk in Rust, keyed by (inst, ray), and compare to the W lines.
    type Key = (usize, usize);
    let mut want: std::collections::BTreeMap<Key, Vec<(i32, [i32; 3], f64, [f64; 3])>> =
        Default::default();
    for line in REFERENCE.lines() {
        let t = toks(line);
        if t.first() == Some(&"W") && t[1] == kind {
            let key = (t[2].parse().unwrap(), t[3].parse().unwrap());
            want.entry(key).or_default().push((
                int(t[4]),
                [int(t[5]), int(t[6]), int(t[7])],
                hex(t[8]),
                [hex(t[9]), hex(t[10]), hex(t[11])],
            ));
        }
    }
    fn unit(x: f64, y: f64, z: f64) -> Direction {
        let n = (x * x + y * y + z * z).sqrt();
        Direction::new(x / n, y / n, z / n)
    }
    let dirs = [
        unit(1.0, 0.7, 0.3),
        unit(-0.4, 1.0, -0.2),
        unit(1.0, 1.0, 0.0),
        unit(-1.0, -1.0, 0.5),
        unit(0.2, -1.0, 0.9),
    ];
    for &inst in insts {
        for (k, u) in dirs.iter().enumerate() {
            let w = &want[&(inst, k)];
            let (start, mut i, lat_rect, lat_hex) = if kind == "rect" {
                let l = rect(inst);
                let s = Position::new(
                    l.lower_left.x + 0.37 * l.pitch[0],
                    l.lower_left.y + 0.41 * l.pitch[1],
                    if l.n[2] > 1 { l.lower_left.z + 0.43 * l.pitch[2] } else { 0.0 },
                );
                let i = l.get_indices(s, *u);
                (s, i, Some(l), None)
            } else {
                let l = hexl(inst);
                let s = Position::new(
                    l.center.x + 0.31 * l.pitch[0],
                    l.center.y - 0.17 * l.pitch[0],
                    if l.n_axial > 1 { l.center.z + 0.2 * l.pitch[1] } else { 0.0 },
                );
                let i = l.get_indices(s, *u);
                (s, i, None, Some(l))
            };
            let mut r = start;
            tally.total += 1;
            if i == w[0].1 {
                tally.value_identical += 1;
                tally.trans_identical += 1;
            }
            let mut diverged = false;
            for (step, wstep) in w.iter().enumerate().skip(1) {
                let (d, tr, local) = if let Some(l) = &lat_rect {
                    let rl = l.get_local_position(r, i);
                    let (d, tr) = l.distance(rl, *u);
                    (d, tr, rl)
                } else {
                    let l = lat_hex.as_ref().unwrap();
                    let rl = l.get_local_position(r, i);
                    let (d, tr) = l.distance_in_lattice_frame(Position::new(r.x, r.y, rl.z), *u, i);
                    (d, tr, rl)
                };
                let _ = local;
                r = Position::new(r.x + d * u.u, r.y + d * u.v, r.z + d * u.w);
                i = [i[0] + tr[0], i[1] + tr[1], i[2] + tr[2]];
                tally.total += 1;
                let same = !diverged
                    && i == wstep.1
                    && d.to_bits() == wstep.2.to_bits()
                    && r.x.to_bits() == wstep.3[0].to_bits()
                    && r.y.to_bits() == wstep.3[1].to_bits()
                    && r.z.to_bits() == wstep.3[2].to_bits();
                if same {
                    tally.value_identical += 1;
                    tally.trans_identical += 1;
                } else if !diverged {
                    diverged = true;
                    if tally.bad.len() < 6 {
                        tally.bad.push(format!(
                            "{kind} {inst} ray {k} step {}: idx {i:?} vs {:?}, d {d:e} vs {:e}",
                            step - 1,
                            wstep.1,
                            wstep.2
                        ));
                    }
                }
            }
        }
    }
    tally
}

#[test]
fn rect_lattice_walk_matches_openmc() {
    walks("rect", &[0, 1, 2, 3]).assert_ok("rect lattice walk");
}

#[test]
fn hex_lattice_walk_matches_openmc() {
    walks("hex", &[0, 1, 2, 3, 4, 5]).assert_ok("hex lattice walk");
}
