//! Parse the MF=1/MT=451 general information section.
//!
//! The first three CONT records of MF=1/MT=451 carry the material header used
//! throughout RECONR: ZA, AWR, LRP (resonance flag), LFI (fissile), and the
//! maximum evaluation energy. Ported from the MF=1 processing block in NJOY2016
//! `reconr.f90` (lines ~219–275).

use crate::{
    endf::{records::SectionCursor, tape::Section},
    NjoyError,
};

/// Properties of a material read from MF=1/MT=451.
///
/// Corresponds to the first three CONT records of MF=1/MT=451 in ENDF-6 format:
/// - CONT 1: ZA, AWR, LRP, LFI, NLIB, NMOD
/// - CONT 2: ELIS, STA, LIS, LISO, 0, NFOR
/// - CONT 3: AWI, EMAX, LREL, 0, NSUB, NVER
#[derive(Debug, Clone)]
pub struct MaterialInfo {
    /// Z×1000 + A (e.g. 128 for H-2, 9228 for U-235).
    pub za: f64,
    /// Atomic weight ratio: nuclide mass / neutron mass.
    pub awr: f64,
    /// Resonance parameter flag: 0 = no resonances, 1 = parameters present,
    /// 2 = no parameters but potential scattering radius AP is given.
    pub lrp: i32,
    /// Fissile flag: 0 = non-fissile, 1 = fissile.
    pub lfi: i32,
    /// Library type code (0 = ENDF/B).
    pub nlib: i32,
    /// Target excitation energy \[eV\] (0 for ground state).
    pub elis: f64,
    /// ENDF-6 format version code.
    pub nfor: i32,
    /// Upper energy limit of the evaluation \[eV\].
    pub emax: f64,
}

/// Parse MF=1/MT=451 and return the material header.
pub fn parse_material_info(sec: &Section) -> Result<MaterialInfo, NjoyError> {
    let mut cur = SectionCursor::new(&sec.rows);

    // CONT 1: ZA, AWR, LRP, LFI, NLIB, NMOD
    let c1 = cur.read_cont()?;
    let za = c1.c1;
    let awr = c1.c2;
    let lrp = c1.l1;
    let lfi = c1.l2;
    let nlib = c1.n1;

    // CONT 2: ELIS, STA, LIS, LISO, 0, NFOR
    let c2 = cur.read_cont()?;
    let elis = c2.c1;
    let nfor = c2.n2;

    // CONT 3: AWI, EMAX, LREL, 0, NSUB, NVER
    let c3 = cur.read_cont()?;
    let emax = c3.c2;

    Ok(MaterialInfo {
        za,
        awr,
        lrp,
        lfi,
        nlib,
        elis,
        nfor,
        emax,
    })
}
