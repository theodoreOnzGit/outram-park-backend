// Ported from NJOY2016 (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da):
//   - `src/errorr.f90` `egngpn` (the ERRORR ign->grid entry point, l.9716-9807), and
//   - `src/groupr.f90` `gengpn` (the actual boundary-table source, l.1599-4649), plus
//   - `src/util.f90` `sigfig` (l.361-393, the lethargy-grid rounding helper).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! Built-in neutron group structures for the ERRORR module.
//!
//! # What this module computes
//!
//! ERRORR's `egngpn(ign,ngn,egn)` (`errorr.f90:9716-9807`) selects one of NJOY's
//! built-in multigroup neutron energy-boundary structures by the integer index
//! `ign`, filling the group-boundary array `egn` (length `ngn+1`) and returning
//! the group count `ngn`. In upstream Fortran, `egngpn` is a thin wrapper that
//! **delegates the boundary tables to `gengpn`** in the GROUPR module
//! (`groupr.f90:1599-4649`); the union-with-covariance-grid step that `egngpn`
//! adds on top is ERRORR-internal plumbing and is *not* part of the group
//! structure itself. This module ports the boundary-table half faithfully — the
//! part a caller means when they say "give me the SCALE-238 / VITAMIN-J / XMAS
//! grid" — from `gengpn`, which is the single source of truth for the tables.
//!
//! # Energy unit and ordering
//!
//! All boundaries are in **electron-volts (eV)**. The returned `Vec<f64>` has
//! length `ngn + 1` and is given in the **same element order NJOY produces**.
//! For every built-in structure ported here that order is **ascending in
//! energy** (lowest boundary first, highest last) — either because the source
//! table is ascending, because NJOY reverses a descending table into `egn`
//! (e.g. XMAS, `ign=18`), or because a lethargy grid `u` is mapped to energy by
//! `E = E0 * exp(-u)` which inverts the ordering. Callers that need descending
//! order should reverse the result.
//!
//! # `ign` index map (from `groupr.f90:1604-1642`)
//!
//! | `ign` | structure | groups |
//! |------:|-----------|-------:|
//! | ±1 | arbitrary structure, **read from input** (not a table) | — |
//! | 2  | CSEWG (header says 239; code sets ngn=240) | 240 |
//! | 3  | LANL | 30 |
//! | 4  | ANL | 27 |
//! | 5  | RRD | 50 |
//! | 6  | GAM-I | 68 |
//! | 7  | GAM-II | 100 |
//! | 8  | LASER-THERMOS | 35 |
//! | 9  | EPRI-CPM/WIMS | 69 |
//! | 10 | LANL | 187 |
//! | 11 | LANL | 70 |
//! | 12 | SAND-II | 620 |
//! | 13 | LANL | 80 |
//! | 14 | EURLIB | 100 |
//! | 15 | SAND-IIA | 640 |
//! | 16 | VITAMIN-E | 174 |
//! | 17 | VITAMIN-J | 175 |
//! | 18 | XMAS | 172 |
//! | 19 | ECCO | 33 |
//! | 20 | ECCO | 1968 |
//! | 21 | TRIPOLI | 315 |
//! | 22 | XMAS LWPC | 172 |
//! | 23 | VIT-J LWPC | 175 |
//! | 24 | SHEM CEA | 281 |
//! | 25 | SHEM EPM | 295 |
//! | 26 | SHEM CEA/EPM | 361 |
//! | 27 | SHEM EPM | 315 |
//! | 28 | RAHAB AECL | 89 |
//! | 29 | CCFE | 660 |
//! | 30 | UKAEA | 1025 |
//! | 31 | UKAEA | 1067 |
//! | 32 | UKAEA | 1102 |
//! | 33 | UKAEA | 142 |
//! | 34 | LANL | 618 |
//! | 35 | APOLLO | 99 |
//! | 36 | ECCO | 1962 |
//!
//! # Not ported: read-from-input structures
//!
//! `abs(ign) == 1` (i.e. `ign = 1` or `ign = -1`) is the *arbitrary* structure:
//! NJOY reads `ngn` and then the `ngn+1` boundaries from the system input file
//! as a free-format list (`groupr.f90:4156-4165`). There is no hardcoded table
//! to reproduce, so [`neutron_group_structure`] returns
//! [`NjoyError::NotPorted`] for those values rather than fabricating boundaries.
//! Any other unrecognised `ign` (NJOY's "illegal group structure" abort at
//! `groupr.f90:4557-4558`) is reported as an error instead of panicking.
//!
//! # File-size note (for the crate maintainer)
//!
//! The literal boundary tables total ~9800 `f64` values across 39 arrays. To
//! honour the crate's 1000/1500-line file-size cap, those `EG*`/`GL*` `const`
//! tables now live in the sibling [`tables`] module (`groups/tables.rs`); this
//! file keeps the dispatch logic and the [`NeutronGroupStructure`] enum. The
//! split moved the constants verbatim — no values or logic changed.

