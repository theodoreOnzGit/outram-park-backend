//! The physics and readers behind the "A fragment's story" chapter of the
//! Fission products lessons (maintainer design, 2026-10-20): recoil
//! kinematics, Q-values from mass excesses, the NuDat mass table, the U-235
//! MF=1/MT=458 fragment energy, and the main path of a decay chain read from
//! boon-lay's decay library. Shared by `main.rs` (which writes the page data)
//! and `tests/lesson_fission_cascades.rs` (which reaches every function here)
//! through `#[path]`.
//!
//! Nothing here is tuned, and no number is typed in from memory. Inputs:
//!
//! - **Masses:** the NNDC NuDat mass-excess export that fission-yields-data
//!   0.1.5 ships beside its source (`nndc_nudat_z_n_and_mass_excess.csv`).
//!   It is rounded to three significant figures there, so every Q-value here
//!   carries an interval from that rounding ([`rounding_half_unit_kev`]).
//!   The reader is a port of `mass_excess_table` and `fission_yields_data_src`
//!   in `crates/outram-mc-libs/examples/lesson_fission_products/data.rs`
//!   (lessons 1-3's generator), which this crate cannot include by path: that
//!   file also imports `fission-yields-data`, which dhoby-ghaut does not
//!   depend on.
//! - **Constants:** CODATA 2018 (Tiesinga et al., 2021): the atomic mass unit
//!   and the electron mass in keV, the same source as `lesson-kit.js`.
//! - **Fragment kinetic energy:** U-235 MF=1/MT=458 `EFR` from the
//!   ENDF/B-VIII.0 tape in `reference-data/endf/`, read from the text (the
//!   first data line of the section, ENDF-102 order EFR, dEFR, ENP, ...).
//! - **Half-lives and decay modes:** boon-lay's `DecayLibrary`
//!   (ENDF/B-VIII.0 depletion chain, `openmc-endf-8-depletion-lib-*`).
#![allow(dead_code)]

use petir::mathf::RealMath;

use std::collections::BTreeMap;
use std::path::PathBuf;

/// Atomic mass unit [keV], CODATA 2018.
pub const U_KEV: f64 = 931_494.102_42;
/// Electron rest energy [keV], CODATA 2018.
pub const ME_KEV: f64 = 510.998_950_00;
/// Speed of light [m/s], exact.
pub const C_M_S: f64 = 299_792_458.0;

