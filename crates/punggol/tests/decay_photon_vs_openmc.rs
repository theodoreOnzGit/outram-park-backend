//! **Decay-photon spectra against OpenMC, code to code** (gh:#1391).
//!
//! # Methodology
//!
//! `verification_and_validation/decay_photon_spectra/openmc_inputs/decay_photon_reference.py`
//! runs OpenMC `8f79d9d02483c90e4134f03a9df4b75ce6b062bf` (Python API; numpy
//! 2.4.6, Python 3.11) on two of OpenMC's own test chains, committed beside it:
//!
//! - `chain_simple_decay.xml`: discrete photon lines (I135, U235, U238) and a
//!   histogram continuum (Xe135), so every `combine_distributions` branch
//!   (discrete merge, Tabular + Discrete mixture, a lone Tabular, a lone
//!   Discrete) is reached;
//! - `chain_ni.xml`: 13 activation products with discrete photon lines.
//!
//! and writes every number to `openmc_outputs/*.txt` (floats as Python `repr`,
//! which round-trips). This test reads the **same chain XML** through
//! `outram_mc_libs::depletion::DepletionChainXml`, repeats each call with the
//! same inputs (the reference's own atom densities and mass density, so
//! `Material`'s density bookkeeping is not part of the comparison), and
//! compares:
//!
//! 1. `openmc.data.decay_photon_energy(nuclide)`: kind, `x`, `p`, integral, mean;
//! 2. `Material.get_decay_photon_energy` for the case table below: kind (and
//!    mixture structure), `x`, `p`, probabilities, integral, mean and the
//!    `Mixture.clip` warning value;
//! 3. `openmc.stats.DecaySpectrum`: `integral()`, `to_distribution()`, `mean()`
//!    and `clip(1e-3)`, `clip(1e-9)`.
//!
//! Pass criterion: **bit identity** is the goal; the gate is a relative
//! difference of at most 1e-9 (workspace round-off allowance, root
//! `CLAUDE.md`), and every element that is not bit-identical is counted and
//! its worst case printed. Structure (kinds, lengths, kept indices) must match
//! exactly.
//!
//! The C++ `DecaySpectrum` (`src/distribution.cpp:778`) is **not run**: no
//! OpenMC binary of this commit or photon library is available here. Its
//! emission rate is cross-checked against Python's `DecaySpectrum.integral()`
//! (a different summation route over the same quantity, so a round-off
//! tolerance, not a bit-identity claim), and its sampler by a statistical test
//! in the unit tests.
//!
//! # Results
//!
//! See `verification_and_validation/decay_photon_spectra/README.md`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use outram_mc_libs::depletion::DepletionChainXml;
use punggol::decay_spectrum::{
    decay_photon_energy, get_decay_photon_energy, DecayPhotonOptions, DecayPhotonUnits, DecaySpectrum,
    PhotonSpectrum, ResolvedDecaySpectrum, SpectrumComponent,
};
use outram_mc_libs::source::distribution::TabularInterp;

const GATE: f64 = 1e-9;

fn vv_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("verification_and_validation/decay_photon_spectra")
}

/// Reference lines: tag, then the rest.
struct Reference(HashMap<String, Vec<String>>);

impl Reference {
    fn load(name: &str) -> Self {
        let text = std::fs::read_to_string(vv_dir().join("openmc_outputs").join(name)).unwrap();
        let mut m = HashMap::new();
        for line in text.lines() {
            let mut it = line.split(' ');
            let tag = it.next().unwrap().to_string();
            m.insert(tag, it.map(str::to_string).collect());
        }
        Reference(m)
    }

    fn get(&self, tag: &str) -> &[String] {
        self.0.get(tag).unwrap_or_else(|| panic!("no reference line {tag}"))
    }

    fn floats(&self, tag: &str) -> Vec<f64> {
        self.get(tag).iter().map(|s| s.parse::<f64>().unwrap()).collect()
    }

    fn float(&self, tag: &str) -> f64 {
        self.floats(tag)[0]
    }

    fn strings(&self, tag: &str) -> Vec<String> {
        self.get(tag).to_vec()
    }
}

#[derive(Default)]
struct Acc {
    exact: usize,
    total: usize,
    worst: f64,
    worst_label: String,
}

