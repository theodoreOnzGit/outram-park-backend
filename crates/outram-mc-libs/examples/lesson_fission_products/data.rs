//! The data readers behind the Fission products lessons (gh:#906 track,
//! maintainer design 2026-10-20). Shared by `main.rs` (which writes the
//! lesson data files), `spectrum.rs` and `tests/lesson_fission_products.rs` (which reaches
//! every function here), through `#[path]`.
//!
//! Every value comes from a file the workspace already holds or depends on;
//! nothing here is tuned, and nothing is typed in from memory:
//!
//! - **ENDF/B-VIII.0 tapes** in `reference-data/endf/`, read through
//!   `njoy-outram-park-fork`: MF=1/MT=452 (`NuBar`), MT=456 (`NuRepr`),
//!   MT=455 (`DelayedNuBar`), MT=458 (read here with the crate's ENDF record
//!   cursor, in the layout `heatr.f90`'s `hinit` reads it, `:618-776`, ported
//!   as `heatr::driver::hinit::read_mt458`, which is private to HEATR),
//!   MF=2/MT=151 (`reconr::mf2::parse_resonance_info`), MF=3/MT=102's QM,
//!   MF=5/MT=18 (`FissionSpectrum::from_endf_mf5`) and MF=5/MT=455
//!   (`DelayedChi`).
//! - **fission-yields-data 0.1.5**: the ENDF/B-VIII.0 independent fission
//!   yields through `fission_yield_linear_interpolation`, and the NNDC NuDat
//!   tables the crate ships beside its source
//!   (`nndc_nudat_z_n_and_mass_excess.csv`,
//!   `half_life_branching_ratio_nndc_nudat_data_export.csv`), found through
//!   `cargo metadata` because the crate has no API for them.
#![allow(dead_code)]

use fission_yields_data::prelude::{fission_yield_linear_interpolation, Nuclide};
use njoy_outram_park_fork::endf::records::SectionCursor;
use njoy_outram_park_fork::endf::tape::Tape;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::str::FromStr;
use uom::si::energy::electronvolt;
use uom::si::f64::Energy;
use uom::si::ratio::ratio;

/// Element symbols by Z (index 0 is the neutron's placeholder), as the
/// fission-yields-data `Nuclide` enum spells them (`Kr92`, `Ag110m`).
pub const SYMBOLS: [&str; 103] = [
    "n", "H", "He", "Li", "Be", "B", "C", "N", "O", "F", "Ne", "Na", "Mg", "Al", "Si", "P", "S", "Cl", "Ar", "K", "Ca",
    "Sc", "Ti", "V", "Cr", "Mn", "Fe", "Co", "Ni", "Cu", "Zn", "Ga", "Ge", "As", "Se", "Br", "Kr", "Rb", "Sr", "Y",
    "Zr", "Nb", "Mo", "Tc", "Ru", "Rh", "Pd", "Ag", "Cd", "In", "Sn", "Sb", "Te", "I", "Xe", "Cs", "Ba", "La", "Ce",
    "Pr", "Nd", "Pm", "Sm", "Eu", "Gd", "Tb", "Dy", "Ho", "Er", "Tm", "Yb", "Lu", "Hf", "Ta", "W", "Re", "Os", "Ir",
    "Pt", "Au", "Hg", "Tl", "Pb", "Bi", "Po", "At", "Rn", "Fr", "Ra", "Ac", "Th", "Pa", "U", "Np", "Pu", "Am", "Cm",
    "Bk", "Cf", "Es", "Fm", "Md", "No",
];

/// The parents the lessons offer: (label, Z, A, `Nuclide`, ENDF tape or
/// `None` when `reference-data/endf/` holds no ENDF/B-VIII.0 tape for it).
pub fn parents() -> Vec<(&'static str, u32, u32, Nuclide, Option<&'static str>)> {
    vec![
        ("U-235", 92, 235, Nuclide::U235, Some("n-092_U_235-ENDF8.0.endf")),
        ("U-238", 92, 238, Nuclide::U238, Some("n-092_U_238.endf")),
        ("U-233", 92, 233, Nuclide::U233, None),
        ("Pu-239", 94, 239, Nuclide::Pu239, None),
    ]
}