use crate::NjoyError;

/// Return the neutron group-boundary structure selected by `ign`, in eV.
///
/// This is the faithful port of NJOY2016's `gengpn` boundary tables reached
/// through ERRORR's `egngpn` (`errorr.f90:9716`). The returned vector has
/// length `ngn + 1` (one more than the group count) and is **ascending in
/// energy**. See the module-level documentation for the `ign` map, the energy
/// unit, and ordering.
///
/// # Errors
///
/// - `abs(ign) == 1` selects the read-from-input arbitrary structure, which has
///   no built-in table; returns [`NjoyError::NotPorted`].
/// - Any `ign` outside the built-in set returns an error (NJOY aborts here).
///
/// # Source
///
/// `groupr.f90:1599-4649` (`gengpn`); dispatch mirror of `groupr.f90:4156-4567`.
pub fn neutron_group_structure(ign: i32) -> Result<Vec<f64>, NjoyError> {
    // Scalar parameters, groupr.f90:4133-4150.
    const EZERO: f64 = 1.0e7;
    const EIGHTH: f64 = 0.125;
    const BGAM2: f64 = 27.631021;
    const TGAM2: f64 = -0.53062825;
    const U187A: f64 = -15.5;
    const U187B: f64 = -14.125;
    const U187C: f64 = -5.875;
    const E187D: f64 = 2.6058e4;
    const E187E: f64 = 6.868;
    const SANDA: f64 = 1.0e-4;
    const SANDB: f64 = 1.0e-6;
    const SANDC: f64 = 2.8e-4;
    const SANDD: f64 = 1.0e6;
    const SANDE: f64 = 1.0e5;
    const UU80: f64 = 0.6931472;
    const E175: f64 = 1.284e7;

    // Build `egn` and a `lflag` marking lethargy grids that need the final
    // `E = E0*exp(-u)` conversion (groupr.f90:4562-4567). `egn` is filled with
    // 1-based Fortran indices translated to 0-based `[i - 1]` throughout, so the
    // index arithmetic stays line-traceable to the Fortran.
    let (mut egn, lflag): (Vec<f64>, bool) = match ign {
        // abs(ign)==1: arbitrary structure read from input (groupr.f90:4156-4165).
        1 | -1 => {
            return Err(NjoyError::NotPorted(
                "errorr::egngpn::ign1 (arbitrary group structure read from input file)",
            ));
        }

        // CSEWG 239 group structure (groupr.f90:4168-4175). Lethargy grid.
        2 => (GL2.to_vec(), true),

        // LANL 30 group structure (groupr.f90:4178-4184). Direct energies.
        3 => (EG3.to_vec(), false),

        // ANL 27 group structure (groupr.f90:4187-4194). Lethargy grid.
        4 => (GL4.to_vec(), true),

        // RRD 50 group structure (groupr.f90:4197-4204). Lethargy grid.
        5 => (GL5.to_vec(), true),

        // GAM-I 68 group structure (groupr.f90:4207-4217). Lethargy grid.
        6 => {
            let ngp = 69usize;
            let mut egn = vec![0.0f64; ngp];
            let mut u = -0.25f64;
            let du = 0.25f64;
            for ig in 1..=ngp {
                u += du;
                egn[(70 - ig) - 1] = u;
            }
            (egn, true)
        }

        // GAM-II 100 group structure (groupr.f90:4220-4234). Lethargy grid.
        7 => {
            let ngp = 101usize;
            let mut egn = vec![0.0f64; ngp];
            let mut u = -0.4f64;
            let mut du = 0.1f64;
            for ig in 1..=99usize {
                u += du;
                egn[(101 - ig) - 1] = u;
                if ig == 49 {
                    du = 0.25f64;
                }
            }
            egn[0] = BGAM2; // upper edge, groupr.f90:4231
            egn[100] = TGAM2; // upper limit changed to 17 MeV, groupr.f90:4233
            (egn, true)
        }

        // LASER-THERMOS 35 group structure (groupr.f90:4237-4243). Direct energies.
        8 => (EG6.to_vec(), false),

        // EPRI-CPM/WIMS 69 group structure (groupr.f90:4246-4252). Direct energies.
        9 => (EG9.to_vec(), false),

        // LANL 187 group structure (groupr.f90:4255-4281). Direct energies.
        10 => {
            let ngp = 188usize;
            let mut egn = vec![0.0f64; ngp];
            for ig in 1..=48usize {
                egn[ig - 1] = EG10A[ig - 1];
            }
            let mut u = U187A;
            for ig in 49..=59usize {
                egn[ig - 1] = EZERO * u.exp();
                u += EIGHTH;
            }
            egn[60 - 1] = E187E;
            u = U187B;
            for ig in 61..=126usize {
                egn[ig - 1] = EZERO * u.exp();
                u += EIGHTH;
            }
            egn[127 - 1] = E187D;
            u = U187C;
            for ig in 128..=175usize {
                egn[ig - 1] = EZERO * u.exp();
                u += EIGHTH;
            }
            for ig in 176..=188usize {
                egn[ig - 1] = EG10B[(ig - 175) - 1];
            }
            (egn, false)
        }

        // LANL 70 group structure (groupr.f90:4284-4290). Direct energies.
        11 => (EG11.to_vec(), false),

        // SAND-II 620 / SAND-IIA 640 group structures (groupr.f90:4293-4318).
        12 | 15 => {
            let ngn: usize = if ign == 15 { 640 } else { 620 };
            let ngp = ngn + 1;
            let mut egn = vec![0.0f64; ngp];
            egn[0] = SANDA;
            // first 45 boundaries in 8 constant-spacing bands
            for ig in 1..=8usize {
                let delta = DELTL[ig - 1] * SANDB;
                let n1 = NDELTA[ig - 1];
                let n2 = NDELTA[ig] - 1; // ndelta(ig+1)-1
                for n in n1..=n2 {
                    egn[n - 1] = egn[(n - 1) - 1] + delta;
                }
            }
            egn[21 - 1] = SANDC; // correct group 21
                                 // groups 46..450 are 10x the group 45 lower
            for ig in 46..=450usize {
                egn[ig - 1] = egn[(ig - 45) - 1] * 10.0;
            }
            // groups 451..ngp have constant spacing of 1e5
            egn[451 - 1] = SANDD;
            for ig in 452..=ngp {
                egn[ig - 1] = egn[(ig - 1) - 1] + SANDE;
            }
            (egn, false)
        }

        // LANL 80 group structure (groupr.f90:4321-4330). Direct energies.
        13 => {
            let ngn = 80usize;
            let ngp = ngn + 1;
            let mut egn = vec![0.0f64; ngp];
            let mut u = UU80;
            for ig in 1..=ngp {
                egn[(82 - ig) - 1] = EZERO * u.exp();
                if ig <= ngn {
                    u -= U80[ig - 1];
                }
            }
            egn[81 - 1] = 2.0 * EZERO;
            (egn, false)
        }

        // EURLIB 100 group structure (groupr.f90:4333-4343). Lethargy grid.
        14 => {
            let ngp = 101usize;
            let mut egn = vec![0.0f64; ngp];
            egn[101 - 1] = -0.4f64; // -4*tenth
            let mut ic = 0usize;
            for ig in 2..=101usize {
                if ic < IG14.len() && ig == IG14[ic] {
                    ic += 1;
                }
                egn[(102 - ig) - 1] = egn[(103 - ig) - 1] + GL14[ic - 1];
            }
            (egn, true)
        }

        // VITAMIN-E 174 / VITAMIN-J 175 group structures (groupr.f90:4346-4362).
        16 | 17 => {
            let ngn: usize = if ign == 16 { 174 } else { 175 };
            let ngp = ngn + 1;
            let mut egn = vec![0.0f64; ngp];
            for ig in 1..=84usize {
                egn[ig - 1] = EG15A[ig - 1];
            }
            for ig in 85..=175usize {
                egn[ig - 1] = EG15B[(ig - 84) - 1];
            }
            if ign != 16 {
                egn[166 - 1] = E175;
                for ig in 167..=176usize {
                    egn[ig - 1] = EG15B[(ig - 85) - 1];
                }
            }
            (egn, false)
        }

        // XMAS 172 group structure (groupr.f90:4365-4371). Reversed table.
        18 => {
            let ngp = 173usize;
            let mut egn = vec![0.0f64; ngp];
            for ig in 1..=ngp {
                egn[ig - 1] = EG18[(174 - ig) - 1];
            }
            (egn, false)
        }

        // ECCO 33 group structure (groupr.f90:4374-4380). Direct energies.
        19 => (EG19.to_vec(), false),

        // ECCO 1968 group structure (groupr.f90:4383-4404). Direct energies.
        20 => {
            let mut egn = vec![0.0f64; 1969];
            for ig in 0..392 {
                egn[ig] = EG20A[ig];
                egn[ig + 392] = EG20B[ig];
                egn[ig + 784] = EG20C[ig];
                egn[ig + 1176] = EG20D[ig];
                egn[ig + 1568] = EG20E[ig];
            }
            for ig in 0..9 {
                egn[ig + 1960] = EG20F[ig];
            }
            (egn, false)
        }

        // TRIPOLI 315 group structure (groupr.f90:4407-4413). Direct energies.
        21 => (EG21.to_vec(), false),

        // XMAS LWPC 172 group structure (groupr.f90:4416-4422). Direct energies.
        22 => (EG22.to_vec(), false),

        // VIT-J LWPC 175 group structure (groupr.f90:4425-4431). Direct energies.
        23 => (EG23.to_vec(), false),

        // SHEM CEA 281 group structure (groupr.f90:4434-4440). Direct energies.
        24 => (EG24.to_vec(), false),

        // SHEM EPM 295 group structure (groupr.f90:4443-4449). Direct energies.
        25 => (EG25.to_vec(), false),

        // SHEM CEA/EPM 361 group structure (groupr.f90:4452-4458). Direct energies.
        26 => (EG26.to_vec(), false),

        // SHEM EPM 315 group structure (groupr.f90:4461-4467). Direct energies.
        27 => (EG27.to_vec(), false),

        // RAHAB AECL 89 group structure (groupr.f90:4470-4476). Direct energies.
        28 => (EG28.to_vec(), false),

        // CCFE 660 group structure (groupr.f90:4479-4485). Direct energies.
        29 => (EG29.to_vec(), false),

        // UKAEA 1025 group structure (groupr.f90:4488-4494). Direct energies.
        30 => (EG30.to_vec(), false),

        // UKAEA 1067 group structure (groupr.f90:4497-4503). Direct energies.
        31 => (EG31.to_vec(), false),

        // UKAEA 1102 group structure (groupr.f90:4506-4512). Direct energies.
        32 => (EG32.to_vec(), false),

        // UKAEA 142 group structure (groupr.f90:4515-4521). Direct energies.
        33 => (EG33.to_vec(), false),

        // LANL 618 group structure (groupr.f90:4524-4530). Direct energies.
        34 => (EG618.to_vec(), false),

        // APOLLO 99 group structure (groupr.f90:4533-4539). Direct energies.
        35 => (EG35.to_vec(), false),

        // ECCO 1962 group structure (groupr.f90:4542-4554). Direct energies.
        36 => {
            let ngp = 1963usize;
            let mut egn = vec![0.0f64; ngp];
            for ig in 0..392 {
                egn[ig] = EG20A[ig];
                egn[ig + 392] = EG20B[ig];
            }
            let f = (1.0f64 / 120.0f64).exp();
            for ig in 785..=ngp {
                egn[ig - 1] = egn[(ig - 1) - 1] * f;
            }
            (egn, false)
        }

        // Illegal ign: NJOY calls error() and aborts (groupr.f90:4557-4558).
        other => {
            return Err(NjoyError::EndfParse(format!(
                "gengpn: illegal neutron group structure ign={other}"
            )));
        }
    };

    // Convert lethargy grid to energies (groupr.f90:4562-4567).
    if lflag {
        for e in egn.iter_mut() {
            *e = sigfig(EZERO * (-*e).exp(), 7, 0);
        }
    }

    Ok(egn)
}

