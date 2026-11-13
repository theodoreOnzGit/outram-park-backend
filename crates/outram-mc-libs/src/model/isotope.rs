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

/// A chemical element, for **natural** isotopic mixtures
/// ([`Element::natural`]) and for naming the element of an
/// [`Isotope`]. Discriminant = proton number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(u8)]
pub enum Element {
    /// Z = 1 (H).
    Hydrogen = 1,
    /// Z = 2 (He).
    Helium = 2,
    /// Z = 3 (Li).
    Lithium = 3,
    /// Z = 4 (Be).
    Beryllium = 4,
    /// Z = 5 (B).
    Boron = 5,
    /// Z = 6 (C).
    Carbon = 6,
    /// Z = 7 (N).
    Nitrogen = 7,
    /// Z = 8 (O).
    Oxygen = 8,
    /// Z = 9 (F).
    Fluorine = 9,
    /// Z = 10 (Ne).
    Neon = 10,
    /// Z = 11 (Na).
    Sodium = 11,
    /// Z = 12 (Mg).
    Magnesium = 12,
    /// Z = 13 (Al).
    Aluminium = 13,
    /// Z = 14 (Si).
    Silicon = 14,
    /// Z = 15 (P).
    Phosphorus = 15,
    /// Z = 16 (S).
    Sulfur = 16,
    /// Z = 17 (Cl).
    Chlorine = 17,
    /// Z = 18 (Ar).
    Argon = 18,
    /// Z = 19 (K).
    Potassium = 19,
    /// Z = 20 (Ca).
    Calcium = 20,
    /// Z = 21 (Sc).
    Scandium = 21,
    /// Z = 22 (Ti).
    Titanium = 22,
    /// Z = 23 (V).
    Vanadium = 23,
    /// Z = 24 (Cr).
    Chromium = 24,
    /// Z = 25 (Mn).
    Manganese = 25,
    /// Z = 26 (Fe).
    Iron = 26,
    /// Z = 27 (Co).
    Cobalt = 27,
    /// Z = 28 (Ni).
    Nickel = 28,
    /// Z = 29 (Cu).
    Copper = 29,
    /// Z = 30 (Zn).
    Zinc = 30,
    /// Z = 31 (Ga).
    Gallium = 31,
    /// Z = 32 (Ge).
    Germanium = 32,
    /// Z = 33 (As).
    Arsenic = 33,
    /// Z = 34 (Se).
    Selenium = 34,
    /// Z = 35 (Br).
    Bromine = 35,
    /// Z = 36 (Kr).
    Krypton = 36,
    /// Z = 37 (Rb).
    Rubidium = 37,
    /// Z = 38 (Sr).
    Strontium = 38,
    /// Z = 39 (Y).
    Yttrium = 39,
    /// Z = 40 (Zr).
    Zirconium = 40,
    /// Z = 41 (Nb).
    Niobium = 41,
    /// Z = 42 (Mo).
    Molybdenum = 42,
    /// Z = 43 (Tc).
    Technetium = 43,
    /// Z = 44 (Ru).
    Ruthenium = 44,
    /// Z = 45 (Rh).
    Rhodium = 45,
    /// Z = 46 (Pd).
    Palladium = 46,
    /// Z = 47 (Ag).
    Silver = 47,
    /// Z = 48 (Cd).
    Cadmium = 48,
    /// Z = 49 (In).
    Indium = 49,
    /// Z = 50 (Sn).
    Tin = 50,
    /// Z = 51 (Sb).
    Antimony = 51,
    /// Z = 52 (Te).
    Tellurium = 52,
    /// Z = 53 (I).
    Iodine = 53,
    /// Z = 54 (Xe).
    Xenon = 54,
    /// Z = 55 (Cs).
    Caesium = 55,
    /// Z = 56 (Ba).
    Barium = 56,
    /// Z = 57 (La).
    Lanthanum = 57,
    /// Z = 58 (Ce).
    Cerium = 58,
    /// Z = 59 (Pr).
    Praseodymium = 59,
    /// Z = 60 (Nd).
    Neodymium = 60,
    /// Z = 61 (Pm).
    Promethium = 61,
    /// Z = 62 (Sm).
    Samarium = 62,
    /// Z = 63 (Eu).
    Europium = 63,
    /// Z = 64 (Gd).
    Gadolinium = 64,
    /// Z = 65 (Tb).
    Terbium = 65,
    /// Z = 66 (Dy).
    Dysprosium = 66,
    /// Z = 67 (Ho).
    Holmium = 67,
    /// Z = 68 (Er).
    Erbium = 68,
    /// Z = 69 (Tm).
    Thulium = 69,
    /// Z = 70 (Yb).
    Ytterbium = 70,
    /// Z = 71 (Lu).
    Lutetium = 71,
    /// Z = 72 (Hf).
    Hafnium = 72,
    /// Z = 73 (Ta).
    Tantalum = 73,
    /// Z = 74 (W).
    Tungsten = 74,
    /// Z = 75 (Re).
    Rhenium = 75,
    /// Z = 76 (Os).
    Osmium = 76,
    /// Z = 77 (Ir).
    Iridium = 77,
    /// Z = 78 (Pt).
    Platinum = 78,
    /// Z = 79 (Au).
    Gold = 79,
    /// Z = 80 (Hg).
    Mercury = 80,
    /// Z = 81 (Tl).
    Thallium = 81,
    /// Z = 82 (Pb).
    Lead = 82,
    /// Z = 83 (Bi).
    Bismuth = 83,
    /// Z = 84 (Po).
    Polonium = 84,
    /// Z = 85 (At).
    Astatine = 85,
    /// Z = 86 (Rn).
    Radon = 86,
    /// Z = 87 (Fr).
    Francium = 87,
    /// Z = 88 (Ra).
    Radium = 88,
    /// Z = 89 (Ac).
    Actinium = 89,
    /// Z = 90 (Th).
    Thorium = 90,
    /// Z = 91 (Pa).
    Protactinium = 91,
    /// Z = 92 (U).
    Uranium = 92,
    /// Z = 93 (Np).
    Neptunium = 93,
    /// Z = 94 (Pu).
    Plutonium = 94,
    /// Z = 95 (Am).
    Americium = 95,
    /// Z = 96 (Cm).
    Curium = 96,
    /// Z = 97 (Bk).
    Berkelium = 97,
    /// Z = 98 (Cf).
    Californium = 98,
    /// Z = 99 (Es).
    Einsteinium = 99,
    /// Z = 100 (Fm).
    Fermium = 100,
    /// Z = 101 (Md).
    Mendelevium = 101,
    /// Z = 102 (No).
    Nobelium = 102,
    /// Z = 103 (Lr).
    Lawrencium = 103,
    /// Z = 104 (Rf).
    Rutherfordium = 104,
    /// Z = 105 (Db).
    Dubnium = 105,
    /// Z = 106 (Sg).
    Seaborgium = 106,
    /// Z = 107 (Bh).
    Bohrium = 107,
    /// Z = 108 (Hs).
    Hassium = 108,
    /// Z = 109 (Mt).
    Meitnerium = 109,
    /// Z = 110 (Ds).
    Darmstadtium = 110,
    /// Z = 111 (Rg).
    Roentgenium = 111,
    /// Z = 112 (Cn).
    Copernicium = 112,
    /// Z = 113 (Nh).
    Nihonium = 113,
    /// Z = 114 (Fl).
    Flerovium = 114,
    /// Z = 115 (Mc).
    Moscovium = 115,
    /// Z = 116 (Lv).
    Livermorium = 116,
    /// Z = 117 (Ts).
    Tennessine = 117,
    /// Z = 118 (Og).
    Oganesson = 118,
}