/// The incident energies [eV] at which fission-yields-data tabulates a
/// parent's yields: read from the crate's interpolation routine for each
/// parent (`u235/mod.rs` etc.: 0.0253 eV, 500 keV, 14 MeV; Pu-239 also
/// 2 MeV). A table whose yields sum to zero is reported by
/// [`yield_sum`] and left out by the caller.
pub fn tabulated_energies(parent: Nuclide) -> Vec<f64> {
    match parent {
        Nuclide::Pu239 => vec![0.0253, 5.0e5, 2.0e6, 1.4e7],
        _ => vec![0.0253, 5.0e5, 1.4e7],
    }
}

/// Every product nuclide name the yield enum knows for Z in 20..=72 and
/// A in 60..=180, with its (Z, A): the ground state and the isomers `m`,
/// `m1`, `m2` the enum spells. The enum has no iterator, so names are tried
/// with `FromStr`; `get_z_a` confirms each.
pub fn product_nuclides() -> Vec<(Nuclide, u32, u32)> {
    let mut out = Vec::new();
    for z in 20..=72u32 {
        for a in 60..=180u32 {
            for suffix in ["", "m", "m1", "m2"] {
                let name = format!("{}{a}{suffix}", SYMBOLS[z as usize]);
                if let Ok(n) = Nuclide::from_str(&name) {
                    let (zz, aa) = n.get_z_a();
                    if zz == z && aa == a {
                        out.push((n, z, a));
                    }
                }
            }
        }
    }
    out
}

/// Independent yields of `parent` at incident energy `e_ev` [eV], summed
/// over isomeric states, keyed by (Z, A). Zeros are dropped.
pub fn independent_yields(parent: Nuclide, e_ev: f64, products: &[(Nuclide, u32, u32)]) -> BTreeMap<(u32, u32), f64> {
    let e = Energy::new::<electronvolt>(e_ev);
    let mut m = BTreeMap::new();
    for &(n, z, a) in products {
        let y = fission_yield_linear_interpolation(parent, n, e).get::<ratio>();
        if y > 0.0 {
            *m.entry((z, a)).or_insert(0.0) += y;
        }
    }
    m
}

/// Σ of the yields: 2 per fission for a complete binary table.
pub fn yield_sum(y: &BTreeMap<(u32, u32), f64>) -> f64 {
    y.values().sum()
}

/// The mass distribution Y(A) = Σ_Z Y(Z, A).
pub fn mass_yields(y: &BTreeMap<(u32, u32), f64>) -> BTreeMap<u32, f64> {
    let mut m = BTreeMap::new();
    for (&(_, a), &v) in y {
        *m.entry(a).or_insert(0.0) += v;
    }
    m
}

/// Peak-to-valley numbers of a mass distribution: the highest Y(A) on the
/// light side (A < A_sym) and on the heavy side, the lowest Y(A) between the
/// two peaks (the valley), and the ratio of the mean of the two peaks to it.
/// `a_sym` is half the compound nucleus's mass number, (A_target + 1) / 2.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PeakValley {
    pub light_peak: (u32, f64),
    pub heavy_peak: (u32, f64),
    pub valley: (u32, f64),
    pub ratio: f64,
}

pub fn peak_valley(m: &BTreeMap<u32, f64>, a_sym: f64) -> Option<PeakValley> {
    let light = m.iter().filter(|(&a, _)| (a as f64) < a_sym).max_by(|x, y| x.1.total_cmp(y.1))?;
    let heavy = m.iter().filter(|(&a, _)| (a as f64) > a_sym).max_by(|x, y| x.1.total_cmp(y.1))?;
    let valley = m.range(*light.0..=*heavy.0).min_by(|x, y| x.1.total_cmp(y.1))?;
    let pv = 0.5 * (light.1 + heavy.1) / valley.1;
    Some(PeakValley { light_peak: (*light.0, *light.1), heavy_peak: (*heavy.0, *heavy.1), valley: (*valley.0, *valley.1), ratio: pv })
}