/// Adjust `x` to have `ndig` significant figures, optionally shading it up or
/// down by `idig` in the last digit.
///
/// Faithful port of NJOY2016's `sigfig` (`util.f90:361-393`). Used here to round
/// the energies produced from lethargy grids exactly the way NJOY does, so the
/// boundaries match the Fortran output bit-for-bit at 7 significant figures.
fn sigfig(x: f64, ndig: i32, idig: i32) -> f64 {
    const BIAS: f64 = 1.000_000_000_000_1_f64;
    const TEN: f64 = 10.0;
    let mut xx = 0.0f64;
    if x != 0.0 {
        let aa = x.abs().log10();
        let mut ipwr = aa as i32; // Fortran int(): truncate toward zero
        if aa < 0.0 {
            ipwr -= 1;
        }
        ipwr = ndig - 1 - ipwr;
        // nint(): round half away from zero, matching Fortran and f64::round.
        let mut ii = (x * TEN.powi(ipwr) + TEN.powi(ndig - 11)).round() as i64;
        let mut ipwr2 = ipwr;
        if ii >= 10i64.pow(ndig as u32) {
            ii /= 10;
            ipwr2 -= 1;
        }
        ii += idig as i64;
        xx = (ii as f64) * TEN.powi(-ipwr2);
    }
    xx *= BIAS;
    xx
}

