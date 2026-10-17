//! **Beginner materials**: what a region is made of, described the way an
//! engineer or a benchmark specification describes it (gh #937, epic #925).
//!
//! Four ways in, and a material carries **no temperature** (temperature is
//! set on the geometry only; maintainer decision, #937, 2026-10-17):
//!
//! | constructor | when |
//! |---|---|
//! | [`Material::fuel`] | heavy metal + chemical family + density; `.contaminant(..)` for impurities |
//! | [`Material::from_atom_densities`] | a specification's own atom densities, entered exactly |
//! | [`Material::from_fractions`] | weight or atom fractions plus a density (alloys, compounds) |
//! | [`Material::water`] | pure light water at `(T, p)` from the IAPWS-IF97 steam tables |
//!
//! Presets with a citation ([`Material::aluminium_1100`]) are starting
//! points, not locks.
//!
//! # The user's input is always described explicitly
//!
//! [`Basis`], [`PpmOf`] and [`Placement`] are **required enums with no
//! default** and no warnings (maintainer decisions on #925, 2026-10-17): an
//! unstated basis cost this workspace a reading once (HTR-10's "17 %",
//! `pebble_beds::htr10`; the boron ppm, gh:#424). A choice is the user's
//! responsibility, and the resolved material's [`MaterialReport`] lists
//! each one.
//!
//! # Physics is on and hidden
//!
//! Thermal scattering is attached from the material's description
//! ([`Material::bound_in`]), never named by the user. Nothing here switches
//! physics off; ablation is reserved for an explicit `advanced` entry (gh
//! #942, ~~not yet built~~ built 2026-10-17: `super::advanced`).
//!
//! The numbers are turned into atom densities only when the material meets
//! a nuclear-data library (see [`super::resolve`]), because weight bases need
//! the library's atomic masses.

use uom::si::f64::{MassDensity, Pressure, Ratio, ThermodynamicTemperature, VolumetricNumberDensity};
use uom::si::volumetric_number_density::per_cubic_centimeter;

use super::isotope::{isotopes, Element, Isotope, Species};
use super::masses::BARN_CM_PER_CUBIC_CM;
use njoy_outram_park_fork::leapr::decks::SabMaterial;

/// A number density. The transport unit is atoms per barn·cm; build one
/// with [`atoms_per_barn_cm`] and read it back with
/// [`to_atoms_per_barn_cm`].
pub type AtomDensity = VolumetricNumberDensity;

/// `value` atoms per barn·cm (`1 /b·cm = 1e24 /cm³`), as a [`AtomDensity`].
pub fn atoms_per_barn_cm(value: f64) -> AtomDensity {
    AtomDensity::new::<per_cubic_centimeter>(value / BARN_CM_PER_CUBIC_CM)
}

/// An [`AtomDensity`] in atoms per barn·cm.
pub fn to_atoms_per_barn_cm(density: AtomDensity) -> f64 {
    density.get::<per_cubic_centimeter>() * BARN_CM_PER_CUBIC_CM
}

/// Whether a fraction, a ppm or an enrichment is **by weight or by atom**.
/// Always stated; there is no default.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Basis {
    /// Mass fraction.
    Weight,
    /// Number (atom) fraction.
    Atom,
}

/// The fissile uranium isotope an [`Enrichment`] counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FissileUranium {
    /// U-235 (the usual case).
    U235,
    /// U-233 (thorium-cycle fuel).
    U233,
}

impl FissileUranium {
    /// The isotope.
    pub fn isotope(self) -> Isotope {
        match self {
            FissileUranium::U235 => isotopes::U235,
            FissileUranium::U233 => isotopes::U233,
        }
    }
}

/// **Uranium enrichment: fissile / (fissile + fertile)**, the fertile isotope
/// being U-238, by weight or by atom.
///
/// Contaminants are outside it: a U-234 contaminant does not change what the
/// enrichment means (maintainer decision, #925, "contaminants add on"). A
/// specification that lists isotopic weight % summing to 100 with U-234
/// included defines its enrichment over all the uranium; enter such a spec
/// with [`Material::from_atom_densities`], or convert it and say so.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Enrichment {
    fissile: FissileUranium,
    basis: Basis,
    value: Ratio,
}