impl Element {
    /// The element with proton number `z` (1..=118, hydrogen to
    /// oganesson); `None` outside that range.
    pub fn from_z(z: u8) -> Option<Self> {
        match z {
            1 => Some(Self::Hydrogen),
            2 => Some(Self::Helium),
            3 => Some(Self::Lithium),
            4 => Some(Self::Beryllium),
            5 => Some(Self::Boron),
            6 => Some(Self::Carbon),
            7 => Some(Self::Nitrogen),
            8 => Some(Self::Oxygen),
            9 => Some(Self::Fluorine),
            10 => Some(Self::Neon),
            11 => Some(Self::Sodium),
            12 => Some(Self::Magnesium),
            13 => Some(Self::Aluminium),
            14 => Some(Self::Silicon),
            15 => Some(Self::Phosphorus),
            16 => Some(Self::Sulfur),
            17 => Some(Self::Chlorine),
            18 => Some(Self::Argon),
            19 => Some(Self::Potassium),
            20 => Some(Self::Calcium),
            21 => Some(Self::Scandium),
            22 => Some(Self::Titanium),
            23 => Some(Self::Vanadium),
            24 => Some(Self::Chromium),
            25 => Some(Self::Manganese),
            26 => Some(Self::Iron),
            27 => Some(Self::Cobalt),
            28 => Some(Self::Nickel),
            29 => Some(Self::Copper),
            30 => Some(Self::Zinc),
            31 => Some(Self::Gallium),
            32 => Some(Self::Germanium),
            33 => Some(Self::Arsenic),
            34 => Some(Self::Selenium),
            35 => Some(Self::Bromine),
            36 => Some(Self::Krypton),
            37 => Some(Self::Rubidium),
            38 => Some(Self::Strontium),
            39 => Some(Self::Yttrium),
            40 => Some(Self::Zirconium),
            41 => Some(Self::Niobium),
            42 => Some(Self::Molybdenum),
            43 => Some(Self::Technetium),
            44 => Some(Self::Ruthenium),
            45 => Some(Self::Rhodium),
            46 => Some(Self::Palladium),
            47 => Some(Self::Silver),
            48 => Some(Self::Cadmium),
            49 => Some(Self::Indium),
            50 => Some(Self::Tin),
            51 => Some(Self::Antimony),
            52 => Some(Self::Tellurium),
            53 => Some(Self::Iodine),
            54 => Some(Self::Xenon),
            55 => Some(Self::Caesium),
            56 => Some(Self::Barium),
            57 => Some(Self::Lanthanum),
            58 => Some(Self::Cerium),
            59 => Some(Self::Praseodymium),
            60 => Some(Self::Neodymium),
            61 => Some(Self::Promethium),
            62 => Some(Self::Samarium),
            63 => Some(Self::Europium),
            64 => Some(Self::Gadolinium),
            65 => Some(Self::Terbium),
            66 => Some(Self::Dysprosium),
            67 => Some(Self::Holmium),
            68 => Some(Self::Erbium),
            69 => Some(Self::Thulium),
            70 => Some(Self::Ytterbium),
            71 => Some(Self::Lutetium),
            72 => Some(Self::Hafnium),
            73 => Some(Self::Tantalum),
            74 => Some(Self::Tungsten),
            75 => Some(Self::Rhenium),
            76 => Some(Self::Osmium),
            77 => Some(Self::Iridium),
            78 => Some(Self::Platinum),
            79 => Some(Self::Gold),
            80 => Some(Self::Mercury),
            81 => Some(Self::Thallium),
            82 => Some(Self::Lead),
            83 => Some(Self::Bismuth),
            84 => Some(Self::Polonium),
            85 => Some(Self::Astatine),
            86 => Some(Self::Radon),
            87 => Some(Self::Francium),
            88 => Some(Self::Radium),
            89 => Some(Self::Actinium),
            90 => Some(Self::Thorium),
            91 => Some(Self::Protactinium),
            92 => Some(Self::Uranium),
            93 => Some(Self::Neptunium),
            94 => Some(Self::Plutonium),
            95 => Some(Self::Americium),
            96 => Some(Self::Curium),
            97 => Some(Self::Berkelium),
            98 => Some(Self::Californium),
            99 => Some(Self::Einsteinium),
            100 => Some(Self::Fermium),
            101 => Some(Self::Mendelevium),
            102 => Some(Self::Nobelium),
            103 => Some(Self::Lawrencium),
            104 => Some(Self::Rutherfordium),
            105 => Some(Self::Dubnium),
            106 => Some(Self::Seaborgium),
            107 => Some(Self::Bohrium),
            108 => Some(Self::Hassium),
            109 => Some(Self::Meitnerium),
            110 => Some(Self::Darmstadtium),
            111 => Some(Self::Roentgenium),
            112 => Some(Self::Copernicium),
            113 => Some(Self::Nihonium),
            114 => Some(Self::Flerovium),
            115 => Some(Self::Moscovium),
            116 => Some(Self::Livermorium),
            117 => Some(Self::Tennessine),
            118 => Some(Self::Oganesson),
            _ => None,
        }
    }

