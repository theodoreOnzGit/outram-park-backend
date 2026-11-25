//! # A simplified EBR-II full core, Run 138B (SHRT-45R) layout, fresh fuel (gh:#1762)
//!
//! Maintainer, 2026-11-25: "Pls also do an ebr ii full core mc simulation for
//! the EBR Ii page"; "Do it like the msre"; option **A**, "I know this is a
//! simplified EBR ii core but that's ok". The whole 637-position core in one
//! Monte Carlo model, for the EBR-II simulator's MC page (as MSRE's N-1
//! model is for its k_eff tab, [`crate::msre_n1`]).
//!
//! > Research, education and V&V only. Not for reactor operation, licensing
//! > or any safety decision (`RESPONSIBLE_USE.md`). AI draft, unreviewed.
//! > **A SIMPLIFIED, FRESH CORE: not the Run 138B burnup state.** Its `k` is
//! > not a prediction of EBR-II's critical state and must not be compared
//! > with it as if it were.
//!
//! ## What is modelled
//!
//! - **Layout:** every position of Run 138B as transcribed on gh:#1313
//!   (`reference-data/ebr2/ebr2_loading_maps.csv`, ARC-228 Fig. 3.1 and
//!   ARC-226 Fig. 4.5; AI-read, unreviewed). [`RUN_138B`] holds each
//!   position's axial coordinates `(q, r)` and its kind ([`SaKind`]);
//!   `tests::the_table_is_the_transcription` pins it to the CSV.
//! - **Lattice:** hexagonal, pitch 5.8929 cm (ARC-228 Table 3.3, p.24), 16
//!   rings, centre `x = p (q + r/2)`, `y = −p r √3/2` (the CSV's convention,
//!   page-up as +y), as a CSG `HexLattice` (OpenMC's port). Positions the
//!   truncated rings 15 and 16 do not hold are sodium.
//! - **Each subassembly is homogenised** over its lattice cell (duct and
//!   the sodium between ducts included) at ARC-228 Table 3.3's volume
//!   fractions (pp.23-24):
//!
//! | kind | fuel alloy | steel | sodium | source |
//! |---|---|---|---|---|
//! | driver (47 + 23 high-flow + 3 fuelled experiments) | U-5Fs 26.92 % | 22.29 % | 50.79 % | Table 3.3 |
//! | half-worth driver (13) | U-5Fs **13.46 %** | **35.75 %** | 50.79 % | **ASSUMED**: half the driver's fuel volume as steel |
//! | control and safety rod fuel, XX09 | U-5Fs 18.04 % | 22.69 % | 59.27 % | Table 3.3 |
//! | high-worth control rod poison | B₄C **18.04 %** | 22.69 % | 59.27 % | **ASSUMED**: B₄C (2.40 g/cm³, ARC-228 p.20) in the fuel's volume |
//! | outer blanket (330) | depleted U 61.13 % | 17.64 % | 21.23 % | Table 3.3 |
//! | reflector, SST dummies, unfuelled experiments | — | **78.77 %** | **21.23 %** | **ASSUMED**: the blanket's layout with steel in place of uranium |
//!
//! - **Compositions:** fresh U-5Fs at 67 wt% U-235 and 18.2 g/cm³, and 316SS
//!   (PNNL-15870 material 300) and sodium (0.971 g/cm³), exactly as
//!   [`crate::ebr2_fresh_driver`] builds them, arm A (Ru, Rh, Pd, Nb
//!   omitted: no tape). All steel is 316SS (the blanket and reflector's
//!   304SS is approximated by it). The blanket's uranium is **pure U-238 at
//!   19.05 g/cm³: ASSUMED**, uranium metal's density; ARC-228 gives "~0.0"
//!   wt% U-235 and no density.
//! - **Axially:** every column spans 139.7 cm (Table 3.3's radial
//!   blanket and reflector length), **centred on the core mid-plane:
//!   ASSUMED**. The driver fuel is 34.29 cm (fresh, Table 3.3); above and
//!   below it the driver column is the reflector mix (**ASSUMED**: the real
//!   upper and lower reflectors, 38.9 and 57.2 cm, are not given as
//!   compositions).
//! - **Rods** (ARC-228 pp.19-20): the high-worth control rods' fuel bundle
//!   sits below a B₄C bundle (**ASSUMED** 34.29 cm long); lowering the rod
//!   moves the fuel out of the core and the B₄C in. [`Study::control_insertion`]
//!   is the drop of the fuel bundle's bottom below the drivers' fuel bottom
//!   as a fraction of the 43.815 cm stroke; SHRT-45R started at 21.06 cm
//!   ([`SHRT45R_CONTROL_INSERTION`]). The regular control rod (5D1) moves
//!   with them and carries no B₄C. The two safety rods sit 15.97 cm low.
//! - **Temperature:** one, every material ([`Study::temperature_k`]),
//!   293.15 K by default: the dimensions and densities are ARC-228's "at
//!   20 C", as for the fresh driver. A hotter study broadens the nuclear
//!   data only; nothing expands.
//!
//! **Not modelled:** burnup (the Appendix B/C compositions are not held,
//! gh:#1313 A6), the pins (homogenised), the plenums and end fittings, the
//! upper and lower axial reflectors' real make-up, thermal expansion, the
//! vessel and the pool beyond the cylinder of radius [`R_OUTER_CM`].
//!
//! The geometry is drawn (the drawing rule) by
//! `examples/ebr2_core_images.rs` into
//! `verification_and_validation/ebr2_core/`.

use std::path::Path;

use outram_mc_libs::geometry::cell::{Cell, CellFill, HalfSpaceSense, RegionToken};
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::lattice::{HexLattice, HexOrientation, Lattice};
use outram_mc_libs::geometry::position::{Direction, Position};
use outram_mc_libs::geometry::surface::{BoundaryType, SurfaceKind, ZCylinder, ZPlane};
use outram_mc_libs::geometry::universe::Universe;
use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::model::{AtomicMasses, Element, Isotope, AVOGADRO_PER_MOL};
use outram_mc_libs::physics::transport_csg::SourceBox;

use crate::ebr2_fresh_driver::{self as fd, GapTreatment};

