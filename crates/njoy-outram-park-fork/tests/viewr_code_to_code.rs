// SPDX-License-Identifier: AGPL-3.0-only
//! **VIEWR / graph, function level against compiled NJOY2016.80** (gh:#1369, differential #1366).
//!
//! Methodology: `verification_and_validation/viewr/extract.sh` compiles NJOY2016.80's `graph.f90`
//! **unmodified** (with `locale`, `phys`, `mainio`, `util`) plus the private routines `rget`,
//! `iget`, `stripv` (graph.f90) and `ascalv` (viewr.f90), copied out by line range after a
//! SHA-256 check of both files, and runs `viewr_driver.f90`: 14 `iget`/`rget` strings, 5 `stripv`
//! strings, 28 `ascalv` cases (every branch), the widths of 14 text strings in two fonts, twelve
//! `trans3` points under two perspective set-ups, four scale functions, and six PostScript
//! scenes (linear axes, log axes, all 26 symbols, text with instructions, 3-d axes/grid/curve/
//! arrow, curves with dashes, symbols, shading and hatching). Numbers are compared as hex bit
//! patterns; each scene's PostScript text line by line. `reference.txt` is the driver's output.
//!
//! The six scenes run in one process in both codes, in order, because `graph`'s module
//! variables (and `drawh`'s saved line width and dash) live for the whole process.
//!
//! Prediction (before the run): bit-identical numbers and text. The only libm exposure is
//! `log10` and the real power in the axis routines (PETIR against glibc), which could move a
//! last bit; none of the scenes passes a value where that changes a printed `Fw.d` field.
//!
//! Result: see each test and `verification_and_validation/viewr/README.md`.

use std::collections::HashMap;

use njoy_outram_park_fork::viewr::{ascalv, iget, rget, stripv, Graph};
use petir::mathf::RealMath;

const REFERENCE: &str = include_str!("../verification_and_validation/viewr/reference.txt");

/// The numbers part of the reference: `(tag, index) -> bits`.
fn numbers() -> HashMap<(String, usize), u64> {
    let mut m = HashMap::new();
    for line in REFERENCE.lines().take_while(|l| !l.starts_with("=== scene")) {
        let w: Vec<&str> = line.split_whitespace().collect();
        if w.len() >= 3 && w[2].len() == 16 && w[2].bytes().all(|b| b.is_ascii_hexdigit()) {
            m.insert((w[0].to_string(), w[1].parse().unwrap()), u64::from_str_radix(w[2], 16).unwrap());
        }
    }
    m
}

fn bits(nums: &HashMap<(String, usize), u64>, tag: &str, k: usize) -> u64 {
    nums[&(tag.to_string(), k)]
}

/// The lines of the numbers part starting with `tag`, split into words.
fn tagged(tag: &str) -> Vec<Vec<String>> {
    REFERENCE
        .lines()
        .take_while(|l| !l.starts_with("=== scene"))
        .filter(|l| l.split_whitespace().next() == Some(tag))
        .map(|l| l.split_whitespace().map(str::to_string).collect())
        .collect()
}