impl Enrichment {
    /// `value` of `fissile` by weight: `m_fissile / (m_fissile + m_U238)`.
    pub fn by_weight(fissile: FissileUranium, value: Ratio) -> Self {
        Self {
            fissile,
            basis: Basis::Weight,
            value,
        }
    }

    /// `value` of `fissile` by atom: `N_fissile / (N_fissile + N_U238)`.
    pub fn by_atom(fissile: FissileUranium, value: Ratio) -> Self {
        Self {
            fissile,
            basis: Basis::Atom,
            value,
        }
    }

    /// The fissile isotope.
    pub fn fissile(&self) -> FissileUranium {
        self.fissile
    }

    /// The basis it was given in.
    pub fn basis(&self) -> Basis {
        self.basis
    }

    /// The value as given.
    pub fn value(&self) -> Ratio {
        self.value
    }
}

/// **A plutonium isotopic vector**, by weight or by atom. Plutonium has no
/// single "enrichment"; the whole vector is given. The fractions must sum to
/// one (within `1e-6`); they are never renormalised silently.
#[derive(Debug, Clone, PartialEq)]
pub struct PuVector {
    basis: Basis,
    fractions: Vec<(Isotope, Ratio)>,
}

impl PuVector {
    /// Weight fractions of each plutonium isotope.
    pub fn by_weight(fractions: impl IntoIterator<Item = (Isotope, Ratio)>) -> Self {
        Self {
            basis: Basis::Weight,
            fractions: fractions.into_iter().collect(),
        }
    }

    /// Atom fractions of each plutonium isotope.
    pub fn by_atom(fractions: impl IntoIterator<Item = (Isotope, Ratio)>) -> Self {
        Self {
            basis: Basis::Atom,
            fractions: fractions.into_iter().collect(),
        }
    }

    /// The basis it was given in.
    pub fn basis(&self) -> Basis {
        self.basis
    }

    /// The fractions as given.
    pub fn fractions(&self) -> &[(Isotope, Ratio)] {
        &self.fractions
    }
}

/// **The heavy metal of a fuel.** Uranium, plutonium, or both (MOX).
#[derive(Debug, Clone, PartialEq)]
pub enum HeavyMetal {
    /// Uranium at an enrichment (fissile U-235 or U-233 against U-238).
    Uranium(Enrichment),
    /// Plutonium with its isotopic vector.
    Plutonium(PuVector),
    /// Mixed oxide and the like: `plutonium_fraction` is Pu / (U + Pu) on
    /// `basis`.
    Mixed {
        /// The uranium part.
        uranium: Enrichment,
        /// The plutonium part.
        plutonium: PuVector,
        /// Pu / (U + Pu).
        plutonium_fraction: Ratio,
        /// Whether `plutonium_fraction` is by weight or by atom.
        basis: Basis,
    },
}

/// The molten-salt carrier of a [`FuelForm::MoltenSalt`] fuel.
///
/// **Not yet supported**: the carrier compositions and how the heavy metal
/// enters them are open on #937. A material with this form fails to resolve
/// with [`super::MaterialError::UnsupportedFuelForm`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaltCarrier {
    /// LiF-BeF2.
    Flibe,
    /// LiF-NaF-KF.
    Flinak,
}

/// **The fuel's chemical family** (maintainer decision, #925: families, not
/// compounds). The family fixes the non-heavy-metal atoms per heavy-metal
/// atom; those atoms enter at their natural isotopic composition.
///
/// The stoichiometries below are **provisional** (whether each family
/// assumes one, or carries it as data like [`FuelForm::Oxycarbide`], is open
/// on #937):
///
/// | family | partner atoms per heavy-metal atom |
/// |---|---|
/// | `Metallic` | none |
/// | `Oxide` | 2 O (UO2, PuO2) |
/// | `Carbide` | 1 C (UC) |
/// | `Oxycarbide` | as given |
/// | `Nitride` | 1 N (UN) |
/// | `Silicide` | 2/3 Si (U3Si2) |
/// | `Hydride`, `MoltenSalt` | not yet supported (resolution error) |
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FuelForm {
    /// Metal (Godiva, Jemima).
    Metallic,
    /// Dioxide.
    Oxide,
    /// Monocarbide.
    Carbide,
    /// Oxycarbide (UCO kernels), with its carbon and oxygen per heavy-metal
    /// atom.
    Oxycarbide {
        /// C / HM, atoms.
        c_to_hm: Ratio,
        /// O / HM, atoms.
        o_to_hm: Ratio,
    },
    /// Mononitride.
    Nitride,
    /// U3Si2.
    Silicide,
    /// U-ZrH (TRIGA). Not yet supported.
    Hydride,
    /// Heavy metal in a salt carrier. Not yet supported.
    MoltenSalt(SaltCarrier),
}