impl Acc {
    fn one(&mut self, label: &str, ours: f64, theirs: f64) {
        self.total += 1;
        if ours.to_bits() == theirs.to_bits() {
            self.exact += 1;
            return;
        }
        let rel = if theirs == 0.0 { ours.abs() } else { ((ours - theirs) / theirs).abs() };
        if rel > self.worst {
            self.worst = rel;
            self.worst_label = label.to_string();
        }
        assert!(rel <= GATE, "{label}: {ours:e} vs OpenMC {theirs:e} (rel {rel:e})");
    }

    fn many(&mut self, label: &str, ours: &[f64], theirs: &[f64]) {
        assert_eq!(ours.len(), theirs.len(), "{label}: length");
        for (i, (a, b)) in ours.iter().zip(theirs).enumerate() {
            self.one(&format!("{label}[{i}]"), *a, *b);
        }
    }
}

fn interp_name(i: TabularInterp) -> &'static str {
    match i {
        TabularInterp::Histogram => "histogram",
        TabularInterp::LinLin => "linear-linear",
        TabularInterp::LogLin => "log-linear",
        TabularInterp::LogLog => "log-log",
    }
}

fn check_component(acc: &mut Acc, tag: &str, c: &SpectrumComponent, r: &Reference) {
    let kind = &r.get(tag)[0];
    match c {
        SpectrumComponent::Discrete(d) => {
            assert_eq!(kind, "D", "{tag}");
            acc.many(&format!("{tag}.x"), d.x(), &r.floats(&format!("{tag}.x")));
            acc.many(&format!("{tag}.p"), d.p(), &r.floats(&format!("{tag}.p")));
            acc.one(&format!("{tag}.integral"), d.integral(), r.float(&format!("{tag}.integral")));
            acc.one(&format!("{tag}.mean"), d.mean(), r.float(&format!("{tag}.mean")));
        }
        SpectrumComponent::Tabular(t) => {
            assert_eq!(kind, "T", "{tag}");
            assert_eq!(r.get(tag)[1], interp_name(t.interpolation()), "{tag}");
            acc.many(&format!("{tag}.x"), t.x(), &r.floats(&format!("{tag}.x")));
            acc.many(&format!("{tag}.p"), t.p(), &r.floats(&format!("{tag}.p")));
            acc.one(&format!("{tag}.integral"), t.integral().unwrap(), r.float(&format!("{tag}.integral")));
            acc.one(&format!("{tag}.mean"), t.mean().unwrap(), r.float(&format!("{tag}.mean")));
        }
    }
}

fn check_spectrum(acc: &mut Acc, tag: &str, s: &PhotonSpectrum, r: &Reference) {
    match s {
        PhotonSpectrum::Discrete(d) => check_component(acc, tag, &SpectrumComponent::Discrete(d.clone()), r),
        PhotonSpectrum::Tabular(t) => check_component(acc, tag, &SpectrumComponent::Tabular(t.clone()), r),
        PhotonSpectrum::Mixture(m) => {
            assert_eq!(r.get(tag)[0], "M", "{tag}");
            assert_eq!(r.get(tag)[1].parse::<usize>().unwrap(), m.components().len(), "{tag}");
            acc.many(&format!("{tag}.prob"), m.probability(), &r.floats(&format!("{tag}.prob")));
            for (i, c) in m.components().iter().enumerate() {
                check_component(acc, &format!("{tag}.{i}"), c, r);
            }
            acc.one(&format!("{tag}.integral"), m.integral().unwrap(), r.float(&format!("{tag}.integral")));
            acc.one(&format!("{tag}.mean"), m.mean().unwrap(), r.float(&format!("{tag}.mean")));
        }
    }
}