/// The PostScript of scene `n` of the reference.
fn scene(n: usize) -> String {
    let mut out = String::new();
    let mut on = false;
    for line in REFERENCE.lines() {
        if let Some(rest) = line.strip_prefix("=== scene ") {
            on = rest.trim() == n.to_string();
            continue;
        }
        if on {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

/// Compare two texts line by line; returns the number of differing lines and a first-difference note.
fn compare(ours: &str, theirs: &str) -> (usize, String) {
    let (a, b): (Vec<&str>, Vec<&str>) = (ours.lines().collect(), theirs.lines().collect());
    let diff: Vec<usize> = (0..a.len().max(b.len())).filter(|&i| a.get(i) != b.get(i)).collect();
    let note = diff.first().map_or(String::new(), |&i| format!("line {}: ours {:?}, NJOY {:?}", i + 1, a.get(i), b.get(i)));
    (diff.len(), note)
}

/// The driver's `begin`: page, window and the graph frame, in the Graph `g`.
fn begin_in(g: &mut Graph) {
    g.out.clear();
    g.gplot(1, 10.0, 7.5);
    g.initp(1, 10.0, 7.5, 2, 0.3, 0.0134, 0, 1);
    g.window(0.0, 0.0, 10.0, 7.5, 0.0, b"<t>itle one", 11, b"<t>wo", 5, 0);
    let _ = g.init2(0, 0);
}

/// The driver's `finish`: end window and page, trailer; the PostScript written since `begin`.
fn finish_in(g: &mut Graph) -> String {
    g.endw();
    g.endp();
    g.gdone();
    std::mem::take(&mut g.out)
}

/// `axis3` with the driver's argument order.
#[allow(clippy::too_many_arguments)]
fn ax(g: &mut Graph, a: [f64; 3], label: &[u8], nl: i32, ti: i32, nn: i32, asz: f64, o: [f64; 3], xv: [f64; 3], yv: [f64; 3], numr: i32) {
    g.axis3(a[0], a[1], a[2], label, nl, ti, nn, asz, o[0], o[1], o[2], xv[0], xv[1], xv[2], yv[0], yv[1], yv[2], numr);
}

/// **`iget`, `rget`, `stripv`**: 14 + 14 + 5 cases, every item against the Fortran.
/// Result: 33 of 33 identical (`rget` bits included).
#[test]
fn getters_and_stripv_match_fortran() {
    let sr = ["12", "-3", "0", ".5", ".8", "1.25", "-.7", "10.5", "D", "X", "0.05", "-12.5", "3-4", "7"];
    let nums = numbers();
    let iget_ref = tagged("IGET");
    let mut n = 0;
    for (k, s) in sr.iter().enumerate() {
        let b = s.as_bytes();
        let want: i32 = iget_ref[k][2].parse().unwrap();
        assert_eq!(iget(b, b.len()), want, "iget {s}");
        assert_eq!(rget(b, b.len()).to_bits(), bits(&nums, "RGET", k + 1), "rget {s}");
        n += 2;
    }
    let strip_ref = tagged("STRIPV");
    let st = ["  abc  ", "   ", "x y  z   ", "ab", "  1  2  "];
    for (k, s) in st.iter().enumerate() {
        let (out, len) = stripv(s.as_bytes());
        assert_eq!(len.to_string(), strip_ref[k][2], "stripv length {s:?}");
        let theirs = REFERENCE.lines().find(|l| l.starts_with(&format!("STRIPV {:>2} ", k + 1))).unwrap();
        let text = &theirs[theirs.find('[').unwrap() + 1..theirs.rfind(']').unwrap()];
        assert_eq!(String::from_utf8(out).unwrap().trim(), text.trim(), "stripv text {s:?}");
        n += 1;
    }
    println!("getters: {n} of 33 identical");
    assert_eq!(n, 33);
}

/// **`ascalv`**: 28 cases chosen to reach every branch (unreasonable input, zero ends, huge and
/// tiny ratios, close values, `p < 1`, `p >= 1e4`, each of the 1/2/5 choices, `m = 20`).
/// Result: 28 of 28 identical in `z1`, `z2`, `major`, `minor` (112 values).
#[test]
fn ascalv_matches_fortran() {
    let cases: [(i32, f64, f64); 28] = [
        (4, 0.3, 7.2),
        (4, 5.0, 5.0),
        (0, 1.0, 5.0),
        (21, 1.0, 5.0),
        (4, 0.0, 13.0),
        (4, -3.0, 0.0),
        (4, 1.0, 5000.0),
        (4, 1.0, 1.0005),
        (4, 100.0, 100.001),
        (4, 0.001, 0.0095),
        (4, 1.0e-6, 3.0e-6),
        (4, 0.0, 3.0e6),
        (4, 0.0, 9.0e9),
        (4, 0.0, 8.0),
        (4, 0.0, 8.04),
        (4, 0.0, 8.1),
        (4, 0.0, 19.0),
        (4, 0.0, 20.4),
        (4, -17.0, 23.0),
        (2, 0.2, 1.7),
        (20, 0.0, 1.0),
        (4, -0.5, -0.1),
        (4, 2.0e-4, 0.9),
        (4, 1.0, 2.0),
        (4, 50.0, 50.04),
        (4, 1.0e4, 7.5e4),
        (4, 3.0, 9.99e3),
        (4, -8.0e-3, 2.5e-2),
    ];
    let nums = numbers();
    let mm = tagged("ASCALV_MM");
    for (k, &(m, z1, z2)) in cases.iter().enumerate() {
        let (lo, hi, major, minor) = ascalv(m, z1, z2);
        assert_eq!(lo.to_bits(), bits(&nums, "ASCALV_Z1", k + 1), "case {} z1", k + 1);
        assert_eq!(hi.to_bits(), bits(&nums, "ASCALV_Z2", k + 1), "case {} z2", k + 1);
        assert_eq!((major.to_string(), minor.to_string()), (mm[k][2].clone(), mm[k][3].clone()), "case {}", k + 1);
    }
    println!("ascalv: 28 of 28 cases identical (112 values)");
}

const TX: [&str; 14] = [
    "<c>ross section (barns)",
    "E#EH.8<x#HXEX<=mc",
    "[a]lpha [b]eta [G]amma",
    "a#F3<greek",
    "a#M1<abc#M0< def",
    "a##b <<c>> [[d]]",
    "a#E.3<up#X< and #L.2<down#X<",
    "a#H1.5<big#HX< small",
    "a#E<x #L<x #H<x",
    "10#EH.8<-12#HXEX<",
    "*10#EH.8<3#HXEX<",
    "(paren) end<",
    "a#O1<xyz#F1< times",
    "ab#E.3<#H.5<cd",
];

/// **`txtlen`** on 14 strings with shift characters, instructions, doubled shift characters, a
/// trailing shift character and consecutive instructions, in Helvetica at 0.3 and Times-Roman at
/// 0.25. Result: 28 of 28 bit-identical.
#[test]
fn txtlen_matches_fortran() {
    let mut g = Graph::default();
    begin_in(&mut g);
    let nums = numbers();
    for (k, s) in TX.iter().enumerate() {
        let b = s.as_bytes();
        assert_eq!(g.txtlen(b, b.len() as i32, 2, 0.3).to_bits(), bits(&nums, "TXTLEN2", k + 1), "{s}");
        assert_eq!(g.txtlen(b, b.len() as i32, 1, 0.25).to_bits(), bits(&nums, "TXTLEN1", k + 1), "{s}");
    }
    println!("txtlen: 28 of 28 bit-identical");
}

fn scene1(g: &mut Graph) -> String {
    begin_in(g);
    let (x, y, z, o) = ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [0.0, 0.0, 0.0]);
    let ny = [-1.0, 0.0, 0.0];
    ax(g, [0.0, 10.0, 2.0], b"abc", -3, -1, 0, 5.0, o, x, y, 0);
    ax(g, [0.0, 1.0, 0.25], b"ab", 2, 1, 1, 5.0, [0.0, 1.0, 0.0], x, y, 0);
    ax(g, [-5.0, 5.0, 1.0], b"y", 1, -1, 2, 4.0, [6.0, 0.0, 0.0], y, ny, 1);
    ax(g, [0.0, 0.1, 0.02], b"frac", -4, 1, 3, 4.0, [0.0, 2.0, 0.0], x, y, 0);
    ax(g, [0.0, 10000.0, 2000.0], b"big", -3, -1, 0, 5.0, [0.0, 3.0, 0.0], x, y, 0);
    ax(g, [0.0, 3.5, 0.5], b"half", 3, 0, 0, 5.0, [0.0, 4.0, 0.0], x, y, 0);
    ax(g, [0.0, 10.0, 5.0], b"rot", -3, 1, 0, 4.0, o, y, ny, 1);
    ax(g, [0.0, 10.0, 2.0], b"z", 1, 1, 0, 3.0, o, z, y, 0);
    ax(g, [0.0, 5.0e-3, 1.0e-3], b".", -1, -1, 0, 4.0, [0.0, 5.0, 0.0], x, y, 0);
    ax(g, [0.0, 2.0e7, 5.0e6], b"huge", 1, -1, 1, 4.0, [0.0, 6.0, 0.0], x, y, 0);
    ax(g, [0.0, 10.0, 2.0], b"p<q", 3, 1, 2, 4.0, [0.0, 6.5, 0.0], x, y, 1);
    finish_in(g)
}

fn scene2(g: &mut Graph) -> String {
    begin_in(g);
    let (x, y) = ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
    let (yv, ny) = ([0.0, 1.0, 0.0], [-1.0, 0.0, 0.0]);
    ax(g, [1.0, 1000.0, 0.0], b"log", -3, -1, 0, 5.0, [0.0, 0.0, 0.0], x, y, 0);
    ax(g, [0.3, 5.0e5, 0.0], b"decades", 3, 1, 0, 8.0, [0.0, 1.0, 0.0], x, y, 0);
    ax(g, [1.0e-12, 1.0e-1, 0.0], b"neg", -3, -1, 0, 8.0, [0.0, 2.0, 0.0], x, y, 1);
    ax(g, [2.0, 50.0, 0.0], b"nondecade", -9, -1, 1, 5.0, [0.0, 3.0, 0.0], x, y, 0);
    ax(g, [4.0, 6.0, 0.0], b"short", 5, 1, 3, 5.0, [0.0, 4.0, 0.0], x, y, 0);
    ax(g, [1.0e-3, 1.0e20, 0.0], b"wide", -4, 0, 2, 7.0, [0.0, 5.0, 0.0], x, y, 0);
    ax(g, [3.0e-2, 4.0e1, 0.0], b"vertical", 3, 1, 0, 5.0, [8.0, 0.0, 0.0], yv, ny, 1);
    ax(g, [1.0e-3, 1.0e3, 0.0], b"upwards", -3, 1, 1, 5.0, [9.0, 0.0, 0.0], yv, ny, 0);
    ax(g, [1.0, 1.0e12, 0.0], b"ten12", -4, -1, 0, 5.0, [0.0, 6.0, 0.0], x, y, 0);
    finish_in(g)
}

fn scene3(g: &mut Graph) -> String {
    begin_in(g);
    for i in -1..=25_i32 {
        g.dsym(1.0 + 0.3 * f64::from(i), 3.0, i, 0.2);
    }
    g.dsym(2.0, 4.0, 3, 0.4);
    finish_in(g)
}

fn scene4(g: &mut Graph) -> String {
    begin_in(g);
    for (k, s) in TX.iter().enumerate() {
        let b = s.as_bytes();
        g.text3(b, b.len() as i32, 2, 0.3, 0.5, 0.5 * (k + 1) as f64, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0);
    }
    let (s1, s2) = (TX[0].as_bytes(), TX[1].as_bytes());
    g.text3(s1, s1.len() as i32, 1, 0.3, 5.0, 1.0, 0.0, 0.0, 1.0, 0.0, -1.0, 0.0, 0.0);
    g.text3(s2, s2.len() as i32, 3, 0.2, 5.0, 2.0, 0.0, 0.8, 0.6, 0.0, -0.6, 0.8, 0.0);
    finish_in(g)
}

/// Scene 5 and the 24 `trans3` values it prints; returns the PostScript and the count of
/// bit-identical `trans3` values.
fn scene5(g: &mut Graph) -> (String, usize) {
    let nums = numbers();
    g.out.clear();
    g.gplot(0, 7.5, 10.0);
    g.initp(0, 7.5, 10.0, 2, 0.2, 0.0134, 0, 2);
    g.window(0.0, 0.0, 7.5, 10.0, 0.0, b"<t>hree d", 7, b" ", 0, 0);
    g.init3(2.5, 6.5, 2.5, 15.0, -15.0, 15.0, 0);
    let (x, y, z, o) = ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [0.0, 0.0, 0.0]);
    ax(g, [0.0, 10.0, 2.0], b"x", -1, -1, 0, 2.5, o, x, y, 0);
    ax(g, [1.0, 1.0e3, 0.0], b"y", -1, -1, 0, 6.5, [2.5, 0.0, 0.0], y, [-1.0, 0.0, 0.0], 0);
    ax(g, [0.0, 5.0, 1.0], b"z", 1, 1, 1, 2.5, o, z, [0.0, -1.0, 0.0], 1);
    let mut same = 0;
    for k in 1..=6_usize {
        let kf = k as f64;
        let (u, v) = g.trans3(0.5 * kf, 1.0 * kf, 0.25 * kf);
        same += usize::from(u.to_bits() == bits(&nums, "TRANS3U", k)) + usize::from(v.to_bits() == bits(&nums, "TRANS3V", k));
    }
    g.grid3(1, 1, 0);
    g.grid3(0, 1, 1);
    g.grid3(1, -1, 1);
    let mut xs = vec![0.0; 2000];
    let mut ys = vec![0.0; 2000];
    let mut zs = vec![0.0; 2000];
    for k in 1..=5_usize {
        let kf = k as f64;
        xs[k - 1] = 0.5 * kf;
        ys[k - 1] = 1.0 + 0.5 * kf;
        zs[k - 1] = 0.4 * kf * f64::from(6 - k as i32);
    }
    xs[0] = 0.1;
    g.curv3(&mut xs, &mut ys, &mut zs, 5, 0.0134);
    g.vect3(0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 0.0134, 1021);
    g.init3(-2.5, 6.5, 2.5, 12.0, -10.0, 8.0, 0);
    for k in 1..=6_usize {
        let kf = k as f64;
        let (u, v) = g.trans3(0.5 * kf, 1.0 * kf, 0.25 * kf);
        same += usize::from(u.to_bits() == bits(&nums, "TRANS3NU", k)) + usize::from(v.to_bits() == bits(&nums, "TRANS3NV", k));
    }
    (finish_in(g), same)
}

/// Scene 6 and the four scale values; returns the PostScript and the count of identical values.
fn scene6(g: &mut Graph) -> (String, usize) {
    let nums = numbers();
    begin_in(g);
    let (x, y, o) = ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 0.0]);
    ax(g, [1.0, 1.0e4, 0.0], b"e", -1, -1, 0, 8.0, o, x, y, 0);
    ax(g, [0.0, 10.0, 2.0], b"s", 1, -1, 0, 5.0, o, y, [-1.0, 0.0, 0.0], 1);
    g.grid2(1, 1);
    let mut xs = [0.0_f64; 8];
    let mut ys = [0.0_f64; 8];
    for i in 1..=8_usize {
        let f = i as f64;
        xs[i - 1] = RealMath::r_powi(1.7_f64, i as i32);
        ys[i - 1] = 1.0 + 0.8 * f - 0.07 * f * f;
    }
    g.frame2(8.0, 5.0, 0.0, 2);
    g.curv2(&xs, &ys, 8, 0, 0.0134, 0, 0, 0.0, 0);
    g.curv2(&xs, &ys, 8, 1, 0.0134, 2, 1, 0.15, 0);
    g.curv2(&xs, &ys, 8, 2, 0.0134, -1, 2, 0.15, 0);
    g.curv2(&xs, &ys, 8, 3, 0.0134, 3, 5, 0.15, 0);
    g.curv2(&xs, &ys, 8, 4, 0.0268, 0, 0, 0.15, 0);
    g.endfr();
    for (col, w, shade) in [(2, 0.0134, 3), (3, 0.0134, 15), (4, 0.0, 25), (5, 0.0134, 35), (6, 0.0134, 45), (7, 0.0134, 75)] {
        g.frame2(8.0, 5.0, 0.0, col);
        g.curv2(&xs, &ys, 8, 0, w, 0, 0, 0.15, shade);
        g.endfr();
    }
    g.frame2(8.0, 5.0, 0.0, 0);
    for i in 0..=25_i32 {
        g.dsym(1.0 + 0.2 * f64::from(i), 0.5, i, 0.1);
    }
    let same = usize::from(g.xscale(100.0).to_bits() == bits(&nums, "XSCALE", 1))
        + usize::from(g.yscale(3.0).to_bits() == bits(&nums, "YSCALE", 1))
        + usize::from(g.xinvrs(2.0).to_bits() == bits(&nums, "XINVRS", 1))
        + usize::from(g.yinvrs(2.0).to_bits() == bits(&nums, "YINVRS", 1));
    g.endfr();
    (finish_in(g), same)
}