/// Named handle for a built-in neutron group structure.
///
/// This is a convenience wrapper over the integer `ign` index used by NJOY. Each
/// variant maps to one `ign`; [`NeutronGroupStructure::boundaries`] returns the
/// eV boundaries (ascending in energy, length = groups + 1) by delegating to
/// [`neutron_group_structure`]. The `Arbitrary` variant corresponds to
/// `abs(ign)==1`, which is read from input and has no built-in table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NeutronGroupStructure {
    /// Arbitrary structure, read from input file (`ign = 1`). No built-in table.
    Arbitrary,
    /// CSEWG 239-group structure (`ign = 2`).
    Csewg239,
    /// LANL 30-group structure (`ign = 3`).
    Lanl30,
    /// ANL 27-group structure (`ign = 4`).
    Anl27,
    /// RRD 50-group structure (`ign = 5`).
    Rrd50,
    /// GAM-I 68-group structure (`ign = 6`).
    GamI68,
    /// GAM-II 100-group structure (`ign = 7`).
    GamII100,
    /// LASER-THERMOS 35-group structure (`ign = 8`).
    LaserThermos35,
    /// EPRI-CPM/WIMS 69-group structure (`ign = 9`).
    EpriCpm69,
    /// LANL 187-group structure (`ign = 10`).
    Lanl187,
    /// LANL 70-group structure (`ign = 11`).
    Lanl70,
    /// SAND-II 620-group structure (`ign = 12`).
    SandII620,
    /// LANL 80-group structure (`ign = 13`).
    Lanl80,
    /// EURLIB 100-group structure (`ign = 14`).
    Eurlib100,
    /// SAND-IIA 640-group structure (`ign = 15`).
    SandIIA640,
    /// VITAMIN-E 174-group structure (`ign = 16`).
    VitaminE174,
    /// VITAMIN-J 175-group structure (`ign = 17`).
    VitaminJ175,
    /// XMAS 172-group structure (`ign = 18`).
    Xmas172,
    /// ECCO 33-group structure (`ign = 19`).
    Ecco33,
    /// ECCO 1968-group structure (`ign = 20`).
    Ecco1968,
    /// TRIPOLI 315-group structure (`ign = 21`).
    Tripoli315,
    /// XMAS LWPC 172-group structure (`ign = 22`).
    XmasLwpc172,
    /// VIT-J LWPC 175-group structure (`ign = 23`).
    VitJLwpc175,
    /// SHEM CEA 281-group structure (`ign = 24`).
    ShemCea281,
    /// SHEM EPM 295-group structure (`ign = 25`).
    ShemEpm295,
    /// SHEM CEA/EPM 361-group structure (`ign = 26`).
    ShemCeaEpm361,
    /// SHEM EPM 315-group structure (`ign = 27`).
    ShemEpm315,
    /// RAHAB AECL 89-group structure (`ign = 28`).
    RahabAecl89,
    /// CCFE 660-group structure (`ign = 29`).
    Ccfe660,
    /// UKAEA 1025-group structure (`ign = 30`).
    Ukaea1025,
    /// UKAEA 1067-group structure (`ign = 31`).
    Ukaea1067,
    /// UKAEA 1102-group structure (`ign = 32`).
    Ukaea1102,
    /// UKAEA 142-group structure (`ign = 33`).
    Ukaea142,
    /// LANL 618-group structure (`ign = 34`).
    Lanl618,
    /// APOLLO 99-group structure (`ign = 35`).
    Apollo99,
    /// ECCO 1962-group structure (`ign = 36`).
    Ecco1962,
}