/// Subassembly pitch, cm (ARC-228 Table 3.3, p.24).
pub const PITCH_CM: f64 = 5.8929;
/// Rings in the grid (ARC-226 p.38).
pub const RINGS: usize = 16;
/// Column length, cm (ARC-228 Table 3.3: radial reflector and blanket).
pub const COLUMN_CM: f64 = 139.7;
/// Fresh driver fuel length, cm (ARC-228 Table 3.3, 0 at.% burnup).
pub const FUEL_CM: f64 = 34.29;
/// Control-rod stroke, cm (ARC-228 p.19).
pub const STROKE_CM: f64 = 43.815;
/// The control rods' fuel-bundle bottom below the drivers' at the start of
/// SHRT-45R, cm (ARC-228 p.19).
pub const SHRT45R_CONTROL_DROP_CM: f64 = 21.06;
/// [`SHRT45R_CONTROL_DROP_CM`] as a fraction of the stroke.
pub const SHRT45R_CONTROL_INSERTION: f64 = SHRT45R_CONTROL_DROP_CM / STROKE_CM;
/// The safety rods' fuel-bundle bottom below the drivers', cm (ARC-228 p.20).
pub const SAFETY_DROP_CM: f64 = 15.97;
/// B₄C density, g/cm³ (ARC-228 p.20).
pub const B4C_G_CC: f64 = 2.40;
/// Blanket uranium density, g/cm³: **ASSUMED** (module docs).
pub const DU_G_CC: f64 = 19.05;
/// The outer cylinder, cm (vacuum outside): past ring 16's outermost hex
/// corner (15 p + p/√3 = 91.8 cm).
pub const R_OUTER_CM: f64 = 96.0;
/// The default temperature, K (ARC-228's "at 20 C").
pub const TEMPERATURE_K: f64 = 293.15;

/// A subassembly kind, by what its column holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaKind {
    /// Driver, high-flow driver, or a fuelled experiment (X402A, X412,
    /// C2776A XETAGS).
    Driver,
    /// Half-worth (partial) driver.
    HalfWorth,
    /// High-worth control rod: fuel below B₄C.
    HighWorthControl,
    /// The regular-worth control rod at 5D1: fuel, no B₄C.
    RegularControl,
    /// Safety rod: fuel, no B₄C.
    Safety,
    /// XX09, the fuelled instrumented subassembly (61-pin fractions).
    Xx09,
    /// Outer blanket.
    Blanket,
    /// Steel reflector, SST dummy, unfuelled experiment or XX10.
    Reflector,
}

impl SaKind {
    /// The kind of a [`RUN_138B`] code letter.
    pub fn from_code(c: u8) -> Option<Self> {
        Some(match c {
            b'D' => SaKind::Driver,
            b'H' => SaKind::HalfWorth,
            b'C' => SaKind::HighWorthControl,
            b'R' => SaKind::RegularControl,
            b'S' => SaKind::Safety,
            b'X' => SaKind::Xx09,
            b'B' => SaKind::Blanket,
            b'F' => SaKind::Reflector,
            _ => return None,
        })
    }

    /// Every kind, in universe order (universe `i + 1`; 0 is the root).
    pub const ALL: [SaKind; 8] = [
        SaKind::Driver,
        SaKind::HalfWorth,
        SaKind::HighWorthControl,
        SaKind::RegularControl,
        SaKind::Safety,
        SaKind::Xx09,
        SaKind::Blanket,
        SaKind::Reflector,
    ];
}

/// The homogenised materials, in [`Study::materials`] order.
pub const MATERIALS: [&str; 7] = [
    "Driver (U-5Fs / 316SS / Na)",
    "Half-worth driver (ASSUMED)",
    "Control / safety rod fuel (61-pin)",
    "Control rod B4C (ASSUMED)",
    "Outer blanket (depleted U)",
    "Reflector (316SS / Na, ASSUMED)",
    "Sodium",
];
/// Indices into [`MATERIALS`].
pub const DRIVER: usize = 0;
pub const HALF: usize = 1;
pub const ROD_FUEL: usize = 2;
pub const B4C: usize = 3;
pub const BLANKET: usize = 4;
pub const REFLECTOR: usize = 5;
pub const SODIUM: usize = 6;

/// `(nuclide, ENDF/B-VIII.0 tape)`, paths relative to `reference-data/endf/`
/// (Ni, Zr and Mo from the `reference-data/ace` submodule's `endf/`, as
/// [`crate::msre_n1`] reads them). The fresh driver's tapes
/// ([`fd::tape_paths`]) plus boron for the B₄C.
pub const NUCLIDES: [(&str, &str); 41] = [
    ("U235", "n-092_U_235-ENDF8.0.endf"),
    ("U238", "n-092_U_238.endf"),
    ("Na23", "n-011_Na_023-ENDF8.0.endf"),
    ("Fe54", "n-026_Fe_054-ENDF8.0.endf"),
    ("Fe56", "n-026_Fe_056-ENDF8.0.endf"),
    ("Fe57", "n-026_Fe_057-ENDF8.0.endf"),
    ("Fe58", "n-026_Fe_058-ENDF8.0.endf"),
    ("Cr50", "n-024_Cr_050-ENDF8.0.endf"),
    ("Cr52", "n-024_Cr_052-ENDF8.0.endf"),
    ("Cr53", "n-024_Cr_053-ENDF8.0.endf"),
    ("Cr54", "n-024_Cr_054-ENDF8.0.endf"),
    ("Mn55", "n-025_Mn_055-ENDF8.0.endf"),
    ("Si28", "n-014_Si_028-ENDF8.0.endf"),
    ("Si29", "n-014_Si_029-ENDF8.0.endf"),
    ("Si30", "n-014_Si_030-ENDF8.0.endf"),
    ("C12", "n-006_C_012-ENDF8.0.endf"),
    ("C13", "n-006_C_013-ENDF8.0.endf"),
    ("P31", "n-015_P_031-ENDF8.0.endf"),
    ("S32", "n-016_S_032-ENDF8.0.endf"),
    ("S33", "n-016_S_033-ENDF8.0.endf"),
    ("S34", "n-016_S_034-ENDF8.0.endf"),
    ("S36", "n-016_S_036-ENDF8.0.endf"),
    ("B10", "n-005_B_010-ENDF8.0.endf"),
    ("B11", "n-005_B_011-ENDF8.0.endf"),
    ("Ni58", "../ace/endf/endf-b-viii.0/n-028_Ni_058-ENDF8.0.endf"),
    ("Ni60", "../ace/endf/endf-b-viii.0/n-028_Ni_060-ENDF8.0.endf"),
    ("Ni61", "../ace/endf/endf-b-viii.0/n-028_Ni_061-ENDF8.0.endf"),
    ("Ni62", "../ace/endf/endf-b-viii.0/n-028_Ni_062-ENDF8.0.endf"),
    ("Ni64", "../ace/endf/endf-b-viii.0/n-028_Ni_064-ENDF8.0.endf"),
    ("Zr90", "../ace/endf/endf-b-viii.0/n-040_Zr_090-ENDF8.0.endf"),
    ("Zr91", "../ace/endf/endf-b-viii.0/n-040_Zr_091-ENDF8.0.endf"),
    ("Zr92", "../ace/endf/endf-b-viii.0/n-040_Zr_092-ENDF8.0.endf"),
    ("Zr94", "../ace/endf/endf-b-viii.0/n-040_Zr_094-ENDF8.0.endf"),
    ("Zr96", "../ace/endf/endf-b-viii.0/n-040_Zr_096-ENDF8.0.endf"),
    ("Mo92", "../ace/endf/endf-b-viii.0/n-042_Mo_092-ENDF8.0.endf"),
    ("Mo94", "../ace/endf/endf-b-viii.0/n-042_Mo_094-ENDF8.0.endf"),
    ("Mo95", "../ace/endf/endf-b-viii.0/n-042_Mo_095-ENDF8.0.endf"),
    ("Mo96", "../ace/endf/endf-b-viii.0/n-042_Mo_096-ENDF8.0.endf"),
    ("Mo97", "../ace/endf/endf-b-viii.0/n-042_Mo_097-ENDF8.0.endf"),
    ("Mo98", "../ace/endf/endf-b-viii.0/n-042_Mo_098-ENDF8.0.endf"),
    ("Mo100", "../ace/endf/endf-b-viii.0/n-042_Mo_100-ENDF8.0.endf"),
];