/// The `get_decay_photon_energy` arguments of each case in the driver.
fn options_of(label: &str, chain_set: &str, mass_density: f64) -> DecayPhotonOptions {
    let base = DecayPhotonOptions::default();
    let names = |v: &[&str]| Some(v.iter().map(|s| s.to_string()).collect::<Vec<_>>());
    match (chain_set, label) {
        ("simple_decay", "a_bq") => DecayPhotonOptions { units: DecayPhotonUnits::Bq, volume_cm3: Some(100.0), ..base },
        ("simple_decay", "b_cm3_clip") => DecayPhotonOptions { units: DecayPhotonUnits::BqPerCm3, clip_tolerance: 1e-3, ..base },
        ("simple_decay", "c_m3_include") => DecayPhotonOptions {
            units: DecayPhotonUnits::BqPerM3,
            include_nuclides: names(&["I135", "Xe135"]),
            clip_tolerance: 0.0,
            ..base
        },
        ("simple_decay", "d_g") => DecayPhotonOptions {
            units: DecayPhotonUnits::BqPerG,
            mass_density_g_cm3: Some(mass_density),
            clip_tolerance: 0.0,
            ..base
        },
        ("simple_decay", "e_kg_exclude") => DecayPhotonOptions {
            units: DecayPhotonUnits::BqPerKg,
            mass_density_g_cm3: Some(mass_density),
            exclude_nuclides: names(&["Xe135"]),
            ..base
        },
        ("simple_decay", "f_tabular_only") => DecayPhotonOptions {
            units: DecayPhotonUnits::BqPerCm3,
            include_nuclides: names(&["Xe135"]),
            ..base
        },
        ("simple_decay", "g_heavy_clip") => DecayPhotonOptions { units: DecayPhotonUnits::BqPerCm3, clip_tolerance: 0.1, ..base },
        ("simple_decay", "h_none") => DecayPhotonOptions {
            units: DecayPhotonUnits::BqPerCm3,
            include_nuclides: names(&["Xe135_m1"]),
            ..base
        },
        ("ni", "a_bq") => DecayPhotonOptions { units: DecayPhotonUnits::Bq, volume_cm3: Some(250.0), ..base },
        ("ni", "b_cm3_clip") => DecayPhotonOptions { units: DecayPhotonUnits::BqPerCm3, clip_tolerance: 1e-2, ..base },
        ("ni", "c_exclude") => DecayPhotonOptions {
            units: DecayPhotonUnits::BqPerM3,
            exclude_nuclides: names(&["Co60"]),
            ..base
        },
        ("ni", "d_include") => DecayPhotonOptions {
            units: DecayPhotonUnits::BqPerCm3,
            include_nuclides: names(&["Co60", "Ni63", "Fe55"]),
            ..base
        },
        ("ni", "e_g") => DecayPhotonOptions {
            units: DecayPhotonUnits::BqPerG,
            mass_density_g_cm3: Some(mass_density),
            clip_tolerance: 1e-9,
            ..base
        },
        _ => panic!("no options for {chain_set}/{label}"),
    }
}

struct Totals {
    nuc: Acc,
    mat: Acc,
    ds: Acc,
    cases: usize,
    kinds: Vec<String>,
}