/// **What a ppm is relative to.** Required, no default (#925).
///
/// Every reference is the material **before** contaminants are applied, so
/// the order contaminants are listed in changes nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PpmOf {
    /// Relative to the heavy metal (all isotopes with `Z >= 89`), e.g. ppm of
    /// the uranium. The maintainer's expected usual choice for fuel.
    HeavyMetal,
    /// Relative to the whole material (all atoms, or all mass).
    WholeMaterial,
    /// Atoms per atom of the host material, e.g. HTR-10's boron as atoms
    /// per carbon atom in graphite (gh:#424). Needs [`Basis::Atom`]. For a
    /// one-element host this is the same number as `WholeMaterial` by atom;
    /// it exists so the reading is said plainly.
    PerHostAtom,
}

/// **How a contaminant enters the composition.** Required, no default and
/// no warnings (maintainer decision, #925, 2026-10-17, correcting an earlier
/// default of `AddOn`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Placement {
    /// Adds atoms on top of the fixed material and displaces nothing. The
    /// given density is the base material's, so the total mass density ends
    /// up slightly higher (the [`MaterialReport`] gives it). **Correct in the
    /// dilute limit** (ppm-level contaminants), where the added mass is
    /// negligible; at the per-cent level it is the user's statement that
    /// the contaminant really is extra.
    AddOn,
    /// Takes its amount out of the fertile isotope (U-238), in the ppm's own
    /// basis: by atom it removes as many U-238 atoms as it adds, by weight
    /// as much U-238 mass (so the given density is kept exactly). Keeps the
    /// fissile content. Not defined for plutonium-only fuel (open on #937).
    DisplaceFertile,
    /// Takes its amount out of all heavy metal pro rata, in the ppm's own
    /// basis. Keeps the enrichment ratio.
    DisplaceHeavyMetal,
}

/// **A contaminant amount**: a ppm with every description of it explicit.
///
/// ```
/// use outram_mc_libs::model::prelude::*;
/// use uom::si::{f64::Ratio, ratio::part_per_million};
/// let c = Contaminant::ppm(Ratio::new::<part_per_million>(4.0), Basis::Atom, PpmOf::HeavyMetal, Placement::AddOn);
/// assert_eq!(c.placement(), Placement::AddOn);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Contaminant {
    value: Ratio,
    basis: Basis,
    of: PpmOf,
    placement: Placement,
}

impl Contaminant {
    /// `value` (use `uom::si::ratio::part_per_million`) on `basis`, relative
    /// to `of`, entering by `placement`. All four are required.
    pub fn ppm(value: Ratio, basis: Basis, of: PpmOf, placement: Placement) -> Self {
        Self {
            value,
            basis,
            of,
            placement,
        }
    }

    /// The amount.
    pub fn value(&self) -> Ratio {
        self.value
    }

    /// By weight or by atom.
    pub fn basis(&self) -> Basis {
        self.basis
    }

    /// What it is relative to.
    pub fn of(&self) -> PpmOf {
        self.of
    }

    /// How it enters.
    pub fn placement(&self) -> Placement {
        self.placement
    }
}

/// Which graphite a [`Moderator::Graphite`] material is. The three ENDF/B-VIII.0
/// graphite thermal laws differ by porosity; the choice is the user's
/// description, so it has no default.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphiteKind {
    /// Perfect single-crystal graphite (`tsl-crystalline-graphite`).
    Crystalline,
    /// Reactor graphite, 10 % porosity (`tsl-reactor-graphite-10P`).
    ReactorPorosity10,
    /// Reactor graphite, 30 % porosity (`tsl-reactor-graphite-30P`).
    ReactorPorosity30,
}

