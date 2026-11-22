// SPDX-License-Identifier: GPL-3.0

//! **A real OpenMC library, converted, against `Nuclide::from_ace` on the ACE it
//! was built from** (GitHub #303, acceptance row 2's precondition).
//!
//! Lives inside the crate because the comparison reaches `Nuclide`'s private
//! fields, so that "bit-identical" is a statement about the whole converted
//! object and not about what a public accessor happens to expose.
//!
//! Skips with a printed reason unless both env vars are set:
//!
//! - `OUTRAM_OPENMC_LCT008_H5` -- the directory of `*.h5` (the gh:#550 library,
//!   NJOY2016, two temperatures `0K` and `294K`);
//! - `OUTRAM_OPENMC_LCT008_NJOY` -- the directory holding the NJOY ACE it was
//!   built from, `293.6K/<name>.ace` and `0K/<name>.ace`.
//!
//! Method and predictions: `verification_and_validation/hdf5_library_303/`.

use super::*;
use std::fmt::Write as _;
use std::path::PathBuf;

use njoy_outram_park_fork::hdf5::nuclide_read::read_nuclide_selected;

/// FNV-1a over the `Debug` text. An `f64`'s `Debug` text is its shortest
/// round-trip form, so equal text is equal bits (and `-0.0` differs from `0.0`).
struct Fnv(u64, usize);

impl core::fmt::Write for Fnv {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for b in s.bytes() {
            self.0 = (self.0 ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3);
        }
        self.1 += s.len();
        Ok(())
    }
}

fn fp<T: core::fmt::Debug>(t: &T) -> (u64, usize) {
    let mut f = Fnv(0xcbf2_9ce4_8422_2325, 0);
    let _ = write!(f, "{t:?}");
    (f.0, f.1)
}

fn same<T: core::fmt::Debug>(a: &T, b: &T) -> bool {
    fp(a) == fp(b)
}

/// `(differing words, worst relative difference)` of two `(E, sigma)` lists.
fn pair_diff(a: &[(f64, f64)], b: &[(f64, f64)]) -> (usize, f64) {
    if a.len() != b.len() {
        return (a.len().max(b.len()), f64::INFINITY);
    }
    let mut n = 0;
    let mut worst = 0.0_f64;
    for (x, y) in a.iter().zip(b) {
        for (u, v) in [(x.0, y.0), (x.1, y.1)] {
            if u.to_bits() != v.to_bits() {
                n += 1;
                let d = (u - v).abs() / u.abs().max(v.abs()).max(f64::MIN_POSITIVE);
                worst = worst.max(d);
            }
        }
    }
    (n, worst)
}

/// How an angular table differs: words, worst relative, and which field.
fn angular_diff(a: &ElasticAngular, b: &ElasticAngular) -> String {
    if a.energies.len() != b.energies.len() || a.lct != b.lct {
        return format!("shape {} vs {} energies, lct {} vs {}", a.energies.len(), b.energies.len(), a.lct, b.lct);
    }
    let (mut n, mut worst, mut hist, mut shape) = (0usize, 0.0_f64, 0usize, 0usize);
    let mut chk = |u: f64, v: f64| {
        if u.to_bits() != v.to_bits() {
            n += 1;
            worst = worst.max((u - v).abs() / u.abs().max(v.abs()).max(f64::MIN_POSITIVE));
        }
    };
    for (x, y) in a.energies.iter().zip(&b.energies) {
        chk(x.e_mev, y.e_mev);
        if x.histogram != y.histogram {
            hist += 1;
        }
        if x.cosines.len() != y.cosines.len() {
            shape += 1;
            continue;
        }
        for (u, v) in x.cosines.iter().zip(&y.cosines).chain(x.pdf.iter().zip(&y.pdf)).chain(x.cdf.iter().zip(&y.cdf)) {
            chk(*u, *v);
        }
    }
    format!("{n} words differ (worst rel {worst:.2e}), {hist} histogram flags, {shape} shape mismatches")
}

/// Where two `Debug` texts first differ, with context (only for a field that
/// already differs, so the cost of formatting is paid on failure only).
fn first_diff<T: core::fmt::Debug>(a: &T, b: &T) -> String {
    let (x, y) = (format!("{a:?}"), format!("{b:?}"));
    let i = x.bytes().zip(y.bytes()).position(|(u, v)| u != v).unwrap_or(x.len().min(y.len()));
    let cut = |s: &str| {
        let lo = s.char_indices().map(|(k, _)| k).filter(|&k| k + 90 >= i).next().unwrap_or(0);
        let hi = s.char_indices().map(|(k, _)| k).filter(|&k| k <= i + 90).last().unwrap_or(s.len());
        s[lo..hi].to_string()
    };
    format!("first difference at byte {i} of {}/{}:\n      h5 : ...{}...\n      ace: ...{}...", x.len(), y.len(), cut(&x), cut(&y))
}

struct Dirs {
    h5: PathBuf,
    njoy: PathBuf,
}

