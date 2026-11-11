// Translated from the input blocks of NJOY2016.80 (commit ab27c6414a0f) `src/moder.f90`,
// `reconr.f90` (`reconr`, `ruina`), `broadr.f90`, `heatr.f90`, `thermr.f90`, `groupr.f90`
// (`groupr`, `ruinb`, `gengpn`, `gengpg`, `genwtf`) and `gaspr.f90`.
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! Per-module **card readers**: each translates the `read(nsysi,*)` statements
//! of one NJOY module, in order and with upstream's defaults, and builds the
//! crate's existing typed input where there is one ([`HeatrInput`],
//! [`GrouprInput`]).
//!
//! Every function here consumes exactly the records its module consumes, so
//! the deck driver stays in step with the stream: the next record is the next
//! module's name.

use crate::deck::cards::{CardReader, Slot};
use crate::groupr::{
    input::neutron_group_from_ign, GrouprInput, PhotonGroupStructure, PrintOption, ReactionRequest, SmoothingOption,
    UnitAssignments, WeightSelection,
};
use crate::heatr::{HeatrInput, HeatrUnits};
use crate::NjoyError;

fn bad(msg: impl Into<String>) -> NjoyError {
    NjoyError::InvalidInput(format!("deck: {}", msg.into()))
}

fn ints(r: &mut CardReader, defaults: &[i32]) -> Result<Vec<i32>, NjoyError> {
    r.read_ints(defaults)
}

fn count(n: i32, what: &str) -> Result<usize, NjoyError> {
    usize::try_from(n).map_err(|_| bad(format!("negative count for {what}: {n}")))
}

// ── MODER ───────────────────────────────────────────────────────────────

/// MODER's cards (`moder.f90:81-205`).
#[derive(Debug, Clone, PartialEq)]
pub enum ModerCards {
    /// `|nin| >= 20`: convert `nin` to `nout` (the mode follows each unit's sign).
    Convert {
        /// Input unit.
        nin: i32,
        /// Output unit.
        nout: i32,
    },
    /// `1 <= |nin| <= 19`: the selection loop — put the listed materials of
    /// the listed tapes on `nout` under tape label `tpid`.
    Select {
        /// Output unit.
        nout: i32,
        /// `nin` card 1 (the flag that selects the loop; `2` GENDF, `3` covariance).
        nin_flag: i32,
        /// Tape identification (`character(66)`).
        tpid: String,
        /// `(nin, matd)` requests, in deck order, ended by `nin = 0`.
        picks: Vec<(i32, i32)>,
    },
}

/// Read MODER's cards.
pub fn read_moder(r: &mut CardReader) -> Result<ModerCards, NjoyError> {
    let c = ints(r, &[0, 0])?;
    let (nin, nout) = (c[0], c[1]);
    if (1..=19).contains(&nin.abs()) {
        let tpid = r.read_string(66)?;
        let mut picks = Vec::new();
        loop {
            let p = ints(r, &[0, 0])?;
            if p[0] == 0 {
                break;
            }
            picks.push((p[0], p[1]));
        }
        Ok(ModerCards::Select { nout, nin_flag: nin, tpid, picks })
    } else {
        Ok(ModerCards::Convert { nin, nout })
    }
}

// ── RECONR ──────────────────────────────────────────────────────────────

/// One material's RECONR cards (`ruina`, `reconr.f90:415-460`).
#[derive(Debug, Clone, PartialEq)]
pub struct ReconrMaterial {
    /// `mata`.
    pub mat: i32,
    /// `err`: fractional reconstruction tolerance.
    pub err: f64,
    /// `tempr` \[K\].
    pub tempr: f64,
    /// `errmax` as read (`-1` unset; upstream then uses `10 err`).
    pub errmax: f64,
    /// `errint` as read (`-1` unset; upstream then uses `err/20000`).
    pub errint: f64,
    /// The `ncards` descriptive text cards (`character(66)`).
    pub cards: Vec<String>,
    /// `enode`: extra grid energies (`ngrid` values).
    pub enode: Vec<f64>,
}

/// RECONR's cards: the two units, the tape label, and the materials.
#[derive(Debug, Clone, PartialEq)]
pub struct ReconrCards {
    /// `nendf`.
    pub nendf: i32,
    /// `npend`.
    pub npend: i32,
    /// `tlabel` (`character(66)`).
    pub label: String,
    /// Materials in deck order (the terminating `mata = 0` is not stored).
    pub materials: Vec<ReconrMaterial>,
}