/// **The six PostScript scenes**, in the driver's order, in one `Graph` (as the Fortran process
/// keeps one set of module variables):
///
/// 1. eleven linear axes (labels above/below, ticks both ways, `nonum` 0-3, `numr` 0/1,
///    fractions, `*10^n` factors of both signs, a `.` label, the three axis directions);
/// 2. nine log axes (one decade, many decades with label skipping, exponents `<= -10`,
///    non-decade ends, short ranges, `10^20`, vertical and rotated numbers, `nonum` 1-3);
/// 3. the 25 symbols and the default for `isym` -1 and 25;
/// 4. `text3` on 14 strings (upright, rotated, Symbol);
/// 5. 3-d: `init3` for two viewpoints, a log y axis, `trans3` of 12 points under each (24 values),
///    `grid3` on three planes, `curv3`, an arrow `vect3` (`ivec = 1021`);
/// 6. a log-x frame with grid, eight curves (dash types 0-4, symbols every `i` points,
///    unconnected symbols, widths, gray shades, both hatch slants and cross hatching, coloured
///    shades), the symbols again, and `xscale`, `yscale`, `xinvrs`, `yinvrs`.
///
/// Result: every line of all six scenes identical, all 28 numbers bit-identical (see the log).
#[test]
fn six_postscript_scenes_and_scales_match_fortran() {
    let mut g = Graph::default();
    let mut bad = Vec::new();
    let mut total = 0;
    let mut check = |n: usize, ours: String| {
        let theirs = scene(n);
        let (d, note) = compare(&ours, &theirs);
        println!("scene {n}: {d} differing lines of {}", theirs.lines().count());
        total += theirs.lines().count();
        if d != 0 {
            bad.push(format!("scene {n}: {note}"));
        }
    };
    check(1, scene1(&mut g));
    check(2, scene2(&mut g));
    check(3, scene3(&mut g));
    check(4, scene4(&mut g));
    let (ps5, n5) = scene5(&mut g);
    check(5, ps5);
    let (ps6, n6) = scene6(&mut g);
    check(6, ps6);
    println!("trans3 + scale functions: {} of 28 values bit-identical; {total} PostScript lines", n5 + n6);
    assert!(bad.is_empty(), "{bad:?}");
    assert_eq!((n5, n6), (24, 4));
}