impl NeutronGroupStructure {
    /// The integer `ign` index NJOY uses for this structure.
    pub fn ign(self) -> i32 {
        match self {
            Self::Arbitrary => 1,
            Self::Csewg239 => 2,
            Self::Lanl30 => 3,
            Self::Anl27 => 4,
            Self::Rrd50 => 5,
            Self::GamI68 => 6,
            Self::GamII100 => 7,
            Self::LaserThermos35 => 8,
            Self::EpriCpm69 => 9,
            Self::Lanl187 => 10,
            Self::Lanl70 => 11,
            Self::SandII620 => 12,
            Self::Lanl80 => 13,
            Self::Eurlib100 => 14,
            Self::SandIIA640 => 15,
            Self::VitaminE174 => 16,
            Self::VitaminJ175 => 17,
            Self::Xmas172 => 18,
            Self::Ecco33 => 19,
            Self::Ecco1968 => 20,
            Self::Tripoli315 => 21,
            Self::XmasLwpc172 => 22,
            Self::VitJLwpc175 => 23,
            Self::ShemCea281 => 24,
            Self::ShemEpm295 => 25,
            Self::ShemCeaEpm361 => 26,
            Self::ShemEpm315 => 27,
            Self::RahabAecl89 => 28,
            Self::Ccfe660 => 29,
            Self::Ukaea1025 => 30,
            Self::Ukaea1067 => 31,
            Self::Ukaea1102 => 32,
            Self::Ukaea142 => 33,
            Self::Lanl618 => 34,
            Self::Apollo99 => 35,
            Self::Ecco1962 => 36,
        }
    }