/// The fission-yields-data 0.1.5 source directory, which holds the NuDat
/// exports. `FISSION_YIELDS_DATA_SRC` overrides; otherwise the cargo registry
/// is searched. Port of lesson 1-3's `data.rs::fission_yields_data_src`.
pub fn fission_yields_data_src() -> Result<PathBuf, String> {
    if let Ok(p) = std::env::var("FISSION_YIELDS_DATA_SRC") {
        return Ok(PathBuf::from(p));
    }
    let home = std::env::var("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|_| std::env::var("HOME").map(|h| PathBuf::from(h).join(".cargo")))
        .map_err(|_| "no CARGO_HOME or HOME")?;
    let reg = home.join("registry").join("src");
    for entry in std::fs::read_dir(&reg).map_err(|e| format!("{}: {e}", reg.display()))? {
        let dir = entry.map_err(|e| e.to_string())?.path().join("fission-yields-data-0.1.5").join("src");
        if dir.join("nndc_nudat_z_n_and_mass_excess.csv").is_file() {
            return Ok(dir);
        }
    }
    Err(format!("fission-yields-data-0.1.5 not found under {}", reg.display()))
}

/// NuDat mass excesses [keV] by (Z, A), ground state (the lower of two rows
/// for one (Z, A)). Port of lesson 1-3's `data.rs::mass_excess_table`.
pub fn mass_excess_table(csv: &str) -> BTreeMap<(u32, u32), f64> {
    let mut m: BTreeMap<(u32, u32), f64> = BTreeMap::new();
    for line in csv.lines().skip(1) {
        let f: Vec<&str> = line.trim_start_matches('\u{feff}').split(',').collect();
        if f.len() < 4 {
            continue;
        }
        let (Ok(z), Ok(n), Ok(d)) = (f[1].trim().parse::<u32>(), f[2].trim().parse::<u32>(), f[3].trim().parse::<f64>()) else {
            continue;
        };
        let e = m.entry((z, z + n)).or_insert(d);
        if d < *e {
            *e = d;
        }
    }
    m
}

/// Half the last kept unit of a value rounded to three significant figures
/// [keV]: the most the NuDat rounding can have moved it. 0 for an exact zero.
pub fn rounding_half_unit_kev(d: f64) -> f64 {
    if d == 0.0 {
        return 0.0;
    }
    let mag = d.abs().r_log10().floor();
    0.5 * 10f64.r_powf(mag - 2.0)
}

/// A value with the interval the mass rounding allows: (central, low, high).
pub type Span = (f64, f64, f64);

/// `sum(sign_i * delta_i)` over mass excesses [keV], with the worst-case
/// interval from each term's rounding.
pub fn q_from_excesses(terms: &[(f64, f64)]) -> Span {
    let mut q = 0.0;
    let mut w = 0.0;
    for &(sign, d) in terms {
        q += sign * d;
        w += rounding_half_unit_kev(d);
    }
    (q, q - w, q + w)
}

/// Atomic mass [keV] of (Z, A) from its mass excess.
pub fn atomic_mass_kev(a: u32, excess_kev: f64) -> f64 {
    a as f64 * U_KEV + excess_kev
}

/// Momentum times c [keV] of a particle of rest energy `m_kev` and kinetic
/// energy `t_kev` (relativistic).
pub fn pc_kev(t_kev: f64, m_kev: f64) -> f64 {
    (t_kev * t_kev + 2.0 * t_kev * m_kev).sqrt()
}

/// Kinetic energy [keV] of a nucleus of rest energy `m_kev` recoiling with
/// momentum `pc_kev` (non-relativistic: the recoil is slow).
pub fn recoil_from_pc_kev(pc: f64, m_kev: f64) -> f64 {
    pc * pc / (2.0 * m_kev)
}

/// The largest recoil [keV] a beta-minus decay of energy release `q_kev` can
/// give its daughter (rest energy `m_daughter_kev`): the electron leaves with
/// all of Q, which maximises the momentum the daughter must balance (the
/// electron's momentum grows faster than Q/c, so sharing with the neutrino
/// only lowers it).
pub fn beta_recoil_max_kev(q_kev: f64, m_daughter_kev: f64) -> f64 {
    recoil_from_pc_kev(pc_kev(q_kev.max(0.0), ME_KEV), m_daughter_kev)
}

/// Recoil [keV] of a nucleus (rest energy `m_kev`) that emits a gamma ray of
/// energy `e_gamma_kev`.
pub fn gamma_recoil_kev(e_gamma_kev: f64, m_kev: f64) -> f64 {
    recoil_from_pc_kev(e_gamma_kev, m_kev)
}

/// Two-body break-up at rest, non-relativistic: of an energy release `q_kev`
/// shared between a light particle (mass `m_light`) and the recoiling
/// nucleus (`m_heavy`), the nucleus takes `q * m_light / (m_light + m_heavy)`.
/// Used for alpha decay, and as the upper bound of the recoil from a
/// beta-delayed neutron (the neutron taking all the energy).
pub fn two_body_recoil_kev(q_kev: f64, m_light: f64, m_heavy: f64) -> f64 {
    q_kev * m_light / (m_light + m_heavy)
}

/// The two fragments' kinetic energies [eV] from the total `efr_ev`, split
/// by momentum conservation (equal and opposite momenta, non-relativistic):
/// the light one takes `EFR * A_H / (A_L + A_H)`. The same split as boon-lay's
/// `examples/rustbca_cascades.rs::fragment_energies_ev`.
pub fn fragment_energies_ev(efr_ev: f64, a_light: f64, a_heavy: f64) -> (f64, f64) {
    let t = a_light + a_heavy;
    (efr_ev * a_heavy / t, efr_ev * a_light / t)
}

/// Speed [m/s] of a particle of kinetic energy `t_kev` and rest energy `m_kev`
/// (relativistic).
pub fn speed_m_s(t_kev: f64, m_kev: f64) -> f64 {
    let g = 1.0 + t_kev / m_kev;
    C_M_S * (1.0 - 1.0 / (g * g)).sqrt()
}

/// What a decay kick does to a fragment still in flight: the largest change
/// of direction [rad] (`dp / p`) and of kinetic energy [keV] (`v dp`, to first
/// order in `dp`) that a momentum kick `dp_kev` (as p·c) can make on a
/// fragment of kinetic energy `t_kev` and rest energy `m_kev`.
pub fn in_flight_kick(t_kev: f64, m_kev: f64, dp_kev: f64) -> (f64, f64) {
    let p = pc_kev(t_kev, m_kev);
    let beta = speed_m_s(t_kev, m_kev) / C_M_S;
    (dp_kev / p, beta * dp_kev)
}

/// Parses one ENDF-6 11-column float (`" 1.691300+8"`, `"-2.5-3"`, or plain).
pub fn endf_float(s: &str) -> Option<f64> {
    let t = s.trim();
    if t.is_empty() {
        return Some(0.0);
    }
    if let Ok(v) = t.parse::<f64>() {
        return Some(v);
    }
    // An exponent sign after the first character, without an 'E'.
    let b = t.as_bytes();
    for i in (1..b.len()).rev() {
        if (b[i] == b'+' || b[i] == b'-') && b[i - 1] != b'e' && b[i - 1] != b'E' {
            let (m, e) = t.split_at(i);
            return Some(m.parse::<f64>().ok()? * 10f64.r_powi(e.parse::<i32>().ok()?));
        }
    }
    None
}

/// MF=1/MT=458's thermal-point `EFR` and `ENP` [eV] of material `mat`, from
/// the text of an ENDF-6 tape: the third line of the section (HEAD, the LIST
/// record's header, then the first data line, EFR, dEFR, ENP, dENP, END,
/// dEND).
pub fn mt458_efr_enp(tape_text: &str, mat: u32) -> Option<(f64, f64)> {
    let tag = format!("{mat:4} 1458");
    let line = tape_text.lines().filter(|l| l.len() >= 75 && &l[66..75] == tag).nth(2)?;
    let f = |k: usize| endf_float(&line[11 * k..11 * (k + 1)]);
    Some((f(0)?, f(2)?))
}

/// "Sr95" -> "Sr-95", "Ag110m" -> "Ag-110m" (the Nuclide enum's spelling).
pub fn pretty_name(raw: &str) -> String {
    match raw.find(|c: char| c.is_ascii_digit()) {
        Some(i) => format!("{}-{}", &raw[..i], &raw[i..]),
        None => raw.to_string(),
    }
}

/// The kind of particles a decay mode sends out, as the page names them.
pub fn mode_label(debug_name: &str) -> &'static str {
    match debug_name {
        "BetaMinus" => "beta-",
        "BetaMinusAndNeutron" => "beta-,n",
        "Alpha" => "alpha",
        "IsomericTransition" => "IT",
        "ElectronCaptureBetaPlus" => "EC/beta+",
        _ => "other",
    }
}
