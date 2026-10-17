//! **Which nuclide**: an [`Isotope`] (Z, A, metastable state), a natural
//! [`Element`], and [`Species`], the one type a material entry accepts for
//! either.
//!
//! Nothing here holds nuclear data. An isotope is an identity; the data for
//! it is found in a [`super::data::NuclearData`] library by that identity
//! (Z, A and the metastable state read from each tape's own header), never
//! by file name.
//!
//! Natural isotopic abundances are OpenMC's `NATURAL_ABUNDANCE`
//! (`openmc/data/data.py:30-128`), already transcribed in this crate as
//! [`crate::plotter::endf_tables::natural_isotopes`]. Upstream cites them as
//! Meija, Coplen et al., *Isotopic compositions of the elements 2013 (IUPAC
//! Technical Report)*, Pure Appl. Chem. 88 (3), 293-306: the
//! "representative isotopic abundance" column, or the "best measurement"
//! where an interval is given. They are **atom** fractions.

use crate::plotter::endf_tables::{gnds_name, natural_isotopes, zam};

/// One nuclide, by proton number `Z`, mass number `A` and metastable state.
///
/// `A = 0` names a **natural-element evaluation** (e.g. ENDF/B-VII.0's
/// `C-0`, natural carbon in one tape), as ENDF does.
///
/// Named constants for the nuclides the tutorials use live in
/// [`isotopes`](super::isotopes) (`U235`, `H1`, ...); any other is
/// [`Isotope::new`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Isotope {
    z: u8,
    a: u16,
    m: u8,
}

impl Isotope {
    /// The ground state of nuclide `(Z, A)`. `Z` is 1..=118; `A` is 0 (a
    /// natural-element evaluation) or at least `Z`.
    ///
    /// # Errors
    /// [`IsotopeError::OutOfRange`] for any other pair.
    pub fn new(z: u8, a: u16) -> Result<Self, IsotopeError> {
        Self::metastable(z, a, 0)
    }

    /// Nuclide `(Z, A)` in metastable state `m` (0 is the ground state, 1 the
    /// first isomer, as ENDF's `LISO`).
    ///
    /// # Errors
    /// [`IsotopeError::OutOfRange`] when `Z` is not 1..=118 or `A` is neither
    /// 0 nor at least `Z`.
    pub fn metastable(z: u8, a: u16, m: u8) -> Result<Self, IsotopeError> {
        if !(1..=118).contains(&z) || (a != 0 && a < u16::from(z)) {
            return Err(IsotopeError::OutOfRange { z, a });
        }
        Ok(Self { z, a, m })
    }

    /// For the named constants only: the caller guarantees the range.
    pub(crate) const fn known(z: u8, a: u16) -> Self {
        Self { z, a, m: 0 }
    }

    /// Proton number.
    pub fn z(self) -> u8 {
        self.z
    }

    /// Mass number (0 for a natural-element evaluation).
    pub fn a(self) -> u16 {
        self.a
    }

    /// Metastable state (0 = ground state).
    pub fn metastable_state(self) -> u8 {
        self.m
    }

    /// The GNDS name OpenMC uses: `U235`, `Am242_m1`, and `C0` for a
    /// natural-element evaluation.
    pub fn name(self) -> String {
        gnds_name(u32::from(self.z), u32::from(self.a), u32::from(self.m))
            .unwrap_or_else(|| format!("Z{}A{}m{}", self.z, self.a, self.m))
    }

    /// Parse a GNDS name (`"U235"`, `"Am242_m1"`).
    ///
    /// # Errors
    /// [`IsotopeError::BadName`] when the name is not one.
    pub fn from_name(name: &str) -> Result<Self, IsotopeError> {
        let (z, a, m) = zam(name).ok_or_else(|| IsotopeError::BadName(name.to_string()))?;
        let z = u8::try_from(z).map_err(|_| IsotopeError::BadName(name.to_string()))?;
        let a = u16::try_from(a).map_err(|_| IsotopeError::BadName(name.to_string()))?;
        let m = u8::try_from(m).map_err(|_| IsotopeError::BadName(name.to_string()))?;
        Self::metastable(z, a, m)
    }