    /// Group-boundary energies in eV, ascending, length = groups + 1.
    ///
    /// Delegates to [`neutron_group_structure`]. Returns [`NjoyError::NotPorted`]
    /// for [`NeutronGroupStructure::Arbitrary`], which is read from input.
    pub fn boundaries(self) -> Result<Vec<f64>, NjoyError> {
        neutron_group_structure(self.ign())
    }
}

mod tables;
use tables::*;

#[cfg(test)]
mod tests {
    use super::*;

    /// Relative-difference check for energy boundaries.
    fn close(a: f64, b: f64, rtol: f64) -> bool {
        (a - b).abs() <= rtol * b.abs().max(1.0e-30)
    }

    /// LANL 30-group (`ign = 3`) count + endpoints.
    ///
    /// Methodology: `gengpn` case `ign=3` (groupr.f90:4178-4184) copies the
    /// `eg3` table verbatim as energies, so the returned vector must be the 31
    /// boundaries of `eg3`. Expected values read directly from the Fortran
    /// `eg3` array (groupr.f90:1693-1699): first = 1.39e-4 eV, last = 1.70e7 eV,
    /// 30 groups => 31 boundaries, ascending.
    #[test]
    fn lanl30_ign3() {
        let egn = neutron_group_structure(3).unwrap();
        assert_eq!(egn.len(), 31, "LANL-30 must have 31 boundaries");
        assert!(close(egn[0], 1.39e-4, 1e-9), "first = {}", egn[0]);
        assert!(close(egn[30], 1.70e7, 1e-9), "last = {}", egn[30]);
        assert!(egn.windows(2).all(|w| w[0] < w[1]), "must be ascending");
    }

