// Ported from NJOY2016 `src/dtfr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! DTFR user-input card model (`subroutine ruin`, `dtfr.f90:590-767`).
//!
//! Faithful, line-traceable translation of the free-format card sequence DTFR
//! reads to configure a run that reformats GROUPR **GENDF** multigroup data into
//! DTF-IV card-image tables for discrete-ordinates (Sₙ) codes.
//!
//! ## Card summary (`dtfr.f90:75-133`)
//!
//! | Card | Fortran vars | Condition |
//! |------|--------------|-----------|
//! | 1 | `nin nout npend nplot` | always |
//! | 2 | `iprint ifilm iedit` | always |
//! | 3 | `nlmax ng iptotl ipingp itabl ned ntherm` | `iedit==0` |
//! | 3a | `mti mtc nlc` | `iedit==0` and `ntherm!=0` |
//! | 4 | edit names `hednam(iptotl-3)` | `iedit==0`, `ned!=0` |
//! | 5 | `jped mted mult` triplets × `ned` | `iedit==0`, `ned!=0` |
//! | 6 | `nlmax ng` (CLAW) | `iedit==1` |
//! | 7 | `nptabl ngp` | always |
//! | 8 | `hisnam matd jsigz dtemp` | repeat; empty card ends |
//!
//! Field names keep the upstream Fortran identifiers in their doc comments so a
//! discrepancy can be localised to a specific `read(nsysi,*)` statement.

/// Print control (`iprint`), card 2 (`dtfr.f90:84`). Default [`PrintOption::Minimum`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrintOption {
    /// `0`: minimum printout. Default.
    Minimum,
    /// `1`: maximum printout (tables echoed to the listing).
    Maximum,
}

impl PrintOption {
    /// Return the Fortran `iprint` integer (0/1).
    pub fn to_iprint(self) -> i32 {
        match self {
            Self::Minimum => 0,
            Self::Maximum => 1,
        }
    }
    /// Parse `iprint` (0/1). Returns `None` otherwise.
    pub fn from_iprint(v: i32) -> Option<Self> {
        match v {
            0 => Some(Self::Minimum),
            1 => Some(Self::Maximum),
            _ => None,
        }
    }
}

/// Film/plot control (`ifilm`), card 2 (`dtfr.f90:85-86`). Default
/// [`FilmOption::None`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilmOption {
    /// `0`: no plots. Default.
    None,
    /// `1`: plots, one per frame.
    OnePerFrame,
    /// `2`: plots, four per frame.
    FourPerFrame,
}

impl FilmOption {
    /// Return the Fortran `ifilm` integer (0/1/2).
    pub fn to_ifilm(self) -> i32 {
        match self {
            Self::None => 0,
            Self::OnePerFrame => 1,
            Self::FourPerFrame => 2,
        }
    }
    /// Parse `ifilm` (0/1/2). Returns `None` otherwise.
    pub fn from_ifilm(v: i32) -> Option<Self> {
        match v {
            0 => Some(Self::None),
            1 => Some(Self::OnePerFrame),
            2 => Some(Self::FourPerFrame),
            _ => None,
        }
    }
}

/// Edit control (`iedit`), card 2 (`dtfr.f90:87`). Selects DTF layout style.
/// Default [`EditOption::InTable`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditOption {
    /// `0`: edits in the table (user-defined layout, cards 3-5). Default.
    InTable,
    /// `1`: separate CLAW-format tables (standard layout, card 6).
    Separate,
}

impl EditOption {
    /// Return the Fortran `iedit` integer (0/1).
    pub fn to_iedit(self) -> i32 {
        match self {
            Self::InTable => 0,
            Self::Separate => 1,
        }
    }
    /// Parse `iedit` (0/1). Returns `None` otherwise.
    pub fn from_iedit(v: i32) -> Option<Self> {
        match v {
            0 => Some(Self::InTable),
            1 => Some(Self::Separate),
            _ => None,
        }
    }
}