/// **What chemical environment binds the light atoms of a material**, so the
/// right thermal scattering law (S(α,β)) attaches to the right nuclides.
///
/// This is the user's description of the chemistry, not a physics switch:
/// the laws themselves are always on and always regenerated by LEAPR from the
/// ENDF/B-VIII.0 decks compiled into njoy (maintainer direction 2026-10-14:
/// LEAPR always; the published `tsl-*` tapes are oracles).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Moderator {
    /// Light water: H-1 takes H in H2O.
    LightWater,
    /// Heavy water: H-2 takes D in D2O and O-16 takes O in D2O.
    HeavyWater,
    /// Graphite: every carbon nuclide takes the chosen graphite law.
    Graphite(GraphiteKind),
    /// Polyethylene: H-1 takes H in CH2.
    Polyethylene,
    /// Zirconium hydride: H-1 takes H in ZrH, every zirconium nuclide Zr in ZrH.
    ZirconiumHydride,
}

impl Moderator {
    /// The thermal scattering law `isotope` takes in this environment, or
    /// `None` when it stays free-gas.
    pub fn law_for(self, isotope: Isotope) -> Option<SabMaterial> {
        use isotopes::{H1, H2, O16};
        match self {
            Moderator::LightWater => (isotope == H1).then_some(SabMaterial::HInH2O),
            Moderator::HeavyWater => {
                if isotope == H2 {
                    Some(SabMaterial::DInD2O)
                } else if isotope == O16 {
                    Some(SabMaterial::OInD2O)
                } else {
                    None
                }
            }
            Moderator::Graphite(kind) => (isotope.z() == 6).then_some(match kind {
                GraphiteKind::Crystalline => SabMaterial::CrystallineGraphite,
                GraphiteKind::ReactorPorosity10 => SabMaterial::ReactorGraphite10P,
                GraphiteKind::ReactorPorosity30 => SabMaterial::ReactorGraphite30P,
            }),
            Moderator::Polyethylene => (isotope == H1).then_some(SabMaterial::HInCH2),
            Moderator::ZirconiumHydride => {
                if isotope == H1 {
                    Some(SabMaterial::HInZrH)
                } else if isotope.z() == 40 {
                    Some(SabMaterial::ZrInZrH)
                } else {
                    None
                }
            }
        }
    }
}

/// How a [`Material`]'s composition was given. Read by [`super::resolve`].
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Composition {
    Fuel {
        heavy_metal: HeavyMetal,
        form: FuelForm,
    },
    AtomDensities(Vec<(Species, AtomDensity)>),
    Fractions {
        basis: Basis,
        parts: Vec<(Species, Ratio)>,
    },
}

/// **A material**, as described by the user. It becomes atom densities when
/// it meets a nuclear-data library ([`Material::resolve`]).
///
/// It has no temperature: temperature is set on the geometry
/// ([`super::Model::set_temperature`]).
#[derive(Debug, Clone, PartialEq)]
pub struct Material {
    pub(crate) name: String,
    pub(crate) composition: Composition,
    pub(crate) density: Option<MassDensity>,
    pub(crate) contaminants: Vec<(Species, Contaminant)>,
    pub(crate) moderator: Option<Moderator>,
}

impl Material {
    fn with_composition(name: &str, composition: Composition) -> Self {
        Self {
            name: name.to_string(),
            composition,
            density: None,
            contaminants: Vec::new(),
            moderator: None,
        }
    }

    /// **A fuel**: its heavy metal and chemical family. Give its density with
    /// [`Self::density`] (required: resolving without one is an error) and
    /// any impurities with [`Self::contaminant`].
    pub fn fuel(heavy_metal: HeavyMetal, form: FuelForm) -> Self {
        Self::with_composition("fuel", Composition::Fuel { heavy_metal, form })
    }

    /// **A specification's own atom densities, entered exactly**: no
    /// enrichment, no conversion, no density needed. The honest path for a
    /// benchmark that tabulates atom densities (maintainer, #943: "the spec
    /// wins"). A natural element entry expands at natural abundance.
    pub fn from_atom_densities<S: Into<Species>>(
        entries: impl IntoIterator<Item = (S, AtomDensity)>,
    ) -> Self {
        let v = entries.into_iter().map(|(s, d)| (s.into(), d)).collect();
        Self::with_composition("material", Composition::AtomDensities(v))
    }