    /// Chemical symbol (`"U"`, `"B"`).
    pub fn symbol(self) -> &'static str {
        match self {
            Self::Hydrogen => "H",
            Self::Helium => "He",
            Self::Lithium => "Li",
            Self::Beryllium => "Be",
            Self::Boron => "B",
            Self::Carbon => "C",
            Self::Nitrogen => "N",
            Self::Oxygen => "O",
            Self::Fluorine => "F",
            Self::Neon => "Ne",
            Self::Sodium => "Na",
            Self::Magnesium => "Mg",
            Self::Aluminium => "Al",
            Self::Silicon => "Si",
            Self::Phosphorus => "P",
            Self::Sulfur => "S",
            Self::Chlorine => "Cl",
            Self::Argon => "Ar",
            Self::Potassium => "K",
            Self::Calcium => "Ca",
            Self::Scandium => "Sc",
            Self::Titanium => "Ti",
            Self::Vanadium => "V",
            Self::Chromium => "Cr",
            Self::Manganese => "Mn",
            Self::Iron => "Fe",
            Self::Cobalt => "Co",
            Self::Nickel => "Ni",
            Self::Copper => "Cu",
            Self::Zinc => "Zn",
            Self::Gallium => "Ga",
            Self::Germanium => "Ge",
            Self::Arsenic => "As",
            Self::Selenium => "Se",
            Self::Bromine => "Br",
            Self::Krypton => "Kr",
            Self::Rubidium => "Rb",
            Self::Strontium => "Sr",
            Self::Yttrium => "Y",
            Self::Zirconium => "Zr",
            Self::Niobium => "Nb",
            Self::Molybdenum => "Mo",
            Self::Technetium => "Tc",
            Self::Ruthenium => "Ru",
            Self::Rhodium => "Rh",
            Self::Palladium => "Pd",
            Self::Silver => "Ag",
            Self::Cadmium => "Cd",
            Self::Indium => "In",
            Self::Tin => "Sn",
            Self::Antimony => "Sb",
            Self::Tellurium => "Te",
            Self::Iodine => "I",
            Self::Xenon => "Xe",
            Self::Caesium => "Cs",
            Self::Barium => "Ba",
            Self::Lanthanum => "La",
            Self::Cerium => "Ce",
            Self::Praseodymium => "Pr",
            Self::Neodymium => "Nd",
            Self::Promethium => "Pm",
            Self::Samarium => "Sm",
            Self::Europium => "Eu",
            Self::Gadolinium => "Gd",
            Self::Terbium => "Tb",
            Self::Dysprosium => "Dy",
            Self::Holmium => "Ho",
            Self::Erbium => "Er",
            Self::Thulium => "Tm",
            Self::Ytterbium => "Yb",
            Self::Lutetium => "Lu",
            Self::Hafnium => "Hf",
            Self::Tantalum => "Ta",
            Self::Tungsten => "W",
            Self::Rhenium => "Re",
            Self::Osmium => "Os",
            Self::Iridium => "Ir",
            Self::Platinum => "Pt",
            Self::Gold => "Au",
            Self::Mercury => "Hg",
            Self::Thallium => "Tl",
            Self::Lead => "Pb",
            Self::Bismuth => "Bi",
            Self::Polonium => "Po",
            Self::Astatine => "At",
            Self::Radon => "Rn",
            Self::Francium => "Fr",
            Self::Radium => "Ra",
            Self::Actinium => "Ac",
            Self::Thorium => "Th",
            Self::Protactinium => "Pa",
            Self::Uranium => "U",
            Self::Neptunium => "Np",
            Self::Plutonium => "Pu",
            Self::Americium => "Am",
            Self::Curium => "Cm",
            Self::Berkelium => "Bk",
            Self::Californium => "Cf",
            Self::Einsteinium => "Es",
            Self::Fermium => "Fm",
            Self::Mendelevium => "Md",
            Self::Nobelium => "No",
            Self::Lawrencium => "Lr",
            Self::Rutherfordium => "Rf",
            Self::Dubnium => "Db",
            Self::Seaborgium => "Sg",
            Self::Bohrium => "Bh",
            Self::Hassium => "Hs",
            Self::Meitnerium => "Mt",
            Self::Darmstadtium => "Ds",
            Self::Roentgenium => "Rg",
            Self::Copernicium => "Cn",
            Self::Nihonium => "Nh",
            Self::Flerovium => "Fl",
            Self::Moscovium => "Mc",
            Self::Livermorium => "Lv",
            Self::Tennessine => "Ts",
            Self::Oganesson => "Og",
        }
    }
}