/// Card 1 unit assignments (`dtfr.f90:75-82`, read at `:630`).
///
/// Logical unit numbers; a negative number denotes a binary tape, `0` "absent".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct UnitAssignments {
    /// `nin` — input GENDF unit from GROUPR (binary) (`dtfr.f90:76`).
    pub nin: i32,
    /// `nout` — output unit for the DTF tables (coded). Default 0 (none)
    /// (`dtfr.f90:77-78`).
    pub nout: i32,
    /// `npend` — input PENDF unit for point plots. Default 0 (`dtfr.f90:79-80`).
    pub npend: i32,
    /// `nplot` — output plot-info unit for the plotr module. Default 0
    /// (`dtfr.f90:81-82`).
    pub nplot: i32,
}

/// One edit specification triplet (card 5, `dtfr.f90:108-114`, read at `:715`).
///
/// Repeated `ned` times. Positions and reaction types may appear more than once.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EditSpec {
    /// `jpos` (`jped`) — position of the edit quantity in the DTF table
    /// (`dtfr.f90:112`).
    pub jpos: i32,
    /// `mt` (`mted`) — ENDF reaction number contributing to this edit
    /// (`dtfr.f90:113`).
    pub mt: i32,
    /// `mult` (`multed`) — multiplicity applied when adding this MT
    /// (`dtfr.f90:114`).
    pub mult: i32,
}

/// Card 3a thermal MT specification (`dtfr.f90:99-103`, read at `:661`).
/// Present only when `ntherm != 0`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ThermalSpec {
    /// `mti` — MT for thermal incoherent data (`dtfr.f90:101`).
    pub mti: i32,
    /// `mtc` — MT for thermal coherent data. Default 0 (`dtfr.f90:102`).
    pub mtc: i32,
    /// `nlc` — number of coherent Legendre orders. Default 0 (`dtfr.f90:103`).
    pub nlc: i32,
}

/// One material-description card (card 8, `dtfr.f90:126-132`, read at `:170`).
///
/// Repeated until an empty card (`matd == 0`) terminates the run.
#[derive(Debug, Clone, PartialEq)]
pub struct MaterialDesc {
    /// `hisnam` — 6-character isotope name (`dtfr.f90:129`).
    pub hisnam: String,
    /// `matd` — ENDF material number. `0` terminates (`dtfr.f90:130`).
    pub matd: i32,
    /// `jsigz` (`jzd`) — index of the desired sigma-zero (dilution). Default 1
    /// (`dtfr.f90:131`).
    pub jsigz: i32,
    /// `dtemp` — desired temperature in **kelvin**. Default 300
    /// (`dtfr.f90:132`).
    pub dtemp: f64,
}

/// Neutron-table geometry (card 3, `dtfr.f90:91-98`, read at `:655`).
///
/// Only read for `iedit==0`; for `iedit==1` the CLAW standard values are set
/// programmatically (see [`DtfrInput::claw_defaults`]).
#[derive(Debug, Clone, PartialEq)]
pub struct NeutronTables {
    /// `nlmax` — number of neutron tables (Legendre orders) desired
    /// (`dtfr.f90:92`).
    pub nlmax: i32,
    /// `ng` — number of neutron groups (`dtfr.f90:93`).
    pub ng: i32,
    /// `iptotl` — position of the total cross section in the table
    /// (`dtfr.f90:94`).
    pub iptotl: i32,
    /// `ipingp` — position of the in-group scattering cross section
    /// (`dtfr.f90:95`). Must exceed `iptotl` (`dtfr.f90:656`).
    pub ipingp: i32,
    /// `itabl` — desired neutron table length (`dtfr.f90:96`).
    pub itabl: i32,
    /// `ned` — number of edit-table entries. Default 0 (`dtfr.f90:97`).
    pub ned: i32,
    /// `ntherm` — number of thermal groups. Default 0 (`dtfr.f90:98`).
    pub ntherm: i32,
}