    /// The element this isotope belongs to.
    ///
    /// ~~Fell back to hydrogen for `Z > 94`~~ **CORRECTED 2026-10-17**
    /// (found by another agent, fixed in gh #940's wave): `Am241.element()`
    /// returned `Hydrogen`, because [`Element`] stopped at plutonium. It now
    /// lists all 118 elements, and every [`Isotope`] has `1 <= Z <= 118`
    /// (checked by its constructors), so the lookup cannot miss; the
    /// `expect` states that invariant and
    /// `every_isotope_z_has_its_element` pins it.
    pub fn element(self) -> Element {
        Element::from_z(self.z)
            .expect("an Isotope's Z is 1..=118 and Element lists all 118 elements")
    }

    /// Heavy metal in the fuel sense: actinides (`Z >= 89`).
    pub fn is_heavy_metal(self) -> bool {
        self.z >= 89
    }
}

impl std::fmt::Display for Isotope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.name())
    }
}

/// Why an [`Isotope`] could not be made.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum IsotopeError {
    /// `Z` outside 1..=118, or `0 < A < Z`.
    #[error("no nuclide has Z = {z}, A = {a} (Z must be 1..=118 and A either 0 or at least Z)")]
    OutOfRange {
        /// The proton number given.
        z: u8,
        /// The mass number given.
        a: u16,
    },
    /// Not a GNDS nuclide name such as `U235`.
    #[error("'{0}' is not a GNDS nuclide name such as U235 or Am242_m1")]
    BadName(String),
}

macro_rules! elements {
    ($( $name:ident = $z:literal, $sym:literal ;)*) => {
        /// A chemical element, for **natural** isotopic mixtures
        /// ([`Element::natural`]) and for naming the element of an
        /// [`Isotope`]. Discriminant = proton number.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        #[repr(u8)]
        pub enum Element {
            $(
                #[doc = concat!("Z = ", stringify!($z), " (", $sym, ").")]
                $name = $z,
            )*
        }

        impl Element {
            /// The element with proton number `z` (1..=118, hydrogen to
            /// oganesson); `None` outside that range.
            pub fn from_z(z: u8) -> Option<Self> {
                match z {
                    $( $z => Some(Self::$name), )*
                    _ => None,
                }
            }

            /// Chemical symbol (`"U"`, `"B"`).
            pub fn symbol(self) -> &'static str {
                match self {
                    $( Self::$name => $sym, )*
                }
            }
        }
    };
}