/// **The Run 138B loading**, `(q, r, kind code)` per position in the CSV's
/// order (ring by ring), from `reference-data/ebr2/ebr2_loading_maps.csv`
/// (gh:#1313). Codes ([`SaKind::from_code`]): D driver (driver, high-flow,
/// and the fuelled experiments 2776A, X402A, X412), H half-worth, C
/// high-worth control, R regular control (5D1), S safety, X XX09, B outer
/// blanket, F reflector (SST reflector, SST dummy, X320C, XY-16, XX10).
pub const RUN_138B: [(i32, i32, u8); 637] = [
    // ring 1
    (0, 0, b'H'),
    // ring 2
    (-1, 0, b'H'), (0, -1, b'F'), (1, -1, b'H'), (1, 0, b'F'), (0, 1, b'H'), (-1, 1, b'F'),
    // ring 3
    (-2, 0, b'S'), (-1, -1, b'H'), (0, -2, b'D'), (1, -2, b'F'), (2, -2, b'D'), (2, -1, b'H'), (2, 0, b'S'), (1, 1, b'H'),
    (0, 2, b'D'), (-1, 2, b'H'), (-2, 2, b'D'), (-2, 1, b'F'),
    // ring 4
    (-3, 0, b'D'), (-2, -1, b'D'), (-1, -2, b'D'), (0, -3, b'D'), (1, -3, b'D'), (2, -3, b'D'), (3, -3, b'D'), (3, -2, b'D'),
    (3, -1, b'D'), (3, 0, b'D'), (2, 1, b'F'), (1, 2, b'D'), (0, 3, b'D'), (-1, 3, b'D'), (-2, 3, b'D'), (-3, 3, b'D'),
    (-3, 2, b'D'), (-3, 1, b'D'),
    // ring 5
    (-4, 0, b'C'), (-3, -1, b'D'), (-2, -2, b'F'), (-1, -3, b'D'), (0, -4, b'C'), (1, -4, b'D'), (2, -4, b'C'), (3, -4, b'D'),
    (4, -4, b'F'), (4, -3, b'D'), (4, -2, b'C'), (4, -1, b'D'), (4, 0, b'R'), (3, 1, b'D'), (2, 2, b'X'), (1, 3, b'D'),
    (0, 4, b'C'), (-1, 4, b'D'), (-2, 4, b'C'), (-3, 4, b'D'), (-4, 4, b'C'), (-4, 3, b'D'), (-4, 2, b'F'), (-4, 1, b'D'),
    // ring 6
    (-5, 0, b'D'), (-4, -1, b'D'), (-3, -2, b'D'), (-2, -3, b'D'), (-1, -4, b'D'), (0, -5, b'D'), (1, -5, b'D'), (2, -5, b'D'),
    (3, -5, b'H'), (4, -5, b'D'), (5, -5, b'D'), (5, -4, b'D'), (5, -3, b'D'), (5, -2, b'D'), (5, -1, b'H'), (5, 0, b'D'),
    (4, 1, b'D'), (3, 2, b'D'), (2, 3, b'D'), (1, 4, b'D'), (0, 5, b'D'), (-1, 5, b'D'), (-2, 5, b'D'), (-3, 5, b'H'),
    (-4, 5, b'D'), (-5, 5, b'D'), (-5, 4, b'D'), (-5, 3, b'H'), (-5, 2, b'D'), (-5, 1, b'D'),
    // ring 7
    (-6, 0, b'F'), (-5, -1, b'F'), (-4, -2, b'F'), (-3, -3, b'F'), (-2, -4, b'H'), (-1, -5, b'D'), (0, -6, b'D'), (1, -6, b'F'),
    (2, -6, b'D'), (3, -6, b'F'), (4, -6, b'D'), (5, -6, b'F'), (6, -6, b'D'), (6, -5, b'F'), (6, -4, b'D'), (6, -3, b'D'),
    (6, -2, b'F'), (6, -1, b'F'), (6, 0, b'F'), (5, 1, b'F'), (4, 2, b'D'), (3, 3, b'D'), (2, 4, b'F'), (1, 5, b'F'),
    (0, 6, b'D'), (-1, 6, b'F'), (-2, 6, b'F'), (-3, 6, b'D'), (-4, 6, b'D'), (-5, 6, b'F'), (-6, 6, b'F'), (-6, 5, b'F'),
    (-6, 4, b'F'), (-6, 3, b'D'), (-6, 2, b'D'), (-6, 1, b'F'),
    // ring 8
    (-7, 0, b'F'), (-6, -1, b'F'), (-5, -2, b'F'), (-4, -3, b'F'), (-3, -4, b'F'), (-2, -5, b'F'), (-1, -6, b'F'), (0, -7, b'F'),
    (1, -7, b'F'), (2, -7, b'F'), (3, -7, b'F'), (4, -7, b'F'), (5, -7, b'F'), (6, -7, b'F'), (7, -7, b'F'), (7, -6, b'F'),
    (7, -5, b'F'), (7, -4, b'F'), (7, -3, b'F'), (7, -2, b'F'), (7, -1, b'F'), (7, 0, b'F'), (6, 1, b'F'), (5, 2, b'F'),
    (4, 3, b'F'), (3, 4, b'F'), (2, 5, b'F'), (1, 6, b'F'), (0, 7, b'F'), (-1, 7, b'F'), (-2, 7, b'F'), (-3, 7, b'F'),
    (-4, 7, b'F'), (-5, 7, b'F'), (-6, 7, b'F'), (-7, 7, b'F'), (-7, 6, b'F'), (-7, 5, b'F'), (-7, 4, b'F'), (-7, 3, b'F'),
    (-7, 2, b'F'), (-7, 1, b'F'),
    // ring 9
    (-8, 0, b'F'), (-7, -1, b'F'), (-6, -2, b'F'), (-5, -3, b'F'), (-4, -4, b'F'), (-3, -5, b'F'), (-2, -6, b'F'), (-1, -7, b'F'),
    (0, -8, b'F'), (1, -8, b'F'), (2, -8, b'F'), (3, -8, b'F'), (4, -8, b'F'), (5, -8, b'F'), (6, -8, b'F'), (7, -8, b'F'),
    (8, -8, b'F'), (8, -7, b'F'), (8, -6, b'F'), (8, -5, b'F'), (8, -4, b'F'), (8, -3, b'F'), (8, -2, b'F'), (8, -1, b'F'),
    (8, 0, b'F'), (7, 1, b'F'), (6, 2, b'F'), (5, 3, b'F'), (4, 4, b'F'), (3, 5, b'F'), (2, 6, b'F'), (1, 7, b'F'),
    (0, 8, b'F'), (-1, 8, b'F'), (-2, 8, b'F'), (-3, 8, b'F'), (-4, 8, b'F'), (-5, 8, b'F'), (-6, 8, b'F'), (-7, 8, b'F'),
    (-8, 8, b'F'), (-8, 7, b'F'), (-8, 6, b'F'), (-8, 5, b'F'), (-8, 4, b'F'), (-8, 3, b'F'), (-8, 2, b'F'), (-8, 1, b'F'),
    // ring 10
    (-9, 0, b'F'), (-8, -1, b'F'), (-7, -2, b'F'), (-6, -3, b'F'), (-5, -4, b'F'), (-4, -5, b'F'), (-3, -6, b'F'), (-2, -7, b'F'),
    (-1, -8, b'F'), (0, -9, b'F'), (1, -9, b'F'), (2, -9, b'F'), (3, -9, b'F'), (4, -9, b'F'), (5, -9, b'F'), (6, -9, b'F'),
    (7, -9, b'F'), (8, -9, b'F'), (9, -9, b'F'), (9, -8, b'F'), (9, -7, b'F'), (9, -6, b'F'), (9, -5, b'F'), (9, -4, b'F'),
    (9, -3, b'F'), (9, -2, b'F'), (9, -1, b'F'), (9, 0, b'F'), (8, 1, b'F'), (7, 2, b'F'), (6, 3, b'F'), (5, 4, b'F'),
    (4, 5, b'F'), (3, 6, b'F'), (2, 7, b'F'), (1, 8, b'F'), (0, 9, b'F'), (-1, 9, b'F'), (-2, 9, b'F'), (-3, 9, b'F'),
    (-4, 9, b'F'), (-5, 9, b'F'), (-6, 9, b'F'), (-7, 9, b'F'), (-8, 9, b'F'), (-9, 9, b'F'), (-9, 8, b'F'), (-9, 7, b'F'),
    (-9, 6, b'F'), (-9, 5, b'F'), (-9, 4, b'F'), (-9, 3, b'F'), (-9, 2, b'F'), (-9, 1, b'F'),
    // ring 11
    (-10, 0, b'B'), (-9, -1, b'B'), (-8, -2, b'B'), (-7, -3, b'B'), (-6, -4, b'B'), (-5, -5, b'B'), (-4, -6, b'B'), (-3, -7, b'B'),
    (-2, -8, b'B'), (-1, -9, b'B'), (0, -10, b'B'), (1, -10, b'B'), (2, -10, b'B'), (3, -10, b'B'), (4, -10, b'B'), (5, -10, b'B'),
    (6, -10, b'B'), (7, -10, b'B'), (8, -10, b'B'), (9, -10, b'B'), (10, -10, b'B'), (10, -9, b'B'), (10, -8, b'B'), (10, -7, b'B'),
    (10, -6, b'B'), (10, -5, b'B'), (10, -4, b'B'), (10, -3, b'B'), (10, -2, b'B'), (10, -1, b'B'), (10, 0, b'B'), (9, 1, b'B'),
    (8, 2, b'B'), (7, 3, b'B'), (6, 4, b'B'), (5, 5, b'B'), (4, 6, b'B'), (3, 7, b'B'), (2, 8, b'B'), (1, 9, b'B'),
    (0, 10, b'B'), (-1, 10, b'B'), (-2, 10, b'B'), (-3, 10, b'B'), (-4, 10, b'B'), (-5, 10, b'B'), (-6, 10, b'B'), (-7, 10, b'B'),
    (-8, 10, b'B'), (-9, 10, b'B'), (-10, 10, b'B'), (-10, 9, b'B'), (-10, 8, b'B'), (-10, 7, b'B'), (-10, 6, b'B'), (-10, 5, b'B'),
    (-10, 4, b'B'), (-10, 3, b'B'), (-10, 2, b'B'), (-10, 1, b'B'),
    // ring 12
    (-11, 0, b'B'), (-10, -1, b'B'), (-9, -2, b'B'), (-8, -3, b'B'), (-7, -4, b'B'), (-6, -5, b'B'), (-5, -6, b'B'), (-4, -7, b'B'),
    (-3, -8, b'B'), (-2, -9, b'B'), (-1, -10, b'B'), (0, -11, b'B'), (1, -11, b'B'), (2, -11, b'B'), (3, -11, b'B'), (4, -11, b'B'),
    (5, -11, b'B'), (6, -11, b'B'), (7, -11, b'B'), (8, -11, b'B'), (9, -11, b'B'), (10, -11, b'B'), (11, -11, b'B'), (11, -10, b'B'),
    (11, -9, b'B'), (11, -8, b'B'), (11, -7, b'B'), (11, -6, b'B'), (11, -5, b'B'), (11, -4, b'B'), (11, -3, b'B'), (11, -2, b'B'),
    (11, -1, b'B'), (11, 0, b'B'), (10, 1, b'B'), (9, 2, b'B'), (8, 3, b'B'), (7, 4, b'B'), (6, 5, b'B'), (5, 6, b'B'),
    (4, 7, b'B'), (3, 8, b'B'), (2, 9, b'B'), (1, 10, b'B'), (0, 11, b'B'), (-1, 11, b'B'), (-2, 11, b'B'), (-3, 11, b'B'),
    (-4, 11, b'B'), (-5, 11, b'B'), (-6, 11, b'B'), (-7, 11, b'B'), (-8, 11, b'B'), (-9, 11, b'B'), (-10, 11, b'B'), (-11, 11, b'B'),
    (-11, 10, b'B'), (-11, 9, b'B'), (-11, 8, b'B'), (-11, 7, b'B'), (-11, 6, b'B'), (-11, 5, b'B'), (-11, 4, b'B'), (-11, 3, b'B'),
    (-11, 2, b'B'), (-11, 1, b'B'),
    // ring 13
    (-12, 0, b'F'), (-11, -1, b'B'), (-10, -2, b'B'), (-9, -3, b'B'), (-8, -4, b'B'), (-7, -5, b'B'), (-6, -6, b'B'), (-5, -7, b'B'),
    (-4, -8, b'B'), (-3, -9, b'B'), (-2, -10, b'F'), (-1, -11, b'B'), (0, -12, b'B'), (1, -12, b'B'), (2, -12, b'B'), (3, -12, b'B'),
    (4, -12, b'B'), (5, -12, b'B'), (6, -12, b'B'), (7, -12, b'B'), (8, -12, b'B'), (9, -12, b'B'), (10, -12, b'F'), (11, -12, b'B'),
    (12, -12, b'B'), (12, -11, b'B'), (12, -10, b'B'), (12, -9, b'B'), (12, -8, b'B'), (12, -7, b'B'), (12, -6, b'F'), (12, -5, b'B'),
    (12, -4, b'B'), (12, -3, b'B'), (12, -2, b'F'), (12, -1, b'B'), (12, 0, b'B'), (11, 1, b'B'), (10, 2, b'B'), (9, 3, b'B'),
    (8, 4, b'B'), (7, 5, b'F'), (6, 6, b'B'), (5, 7, b'B'), (4, 8, b'B'), (3, 9, b'B'), (2, 10, b'B'), (1, 11, b'B'),
    (0, 12, b'B'), (-1, 12, b'F'), (-2, 12, b'B'), (-3, 12, b'B'), (-4, 12, b'B'), (-5, 12, b'B'), (-6, 12, b'B'), (-7, 12, b'B'),
    (-8, 12, b'B'), (-9, 12, b'B'), (-10, 12, b'F'), (-11, 12, b'B'), (-12, 12, b'B'), (-12, 11, b'B'), (-12, 10, b'F'), (-12, 9, b'B'),
    (-12, 8, b'B'), (-12, 7, b'B'), (-12, 6, b'B'), (-12, 5, b'B'), (-12, 4, b'B'), (-12, 3, b'B'), (-12, 2, b'F'), (-12, 1, b'B'),
    // ring 14
    (-13, 0, b'B'), (-12, -1, b'B'), (-11, -2, b'F'), (-10, -3, b'B'), (-9, -4, b'B'), (-8, -5, b'B'), (-7, -6, b'B'), (-6, -7, b'B'),
    (-5, -8, b'B'), (-4, -9, b'B'), (-3, -10, b'B'), (-2, -11, b'B'), (-1, -12, b'B'), (0, -13, b'B'), (1, -13, b'F'), (2, -13, b'F'),
    (3, -13, b'B'), (4, -13, b'B'), (5, -13, b'B'), (6, -13, b'B'), (7, -13, b'B'), (8, -13, b'B'), (9, -13, b'B'), (10, -13, b'B'),
    (11, -13, b'B'), (12, -13, b'F'), (13, -13, b'B'), (13, -12, b'B'), (13, -11, b'B'), (13, -10, b'B'), (13, -9, b'B'), (13, -8, b'B'),
    (13, -7, b'B'), (13, -6, b'B'), (13, -5, b'B'), (13, -4, b'B'), (13, -3, b'B'), (13, -2, b'B'), (13, -1, b'B'), (13, 0, b'F'),
    (12, 1, b'B'), (11, 2, b'B'), (10, 3, b'B'), (9, 4, b'B'), (8, 5, b'B'), (7, 6, b'B'), (6, 7, b'B'), (5, 8, b'B'),
    (4, 9, b'B'), (3, 10, b'B'), (2, 11, b'F'), (1, 12, b'B'), (0, 13, b'B'), (-1, 13, b'B'), (-2, 13, b'B'), (-3, 13, b'B'),
    (-4, 13, b'F'), (-5, 13, b'B'), (-6, 13, b'B'), (-7, 13, b'B'), (-8, 13, b'B'), (-9, 13, b'F'), (-10, 13, b'B'), (-11, 13, b'F'),
    (-12, 13, b'B'), (-13, 13, b'B'), (-13, 12, b'B'), (-13, 11, b'F'), (-13, 10, b'B'), (-13, 9, b'B'), (-13, 8, b'B'), (-13, 7, b'B'),
    (-13, 6, b'B'), (-13, 5, b'B'), (-13, 4, b'B'), (-13, 3, b'B'), (-13, 2, b'F'), (-13, 1, b'B'),
    // ring 15
    (-12, -2, b'B'), (-11, -3, b'B'), (-10, -4, b'B'), (-9, -5, b'B'), (-8, -6, b'B'), (-7, -7, b'B'), (-6, -8, b'B'), (-5, -9, b'B'),
    (-4, -10, b'B'), (-3, -11, b'B'), (-2, -12, b'B'), (2, -14, b'B'), (3, -14, b'B'), (4, -14, b'B'), (5, -14, b'B'), (6, -14, b'B'),
    (7, -14, b'B'), (8, -14, b'B'), (9, -14, b'B'), (10, -14, b'B'), (11, -14, b'B'), (12, -14, b'B'), (14, -12, b'B'), (14, -11, b'B'),
    (14, -10, b'B'), (14, -9, b'B'), (14, -8, b'B'), (14, -7, b'B'), (14, -6, b'B'), (14, -5, b'B'), (14, -4, b'B'), (14, -3, b'B'),
    (14, -2, b'B'), (12, 2, b'B'), (11, 3, b'B'), (10, 4, b'B'), (9, 5, b'B'), (8, 6, b'B'), (7, 7, b'B'), (6, 8, b'B'),
    (5, 9, b'F'), (4, 10, b'B'), (3, 11, b'B'), (2, 12, b'B'), (-2, 14, b'B'), (-3, 14, b'B'), (-4, 14, b'B'), (-5, 14, b'B'),
    (-6, 14, b'B'), (-7, 14, b'B'), (-8, 14, b'B'), (-9, 14, b'F'), (-10, 14, b'B'), (-11, 14, b'B'), (-12, 14, b'B'), (-14, 12, b'B'),
    (-14, 11, b'B'), (-14, 10, b'B'), (-14, 9, b'B'), (-14, 8, b'B'), (-14, 7, b'B'), (-14, 6, b'B'), (-14, 5, b'F'), (-14, 4, b'B'),
    (-14, 3, b'B'), (-14, 2, b'B'),
    // ring 16
    (-9, -6, b'F'), (-8, -7, b'B'), (-7, -8, b'B'), (-6, -9, b'B'), (6, -15, b'F'), (7, -15, b'B'), (8, -15, b'F'), (9, -15, b'F'),
    (15, -9, b'B'), (15, -8, b'F'), (15, -7, b'B'), (15, -6, b'B'), (9, 6, b'F'), (8, 7, b'B'), (7, 8, b'B'), (6, 9, b'F'),
    (-6, 15, b'F'), (-7, 15, b'B'), (-8, 15, b'B'), (-9, 15, b'F'), (-15, 9, b'F'), (-15, 8, b'B'), (-15, 7, b'F'), (-15, 6, b'F'),
];