/// Read RECONR's cards (`reconr.f90:165-168` and `ruina` until `mata = 0`).
pub fn read_reconr(r: &mut CardReader) -> Result<ReconrCards, NjoyError> {
    let u = ints(r, &[0, 0])?;
    let label = r.read_string(66)?;
    let mut materials = Vec::new();
    loop {
        let c = ints(r, &[0, 0, 0])?;
        if c[0] == 0 {
            break;
        }
        let v = r.read_reals(&[0.0, 0.0, -1.0, -1.0])?;
        let ncards = count(c[1], "ncards")?;
        let ngrid = count(c[2], "ngrid")?;
        let mut cards = Vec::with_capacity(ncards);
        for _ in 0..ncards {
            cards.push(r.read_string(66)?);
        }
        let enode = if ngrid > 0 { r.read_reals(&vec![0.0; ngrid])? } else { Vec::new() };
        materials.push(ReconrMaterial { mat: c[0], err: v[0], tempr: v[1], errmax: v[2], errint: v[3], cards, enode });
    }
    Ok(ReconrCards { nendf: u[0], npend: u[1], label, materials })
}

// ── BROADR ──────────────────────────────────────────────────────────────

/// BROADR's cards (`broadr.f90:189-211`, `:1016`).
#[derive(Debug, Clone, PartialEq)]
pub struct BroadrCards {
    /// `nendf`.
    pub nendf: i32,
    /// `nin`.
    pub nin: i32,
    /// `nout`.
    pub nout: i32,
    /// Materials, each with the parameters in force when it was reached.
    pub materials: Vec<BroadrMaterial>,
}

/// One BROADR material request.
#[derive(Debug, Clone, PartialEq)]
pub struct BroadrMaterial {
    /// `mat1`.
    pub mat: i32,
    /// `istart` (restart flag).
    pub istart: i32,
    /// `istrap` (bootstrap flag).
    pub istrap: i32,
    /// `temp1`: starting material temperature \[K\].
    pub temp1: f64,
    /// `errthn`.
    pub errthn: f64,
    /// `thnmx` as read (default `6.5e6` eV).
    pub thnmx: f64,
    /// `errmax` as read (0 = upstream's `10 errthn`).
    pub errmax: f64,
    /// `errint` as read (0 = upstream's `errthn/20000`).
    pub errint: f64,
    /// Final temperatures `temp2` \[K\].
    pub temps: Vec<f64>,
}

/// Read BROADR's cards, including the trailing material-number cards.
pub fn read_broadr(r: &mut CardReader) -> Result<BroadrCards, NjoyError> {
    let u = ints(r, &[0, 0, 0])?;
    let mut materials = Vec::new();
    let c = {
        let mut s = [Slot::Int(0), Slot::Int(1), Slot::Int(0), Slot::Int(0), Slot::Real(0.0)];
        r.read(&mut s)?;
        s
    };
    let (mat1, ntemp2, istart, istrap, temp1) = (c[0].int(), c[1].int(), c[2].int(), c[3].int(), c[4].real());
    let v = r.read_reals(&[0.0, 6.5e6, 0.0, 0.0])?;
    let temps = r.read_reals(&vec![0.0; count(ntemp2, "ntemp2")?])?;
    materials.push(BroadrMaterial {
        mat: mat1,
        istart,
        istrap,
        temp1,
        errthn: v[0],
        thnmx: v[1],
        errmax: v[2],
        errint: v[3],
        temps: temps.clone(),
    });
    loop {
        let m = ints(r, &[0])?[0];
        if m == 0 {
            break;
        }
        let mut next = materials[0].clone();
        next.mat = m;
        materials.push(next);
    }
    Ok(BroadrCards { nendf: u[0], nin: u[1], nout: u[2], materials })
}

// ── HEATR ───────────────────────────────────────────────────────────────

/// Read HEATR's cards into the crate's [`HeatrInput`] (`heatr.f90:153-195`).
///
/// The `qbar` table of an energy-dependent `Q` is read as upstream does: 1000
/// values preset to the flag `-1e-9`, then cut after the last value above it.
pub fn read_heatr(r: &mut CardReader) -> Result<HeatrInput, NjoyError> {
    let u = ints(r, &[0, 0, 0, 0])?;
    let units = HeatrUnits { nendf: u[0], nin: u[1], nout: u[2], nplot: u[3] };
    let mut s = [
        Slot::Int(0),
        Slot::Int(0),
        Slot::Int(0),
        Slot::Int(0),
        Slot::Int(0),
        Slot::Int(0),
        Slot::Real(0.0),
    ];
    r.read(&mut s)?;
    let npk = count(s[1].int(), "npk")?;
    let nqa = count(s[2].int(), "nqa")?;
    let mut input = HeatrInput {
        matd: s[0].int(),
        ntemp: count(s[3].int(), "ntemp")?,
        local: s[4].int() != 0,
        iprint: s[5].int(),
        ed: s[6].real(),
        plot: units.nplot != 0,
        units,
        ..HeatrInput::default()
    };
    if npk > 0 {
        input.partial_kermas = ints(r, &vec![0; npk])?;
    }
    if nqa > 0 {
        let mta = ints(r, &vec![0; nqa])?;
        let qa = r.read_reals(&vec![0.0; nqa])?;
        for (m, q) in mta.into_iter().zip(qa) {
            input.user_q.push((m, q));
            if q >= 99.0e6 {
                let vals = r.read_reals(&vec![-1.0e-9; 1000])?;
                let nz0 = vals.iter().rposition(|&v| v > -1.0e-9).map_or(0, |p| p + 1);
                input.qbar.push(vals[..nz0].to_vec());
            }
        }
    }
    Ok(input)
}

