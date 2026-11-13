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
        // `ruina` (`reconr.f90:445-455`): no descriptive card is one blank card
        // (`ncards = 1`), which is what NWD = 17 on the tape counts.
        if cards.is_empty() {
            cards.push(" ".repeat(66));
        }
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

// ── VIEWR ───────────────────────────────────────────────────────────────

/// VIEWR's card (`viewr.f90:371`): `infile nps`, the plot-command unit and the
/// PostScript output unit.
pub fn read_viewr(r: &mut CardReader) -> Result<[i32; 2], NjoyError> {
    let c = ints(r, &[0, 0])?;
    Ok([c[0], c[1]])
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
    /// Card 9 (and 9a) lists for the second and later temperatures: upstream reads
    /// one list, ended by `mfd = 0`, per temperature (`groupr.f90:586-630`,
    /// label 590 loops back to label 360). The first temperature's list is
    /// [`GrouprInput::reactions`] and [`GrouprJob::extended`].
    pub later_temperatures: Vec<(Vec<ReactionRequest>, Vec<[i32; 4]>)>,
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
    // card 9, once per temperature (`:586-630`)
    let (reactions, extended) = read_groupr_card9(r)?;
    input.reactions = reactions;
    let mut later_temperatures = Vec::new();
    for _ in 1..input.temperatures.len() {
        later_temperatures.push(read_groupr_card9(r)?);
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
    Ok(GrouprJob { input, weight_cards, extended, later_temperatures, more_materials: more })
}

/// One GROUPR card-9 list, until `mfd = 0` (`groupr.f90:365-383`): the plain
/// requests and the card-9a extended ones (`mfd = -1`).
fn read_groupr_card9(r: &mut CardReader) -> Result<(Vec<ReactionRequest>, Vec<[i32; 4]>), NjoyError> {
    let mut reactions = Vec::new();
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
        reactions.push(ReactionRequest { mfd, mtd: c[1].int(), name: c[2].text().to_string() });
    }
    Ok((reactions, extended))
}

// ── UNRESR ──────────────────────────────────────────────────────────────

/// One material of UNRESR's cards (`unresr.f90:116-127`).
#[derive(Debug, Clone, PartialEq)]
pub struct UnresrMaterial {
    /// `matd`.
    pub mat: i32,
    /// `iprint`.
    pub iprint: i32,
    /// Temperatures \[K\].
    pub temps: Vec<f64>,
    /// Sigma-zero values \[b\].
    pub sigz: Vec<f64>,
}

/// UNRESR's cards: the three units and the materials.
#[derive(Debug, Clone, PartialEq)]
pub struct UnresrCards {
    /// `nendf`.
    pub nendf: i32,
    /// `nin`.
    pub nin: i32,
    /// `nout`.
    pub nout: i32,
    /// Materials, until `matd = 0`.
    pub materials: Vec<UnresrMaterial>,
}

/// Read UNRESR's cards.
pub fn read_unresr(r: &mut CardReader) -> Result<UnresrCards, NjoyError> {
    let u = ints(r, &[0, 0, 0])?;
    let mut materials = Vec::new();
    loop {
        let c = ints(r, &[0, 1, 1, 0])?;
        if c[0] == 0 {
            break;
        }
        let temps = r.read_reals(&vec![0.0; count(c[1], "ntemp")?])?;
        let sigz = r.read_reals(&vec![0.0; count(c[2], "nsigz")?])?;
        materials.push(UnresrMaterial { mat: c[0], iprint: c[3], temps, sigz });
    }
    Ok(UnresrCards { nendf: u[0], nin: u[1], nout: u[2], materials })
}

// ── PURR ────────────────────────────────────────────────────────────────

/// One material of PURR's cards (`purr.f90:176-190`).
#[derive(Debug, Clone, PartialEq)]
pub struct PurrMaterial {
    /// `matd`.
    pub mat: i32,
    /// `nbin`: probability bins.
    pub nbin: i32,
    /// `nladr`: resonance ladders.
    pub nladr: i32,
    /// `iprint` (default 1).
    pub iprint: i32,
    /// `nunx`: ~~extra unresolved energies~~ the number of unresolved energies
    /// processed, `0` (default) all of them (`purr.f90:98`, `:283-292`; corrected
    /// 2026-11-12, gh:#1498).
    pub nunx: i32,
    /// Temperatures \[K\].
    pub temps: Vec<f64>,
    /// Sigma-zero values \[b\].
    pub sigz: Vec<f64>,
}