/// Complete DTFR user-input deck (`dtfr.f90:75-133`).
///
/// Aggregates every card DTFR reads. Option-conditional cards are optional/enum
/// fields whose presence mirrors the Fortran `read` guards. All temperatures are
/// in **kelvin**; MT/position/group fields are dimensionless integers.
#[derive(Debug, Clone, PartialEq)]
pub struct DtfrInput {
    /// Card 1 — unit assignments.
    pub units: UnitAssignments,
    /// Card 2 — `iprint` print control.
    pub iprint: PrintOption,
    /// Card 2 — `ifilm` film control.
    pub ifilm: FilmOption,
    /// Card 2 — `iedit` edit control.
    pub iedit: EditOption,

    /// Card 3 — neutron-table geometry. For `iedit==1` this holds the CLAW
    /// standard values (see [`DtfrInput::claw_defaults`]).
    pub neutron: NeutronTables,
    /// Card 3a — thermal MTs (only when `neutron.ntherm != 0`). `None` otherwise.
    pub thermal: Option<ThermalSpec>,
    /// Card 4 — edit names, `iptotl-3` of them (only for `iedit==0`, `ned!=0`)
    /// (`dtfr.f90:104-107`).
    pub edit_names: Vec<String>,
    /// Card 5 — the `ned` edit specifications (only for `iedit==0`, `ned!=0`).
    pub edits: Vec<EditSpec>,

    /// Card 7 — `nptabl`: number of photon tables. Default 0 (`dtfr.f90:124`).
    pub nptabl: i32,
    /// Card 7 — `ngp`: number of photon groups. Default 0 (`dtfr.f90:125`).
    pub ngp: i32,

    /// Card 8 — the material descriptions to process, in order.
    pub materials: Vec<MaterialDesc>,
}

/// The 50 built-in reaction MTs for the CLAW standard edit table
/// (`kmted`, `dtfr.f90:603-606`). Used when `iedit==1`.
pub const CLAW_MTED: [i32; 50] = [
    2, 4, 16, 6, 7, 8, 9, 17, 102, 107, 103, 19, 20, 21, 18, 104, 105, 106, 111, 112, 113, 114,
    108, 109, 32, 35, 34, 22, 24, 23, 25, 28, 29, 30, 33, 36, 37, 38, 470, 471, 455, 300, 301, 443,
    443, 444, 452, 1, 0, 0,
];

/// The 50 built-in table positions for the CLAW standard edit table
/// (`kjped`, `dtfr.f90:607-609`).
pub const CLAW_JPED: [i32; 50] = [
    1, 2, 3, 3, 3, 3, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
    24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 39, 40, 41, 42, 43, 44, 45,
];

/// The 50 built-in multiplicities for the CLAW standard edit table
/// (`kmultd`, `dtfr.f90:610-612`). Note position 44 (`mt=443`, first copy) is 0.
pub const CLAW_MULTD: [i32; 50] = [
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 1, 1, 1, 1, 1, 1,
];

/// The 50 built-in 5-character edit names for the CLAW table
/// (`hmtid`, `dtfr.f90:613-621`).
pub const CLAW_HMTID: [&str; 50] = [
    "  els", "  ins", "  n2n", "  n3n", "  ngm", "  nal", "   np", " fdir", "  nnf", " n2nf",
    " ftot", "   nd", "   nt", " nhe3", "  n2p", "  npa", " nt2a", " nd2a", "  n2a", "  n3a",
    "  nnd", "nnd2a", "nnhe3", "  nna", " n2na", " nn3a", " n3na", "  nnp", " nn2a", "n2n2a",
    "  nnt", "nnt2a", "  n4n", " n3nf", "  chi", " chid", "  nud", "  phi", "theat", "kerma",
    "tdame", " nusf", " totl", "     ", "     ", "     ", "     ", "     ", "     ", "     ",
];