    /// LANL 70-group (`ign = 11`) count + endpoints.
    ///
    /// Methodology: case `ign=11` (groupr.f90:4284-4290) copies `eg11`
    /// verbatim. Expected from the Fortran `eg11` array (groupr.f90:1753-1768):
    /// first = 10.677 eV, last = 2.0e7 eV, 70 groups => 71 boundaries.
    #[test]
    fn lanl70_ign11() {
        let egn = neutron_group_structure(11).unwrap();
        assert_eq!(egn.len(), 71);
        assert!(close(egn[0], 10.677, 1e-9), "first = {}", egn[0]);
        assert!(close(egn[70], 2.0e7, 1e-9), "last = {}", egn[70]);
        assert!(egn.windows(2).all(|w| w[0] < w[1]));
    }

    /// XMAS 172-group (`ign = 18`) count + endpoints (reversed table).
    ///
    /// Methodology: case `ign=18` (groupr.f90:4365-4371) sets
    /// `egn(ig) = eg18(174-ig)`, i.e. it reverses the descending `eg18` table
    /// into ascending order. Expected from the Fortran `eg18` array
    /// (groupr.f90:1831-1869): reversed, egn[0] = eg18(173) = 1.00001e-5 eV and
    /// egn[172] = eg18(1) = 1.96403e7 eV; 172 groups => 173 boundaries.
    #[test]
    fn xmas172_ign18() {
        let egn = neutron_group_structure(18).unwrap();
        assert_eq!(egn.len(), 173);
        assert!(close(egn[0], 1.00001e-5, 1e-6), "first = {}", egn[0]);
        assert!(close(egn[172], 1.96403e7, 1e-6), "last = {}", egn[172]);
        assert!(egn.windows(2).all(|w| w[0] < w[1]));
    }

    /// ECCO 1968-group (`ign = 20`) count + endpoints (spliced table).
    ///
    /// Methodology: case `ign=20` (groupr.f90:4383-4404) concatenates
    /// eg20a..eg20f (5*392 + 9 = 1969) as energies. Expected from the Fortran
    /// arrays: egn[0] = eg20a(1) = 1.000010e-5 eV, egn[1968] = eg20f(9) =
    /// 1.964033e7 eV; 1968 groups => 1969 boundaries.
    #[test]
    fn ecco1968_ign20() {
        let egn = neutron_group_structure(20).unwrap();
        assert_eq!(egn.len(), 1969);
        assert!(close(egn[0], 1.000010e-5, 1e-6), "first = {}", egn[0]);
        assert!(close(egn[1968], 1.964033e7, 1e-6), "last = {}", egn[1968]);
        assert!(egn.windows(2).all(|w| w[0] < w[1]));
    }

    /// CSEWG 239-group (`ign = 2`) count + lethargy->energy endpoints.
    ///
    /// Methodology: case `ign=2` (groupr.f90:4168-4175, lflag=1) copies the
    /// `gl2` lethargy grid (241 values) then converts each via
    /// `E = sigfig(1e7*exp(-u), 7, 0)` (groupr.f90:4562-4567). Expected values
    /// were computed by evaluating that exact expression on the `gl2` endpoints
    /// (first lethargy 27.631, last -0.69167 from groupr.f90:1652-1692):
    /// egn[0] = 1.0000210e-5 eV, egn[240] = 1.9970480e7 eV; 240 groups =>
    /// 241 boundaries, ascending.
    #[test]
    fn csewg239_ign2_lethargy() {
        let egn = neutron_group_structure(2).unwrap();
        assert_eq!(egn.len(), 241);
        assert!(close(egn[0], 1.0000210e-5, 1e-5), "first = {}", egn[0]);
        assert!(close(egn[240], 1.9970480e7, 1e-5), "last = {}", egn[240]);
        assert!(egn.windows(2).all(|w| w[0] < w[1]), "must be ascending");
    }