impl Element {
    /// Proton number.
    pub fn z(self) -> u8 {
        self as u8
    }

    /// This element with its **natural** isotopic composition (IUPAC 2013,
    /// atom fractions; see the module docs), as a material entry:
    /// `Element::Boron.natural()`.
    ///
    /// **A natural element expands to its isotopes** when a material is
    /// resolved (see [`Self::natural_abundance`] for the list), and **each of
    /// those isotopes must be in the [`NuclearData`](super::NuclearData)
    /// library** in use (a few are folded into a neighbour by OpenMC's rules,
    /// recorded in the resolution report). If one is absent, resolving fails
    /// with [`MaterialError::CannotExpand`](super::MaterialError::CannotExpand).
    /// To see what a library holds, use
    /// [`NuclearData::atomic_masses`](super::NuclearData::atomic_masses)
    /// then `.isotopes()`.
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

    /// Z = 1, A = 1.
    pub const H1: Isotope = Isotope::known(1, 1);
    /// Z = 1, A = 2.
    pub const H2: Isotope = Isotope::known(1, 2);
    /// Z = 2, A = 3.
    pub const He3: Isotope = Isotope::known(2, 3);
    /// Z = 2, A = 4.
    pub const He4: Isotope = Isotope::known(2, 4);
    /// Z = 3, A = 6.
    pub const Li6: Isotope = Isotope::known(3, 6);
    /// Z = 3, A = 7.
    pub const Li7: Isotope = Isotope::known(3, 7);
    /// Z = 4, A = 9.
    pub const Be9: Isotope = Isotope::known(4, 9);
    /// Z = 5, A = 10.
    pub const B10: Isotope = Isotope::known(5, 10);
    /// Z = 5, A = 11.
    pub const B11: Isotope = Isotope::known(5, 11);
    /// Z = 6, A = 12.
    pub const C12: Isotope = Isotope::known(6, 12);
    /// Z = 6, A = 13.
    pub const C13: Isotope = Isotope::known(6, 13);
    /// Z = 7, A = 14.
    pub const N14: Isotope = Isotope::known(7, 14);
    /// Z = 7, A = 15.
    pub const N15: Isotope = Isotope::known(7, 15);
    /// Z = 8, A = 16.
    pub const O16: Isotope = Isotope::known(8, 16);
    /// Z = 8, A = 17.
    pub const O17: Isotope = Isotope::known(8, 17);
    /// Z = 8, A = 18.
    pub const O18: Isotope = Isotope::known(8, 18);
    /// Z = 9, A = 19.
    pub const F19: Isotope = Isotope::known(9, 19);
    /// Z = 11, A = 23.
    pub const Na23: Isotope = Isotope::known(11, 23);
    /// Z = 13, A = 27.
    pub const Al27: Isotope = Isotope::known(13, 27);
    /// Z = 14, A = 28.
    pub const Si28: Isotope = Isotope::known(14, 28);
    /// Z = 14, A = 29.
    pub const Si29: Isotope = Isotope::known(14, 29);
    /// Z = 14, A = 30.
    pub const Si30: Isotope = Isotope::known(14, 30);
    /// Z = 25, A = 55.
    pub const Mn55: Isotope = Isotope::known(25, 55);
    /// Z = 26, A = 54.
    pub const Fe54: Isotope = Isotope::known(26, 54);
    /// Z = 26, A = 56.
    pub const Fe56: Isotope = Isotope::known(26, 56);
    /// Z = 26, A = 57.
    pub const Fe57: Isotope = Isotope::known(26, 57);
    /// Z = 26, A = 58.
    pub const Fe58: Isotope = Isotope::known(26, 58);
    /// Z = 29, A = 63.
    pub const Cu63: Isotope = Isotope::known(29, 63);
    /// Z = 29, A = 65.
    pub const Cu65: Isotope = Isotope::known(29, 65);
    /// Z = 40, A = 90.
    pub const Zr90: Isotope = Isotope::known(40, 90);
    /// Z = 40, A = 91.
    pub const Zr91: Isotope = Isotope::known(40, 91);
    /// Z = 40, A = 92.
    pub const Zr92: Isotope = Isotope::known(40, 92);
    /// Z = 40, A = 94.
    pub const Zr94: Isotope = Isotope::known(40, 94);
    /// Z = 40, A = 96.
    pub const Zr96: Isotope = Isotope::known(40, 96);
    /// Z = 54, A = 135.
    pub const Xe135: Isotope = Isotope::known(54, 135);
    /// Z = 55, A = 137.
    pub const Cs137: Isotope = Isotope::known(55, 137);
    /// Z = 62, A = 149.
    pub const Sm149: Isotope = Isotope::known(62, 149);
    /// Z = 90, A = 232.
    pub const Th232: Isotope = Isotope::known(90, 232);
    /// Z = 92, A = 233.
    pub const U233: Isotope = Isotope::known(92, 233);
    /// Z = 92, A = 234.
    pub const U234: Isotope = Isotope::known(92, 234);
    /// Z = 92, A = 235.
    pub const U235: Isotope = Isotope::known(92, 235);
    /// Z = 92, A = 236.
    pub const U236: Isotope = Isotope::known(92, 236);
    /// Z = 92, A = 238.
    pub const U238: Isotope = Isotope::known(92, 238);
    /// Z = 94, A = 238.
    pub const Pu238: Isotope = Isotope::known(94, 238);
    /// Z = 94, A = 239.
    pub const Pu239: Isotope = Isotope::known(94, 239);
    /// Z = 94, A = 240.
    pub const Pu240: Isotope = Isotope::known(94, 240);
    /// Z = 94, A = 241.
    pub const Pu241: Isotope = Isotope::known(94, 241);
    /// Z = 94, A = 242.
    pub const Pu242: Isotope = Isotope::known(94, 242);
    /// Z = 95, A = 241.
    pub const Am241: Isotope = Isotope::known(95, 241);
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
        assert_eq!(
            Isotope::new(98, 252).unwrap().element(),
            Element::Californium
        );
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