/// The centre of position `(q, r)`, cm (module docs).
pub fn centre_cm(q: i32, r: i32) -> (f64, f64) {
    (PITCH_CM * (q as f64 + 0.5 * r as f64), -PITCH_CM * r as f64 * 3.0_f64.sqrt() / 2.0)
}

/// One study of the core: what the MC page's controls set.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Study {
    /// Every material's temperature, K.
    pub temperature_k: f64,
    /// The control rods' drop as a fraction of the stroke (0: fuel level
    /// with the drivers' and the B₄C above the core; 1: 43.815 cm down).
    pub control_insertion: f64,
}

impl Default for Study {
    /// 293.15 K, the rods where SHRT-45R started.
    fn default() -> Self {
        Study { temperature_k: TEMPERATURE_K, control_insertion: SHRT45R_CONTROL_INSERTION }
    }
}

/// Atoms per barn-cm of `element` at its natural abundance, partial
/// density `g_cc`, with `masses`' isotope masses.
fn natural_atoms(masses: &AtomicMasses, element: Element, g_cc: f64) -> Result<Vec<(Isotope, f64)>, String> {
    let ab = element.natural_abundance();
    let mut m = 0.0;
    for (i, x) in &ab {
        m += x * masses.mass_u(*i).ok_or_else(|| format!("no mass for {}", i.name()))?;
    }
    let n = g_cc * AVOGADRO_PER_MOL / m * 1e-24;
    Ok(ab.into_iter().map(|(i, x)| (i, x * n)).collect())
}