// ── THERMR ──────────────────────────────────────────────────────────────

/// THERMR's cards (`thermr.f90:179-202`).
#[derive(Debug, Clone, PartialEq)]
pub struct ThermrCards {
    /// `nendf`.
    pub nendf: i32,
    /// `nin`.
    pub nin: i32,
    /// `nout`.
    pub nout: i32,
    /// `matde`: ENDF thermal material (0 = free gas).
    pub matde: i32,
    /// `matdp`: the PENDF material.
    pub matdp: i32,
    /// `nbin`: equiprobable angles.
    pub nbin: i32,
    /// `ntemp`.
    pub ntemp: i32,
    /// `iinc`: inelastic option.
    pub iinc: i32,
    /// `icoh`: elastic option.
    pub icoh: i32,
    /// `iform`: emission format.
    pub iform: i32,
    /// `natom`: principal atoms per molecule.
    pub natom: i32,
    /// `mtref`: reference MT (221-250).
    pub mtref: i32,
    /// `iprint`.
    pub iprint: i32,
    /// Temperatures \[K\].
    pub temps: Vec<f64>,
    /// `tol`.
    pub tol: f64,
    /// `emax` \[eV\].
    pub emax: f64,
}

/// Read THERMR's cards.
pub fn read_thermr(r: &mut CardReader) -> Result<ThermrCards, NjoyError> {
    let u = ints(r, &[0, 0, 0])?;
    if u[1] == 0 {
        return Err(bad("thermr: nin=0"));
    }
    let mut s: Vec<Slot> = (0..10).map(|i| Slot::Int(if i == 3 { 1 } else { 0 })).collect();
    r.read(&mut s)?;
    let mtref = s[8].int();
    if !(221..=250).contains(&mtref) {
        return Err(bad("thermr: illegal reference mt"));
    }
    let ntemp = s[3].int();
    let temps = r.read_reals(&vec![0.0; count(ntemp, "ntemp")?])?;
    let te = r.read_reals(&[0.0, 0.0])?;
    Ok(ThermrCards {
        nendf: u[0],
        nin: u[1],
        nout: u[2],
        matde: s[0].int(),
        matdp: s[1].int(),
        nbin: s[2].int(),
        ntemp,
        iinc: s[4].int(),
        icoh: s[5].int(),
        iform: s[6].int(),
        natom: s[7].int(),
        mtref,
        iprint: s[9].int(),
        temps,
        tol: te[0],
        emax: te[1],
    })
}

// ── GASPR ───────────────────────────────────────────────────────────────

/// GASPR's card (`gaspr.f90:64`): `nendf npend noutp`.
pub fn read_gaspr(r: &mut CardReader) -> Result<[i32; 3], NjoyError> {
    let c = ints(r, &[0, 0, 0])?;
    Ok([c[0], c[1], c[2]])
}

// ── GROUPR ──────────────────────────────────────────────────────────────

/// One material's worth of GROUPR cards: the crate's [`GrouprInput`] plus the
/// parts it has no field for.
#[derive(Debug, Clone, PartialEq)]
pub struct GrouprJob {
    /// The typed input (`matb` is the job's first material).
    pub input: GrouprInput,
    /// Card 8a/8b/8d extras that [`GrouprInput`] does not carry: the raw
    /// values read (flux calculator `fehi, sigpot, nflmax, ninwt, jsigz,
    /// alpha2, sam, beta, alpha3, gamma`, or the TAB1 weight values, or
    /// `ninwt`), in card order.
    pub weight_cards: Vec<Vec<f64>>,
    /// Card 9a extended requests `(mfd, mtd, izar, lfs)`.
    pub extended: Vec<[i32; 4]>,
    /// Further materials requested by card 10, each with its own sign
    /// (negative = add/replace); the terminating `0` is not stored.
    pub more_materials: Vec<i32>,
}