fn run_set(chain_file: &str, set: &str, mat_labels: &[&str], ds_labels: &[&str]) -> Totals {
    let chain = DepletionChainXml::read_file(&vv_dir().join("openmc_inputs").join(chain_file)).unwrap();
    let r = Reference::load(&format!("{set}.txt"));
    let mut t = Totals { nuc: Acc::default(), mat: Acc::default(), ds: Acc::default(), cases: 0, kinds: vec![] };

    // 1. decay_photon_energy for every nuclide with a photon source
    let mut n_nuc = 0;
    for n in &chain.nuclides {
        let tag = format!("NUC.{}", n.name);
        let ours = decay_photon_energy(&chain, &n.name).unwrap();
        match (ours, r.0.contains_key(&tag)) {
            (Some(s), true) => {
                check_spectrum(&mut t.nuc, &tag, &s, &r);
                n_nuc += 1;
            }
            (None, false) => {}
            (a, b) => panic!("{tag}: ours {} / OpenMC {b}", a.is_some()),
        }
    }
    assert!(n_nuc > 0);

    // 2. Material.get_decay_photon_energy
    for label in mat_labels {
        t.cases += 1;
        let names = r.strings(&format!("{label}.names"));
        let atoms = r.floats(&format!("{label}.atoms"));
        let dens: Vec<(String, f64)> = names.into_iter().zip(atoms).collect();
        let opts = options_of(label, set, r.float(&format!("{label}.massdensity")));
        let ours = get_decay_photon_energy(&chain, &dens, &opts).unwrap();
        let kind = r.get(label)[0].clone();
        t.kinds.push(format!("{set}/{label}:{kind}"));
        match ours {
            None => assert_eq!(kind, "NONE", "{label}"),
            Some(res) => {
                check_spectrum(&mut t.mat, label, &res.spectrum, &r);
                let warn = &r.get(&format!("{label}.warn"))[0];
                match (res.clip_warning, warn.as_str()) {
                    (None, "none") => {}
                    (Some(w), s) if s != "none" => t.mat.one(&format!("{label}.warn"), w, s.parse().unwrap()),
                    (a, b) => panic!("{label}: clip warning {a:?} vs OpenMC {b}"),
                }
            }
        }
    }

    // 3. openmc.stats.DecaySpectrum
    for label in ds_labels {
        t.cases += 1;
        let names = r.strings(&format!("{label}.names"));
        let dens = r.floats(&format!("{label}.dens"));
        let volume = r.float(&format!("{label}.volume"));
        let ds = DecaySpectrum::new(names.into_iter().zip(dens).collect(), volume).unwrap();
        t.ds.one(&format!("{label}.integral"), ds.integral(&chain), r.float(&format!("{label}.integral")));
        match ds.to_spectrum(&chain).unwrap() {
            None => assert_eq!(r.get(label)[0], "NONE"),
            Some(s) => {
                check_spectrum(&mut t.ds, &format!("{label}.dist"), &s, &r);
                t.ds.one(&format!("{label}.mean"), ds.mean(&chain).unwrap(), r.float(&format!("{label}.mean")));
            }
        }
        for (tol, key) in [(1e-3, "0.001"), (1e-9, "1e-09")] {
            let kept: Vec<String> = ds.clip(&chain, tol).unwrap().nuclides().iter().map(|(n, _)| n.clone()).collect();
            let want = r.strings(&format!("{label}.clip{key}"));
            // an empty list is an empty line tail
            let want: Vec<String> = want.into_iter().filter(|s| !s.is_empty()).collect();
            assert_eq!(kept, want, "{label} clip {key}");
        }
        // the C++ route's emission rate against Python's DecaySpectrum.integral()
        let resolved = ResolvedDecaySpectrum::resolve(&chain, &ds).unwrap();
        let py_rate = r.float(&format!("{label}.integral"));
        assert!(
            (resolved.integral() - py_rate).abs() <= 1e-12 * py_rate,
            "{label}: C++ rate {:e} vs Python {:e}",
            resolved.integral(),
            py_rate
        );
    }
    t
}

/// Print the count and **pin bit identity**: measured 2026-11-20, every value
/// of every case is bit-identical to OpenMC's, so a last-bit change here is a
/// change of the port (moving this pin needs the reasons in the root
/// `CLAUDE.md`, "Moving a pin for rounding").
fn report(name: &str, a: &Acc) {
    println!(
        "{name}: {}/{} values bit-identical, worst relative difference {:e} ({})",
        a.exact, a.total, a.worst, a.worst_label
    );
    assert_eq!(a.exact, a.total, "{name}: not bit-identical, worst {}", a.worst_label);
}

#[test]
fn simple_decay_chain_matches_openmc() {
    let t = run_set(
        "chain_simple_decay.xml",
        "simple_decay",
        &["a_bq", "b_cm3_clip", "c_m3_include", "d_g", "e_kg_exclude", "f_tabular_only", "g_heavy_clip", "h_none"],
        &["ds1", "ds2", "ds3"],
    );
    report("simple_decay NUC", &t.nuc);
    report("simple_decay MAT", &t.mat);
    report("simple_decay DS ", &t.ds);
    println!("kinds: {:?}", t.kinds);
    // the case table reaches every combine branch
    for want in ["a_bq:M", "e_kg_exclude:D", "f_tabular_only:T", "g_heavy_clip:D", "h_none:NONE"] {
        assert!(t.kinds.iter().any(|k| k.ends_with(want)), "{want} not reached: {:?}", t.kinds);
    }
    assert_eq!(t.cases, 11);
}

#[test]
fn ni_chain_matches_openmc() {
    let t = run_set(
        "chain_ni.xml",
        "ni",
        &["a_bq", "b_cm3_clip", "c_exclude", "d_include", "e_g"],
        &["ds1", "ds2"],
    );
    report("ni NUC", &t.nuc);
    report("ni MAT", &t.mat);
    report("ni DS ", &t.ds);
    assert_eq!(t.cases, 7);
}