/// Atoms per barn-cm of one isotope at partial density `g_cc`.
fn isotope_atoms(masses: &AtomicMasses, iso: Isotope, g_cc: f64) -> Result<Vec<(Isotope, f64)>, String> {
    let m = masses.mass_u(iso).ok_or_else(|| format!("no mass for {}", iso.name()))?;
    Ok(vec![(iso, g_cc * AVOGADRO_PER_MOL / m * 1e-24)])
}

/// `Σ fraction × composition`, merged by isotope.
fn mix(parts: &[(f64, &[(Isotope, f64)])]) -> Vec<(String, f64)> {
    let mut out: Vec<(String, f64)> = Vec::new();
    for (f, comp) in parts {
        for (i, n) in comp.iter() {
            let name = i.name();
            match out.iter_mut().find(|(k, _)| *k == name) {
                Some(e) => e.1 += f * n,
                None => out.push((name, f * n)),
            }
        }
    }
    out
}

fn iso(name: &str) -> Result<Isotope, String> {
    Isotope::from_name(name).map_err(|e| format!("{name}: {e}"))
}

impl Study {
    /// The homogenised materials in [`MATERIALS`] order, as `(nuclide,
    /// atoms/b-cm)`, with `masses`' isotope masses (the tapes' AWR, as
    /// [`crate::ebr2_fresh_driver`] uses).
    ///
    /// # Errors
    /// A mass the library does not hold.
    pub fn materials(&self, masses: &AtomicMasses) -> Result<Vec<Vec<(String, f64)>>, String> {
        let t = GapTreatment::on_hand();
        let fuel = fd::u5fs_atoms(masses, &t)?;
        let steel = fd::steel_316_atoms(masses, &t)?;
        let na = isotope_atoms(masses, iso("Na23")?, fd::SODIUM_DENSITY_G_CC)?;
        let du = isotope_atoms(masses, iso("U238")?, DU_G_CC)?;
        // B4C: 4 B and 1 C per molecule, natural boron and carbon.
        let molar = |e: Element| -> Result<f64, String> {
            let mut m = 0.0;
            for (i, x) in e.natural_abundance() {
                m += x * masses.mass_u(i).ok_or_else(|| format!("no mass for {}", i.name()))?;
            }
            Ok(m)
        };
        let (mb, mc) = (molar(Element::Boron)?, molar(Element::Carbon)?);
        let b_g = B4C_G_CC * 4.0 * mb / (4.0 * mb + mc);
        let mut b4c = natural_atoms(masses, Element::Boron, b_g)?;
        b4c.extend(natural_atoms(masses, Element::Carbon, B4C_G_CC - b_g)?);
        Ok(vec![
            mix(&[(0.2692, &fuel), (0.2229, &steel), (0.5079, &na)]),
            mix(&[(0.1346, &fuel), (0.3575, &steel), (0.5079, &na)]),
            mix(&[(0.1804, &fuel), (0.2269, &steel), (0.5927, &na)]),
            mix(&[(0.1804, &b4c), (0.2269, &steel), (0.5927, &na)]),
            mix(&[(0.6113, &du), (0.1764, &steel), (0.2123, &na)]),
            mix(&[(0.7877, &steel), (0.2123, &na)]),
            mix(&[(1.0, &na)]),
        ])
    }