elements! {
    Hydrogen = 1, "H"; Helium = 2, "He"; Lithium = 3, "Li"; Beryllium = 4, "Be";
    Boron = 5, "B"; Carbon = 6, "C"; Nitrogen = 7, "N"; Oxygen = 8, "O";
    Fluorine = 9, "F"; Neon = 10, "Ne"; Sodium = 11, "Na"; Magnesium = 12, "Mg";
    Aluminium = 13, "Al"; Silicon = 14, "Si"; Phosphorus = 15, "P"; Sulfur = 16, "S";
    Chlorine = 17, "Cl"; Argon = 18, "Ar"; Potassium = 19, "K"; Calcium = 20, "Ca";
    Scandium = 21, "Sc"; Titanium = 22, "Ti"; Vanadium = 23, "V"; Chromium = 24, "Cr";
    Manganese = 25, "Mn"; Iron = 26, "Fe"; Cobalt = 27, "Co"; Nickel = 28, "Ni";
    Copper = 29, "Cu"; Zinc = 30, "Zn"; Gallium = 31, "Ga"; Germanium = 32, "Ge";
    Arsenic = 33, "As"; Selenium = 34, "Se"; Bromine = 35, "Br"; Krypton = 36, "Kr";
    Rubidium = 37, "Rb"; Strontium = 38, "Sr"; Yttrium = 39, "Y"; Zirconium = 40, "Zr";
    Niobium = 41, "Nb"; Molybdenum = 42, "Mo"; Technetium = 43, "Tc"; Ruthenium = 44, "Ru";
    Rhodium = 45, "Rh"; Palladium = 46, "Pd"; Silver = 47, "Ag"; Cadmium = 48, "Cd";
    Indium = 49, "In"; Tin = 50, "Sn"; Antimony = 51, "Sb"; Tellurium = 52, "Te";
    Iodine = 53, "I"; Xenon = 54, "Xe"; Caesium = 55, "Cs"; Barium = 56, "Ba";
    Lanthanum = 57, "La"; Cerium = 58, "Ce"; Praseodymium = 59, "Pr"; Neodymium = 60, "Nd";
    Promethium = 61, "Pm"; Samarium = 62, "Sm"; Europium = 63, "Eu"; Gadolinium = 64, "Gd";
    Terbium = 65, "Tb"; Dysprosium = 66, "Dy"; Holmium = 67, "Ho"; Erbium = 68, "Er";
    Thulium = 69, "Tm"; Ytterbium = 70, "Yb"; Lutetium = 71, "Lu"; Hafnium = 72, "Hf";
    Tantalum = 73, "Ta"; Tungsten = 74, "W"; Rhenium = 75, "Re"; Osmium = 76, "Os";
    Iridium = 77, "Ir"; Platinum = 78, "Pt"; Gold = 79, "Au"; Mercury = 80, "Hg";
    Thallium = 81, "Tl"; Lead = 82, "Pb"; Bismuth = 83, "Bi"; Polonium = 84, "Po";
    Astatine = 85, "At"; Radon = 86, "Rn"; Francium = 87, "Fr"; Radium = 88, "Ra";
    Actinium = 89, "Ac"; Thorium = 90, "Th"; Protactinium = 91, "Pa"; Uranium = 92, "U";
    Neptunium = 93, "Np"; Plutonium = 94, "Pu"; Americium = 95, "Am"; Curium = 96, "Cm";
    Berkelium = 97, "Bk"; Californium = 98, "Cf"; Einsteinium = 99, "Es"; Fermium = 100, "Fm";
    Mendelevium = 101, "Md"; Nobelium = 102, "No"; Lawrencium = 103, "Lr";
    Rutherfordium = 104, "Rf"; Dubnium = 105, "Db"; Seaborgium = 106, "Sg"; Bohrium = 107, "Bh";
    Hassium = 108, "Hs"; Meitnerium = 109, "Mt"; Darmstadtium = 110, "Ds";
    Roentgenium = 111, "Rg"; Copernicium = 112, "Cn"; Nihonium = 113, "Nh";
    Flerovium = 114, "Fl"; Moscovium = 115, "Mc"; Livermorium = 116, "Lv";
    Tennessine = 117, "Ts"; Oganesson = 118, "Og";
}

impl Element {
    /// Proton number.
    pub fn z(self) -> u8 {
        self as u8
    }

    /// This element with its **natural** isotopic composition (IUPAC 2013,
    /// atom fractions; see the module docs), as a material entry:
    /// `Element::Boron.natural()`.
    pub fn natural(self) -> Species {
        Species::Natural(self)
    }

    /// The natural isotopes and their **atom** fractions, from OpenMC's
    /// `NATURAL_ABUNDANCE`. Empty for an element with no stable isotope
    /// (Tc, Pm, ...).
    pub fn natural_abundance(self) -> Vec<(Isotope, f64)> {
        natural_isotopes(self.symbol())
            .into_iter()
            .filter_map(|(name, x)| Isotope::from_name(name).ok().map(|i| (i, x)))
            .collect()
    }
}

/// What a material entry names: **one isotope**, or **a natural element**
/// expanded into its isotopes when the material is resolved.
///
/// Built from either by `From`: `U235.into()`, `Element::Boron.natural()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Species {
    /// A single nuclide.
    Isotope(Isotope),
    /// A natural element, at its natural isotopic composition.
    Natural(Element),
}

impl From<Isotope> for Species {
    fn from(i: Isotope) -> Self {
        Species::Isotope(i)
    }
}

impl std::fmt::Display for Species {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Species::Isotope(i) => write!(f, "{i}"),
            Species::Natural(e) => write!(f, "natural {}", e.symbol()),
        }
    }
}

/// Named nuclides: `use outram_mc_libs::model::isotopes::*;` and write
/// `U235`. Any nuclide not listed is [`Isotope::new`]`(z, a)`.
///
/// The names are GNDS names (`U235`, `Al27`), as OpenMC spells them, not
/// upper-case constants: they are what a reader expects to see.
#[allow(non_upper_case_globals)]
pub mod isotopes {
    use super::Isotope;