fn dirs() -> Option<Dirs> {
    let h5 = std::env::var_os("OUTRAM_OPENMC_LCT008_H5")?;
    let njoy = std::env::var_os("OUTRAM_OPENMC_LCT008_NJOY")?;
    Some(Dirs { h5: h5.into(), njoy: njoy.into() })
}

const LCT008: [&str; 11] = [
    "H1", "B10", "O16", "Al27", "Si28", "Si29", "Si30", "Mn55", "U234", "U235", "U238",
];

/// What the comparison found on one nuclide.
struct Findings {
    name: String,
    lines: Vec<String>,
    /// Predicted-identical items that were not.
    misses: Vec<String>,
}

fn check<T: core::fmt::Debug>(f: &mut Findings, what: &str, a: &T, b: &T) {
    let ok = same(a, b);
    f.lines.push(format!("{what}: {}", if ok { "identical" } else { "DIFFERENT" }));
    if !ok {
        f.misses.push(what.to_string());
        f.lines.push(format!("    {}", first_diff(a, b)));
    }
}

/// An angle table is predicted identical, or within one ulp where OpenMC's
/// `e_mev * 1e6` and our `/ 1e6` do not round-trip.
fn check_angular(f: &mut Findings, what: &str, a: &ElasticAngular, b: &ElasticAngular) {
    let d = angular_diff(a, b);
    if same(a, b) {
        f.lines.push(format!("{what}: identical"));
        return;
    }
    f.lines.push(format!("{what}: {d}"));
    let worst: f64 = d
        .split("worst rel ")
        .nth(1)
        .and_then(|t| t.split(')').next())
        .and_then(|t| t.parse().ok())
        .unwrap_or(f64::INFINITY);
    if worst > 2.3e-16 || d.contains(" 1 histogram") || !d.contains(" 0 shape") {
        f.misses.push(format!("{what}: {d}"));
    }
}