    /// The transport materials, each component pointing at its nuclide's
    /// position in [`NUCLIDES`].
    ///
    /// # Errors
    /// As [`Self::materials`], or a nuclide not in [`NUCLIDES`].
    pub fn transport_materials(&self, masses: &AtomicMasses) -> Result<Vec<Material>, String> {
        let mats = self.materials(masses)?;
        let mut out = Vec::with_capacity(mats.len());
        for (k, comps) in mats.into_iter().enumerate() {
            let mut components = Vec::with_capacity(comps.len());
            for (n, d) in comps {
                let i = NUCLIDES.iter().position(|(m, _)| *m == n).ok_or_else(|| format!("{n} is not in NUCLIDES"))?;
                components.push(NuclideComponent { nuclide_idx: i, atom_density: d });
            }
            out.push(Material { id: k as i32 + 1, name: MATERIALS[k].into(), temperature: self.temperature_k, components });
        }
        Ok(out)
    }

    /// The control rods' fuel-bundle bottom, cm (the drivers' is at
    /// `−FUEL_CM / 2`).
    pub fn control_fuel_bottom_cm(&self) -> f64 {
        -0.5 * FUEL_CM - self.control_insertion.clamp(0.0, 1.0) * STROKE_CM
    }

    /// **The assembled geometry**, in [`MATERIALS`]' order (module docs).
    pub fn geometry(&self) -> Geometry {
        let tr = BoundaryType::Transmissive;
        let mut surfaces: Vec<SurfaceKind> = Vec::new();
        let mut cells: Vec<Cell> = Vec::new();
        let mut universes: Vec<Universe> = vec![Universe { id: 0, cell_indices: Vec::new() }];
        let mut plane = |surfaces: &mut Vec<SurfaceKind>, z: f64| {
            surfaces.push(SurfaceKind::ZPlane(ZPlane { z0: z, bc: tr }));
            surfaces.len() - 1
        };
        // A column: materials between ascending z breaks (one more
        // material than breaks), as one universe.
        let mut column = |surfaces: &mut Vec<SurfaceKind>, cells: &mut Vec<Cell>, breaks: &[f64], mats: &[usize]| -> Universe {
            let planes: Vec<usize> = breaks.iter().map(|&z| plane(surfaces, z)).collect();
            let mut idx = Vec::new();
            for (k, &m) in mats.iter().enumerate() {
                let mut region = Vec::new();
                if k > 0 {
                    region.push(hs(planes[k - 1], false));
                }
                if k < planes.len() {
                    region.push(hs(planes[k], true));
                }
                if region.is_empty() {
                    // One material everywhere: a plane far away, both sides.
                    let p = plane(surfaces, 1e6);
                    region = vec![hs(p, true), hs(p, false), RegionToken::Union];
                } else {
                    region = all_of(&region);
                }
                cells.push(Cell::material(cells.len() as i32 + 1, region, m, self.temperature_k));
                idx.push(cells.len() - 1);
            }
            Universe { id: 0, cell_indices: idx }
        };
        let (fb, ft) = (-0.5 * FUEL_CM, 0.5 * FUEL_CM);
        let cb = self.control_fuel_bottom_cm();
        let sb = fb - SAFETY_DROP_CM;
        for kind in SaKind::ALL {
            let u = match kind {
                SaKind::Driver => column(&mut surfaces, &mut cells, &[fb, ft], &[REFLECTOR, DRIVER, REFLECTOR]),
                SaKind::HalfWorth => column(&mut surfaces, &mut cells, &[fb, ft], &[REFLECTOR, HALF, REFLECTOR]),
                SaKind::HighWorthControl => {
                    column(&mut surfaces, &mut cells, &[cb, cb + FUEL_CM, cb + 2.0 * FUEL_CM], &[REFLECTOR, ROD_FUEL, B4C, REFLECTOR])
                }
                SaKind::RegularControl => column(&mut surfaces, &mut cells, &[cb, cb + FUEL_CM], &[REFLECTOR, ROD_FUEL, REFLECTOR]),
                SaKind::Safety => column(&mut surfaces, &mut cells, &[sb, sb + FUEL_CM], &[REFLECTOR, ROD_FUEL, REFLECTOR]),
                SaKind::Xx09 => column(&mut surfaces, &mut cells, &[fb, ft], &[REFLECTOR, ROD_FUEL, REFLECTOR]),
                SaKind::Blanket => column(&mut surfaces, &mut cells, &[], &[BLANKET]),
                SaKind::Reflector => column(&mut surfaces, &mut cells, &[], &[REFLECTOR]),
            };
            universes.push(Universe { id: universes.len() as i32, ..u });
        }
        let na = column(&mut surfaces, &mut cells, &[], &[SODIUM]);
        universes.push(Universe { id: universes.len() as i32, ..na });
        let sodium_u = universes.len() - 1;

        // The lattice: every ring sodium, then each position's kind.
        let rings: Vec<Vec<usize>> = (0..RINGS).map(|k| vec![sodium_u; if k == RINGS - 1 { 1 } else { 6 * (RINGS - 1 - k) }]).collect();
        let mut lat = HexLattice::from_rings(1, HexOrientation::X, Position::ZERO, PITCH_CM, &rings, Some(sodium_u));
        for &(q, r, c) in RUN_138B.iter() {
            let kind = SaKind::from_code(c).expect("a known code");
            let u = 1 + SaKind::ALL.iter().position(|k| *k == kind).expect("listed");
            let (x, y) = centre_cm(q, r);
            let i = lat.get_indices(Position::new(x, y, 0.0), Direction::new(1.0, 0.0, 0.0));
            let f = lat.flat_index(i);
            lat.universes[f] = u as i32;
        }

        // The root: the cylinder between the column's ends, vacuum outside.
        let v = BoundaryType::Vacuum;
        surfaces.push(SurfaceKind::ZCylinder(ZCylinder { x0: 0.0, y0: 0.0, r: R_OUTER_CM, bc: v }));
        let cyl = surfaces.len() - 1;
        surfaces.push(SurfaceKind::ZPlane(ZPlane { z0: -0.5 * COLUMN_CM, bc: v }));
        let zlo = surfaces.len() - 1;
        surfaces.push(SurfaceKind::ZPlane(ZPlane { z0: 0.5 * COLUMN_CM, bc: v }));
        let zhi = surfaces.len() - 1;
        cells.push(Cell::fill(cells.len() as i32 + 1, all_of(&[hs(cyl, true), hs(zlo, false), hs(zhi, true)]), CellFill::Lattice(0), Position::ZERO));
        universes[0].cell_indices = vec![cells.len() - 1];
        Geometry { surfaces, cells, universes, lattices: vec![Lattice::Hex(lat)], root_universe: 0 }
    }
}