impl DtfrInput {
    /// Build the CLAW (`iedit==1`) standard neutron-table geometry and the
    /// standard 48-entry edit table (`dtfr.f90:665-684`).
    ///
    /// Sets `nlmax`/`ng` from the card-6 values, then `iptotl=46, ned=48,
    /// ipingp=iptotl+1, itabl=iptotl+ng`, and fills `edit_names`/`edits` from the
    /// [`CLAW_HMTID`]/[`CLAW_JPED`]/[`CLAW_MTED`]/[`CLAW_MULTD`] tables.
    pub fn claw_defaults(nlmax: i32, ng: i32) -> Self {
        let iptotl = 43 + 3;
        let ned = 48;
        let ipingp = iptotl + 1;
        let itabl = iptotl + ng;
        let nedpos = (iptotl - 3) as usize;
        let edit_names = CLAW_HMTID[..nedpos].iter().map(|s| s.to_string()).collect();
        let edits = (0..ned as usize)
            .map(|i| EditSpec { jpos: CLAW_JPED[i], mt: CLAW_MTED[i], mult: CLAW_MULTD[i] })
            .collect();
        Self {
            units: UnitAssignments::default(),
            iprint: PrintOption::Minimum,
            ifilm: FilmOption::None,
            iedit: EditOption::Separate,
            neutron: NeutronTables {
                nlmax,
                ng,
                iptotl,
                ipingp,
                itabl,
                ned,
                ntherm: 0,
            },
            thermal: None,
            edit_names,
            edits,
            nptabl: 0,
            ngp: 0,
            materials: Vec::new(),
        }
    }

    /// Append the three standard edits DTFR always adds after the user edits
    /// (`dtfr.f90:745-754`): `absorp` at `iptotl-2` (`mt=1000`), `nusigf` at
    /// `iptotl-1` (`mt=1000`), and `total` at `iptotl` (`mt=1`).
    ///
    /// Returns the three [`EditSpec`]s in Fortran order (`ned+1` absorp,
    /// `ned+2` nusigf, `ned+3` total).
    pub fn standard_edits(iptotl: i32) -> [EditSpec; 3] {
        [
            EditSpec { jpos: iptotl - 2, mt: 1000, mult: 1 }, // absorp (ned+1)
            EditSpec { jpos: iptotl - 1, mt: 1000, mult: 1 }, // nusigf (ned+2)
            EditSpec { jpos: iptotl, mt: 1, mult: 1 },        // total  (ned+3)
        ]
    }
}