    /// **Weight or atom fractions** of each entry, which must sum to one
    /// (within `1e-6`; never renormalised silently). Give the mass density
    /// with [`Self::density`]. The way to enter any alloy or compound by its
    /// own composition.
    pub fn from_fractions<S: Into<Species>>(
        basis: Basis,
        parts: impl IntoIterator<Item = (S, Ratio)>,
    ) -> Self {
        let parts = parts.into_iter().map(|(s, r)| (s.into(), r)).collect();
        Self::with_composition("material", Composition::Fractions { basis, parts })
    }

    /// **Pure light water at `(temperature, pressure)`**, density from the
    /// IAPWS-IF97 steam tables (`tampines-steam-tables`,
    /// `try_rho_tp_eqm_single_phase`), as OpenMC's `borated_water` helper
    /// takes T and p (`openmc/model/funcs.py:18`). H2O with natural hydrogen
    /// and oxygen; H-1 takes the H in H2O thermal law (built in).
    ///
    /// Steam-table water is **pure** water: a benchmark that tabulates its
    /// water's atom densities should be entered with
    /// [`Self::from_atom_densities`] (the spec wins, #943).
    ///
    /// # Errors
    /// [`super::MaterialError::WaterNotLiquid`] when `(T, p)` is not
    /// subcooled liquid (the H in H2O law describes liquid water), or the
    /// steam tables' own range error.
    pub fn water(
        temperature: ThermodynamicTemperature,
        pressure: Pressure,
    ) -> Result<Self, super::MaterialError> {
        let rho = super::resolve::liquid_water_density(temperature, pressure)?;
        let third = Ratio::new::<uom::si::ratio::ratio>(1.0 / 3.0);
        let two_thirds = Ratio::new::<uom::si::ratio::ratio>(2.0 / 3.0);
        Ok(Self::from_fractions(
            Basis::Atom,
            [
                (Element::Hydrogen.natural(), two_thirds),
                (Element::Oxygen.natural(), third),
            ],
        )
        .density(rho)
        .bound_in(Moderator::LightWater)
        .named("water"))
    }

    /// **1100 aluminium as the ICSBEP HEU-SOL-THERM-009 case 1 model gives
    /// it**: Al-27 5.9699e-2, Si-28/29/30 (5.5202e-4 /b·cm in total, split
    /// by the model) and Mn-55 1.4853e-5 atoms per barn·cm. Source: the
    /// OpenMC model in `mit-crpg/benchmarks`
    /// (`icsbep/heu-sol-therm-009/openmc/case-1`), as transcribed in
    /// [`crate::vv::hst009`].
    ///
    /// **That model leaves out the alloy's copper and zinc** (recorded in
    /// `vv.rs`), so this is a benchmark's tank, not a full alloy
    /// specification. A preset is a starting point: give your own with
    /// [`Self::from_fractions`].
    pub fn aluminium_1100() -> Self {
        use isotopes::{Al27, Mn55, Si28, Si29, Si30};
        Self::from_atom_densities([
            (Al27, atoms_per_barn_cm(5.9699e-02)),
            (Si28, atoms_per_barn_cm(5.09126279536e-04)),
            (Si29, atoms_per_barn_cm(2.5851979832e-05)),
            (Si30, atoms_per_barn_cm(1.7041740632e-05)),
            (Mn55, atoms_per_barn_cm(1.4853e-05)),
        ])
        .named("1100 aluminium (HEU-SOL-THERM-009 model)")
    }

    /// The mass density of the base material (before add-on contaminants).
    /// Required by [`Self::fuel`] and [`Self::from_fractions`]; ignored by
    /// [`Self::from_atom_densities`], whose densities are already absolute.
    pub fn density(mut self, density: MassDensity) -> Self {
        self.density = Some(density);
        self
    }

    /// Add a contaminant (an isotope, or a natural element:
    /// `Element::Boron.natural()`), with its amount fully described.
    pub fn contaminant(mut self, species: impl Into<Species>, amount: Contaminant) -> Self {
        self.contaminants.push((species.into(), amount));
        self
    }

    /// Say what chemical environment binds this material's light atoms, so the
    /// matching thermal scattering laws attach (see [`Moderator`]).
    pub fn bound_in(mut self, moderator: Moderator) -> Self {
        self.moderator = Some(moderator);
        self
    }