/// The fission source box: the core (rows 1-7) and the fuel's height,
/// with the control rods' fuel at any drop.
pub fn source_box() -> SourceBox {
    let h = 7.0 * PITCH_CM;
    SourceBox {
        lower: Position::new(-h, -h, -0.5 * FUEL_CM - STROKE_CM),
        upper: Position::new(h, h, 0.5 * FUEL_CM),
    }
}

fn hs(surface_idx: usize, inside: bool) -> RegionToken {
    RegionToken::HalfSpace { surface_idx, sense: if inside { HalfSpaceSense::Inside } else { HalfSpaceSense::Outside } }
}

fn all_of(tokens: &[RegionToken]) -> Vec<RegionToken> {
    let mut v = vec![tokens[0].clone()];
    for t in &tokens[1..] {
        v.push(t.clone());
        v.push(RegionToken::Intersection);
    }
    v
}

/// The masses of [`NUCLIDES`] from their tapes (AWR × neutron mass), read
/// natively from the workspace (`reference-data/endf` and the ACE
/// submodule's `endf/`), with the tapes as [`NuclearData`].
///
/// # Errors
/// A tape is missing or unreadable.
pub fn library(endf_dir: &Path) -> Result<outram_mc_libs::model::NuclearData, String> {
    let mut tapes = Vec::with_capacity(NUCLIDES.len());
    for (_, t) in NUCLIDES {
        let p = endf_dir.join(t);
        let bytes = std::fs::read(&p).map_err(|e| format!("{}: {e}", p.display()))?;
        tapes.push((t.rsplit('/').next().unwrap_or(t).to_string(), bytes));
    }
    outram_mc_libs::model::NuclearData::from_endf_bytes("EBR-II core tapes (ENDF/B-VIII.0)", tapes).map_err(|e| e.to_string())
}