/// PURR's cards.
#[derive(Debug, Clone, PartialEq)]
pub struct PurrCards {
    /// `nendf`.
    pub nendf: i32,
    /// `nin`.
    pub nin: i32,
    /// `nout`.
    pub nout: i32,
    /// Materials, until `matd = 0`.
    pub materials: Vec<PurrMaterial>,
}

/// Read PURR's cards.
pub fn read_purr(r: &mut CardReader) -> Result<PurrCards, NjoyError> {
    let u = ints(r, &[0, 0, 0])?;
    let mut materials = Vec::new();
    loop {
        // `nbin` and `nladr` have no default upstream (the manual's 20 and 64
        // are what a deck normally gives); a deck that omits them is refused.
        let c = ints(r, &[0, 1, 1, 0, 0, 1, 0])?;
        if c[0] == 0 {
            break;
        }
        if c[3] <= 0 || c[4] <= 0 {
            return Err(bad("purr: nbin and nladr must be given on card 2"));
        }
        let temps = r.read_reals(&vec![0.0; count(c[1], "ntemp")?])?;
        let sigz = r.read_reals(&vec![0.0; count(c[2], "nsigz")?])?;
        materials.push(PurrMaterial { mat: c[0], nbin: c[3], nladr: c[4], iprint: c[5], nunx: c[6], temps, sigz });
    }
    Ok(PurrCards { nendf: u[0], nin: u[1], nout: u[2], materials })
}

// ── ACER ────────────────────────────────────────────────────────────────

/// ACER's cards (`acer.f90:282-447`), every option's cards read; only the
/// fast-data run is executed by the deck driver.
#[derive(Debug, Clone, PartialEq)]
pub struct AcerCards {
    /// `nendf`.
    pub nendf: i32,
    /// `npend`.
    pub npend: i32,
    /// `ngend`.
    pub ngend: i32,
    /// `nace`.
    pub nace: i32,
    /// `ndir`.
    pub ndir: i32,
    /// `|iopt|`: 1 fast, 2 thermal, 3 dosimetry, 4 photoatomic, 5 photonuclear, 7/8 edit.
    pub iopt: i32,
    /// `iopt < 0`: mcnpx formats.
    pub mcnpx: bool,
    /// `iprint` (default 1).
    pub iprint: i32,
    /// `itype`: ACE file type (default 1).
    pub itype: i32,
    /// `suff`: ZAID suffix (default 0).
    pub suff: f64,
    /// `nxtra`.
    pub nxtra: i32,
    /// `izaoption`.
    pub izaopt: i32,
    /// `hk`: the 70-character comment.
    pub hk: String,
    /// The `nxtra` `(iz, aw)` pairs.
    pub extra: Vec<(i32, f64)>,
    /// `matd` (cards 5, 8, 10, 11).
    pub matd: i32,
    /// `tempd` \[K\] (default 300).
    pub tempd: f64,
    /// `newfor` (fast, default 1).
    pub newfor: i32,
    /// `iopp` (fast, default 1).
    pub iopp: i32,
    /// `ismooth` (fast, default 1).
    pub ismooth: i32,
    /// Fast card 7, `thin(1..3)` as read.
    pub thin: [f64; 3],
    /// Thermal cards 8, 8a and 9.
    pub thermal: Option<ThermalCards>,
}

/// ACER's thermal cards 8, 8a and 9.
#[derive(Debug, Clone, PartialEq)]
pub struct ThermalCards {
    /// `tname`.
    pub tname: String,
    /// Moderator ZAs of card 8a.
    pub iza: Vec<i32>,
    /// `mti`.
    pub mti: i32,
    /// `nbint`.
    pub nbint: i32,
    /// `mte`.
    pub mte: i32,
    /// `ielas`.
    pub ielas: i32,
    /// `nmix`.
    pub nmix: i32,
    /// `emax` \[eV\].
    pub emax: f64,
    /// `iwt`.
    pub iwt: i32,
}