impl Default for DtfrInput {
    /// NJOY-faithful defaults (`dtfr.f90:624-638`): units 0, `iprint=0`,
    /// `ifilm=0`, `iedit=0`, `nptabl=0`, `ngp=0`. Card-3 geometry must be set by
    /// the caller (there is no upstream default for it under `iedit==0`).
    fn default() -> Self {
        Self {
            units: UnitAssignments::default(),
            iprint: PrintOption::Minimum,
            ifilm: FilmOption::None,
            iedit: EditOption::InTable,
            neutron: NeutronTables {
                nlmax: 0,
                ng: 0,
                iptotl: 0,
                ipingp: 0,
                itabl: 0,
                ned: 0,
                ntherm: 0,
            },
            thermal: None,
            edit_names: Vec::new(),
            edits: Vec::new(),
            nptabl: 0,
            ngp: 0,
            materials: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Card-2 selector enums round-trip through their Fortran integers.
    ///
    /// **Methodology.** For each of `iprint`, `ifilm`, `iedit` assert
    /// `from_*(to_*(x)) == x` over the valid range and that out-of-range
    /// integers reject with `None` (`dtfr.f90:84-87`).
    ///
    /// **Result (2026-07-15, commit ac5adf5).** `iprint` 0/1, `ifilm` 0/1/2,
    /// `iedit` 0/1 all round-trip; `iprint(2)`, `ifilm(3)`, `iedit(2)` are
    /// `None`.
    #[test]
    fn selectors_round_trip() {
        for v in 0..=1 {
            assert_eq!(PrintOption::from_iprint(v).unwrap().to_iprint(), v);
            assert_eq!(EditOption::from_iedit(v).unwrap().to_iedit(), v);
        }
        for v in 0..=2 {
            assert_eq!(FilmOption::from_ifilm(v).unwrap().to_ifilm(), v);
        }
        assert!(PrintOption::from_iprint(2).is_none());
        assert!(FilmOption::from_ifilm(3).is_none());
        assert!(EditOption::from_iedit(2).is_none());
    }

    /// The CLAW (`iedit==1`) standard geometry reproduces `dtfr.f90:665-684`.
    ///
    /// **Methodology.** Call `claw_defaults(5, 30)` and assert the derived
    /// geometry: `iptotl=46, ned=48, ipingp=47, itabl=76`, `edit_names` length
    /// `iptotl-3 = 43`, `edits` length 48. Spot-check the built-in tables:
    /// entry 0 is `els`/pos 1/mt 2, entry 40 is `chid`? — actually entry 40
    /// (`kmted(41)=455`, delayed nubar) at pos 41; and the `mt=443`
    /// double-entry has `mult` 0 then 1 (`dtfr.f90:610-612`).
    ///
    /// **Result (2026-07-15).** `iptotl=46, ipingp=47, itabl=76, ned=48`,
    /// `edit_names.len()=43`, `edits.len()=48`; `edits[0]={1,2,1}`,
    /// `edits[43]={39,443,0}` (first mt=443, mult 0), `edits[44]={40,443,1}`
    /// (second mt=443, mult 1).
    #[test]
    fn claw_defaults_match_fortran() {
        let inp = DtfrInput::claw_defaults(5, 30);
        assert_eq!(inp.neutron.nlmax, 5);
        assert_eq!(inp.neutron.ng, 30);
        assert_eq!(inp.neutron.iptotl, 46);
        assert_eq!(inp.neutron.ipingp, 47);
        assert_eq!(inp.neutron.itabl, 76);
        assert_eq!(inp.neutron.ned, 48);
        assert_eq!(inp.edit_names.len(), 43);
        assert_eq!(inp.edits.len(), 48);
        assert_eq!(inp.edits[0], EditSpec { jpos: 1, mt: 2, mult: 1 });
        // the two mt=443 entries (dtfr.f90:605, 610-612): first mult 0, second 1
        assert_eq!(inp.edits[43], EditSpec { jpos: 39, mt: 443, mult: 0 });
        assert_eq!(inp.edits[44], EditSpec { jpos: 40, mt: 443, mult: 1 });
        assert_eq!(inp.edit_names[0], "  els");
    }

    /// The three standard edits are appended at the documented positions.
    ///
    /// **Methodology.** `dtfr.f90:745-754` appends absorp at `iptotl-2`
    /// (mt=1000), nusigf at `iptotl-1` (mt=1000), total at `iptotl` (mt=1). With
    /// `iptotl=46` assert positions 44/45/46 and MTs.
    ///
    /// **Result (2026-07-15).** absorp={44,1000,1}, nusigf={45,1000,1},
    /// total={46,1,1}.
    #[test]
    fn standard_edits_positions() {
        let se = DtfrInput::standard_edits(46);
        assert_eq!(se[0], EditSpec { jpos: 44, mt: 1000, mult: 1 }); // absorp
        assert_eq!(se[1], EditSpec { jpos: 45, mt: 1000, mult: 1 }); // nusigf
        assert_eq!(se[2], EditSpec { jpos: 46, mt: 1, mult: 1 });    // total
    }

    /// Built-in CLAW constant tables have the expected length and key entries.
    ///
    /// **Methodology.** `dtfr.f90:603-621` declares four length-50
    /// `parameter` arrays. Assert lengths and a couple of cross-checked entries
    /// (`kmted(15)=18` fission at index 14; `kjped(9)=5` at index 8).
    ///
    /// **Result (2026-07-15).** All four arrays length 50; `CLAW_MTED[14]=18`,
    /// `CLAW_JPED[8]=5`, `CLAW_HMTID[34]="  chi"`, `CLAW_MULTD[43]=0`.
    #[test]
    fn claw_constant_tables() {
        assert_eq!(CLAW_MTED.len(), 50);
        assert_eq!(CLAW_JPED.len(), 50);
        assert_eq!(CLAW_MULTD.len(), 50);
        assert_eq!(CLAW_HMTID.len(), 50);
        assert_eq!(CLAW_MTED[14], 18);
        assert_eq!(CLAW_JPED[8], 5);
        assert_eq!(CLAW_HMTID[34], "  chi");
        assert_eq!(CLAW_MULTD[43], 0);
    }
}