/// **k_eff of `study`, natively** (the record's runs): every nuclide of
/// [`NUCLIDES`] processed at the study's temperature, `particles × [inactive
/// + active]` at `seed`, surface tracking on every thread. `progress` gets a
/// line per stage.
///
/// # Errors
/// A tape missing, or a nuclide that does not process.
pub fn keff(
    endf_dir: &Path,
    study: &Study,
    particles: usize,
    inactive: usize,
    active: usize,
    seed: u64,
    progress: impl Fn(&str),
) -> Result<outram_mc_libs::physics::keff::KeffResult, String> {
    use outram_mc_libs::physics::keff::KeffSettings;
    progress("reading the tapes");
    let data = library(endf_dir)?;
    let masses = data.atomic_masses();
    let materials = study.transport_materials(&masses)?;
    let mut nuclides = Vec::with_capacity(NUCLIDES.len());
    for (k, (n, _)) in NUCLIDES.iter().enumerate() {
        progress(&format!("processing {n} ({}/{})", k + 1, NUCLIDES.len()));
        nuclides.push(data.load_nuclide(iso(n)?, study.temperature_k, None).map_err(|e| e.to_string())?);
    }
    progress("transport");
    let settings = KeffSettings {
        n_particles: particles,
        n_inactive: inactive,
        n_active: active,
        seed,
        temperature_k: study.temperature_k,
        ..KeffSettings::default()
    };
    Ok(outram_mc_libs::physics::transport_csg::run_keff_csg_par(
        &study.geometry(),
        &materials,
        &nuclides,
        &[],
        None,
        source_box(),
        &settings,
        None,
        &[],
        None,
        outram_mc_libs::physics::compute::ThreadCount::Auto,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// gh:#1762: [`RUN_138B`] is the gh:#1313 transcription, position for
    /// position (the CSV in `reference-data/ebr2/`), with the counts of
    /// ARC-228 Fig. 3.1 and the 87 fuelled drivers of Table 3.3.
    #[test]
    fn the_table_is_the_transcription() {
        let csv = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../reference-data/ebr2/ebr2_loading_maps.csv")).expect("csv");
        let rows: Vec<Vec<&str>> = csv.lines().skip(1).map(|l| l.split(',').collect()).collect();
        assert_eq!(rows.len(), 637);
        for (row, &(q, r, c)) in rows.iter().zip(RUN_138B.iter()) {
            assert_eq!((row[4].parse::<i32>().unwrap(), row[5].parse::<i32>().unwrap()), (q, r), "{}", row[0]);
            let (t, id) = (row[6], row[7]);
            let want = match t {
                "driver" | "high-flow driver" => b'D',
                "half-worth driver" => b'H',
                "control rod" if row[9] == "HW CNTR" => b'C',
                "control rod" => b'R',
                "safety rod" => b'S',
                "experiment" if ["2776A", "X402A", "X412"].contains(&id) => b'D',
                "instrumented" if id == "XX09" => b'X',
                "outer blanket" => b'B',
                _ => b'F',
            };
            assert_eq!(c, want, "{}", row[0]);
        }
        let count = |k: u8| RUN_138B.iter().filter(|e| e.2 == k).count();
        assert_eq!((count(b'D'), count(b'H'), count(b'X')), (73, 13, 1), "87 fuelled drivers");
        assert_eq!((count(b'C'), count(b'R'), count(b'S'), count(b'B')), (7, 1, 2, 330));
    }

    /// Every position lands on its own lattice cell, inside the 16 rings
    /// and the outer cylinder; neighbours are one pitch apart.
    #[test]
    fn every_position_has_its_own_tile() {
        let g = Study::default().geometry();
        let Lattice::Hex(lat) = &g.lattices[0] else { panic!("hex") };
        let mut seen = std::collections::BTreeSet::new();
        for &(q, r, _) in RUN_138B.iter() {
            let (x, y) = centre_cm(q, r);
            assert!(x.hypot(y) + PITCH_CM / 3.0_f64.sqrt() < R_OUTER_CM);
            let i = lat.get_indices(Position::new(x, y, 0.0), Direction::new(1.0, 0.0, 0.0));
            assert!(lat.are_valid_indices(i), "({q}, {r})");
            let c = lat.tile_center(i);
            assert!((c.x - x).abs() < 1e-9 && (c.y - y).abs() < 1e-9, "({q}, {r}) at ({}, {})", c.x, c.y);
            assert!(seen.insert(lat.flat_index(i)), "({q}, {r}) shares a tile");
        }
        let (x1, y1) = centre_cm(1, 0);
        let (x2, y2) = centre_cm(0, 1);
        assert!((x1.hypot(y1) - PITCH_CM).abs() < 1e-12 && (x2.hypot(y2) - PITCH_CM).abs() < 1e-12);
        assert!((SHRT45R_CONTROL_INSERTION * STROKE_CM - 21.06).abs() < 1e-12);
        assert!((Study::default().control_fuel_bottom_cm() + 0.5 * FUEL_CM + 21.06).abs() < 1e-9);
    }
}