/// Read ACER's cards.
pub fn read_acer(r: &mut CardReader) -> Result<AcerCards, NjoyError> {
    let u = ints(r, &[0, 0, 0, 0, 0])?;
    let c = {
        let mut s = [Slot::Int(0), Slot::Int(1), Slot::Int(1), Slot::Real(0.0), Slot::Int(0), Slot::Int(0)];
        r.read(&mut s)?;
        s
    };
    let (iopt_signed, iprint, itype, suff, nxtra, izaopt) =
        (c[0].int(), c[1].int(), c[2].int(), c[3].real(), c[4].int(), c[5].int());
    let hk = r.read_string(70)?;
    let mut extra = Vec::new();
    for _ in 0..nxtra.max(0) {
        let mut s = [Slot::Int(0), Slot::Real(0.0)];
        r.read(&mut s)?;
        extra.push((s[0].int(), s[1].real()));
    }
    let mut a = AcerCards {
        nendf: u[0],
        npend: u[1],
        ngend: u[2],
        nace: u[3],
        ndir: u[4],
        iopt: iopt_signed.abs(),
        mcnpx: iopt_signed < 0,
        iprint,
        itype,
        suff,
        nxtra,
        izaopt,
        hk,
        extra,
        matd: 0,
        tempd: 300.0,
        newfor: 1,
        iopp: 1,
        ismooth: 1,
        thin: [0.0; 3],
        thermal: None,
    };
    match a.iopt {
        1 => {
            let mut s = [Slot::Int(0), Slot::Real(300.0)];
            r.read(&mut s)?;
            a.matd = s[0].int();
            a.tempd = s[1].real();
            let f = ints(r, &[1, 1, 1])?;
            (a.newfor, a.iopp, a.ismooth) = (f[0], f[1], f[2]);
            let z = r.read_reals(&[0.0, 0.0, 0.0])?;
            a.thin = [z[0], z[1], z[2]];
        }
        2 => {
            let mut s = [Slot::Int(0), Slot::Real(300.0), Slot::string(6, ""), Slot::Int(3)];
            r.read(&mut s)?;
            a.matd = s[0].int();
            a.tempd = s[1].real();
            let nza = count(s[3].int(), "nza")?;
            let iza = ints(r, &vec![0; nza])?;
            let mut t = [Slot::Int(0), Slot::Int(0), Slot::Int(0), Slot::Int(0), Slot::Int(1), Slot::Real(1000.0), Slot::Int(0)];
            r.read(&mut t)?;
            a.thermal = Some(ThermalCards {
                tname: s[2].text().to_string(),
                iza,
                mti: t[0].int(),
                nbint: t[1].int(),
                mte: t[2].int(),
                ielas: t[3].int(),
                nmix: t[4].int(),
                emax: t[5].real(),
                iwt: t[6].int(),
            });
        }
        3 => {
            let mut s = [Slot::Int(0), Slot::Real(300.0)];
            r.read(&mut s)?;
            a.matd = s[0].int();
            a.tempd = s[1].real();
        }
        4 | 5 => {
            a.matd = ints(r, &[0])?[0];
        }
        _ => {}
    }
    Ok(a)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Upstream deck 02's GROUPR cards, shortened (`ntemp = 3`): one card-9 list
    /// per temperature, then card 10 (`groupr.f90:586-630`, `:993`). Before
    /// gh:#1369 round 3 the second list's first card was read as card 10.
    #[test]
    fn groupr_reads_one_card9_list_per_temperature() {
        let deck = " -21 -24 0 -25/\n 1050 5 0 4 3 3 7 1/\n '94-pu-238'/\n 300. 900. 2100./\n \
            1.e10 1.e5 1.e4 1000. 100. 10. 1/\n .1 0.025 0.8208e06 1.4e06/\n 3 1 'total'/\n 6 -59 'c'/\n 0/\n \
            3 1 'total'/\n 3 2 'elastic'/\n 0/\n 3 1 'total'/\n 6 2 'elastic'/\n 0/\n 0/\nccccr\n";
        let mut r = CardReader::new(deck);
        let job = read_groupr(&mut r).unwrap();
        assert_eq!(job.input.reactions.len(), 2);
        assert_eq!(job.input.reactions[1].mtd, -59);
        assert_eq!(job.later_temperatures.len(), 2);
        let pairs = |v: &[ReactionRequest]| v.iter().map(|q| (q.mfd, q.mtd)).collect::<Vec<_>>();
        assert_eq!(pairs(&job.later_temperatures[0].0), vec![(3, 1), (3, 2)]);
        assert_eq!(pairs(&job.later_temperatures[1].0), vec![(3, 1), (6, 2)]);
        assert!(job.more_materials.is_empty());
        assert_eq!(job.input.sigma_zeros.len(), 7);
    }
}