    /// A label for output (plots, reports). Identity is the value, not the
    /// name.
    pub fn named(mut self, name: &str) -> Self {
        self.name = name.to_string();
        self
    }

    /// The label.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The moderator description, if one was given.
    pub fn moderator(&self) -> Option<Moderator> {
        self.moderator
    }

    /// The contaminants, as given.
    pub fn contaminants(&self) -> &[(Species, Contaminant)] {
        &self.contaminants
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::ratio::percent;

    /// The descriptive enums map to the right thermal laws, and the
    /// builders keep what they were given.
    #[test]
    fn moderators_attach_laws_by_nuclide_and_builders_keep_input() {
        use isotopes::*;
        assert_eq!(Moderator::LightWater.law_for(H1), Some(SabMaterial::HInH2O));
        assert_eq!(Moderator::LightWater.law_for(O16), None);
        assert_eq!(Moderator::HeavyWater.law_for(H2), Some(SabMaterial::DInD2O));
        assert_eq!(
            Moderator::HeavyWater.law_for(O16),
            Some(SabMaterial::OInD2O)
        );
        assert_eq!(Moderator::HeavyWater.law_for(H1), None);
        assert_eq!(
            Moderator::Graphite(GraphiteKind::ReactorPorosity10).law_for(C12),
            Some(SabMaterial::ReactorGraphite10P)
        );
        assert_eq!(
            Moderator::Graphite(GraphiteKind::Crystalline).law_for(C13),
            Some(SabMaterial::CrystallineGraphite)
        );
        assert_eq!(
            Moderator::Graphite(GraphiteKind::ReactorPorosity30).law_for(C12),
            Some(SabMaterial::ReactorGraphite30P)
        );
        assert_eq!(
            Moderator::Polyethylene.law_for(H1),
            Some(SabMaterial::HInCH2)
        );
        assert_eq!(
            Moderator::ZirconiumHydride.law_for(H1),
            Some(SabMaterial::HInZrH)
        );
        assert_eq!(
            Moderator::ZirconiumHydride.law_for(Zr90),
            Some(SabMaterial::ZrInZrH)
        );
        assert_eq!(Moderator::ZirconiumHydride.law_for(U235), None);

        let e = Enrichment::by_weight(FissileUranium::U235, Ratio::new::<percent>(17.0));
        assert_eq!(
            (e.fissile(), e.basis()),
            (FissileUranium::U235, Basis::Weight)
        );
        assert!((e.value().get::<percent>() - 17.0).abs() < 1e-12);
        assert_eq!(
            Enrichment::by_atom(FissileUranium::U233, e.value())
                .fissile()
                .isotope(),
            U233
        );
        let pu = PuVector::by_atom([(Pu239, Ratio::new::<percent>(100.0))]);
        assert_eq!((pu.basis(), pu.fractions().len()), (Basis::Atom, 1));
        assert_eq!(
            PuVector::by_weight([(Pu239, Ratio::new::<percent>(100.0))]).basis(),
            Basis::Weight
        );
        let c = Contaminant::ppm(
            Ratio::new::<percent>(1.0),
            Basis::Weight,
            PpmOf::WholeMaterial,
            Placement::DisplaceHeavyMetal,
        );
        assert_eq!(
            (c.basis(), c.of(), c.placement()),
            (
                Basis::Weight,
                PpmOf::WholeMaterial,
                Placement::DisplaceHeavyMetal
            )
        );
        assert!((c.value().get::<percent>() - 1.0).abs() < 1e-12);
        let m = Material::fuel(HeavyMetal::Uranium(e), FuelForm::Oxide)
            .contaminant(Element::Boron.natural(), c)
            .bound_in(Moderator::LightWater)
            .named("uo2");
        assert_eq!(
            (m.name(), m.moderator(), m.contaminants().len()),
            ("uo2", Some(Moderator::LightWater), 1)
        );
        assert!((to_atoms_per_barn_cm(atoms_per_barn_cm(4.4994e-2)) - 4.4994e-2).abs() < 1e-17);
        assert_eq!(
            Material::aluminium_1100().name(),
            "1100 aluminium (HEU-SOL-THERM-009 model)"
        );
    }
}