/// The `src/` directory of the fission-yields-data 0.1.5 package Cargo
/// unpacked (the CSV tables ship in it and the crate exposes no API for
/// them): `$FISSION_YIELDS_DATA_SRC` if set, else the one under
/// `$CARGO_HOME/registry/src/*/` (`~/.cargo` by default). The version is
/// the workspace's pin (`Cargo.toml`, `fission-yields-data = "0.1.5"`).
pub fn fission_yields_data_src() -> Result<PathBuf, String> {
    if let Ok(p) = std::env::var("FISSION_YIELDS_DATA_SRC") {
        return Ok(PathBuf::from(p));
    }
    let home = std::env::var("CARGO_HOME").map(PathBuf::from).or_else(|_| std::env::var("HOME").map(|h| PathBuf::from(h).join(".cargo"))).map_err(|_| "no CARGO_HOME or HOME")?;
    let reg = home.join("registry").join("src");
    for entry in std::fs::read_dir(&reg).map_err(|e| format!("{}: {e}", reg.display()))? {
        let dir = entry.map_err(|e| e.to_string())?.path().join("fission-yields-data-0.1.5").join("src");
        if dir.join("nndc_nudat_z_n_and_mass_excess.csv").is_file() {
            return Ok(dir);
        }
    }
    Err(format!("fission-yields-data-0.1.5 not found under {}", reg.display()))
}

/// The NNDC mass excesses [keV] by (Z, A), from
/// `nndc_nudat_z_n_and_mass_excess.csv` (columns name, z, n, massExcess).
/// Where a (Z, A) appears twice (an isomer row, which carries its
/// excitation), the lower value, the ground state, is kept.
pub fn mass_excess_table(csv: &str) -> BTreeMap<(u32, u32), f64> {
    let mut m: BTreeMap<(u32, u32), f64> = BTreeMap::new();
    for line in csv.lines().skip(1) {
        let f: Vec<&str> = line.trim_start_matches('\u{feff}').split(',').collect();
        if f.len() < 4 {
            continue;
        }
        let (Ok(z), Ok(n), Ok(d)) = (f[1].trim().parse::<u32>(), f[2].trim().parse::<u32>(), f[3].trim().parse::<f64>()) else { continue };
        let e = m.entry((z, z + n)).or_insert(d);
        if d < *e {
            *e = d;
        }
    }
    m
}

/// One ground state's decay from the NuDat export: half-life [s] (`None`
/// when the file gives none), the branching ratios [%] of beta-minus (`B-`)
/// and beta-delayed neutron emission (`B-N`), and every mode listed.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Decay {
    pub half_life_s: Option<f64>,
    pub beta_minus: f64,
    pub beta_n: f64,
    pub modes: Vec<String>,
}

/// Seconds per unit of the NuDat `halflifeUnit` column. Years are the Julian
/// year of 365.25 d; widths in energy units (MeV, keV, eV) give `None`
/// (those levels are resonances, not decays one could watch).
pub fn unit_seconds(u: &str) -> Option<f64> {
    Some(match u {
        "fs" => 1e-15,
        "ps" => 1e-12,
        "ns" => 1e-9,
        "us" => 1e-6,
        "ms" => 1e-3,
        "s" => 1.0,
        "m" => 60.0,
        "h" => 3600.0,
        "d" => 86400.0,
        "y" => 365.25 * 86400.0,
        _ => return None,
    })
}