/// Read GROUPR's cards: `ruinb` (`groupr.f90:1061-1125`), the group structure
/// and weight readers (`gengpn`, `gengpg`, `genwtf`), card 9 until `mfd = 0`,
/// and card 10 (`:993`) until `matb = 0`.
pub fn read_groupr(r: &mut CardReader) -> Result<GrouprJob, NjoyError> {
    let u = ints(r, &[0, 0, 0, 0])?;
    let units = UnitAssignments { nendf: u[0], npend: u[1], ngout1: u[2], ngout2: u[3] };
    // matb ign igg iwt lord ntemp nsigz iprint ismooth; iprint, ismooth, ntemp, nsigz preset 1
    let mut s = [
        Slot::Int(0),
        Slot::Int(0),
        Slot::Int(0),
        Slot::Int(0),
        Slot::Int(0),
        Slot::Int(1),
        Slot::Int(1),
        Slot::Int(1),
        Slot::Int(1),
    ];
    r.read(&mut s)?;
    let (matb, ign, igg, iwt, lord, ntemp, nsigz, iprint, ismooth) = (
        s[0].int(),
        s[1].int(),
        s[2].int(),
        s[3].int(),
        s[4].int(),
        count(s[5].int(), "ntemp")?,
        count(s[6].int(), "nsigz")?,
        s[7].int(),
        s[8].int(),
    );
    let title = r.read_string(66)?;
    let temps = r.read_reals(&vec![0.0; ntemp])?;
    let sigz = r.read_reals(&vec![0.0; nsigz])?;
    let mut input = GrouprInput { units, matb, lord, title, temperatures: temps, sigma_zeros: sigz, ..GrouprInput::default() };
    input.iprint = PrintOption::from_iprint(iprint).ok_or_else(|| bad("groupr: illegal iprint"))?;
    input.ismooth = SmoothingOption::from_ismooth(ismooth).ok_or_else(|| bad("groupr: illegal ismooth"))?;
    input.neutron_groups = neutron_group_from_ign(ign).map_err(|e| bad(format!("groupr ign={ign}: {e}")))?;
    input.photon_groups =
        PhotonGroupStructure::from_igg(igg).ok_or_else(|| bad(format!("groupr: illegal igg {igg}")))?;
    // gengpn: ign = +-1 reads ngn then ngn+1 boundaries.
    if ign.abs() == 1 {
        let ngn = count(r.read_ints(&[0])?[0], "ngn")?;
        input.neutron_grid = Some(r.read_reals(&vec![0.0; ngn + 1])?);
    }
    // gengpg: igg = 1 reads ngg then ngg+1 boundaries.
    if igg == 1 {
        let ngg = count(r.read_ints(&[0])?[0], "ngg")?;
        input.photon_grid = Some(r.read_reals(&vec![0.0; ngg + 1])?);
    }
    // genwtf (`groupr.f90:4978-5045`).
    let mut weight_cards: Vec<Vec<f64>> = Vec::new();
    input.weight = WeightSelection::from_iwt(iwt).ok_or_else(|| bad(format!("groupr: illegal iwt {iwt}")))?;
    if iwt < 0 {
        let v = r.read_reals(&[0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0])?;
        if let WeightSelection::ComputeFlux { params, .. } = &mut input.weight {
            *params = Some(crate::groupr::FluxCalcParams {
                fehi_ev: v[0],
                sigpot_barns: v[1],
                nflmax: v[2] as i32,
                ninwt: v[3] as i32,
                jsigz: v[4] as i32,
                alpha2: v[5],
                sam_barns: v[6],
                beta: v[7],
                alpha3: v[8],
                gamma: v[9],
            });
        }
        weight_cards.push(v);
    }
    match iwt.abs() {
        0 => {
            let ninwt = r.read_ints(&[0])?[0];
            input.weight = WeightSelection::ReadResonanceFlux { ninwt: ninwt.abs() };
        }
        1 => weight_cards.push(r.read_reals(&vec![0.0; 10000])?),
        4 => {
            let v = r.read_reals(&[0.0, 0.0, 0.0, 0.0])?;
            input.thermal_fission_params = Some(crate::groupr::ThermalFissionParams {
                thermal_break_ev: v[0],
                thermal_temp_ev: v[1],
                fission_break_ev: v[2],
                fission_temp_ev: v[3],
            });
        }
        _ => {}
    }
    // card 9
    let mut extended = Vec::new();
    loop {
        let mut c = [Slot::Int(0), Slot::Int(-1000), Slot::string(60, "")];
        r.read(&mut c)?;
        let mfd = c[0].int();
        if mfd == 0 {
            break;
        }
        if mfd == -1 {
            let e = ints(r, &[0, 0, 0, 0])?;
            extended.push([e[0], e[1], e[2], e[3]]);
            continue;
        }
        input.reactions.push(ReactionRequest { mfd, mtd: c[1].int(), name: c[2].text().to_string() });
    }
    // card 10
    let mut more = Vec::new();
    loop {
        let m = ints(r, &[0])?[0];
        if m == 0 {
            break;
        }
        more.push(m);
    }
    input.next_material = more.first().copied();
    Ok(GrouprJob { input, weight_cards, extended, more_materials: more })
}