    macro_rules! iso {
        ($( $name:ident = $z:literal, $a:literal ;)*) => {
            $(
                #[doc = concat!("Z = ", stringify!($z), ", A = ", stringify!($a), ".")]
                pub const $name: Isotope = Isotope::known($z, $a);
            )*
        };
    }

    iso! {
        H1 = 1, 1; H2 = 1, 2; He3 = 2, 3; He4 = 2, 4; Li6 = 3, 6; Li7 = 3, 7; Be9 = 4, 9;
        B10 = 5, 10; B11 = 5, 11; C12 = 6, 12; C13 = 6, 13; N14 = 7, 14; N15 = 7, 15;
        O16 = 8, 16; O17 = 8, 17; O18 = 8, 18; F19 = 9, 19; Na23 = 11, 23;
        Al27 = 13, 27; Si28 = 14, 28; Si29 = 14, 29; Si30 = 14, 30; Mn55 = 25, 55;
        Fe54 = 26, 54; Fe56 = 26, 56; Fe57 = 26, 57; Fe58 = 26, 58;
        Cu63 = 29, 63; Cu65 = 29, 65;
        Zr90 = 40, 90; Zr91 = 40, 91; Zr92 = 40, 92; Zr94 = 40, 94; Zr96 = 40, 96;
        Xe135 = 54, 135; Cs137 = 55, 137; Sm149 = 62, 149;
        Th232 = 90, 232; U233 = 92, 233; U234 = 92, 234; U235 = 92, 235; U236 = 92, 236;
        U238 = 92, 238; Pu238 = 94, 238; Pu239 = 94, 239; Pu240 = 94, 240; Pu241 = 94, 241;
        Pu242 = 94, 242; Am241 = 95, 241;
    }
}

#[cfg(test)]
mod tests {
    use super::isotopes::*;
    use super::*;

    /// Names round-trip through GNDS, natural expansion uses the IUPAC 2013
    /// atom fractions as OpenMC tabulates them, and out-of-range pairs are
    /// refused rather than wrapped.
    #[test]
    fn isotopes_elements_and_natural_abundance() {
        assert_eq!(U235.name(), "U235");
        assert_eq!(Isotope::from_name("U235").unwrap(), U235);
        assert_eq!(Isotope::metastable(95, 242, 1).unwrap().name(), "Am242_m1");
        assert!(Isotope::new(0, 1).is_err());
        assert!(Isotope::new(92, 10).is_err());
        assert!(Isotope::from_name("u235").is_err());
        assert_eq!(U238.element(), Element::Uranium);
        assert!(U238.is_heavy_metal() && !O16.is_heavy_metal());
        assert_eq!(Element::from_z(5), Some(Element::Boron));
        assert_eq!(Element::Boron.symbol(), "B");
        let b = Element::Boron.natural_abundance();
        assert_eq!(b, vec![(B10, 0.1982), (B11, 0.8018)]);
        assert!(Element::Technetium.natural_abundance().is_empty());
        assert_eq!(Element::Boron.natural(), Species::Natural(Element::Boron));
        assert_eq!(Species::from(U234).to_string(), "U234");
        assert_eq!(Element::Boron.natural().to_string(), "natural B");
        assert_eq!(format!("{}", Isotope::new(6, 0).unwrap()), "C0");
    }

    /// **Every Z an Isotope can have names its own element**: no fallback
    /// (`Am241.element()` used to be `Hydrogen`), and the symbol matches the
    /// one the isotope's own name carries.
    #[test]
    fn every_isotope_z_has_its_element() {
        assert_eq!(Am241.element(), Element::Americium);
        assert_eq!(Isotope::new(96, 244).unwrap().element(), Element::Curium);
        assert_eq!(Isotope::new(98, 252).unwrap().element(), Element::Californium);
        for z in 1..=118u8 {
            let e = Element::from_z(z).expect("all 118 elements listed");
            assert_eq!(e.z(), z);
            let i = Isotope::new(z, 0).unwrap();
            assert_eq!(i.element(), e);
            assert_eq!(i.name(), format!("{}0", e.symbol()));
        }
        assert_eq!(Element::from_z(0), None);
        assert_eq!(Element::from_z(119), None);
    }
}