/// Ground-state decays by (Z, A) from
/// `half_life_branching_ratio_nndc_nudat_data_export.csv`
/// (z, n, name, levelEnergy, decayMode, branchingRatio, ..., halflife,
/// halflifeUnit). A nuclide absent from the file is stable in it.
pub fn decay_table(csv: &str) -> BTreeMap<(u32, u32), Decay> {
    let mut m: BTreeMap<(u32, u32), Decay> = BTreeMap::new();
    for line in csv.lines().skip(1) {
        let f: Vec<&str> = line.split(',').collect();
        if f.len() < 11 {
            continue;
        }
        let (Ok(z), Ok(n)) = (f[0].parse::<u32>(), f[1].parse::<u32>()) else { continue };
        if f[3].parse::<f64>().map_or(true, |l| l != 0.0) {
            continue;
        }
        let d = m.entry((z, z + n)).or_default();
        let br = f[5].parse::<f64>().unwrap_or(0.0);
        match f[4] {
            "B-" => d.beta_minus = br,
            "B-N" => d.beta_n = br,
            _ => {}
        }
        d.modes.push(f[4].to_string());
        if d.half_life_s.is_none() {
            if let (Ok(t), Some(s)) = (f[9].parse::<f64>(), unit_seconds(f[10])) {
                d.half_life_s = Some(t * s);
            }
        }
    }
    m
}

/// MF=1/MT=458, the fission energy release: the thermal-point components
/// (ENDF-102 order EFR, ENP, END, EGP, EB, EGD, ENU, ER, ET, each with its
/// uncertainty) and, for `LFC = 1`, the tabulated components by IFC
/// (1 EFR … 9 ET), as `hinit` reads them (`heatr.f90:618-776`).
#[derive(Debug, Clone, Default)]
pub struct EnergyRelease {
    pub lfc: i32,
    pub nply: i32,
    pub thermal: [(f64, f64); 9],
    pub tabulated: BTreeMap<i32, Vec<(f64, f64)>>,
}

pub const MT458_NAMES: [&str; 9] = ["EFR", "ENP", "END", "EGP", "EB", "EGD", "ENU", "ER", "ET"];

pub fn read_mt458(tape: &Tape, mat: i32) -> Result<EnergyRelease, String> {
    let sec = tape.section(mat, 1, 458).ok_or("no MF=1/MT=458")?;
    let mut cur = SectionCursor::new(&sec.rows);
    let head = cur.read_cont().map_err(|e| e.to_string())?;
    let list = cur.read_list().map_err(|e| e.to_string())?;
    let mut out = EnergyRelease { lfc: head.l2, nply: list.head.l2, ..Default::default() };
    for k in 0..9 {
        out.thermal[k] = (list.data.get(2 * k).copied().unwrap_or(0.0), list.data.get(2 * k + 1).copied().unwrap_or(0.0));
    }
    if head.l2 == 1 {
        for _ in 0..head.n2.max(0) {
            let t = cur.read_tab1().map_err(|e| e.to_string())?;
            out.tabulated.insert(t.head.l2, t.pairs.clone());
        }
    }
    Ok(out)
}

impl EnergyRelease {
    /// Component `ifc` (1-based, ENDF-102 order) at incident energy `e` [eV]:
    /// the tabulated value (lin-lin, clamped) when `LFC = 1` gives that
    /// component, else the thermal point.
    pub fn at(&self, ifc: i32, e: f64) -> f64 {
        if let Some(t) = self.tabulated.get(&ifc) {
            return linlin(t, e);
        }
        self.thermal[(ifc - 1) as usize].0
    }
}

/// Lin-lin interpolation on sorted (x, y) pairs, clamped at the ends.
pub fn linlin(t: &[(f64, f64)], x: f64) -> f64 {
    if t.is_empty() {
        return 0.0;
    }
    if x <= t[0].0 {
        return t[0].1;
    }
    if x >= t[t.len() - 1].0 {
        return t[t.len() - 1].1;
    }
    let k = t.partition_point(|p| p.0 <= x).clamp(1, t.len() - 1);
    let ((x0, y0), (x1, y1)) = (t[k - 1], t[k]);
    if x1 == x0 {
        return y1;
    }
    y0 + (y1 - y0) * (x - x0) / (x1 - x0)
}