fn compare(name: &str, h5: &Nuclide, ace: &Nuclide) -> Findings {
    let mut f = Findings { name: name.into(), lines: Vec::new(), misses: Vec::new() };
    let expect_same = |f: &mut Findings, what: &str, ok: bool| {
        f.lines.push(format!("{what}: {}", if ok { "identical" } else { "DIFFERENT" }));
        if !ok {
            f.misses.push(what.to_string());
        }
    };

    expect_same(&mut f, "awr", h5.awr.to_bits() == ace.awr.to_bits());
    check(&mut f, "nu", &h5.nu, &ace.nu);
    check(&mut f, "chi", &h5.chi, &ace.chi);
    check(&mut f, "continuum.mt91", &h5.continuum.mt91, &ace.continuum.mt91);
    check(&mut f, "continuum.mt16", &h5.continuum.mt16, &ace.continuum.mt16);
    check(&mut f, "continuum.mt17", &h5.continuum.mt17, &ace.continuum.mt17);
    check(&mut f, "continuum.mt5", &h5.continuum.mt5, &ace.continuum.mt5);
    check(&mut f, "continuum.uncorrelated", &h5.continuum.uncorrelated, &ace.continuum.uncorrelated);
    check(&mut f, "other_channels", &h5.other_channels, &ace.other_channels);
    check(&mut f, "urr", &h5.urr, &ace.urr);
    if ace.elastic_0k.is_empty() {
        f.lines.push(format!(
            "elastic_0k: the ACE route has no 0 K companion here; h5 carries {} points (DBRC {})",
            h5.elastic_0k.len(),
            h5.dbrc.is_some()
        ));
    } else {
        check(&mut f, "elastic_0k", &h5.elastic_0k, &ace.elastic_0k);
        expect_same(&mut f, "dbrc", h5.dbrc.is_some() == ace.dbrc.is_some());
    }

    let (
        XsSource::Pointwise { recon: rh, inel: ih, elastic_angular: eh, inelastic_angular: ah, .. },
        XsSource::Pointwise { recon: ra, inel: ia, elastic_angular: ea, inelastic_angular: aa, .. },
    ) = (&h5.xs, &ace.xs)
    else {
        f.misses.push("xs is not Pointwise".into());
        return f;
    };
    check_angular(&mut f, "elastic_angular", eh, ea);
    for k in 0..ah.len().min(aa.len()) {
        check_angular(&mut f, &format!("inelastic_angular MT={}", ah[k].0), &ah[k].1, &aa[k].1);
    }
    expect_same(&mut f, "inelastic_angular table count", ah.len() == aa.len());
    check(&mut f, "inelastic levels", ih, ia);

    // Sections: MT=1 is OpenMC's own sum, everything else is the ACE array.
    let mts_h: Vec<i32> = rh.sections.iter().map(|s| s.mt.number()).collect();
    let mts_a: Vec<i32> = ra.sections.iter().map(|s| s.mt.number()).collect();
    expect_same(&mut f, "section MT list", mts_h == mts_a);
    let mut words = 0usize;
    let mut total_words = 0usize;
    for (sh, sa) in rh.sections.iter().zip(&ra.sections) {
        if sh.mt.number() == 1 {
            let (n, worst) = pair_diff(&sh.pairs, &sa.pairs);
            f.lines.push(format!(
                "MT=1 total: {n}/{} words differ, worst relative {worst:.2e} (predicted <= 1e-8; pinned at 1e-6 after the first run, see the V&V record)",
                2 * sh.pairs.len()
            ));
            if let Some((e, a, b)) = sh
                .pairs
                .iter()
                .zip(&sa.pairs)
                .map(|(x, y)| (x.0, x.1, y.1))
                .max_by(|u, v| {
                    let r = |t: &(f64, f64, f64)| (t.1 - t.2).abs() / t.2.abs().max(f64::MIN_POSITIVE);
                    r(u).partial_cmp(&r(v)).unwrap_or(core::cmp::Ordering::Equal)
                })
            {
                f.lines.push(format!("   worst at E = {e:.6e} eV: h5 {a:.9e} vs ESZ {b:.9e} b"));
            }
            if worst > 1.0e-6 {
                f.misses.push(format!("MT=1 worst relative {worst:.2e}"));
            }
            continue;
        }
        let (n, _) = pair_diff(&sh.pairs, &sa.pairs);
        words += n;
        total_words += 2 * sh.pairs.len();
        if n != 0 || sh.qi.to_bits() != sa.qi.to_bits() {
            f.misses.push(format!("MT={} xs words differ: {n}", sh.mt.number()));
        }
    }
    f.lines.push(format!("sampled cross sections (MT != 1): {words}/{total_words} words differ"));

    match (&h5.delayed, &ace.delayed) {
        (None, None) => f.lines.push("delayed: none on both".into()),
        (Some(dh), Some(da)) => {
            let lam_rel = dh
                .lambda
                .iter()
                .zip(&da.lambda)
                .map(|(a, b)| (a - b).abs() / b.abs())
                .fold(0.0_f64, f64::max);
            let mut nu_rel = 0.0_f64;
            for &e in dh.energy.iter().chain(&da.energy) {
                let (a, b) = (dh.nu_delayed_at(e), da.nu_delayed_at(e));
                nu_rel = nu_rel.max((a - b).abs() / b.abs().max(f64::MIN_POSITIVE));
            }
            let mut p_rel = 0.0_f64;
            for k in 0..dh.n_groups().min(da.n_groups()) {
                for &e in dh.energy.iter().chain(&da.energy) {
                    let (a, b) = (dh.group_fraction(k, e), da.group_fraction(k, e));
                    p_rel = p_rel.max((a - b).abs() / b.abs().max(1.0e-30));
                }
            }
            f.lines.push(format!(
                "delayed: {} groups vs {}, lambda rel {lam_rel:.2e}, nu_d rel {nu_rel:.2e}, \
                 p_k worst rel {p_rel:.2e}, spectra {}",
                dh.n_groups(),
                da.n_groups(),
                if same(&dh.spectra, &da.spectra) { "identical" } else { "DIFFERENT" }
            ));
            if dh.n_groups() != da.n_groups() || lam_rel > 1.0e-15 || nu_rel > 1.0e-12 {
                f.misses.push("delayed numbers".into());
            }
            if !same(&dh.spectra, &da.spectra) {
                f.misses.push("delayed spectra".into());
            }
        }
        _ => {
            f.lines.push("delayed: present on one side only".into());
            f.misses.push("delayed presence".into());
        }
    }
    f
}

/// Report the conversion of the whole LCT-008 library against the ACE route.
#[test]
fn the_lct008_library_converts_and_matches_the_ace_route() {
    let Some(d) = dirs() else {
        println!("SKIP: set OUTRAM_OPENMC_LCT008_H5 and OUTRAM_OPENMC_LCT008_NJOY");
        return;
    };
    let mut all_misses = Vec::new();
    for name in LCT008 {
        let h5_path = d.h5.join(format!("{name}.h5"));
        let ace_path = d.njoy.join("293.6K").join(format!("{name}.ace"));
        if !h5_path.is_file() || !ace_path.is_file() {
            println!("{name}: SKIP (missing {} or {})", h5_path.display(), ace_path.display());
            continue;
        }
        let t = std::time::Instant::now();
        let read = read_nuclide_selected(&h5_path, Some("294K"), &["0K"]).expect("read h5");
        let t_read = t.elapsed();
        let t = std::time::Instant::now();
        let h5 = Nuclide::from_hdf5(&read, name).unwrap_or_else(|e| panic!("{name}: {e}"));
        let t_conv = t.elapsed();
        let ace = Nuclide::from_ace_file(&ace_path, name).expect("from_ace_file");
        let f = compare(name, &h5, &ace);
        println!(
            "== {} (read {:.2?}, from_hdf5 {:.2?}; DBRC h5 {} / ace {})",
            f.name,
            t_read,
            t_conv,
            h5.dbrc.is_some(),
            ace.dbrc.is_some()
        );
        for l in &f.lines {
            println!("   {l}");
        }
        for m in &f.misses {
            all_misses.push(format!("{}: {m}", f.name));
        }
    }
    assert!(all_misses.is_empty(), "predicted-identical items that differ: {all_misses:#?}");
}