    /// Every built-in `ign` returns a vector whose length is groups + 1.
    ///
    /// Methodology: cross-checks the group counts from the `gengpn` header table
    /// (groupr.f90:1604-1642) against the returned vector length for all ported
    /// `ign` values. This guards the splice/index arithmetic in the computed
    /// cases (10, 12/15, 13, 14, 36) against off-by-one errors.
    #[test]
    fn all_ign_group_counts() {
        // NOTE ign=2: the gengpn header calls this "CSEWG 239", but the code
        // sets ngn=240 (groupr.f90:4169) and the display says "csewg 240 group"
        // (groupr.f90:4572-4573). The actual grid is 240 groups / 241 boundaries.
        let expected: &[(i32, usize)] = &[
            (2, 240),
            (3, 30),
            (4, 27),
            (5, 50),
            (6, 68),
            (7, 100),
            (8, 35),
            (9, 69),
            (10, 187),
            (11, 70),
            (12, 620),
            (13, 80),
            (14, 100),
            (15, 640),
            (16, 174),
            (17, 175),
            (18, 172),
            (19, 33),
            (20, 1968),
            (21, 315),
            (22, 172),
            (23, 175),
            (24, 281),
            (25, 295),
            (26, 361),
            (27, 315),
            (28, 89),
            (29, 660),
            (30, 1025),
            (31, 1067),
            (32, 1102),
            (33, 142),
            (34, 618),
            (35, 99),
            (36, 1962),
        ];
        for &(ign, ngroups) in expected {
            let egn = neutron_group_structure(ign).unwrap();
            assert_eq!(egn.len(), ngroups + 1, "ign={ign} group count");
            assert!(egn.iter().all(|&e| e >= 0.0), "ign={ign} negative energy");
        }
    }

    /// Every built-in `ign` produces a strictly ascending grid.
    ///
    /// Methodology: NJOY group boundaries are strictly monotone; this verifies
    /// none of the ported tables or computed splices introduced a duplicate or
    /// out-of-order boundary.
    #[test]
    fn all_ign_ascending() {
        for ign in 2..=36 {
            let egn = neutron_group_structure(ign).unwrap();
            assert!(
                egn.windows(2).all(|w| w[0] < w[1]),
                "ign={ign} not strictly ascending"
            );
        }
    }

    /// `abs(ign)==1` is read-from-input and reports `NotPorted`, not a table.
    ///
    /// Methodology: groupr.f90:4156-4165 reads the arbitrary structure from the
    /// system input; there is no built-in table, so the port must surface
    /// `NjoyError::NotPorted` rather than fabricate boundaries.
    #[test]
    fn arbitrary_ign1_not_ported() {
        assert!(matches!(
            neutron_group_structure(1),
            Err(NjoyError::NotPorted(_))
        ));
        assert!(matches!(
            neutron_group_structure(-1),
            Err(NjoyError::NotPorted(_))
        ));
    }

    /// An unrecognised `ign` is an error, never a panic (NJOY aborts here).
    #[test]
    fn illegal_ign_errors() {
        assert!(neutron_group_structure(0).is_err());
        assert!(neutron_group_structure(99).is_err());
    }

    /// The named-enum wrapper agrees with the integer entry point.
    #[test]
    fn enum_matches_ign() {
        let a = NeutronGroupStructure::Xmas172.boundaries().unwrap();
        let b = neutron_group_structure(18).unwrap();
        assert_eq!(a, b);
        assert_eq!(NeutronGroupStructure::Ecco1968.ign(), 20);
        assert!(NeutronGroupStructure::Arbitrary.boundaries().is_err());
    }
}