/// The evaluation's MF=3/MT=102 QM [eV]: the capture Q, i.e. the neutron
/// separation energy of the compound nucleus.
pub fn capture_q(tape: &Tape, mat: i32) -> Option<f64> {
    let s = tape.section(mat, 3, 102)?;
    s.rows.get(1).map(|r| r[0])
}

/// The resolved resonances' total widths: (E_r, Γ_n, Γ_γ, Γ_f, Γ) [eV] for
/// every Reich-Moore (LRF=3) resonance, Γ = |Γ_n| + Γ_γ + |Γ_fA| + |Γ_fB|,
/// plus the resolved range [EL, EH] and AWRI, from MF=2/MT=151 parsed by
/// RECONR's own reader.
pub struct Resonances {
    pub el: f64,
    pub eh: f64,
    pub awri: f64,
    pub list: Vec<(f64, f64, f64, f64, f64)>,
}

pub fn resonances(tape: &Tape, mat: i32) -> Result<Resonances, String> {
    let sec = tape.section(mat, 2, 151).ok_or("no MF=2")?;
    let info = njoy_outram_park_fork::reconr::mf2::parse_resonance_info(sec).map_err(|e| e.to_string())?;
    let r = info.resolved_rm_ranges().next().ok_or("no Reich-Moore range")?;
    let mut list = Vec::new();
    let mut awri = 0.0;
    for ls in &r.rm_l_states {
        awri = ls.awri;
        for x in &ls.resonances {
            let gf = x.gfa.abs() + x.gfb.abs();
            list.push((x.er, x.gn.abs(), x.gg, gf, x.gn.abs() + x.gg + gf));
        }
    }
    list.sort_by(|a, b| a.0.total_cmp(&b.0));
    Ok(Resonances { el: r.el, eh: r.eh, awri, list })
}

/// The ENDF-102 channel radius a = 0.123 AWRI^(1/3) + 0.08 [10^-12 cm],
/// as this workspace's RECONR computes it for NAPS = 0
/// (`reconr/slbw.rs`, `RC1`, `RC2`), returned in fm.
pub fn channel_radius_fm(awri: f64) -> f64 {
    (0.123 * awri.powf(1.0 / 3.0) + 0.08) * 10.0
}

/// Thin a density on its own grid for drawing: keep every `step`-th point
/// and the last.
pub fn thin(e: &[f64], p: &[f64], step: usize) -> (Vec<f64>, Vec<f64>) {
    let mut x = Vec::new();
    let mut y = Vec::new();
    for i in (0..e.len()).step_by(step.max(1)) {
        x.push(e[i]);
        y.push(p[i]);
    }
    if e.len() > 1 && (e.len() - 1) % step.max(1) != 0 {
        x.push(e[e.len() - 1]);
        y.push(p[e.len() - 1]);
    }
    (x, y)
}

/// Mean of a tabulated density: ∫ E p dE / ∫ p dE, exact on each lin-lin
/// (`linlin = true`) or histogram panel.
pub fn mean_energy(e: &[f64], p: &[f64], linlin: bool) -> f64 {
    let (mut m0, mut m1) = (0.0, 0.0);
    for k in 1..e.len() {
        let (a, b) = (e[k - 1], e[k]);
        let w = b - a;
        if linlin {
            let (pa, pb) = (p[k - 1], p[k]);
            m0 += 0.5 * (pa + pb) * w;
            // ∫ x (pa + (pb - pa)(x - a)/w) dx over [a, b], exactly
            m1 += pa * (b * b - a * a) / 2.0 + (pb - pa) / w * ((b * b * b - a * a * a) / 3.0 - a * (b * b - a * a) / 2.0);
        } else {
            m0 += p[k - 1] * w;
            m1 += p[k - 1] * (b * b - a * a) / 2.0;
        }
    }
    m1 / m0
}
