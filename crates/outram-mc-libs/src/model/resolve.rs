//! **A [`Material`] meets a library**: its description becomes atom
//! densities, with a [`MaterialReport`] saying how.
//!
//! The arithmetic, all in atoms per barn·cm and `u`:
//!
//! - **Fuel.** The heavy-metal atom fractions `f_i` come from the
//!   enrichment or vector (weight fractions convert as `f ∝ w / m`); the
//!   family adds `s_k` partner atoms per heavy-metal atom, each expanded at
//!   natural abundance. With `M = Σ f_i m_i + Σ s_k m_k`, the heavy-metal
//!   density is `N_HM = ρ N_A / M × 1e-24`.
//! - **Fractions.** Atom fractions `x_j` (weight fractions convert as
//!   above), `N = ρ N_A / Σ x_j m_j × 1e-24`, `N_j = x_j N`.
//! - **Atom densities.** As given; a natural element splits by abundance.
//! - **Contaminants**, each against the material *before* any contaminant
//!   ([`PpmOf`]), then placed ([`Placement`]).
//!
//! A natural element is expanded as OpenMC's `Element.expand` does with a
//! library (`openmc/element.py:150-195`, ported for the plotter in
//! `plotter::xs::expand_element`): all natural isotopes when the library has
//! them; else the library's natural-element evaluation (`A = 0`); else O-17
//! and O-18 fold into O-16, Ta-180m into Ta-181 and W-180 into W-182 (and the
//! report says so); anything else missing is an error.

use uom::si::f64::{MassDensity, Pressure, ThermodynamicTemperature};
use uom::si::mass_density::gram_per_cubic_centimeter;
use uom::si::ratio::ratio;

use super::isotope::{isotopes, Element, Isotope, Species};
use super::masses::{AtomicMasses, MassSource, AVOGADRO_PER_MOL, BARN_CM_PER_CUBIC_CM};
use super::material::{
    atoms_per_barn_cm, to_atoms_per_barn_cm, AtomDensity, Basis, Composition, Contaminant,
    Enrichment, FuelForm, HeavyMetal, Material, Moderator, Placement, PpmOf, PuVector,
};

/// Why a material could not be resolved. Every case names what to change.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum MaterialError {
    /// A nuclide the material needs is not in the library.
    #[error("material '{material}' needs {isotope}, which the nuclear-data library does not hold")]
    NotInLibrary {
        /// The material.
        material: String,
        /// The missing nuclide.
        isotope: Isotope,
    },
    /// `fuel` or `from_fractions` without `.density(..)`.
    #[error("material '{0}' needs a mass density: add .density(..)")]
    MissingDensity(String),
    /// Fractions that do not sum to one.
    #[error("material '{material}': fractions sum to {sum}, not 1 (they are never renormalised silently)")]
    FractionsDoNotSumToOne {
        /// The material.
        material: String,
        /// Their sum.
        sum: f64,
    },
    /// An enrichment or fraction outside `[0, 1]`, or a negative amount.
    #[error("material '{material}': {what} = {value} is outside its valid range")]
    OutOfRange {
        /// The material.
        material: String,
        /// Which input.
        what: &'static str,
        /// The value given.
        value: f64,
    },
    /// A plutonium vector entry that is not a plutonium isotope.
    #[error("material '{material}': {isotope} in a plutonium vector is not plutonium")]
    NotPlutonium {
        /// The material.
        material: String,
        /// The entry.
        isotope: Isotope,
    },
    /// `PpmOf::HeavyMetal` or a displacing placement on a material with no
    /// heavy metal.
    #[error("material '{0}' has no heavy metal (Z >= 89) for PpmOf::HeavyMetal or a displacing placement")]
    NoHeavyMetal(String),
    /// `Placement::DisplaceFertile` with no U-238 to displace.
    #[error("material '{0}' has no U-238 for Placement::DisplaceFertile (not defined for plutonium-only fuel yet, #937)")]
    NoFertileIsotope(String),
    /// A displacement larger than what it displaces from.
    #[error("material '{0}': a contaminant displaces more than the material holds")]
    DisplacementExceedsAvailable(String),
    /// `PpmOf::PerHostAtom` on a weight basis.
    #[error("material '{0}': PpmOf::PerHostAtom counts atoms, so it needs Basis::Atom")]
    PerHostAtomNeedsAtomBasis(String),
    /// A fuel family whose composition is not implemented yet.
    #[error("material '{material}': fuel form {form} is not supported yet (open on #937)")]
    UnsupportedFuelForm {
        /// The material.
        material: String,
        /// The form.
        form: String,
    },
    /// A natural element with no stable isotope.
    #[error("{0:?} has no natural isotopic composition")]
    NoNaturalComposition(Element),
    /// Natural isotopes the library lacks and that OpenMC's rules cannot fold.
    #[error("cannot use natural {element:?}: its isotopes {names} are not in the nuclear-data library ({lib}), so the library does not contain them and no OpenMC rule folds them into an isotope it does hold; add their tapes to the library, or enter the isotopes the library has instead of the natural element", names = isotope_list(.missing), lib = .library)]
    CannotExpand {
        /// The element.
        element: Element,
        /// The natural isotopes the library does not contain.
        missing: Vec<Isotope>,
        /// Where the library came from, when known (its label, which names
        /// the folder or archive it was loaded from).
        library: String,
    },
    /// A material with no entries.
    #[error("material '{0}' has no entries")]
    Empty(String),
    /// [`Material::water`] at a state that is not subcooled liquid.
    #[error("water at {temperature_k} K and {pressure_pa} Pa is not subcooled liquid; the H in H2O law describes liquid water")]
    WaterNotLiquid {
        /// Temperature \[K\].
        temperature_k: f64,
        /// Pressure \[Pa\].
        pressure_pa: f64,
    },
    /// The steam tables refused the state.
    #[error("steam tables: {0}")]
    SteamTables(String),
}

/// `"Fe-54, Fe-56"` for an error message.
fn isotope_list(isotopes: &[Isotope]) -> String {
    isotopes
        .iter()
        .map(|i| i.to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

/// **How a material was resolved**: what a reader needs to check it.
#[derive(Debug, Clone, PartialEq)]
pub struct MaterialReport {
    /// Mass density as given (the base material), when one was.
    pub given_density: Option<MassDensity>,
    /// Total mass density of the resolved material, contaminants included.
    pub total_density: MassDensity,
    /// One line per contaminant: what, how much, on which basis, relative to
    /// what, and how it was placed.
    pub contaminants: Vec<String>,
    /// Natural-element isotopes folded into another, per OpenMC's rule.
    pub folds: Vec<String>,
    /// Where the atomic masses came from.
    pub mass_source: MassSource,
}

/// **A resolved material**: nuclides and atom densities, ready for
/// transport.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedMaterial {
    /// The label.
    pub name: String,
    /// `(nuclide, atom density)` in the order given (base first, then new
    /// contaminant nuclides).
    pub nuclides: Vec<(Isotope, AtomDensity)>,
    /// The chemical environment, which decides the thermal laws.
    pub moderator: Option<Moderator>,
    /// How it was resolved.
    pub report: MaterialReport,
}

impl ResolvedMaterial {
    /// The atom density of `isotope` in atoms per barn·cm (0 when absent).
    pub fn atoms_per_barn_cm(&self, isotope: Isotope) -> f64 {
        self.nuclides
            .iter()
            .filter(|(i, _)| *i == isotope)
            .map(|(_, d)| to_atoms_per_barn_cm(*d))
            .fold(0.0, |a, b| a + b) // `sum()` of nothing is -0.0, printed "-0"
    }

    /// A side-by-side table of this material's and `other`'s atom densities,
    /// with the relative difference per nuclide (the "build it both ways"
    /// check, #925 / #943). [`NumberDensityComparison::max_relative_difference`]
    /// is the single number to gate on.
    pub fn compare_number_densities(&self, other: &ResolvedMaterial) -> NumberDensityComparison {
        let mut all: Vec<Isotope> = self.nuclides.iter().map(|(i, _)| *i).collect();
        for (i, _) in &other.nuclides {
            if !all.contains(i) {
                all.push(*i);
            }
        }
        let rows = all
            .into_iter()
            .map(|i| (i, self.atoms_per_barn_cm(i), other.atoms_per_barn_cm(i)))
            .collect();
        NumberDensityComparison {
            left: self.name.clone(),
            right: other.name.clone(),
            rows,
        }
    }
}

/// Two materials' atom densities side by side. `Display` prints the table.
#[derive(Debug, Clone, PartialEq)]
pub struct NumberDensityComparison {
    /// Label of the first material.
    pub left: String,
    /// Label of the second.
    pub right: String,
    /// `(nuclide, first, second)` in atoms per barn·cm.
    pub rows: Vec<(Isotope, f64, f64)>,
}

impl NumberDensityComparison {
    /// Largest `|a − b| / max(|a|, |b|)` over the nuclides (0 when equal; 1
    /// when a nuclide is in one material only).
    pub fn max_relative_difference(&self) -> f64 {
        self.rows
            .iter()
            .map(|(_, a, b)| {
                let s = a.abs().max(b.abs());
                if s == 0.0 {
                    0.0
                } else {
                    (a - b).abs() / s
                }
            })
            .fold(0.0, f64::max)
    }
}

impl std::fmt::Display for NumberDensityComparison {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(
            f,
            "  nuclide   {:>14}   {:>14}   rel. diff  [atoms/b-cm]",
            self.left, self.right
        )?;
        for (i, a, b) in &self.rows {
            let s = a.abs().max(b.abs());
            let d = if s == 0.0 { 0.0 } else { (a - b) / s };
            writeln!(f, "  {:<8}  {a:>14.6e}   {b:>14.6e}   {d:+.2e}", i.name())?;
        }
        write!(
            f,
            "  largest relative difference: {:.2e}",
            self.max_relative_difference()
        )
    }
}

/// Density of subcooled liquid water at `(t, p)` from the IF97 tables.
pub(crate) fn liquid_water_density(
    t: ThermodynamicTemperature,
    p: Pressure,
) -> Result<MassDensity, MaterialError> {
    use tampines_steam_tables::interfaces::functional_programming::pt_flash_eqm::{
        region_fwd_eqn_single_phase, FwdEqnRegion,
    };
    let rho = tampines_steam_tables::interfaces::checked::try_rho_tp_eqm_single_phase(t, p)
        .map_err(|e| MaterialError::SteamTables(e.to_string()))?;
    match region_fwd_eqn_single_phase(t, p) {
        FwdEqnRegion::Region1 => Ok(rho),
        _ => Err(MaterialError::WaterNotLiquid {
            temperature_k: t.get::<uom::si::thermodynamic_temperature::kelvin>(),
            pressure_pa: p.get::<uom::si::pressure::pascal>(),
        }),
    }
}

/// Working state: `(isotope, atoms/b-cm)` in insertion order.
#[derive(Default)]
struct Densities(Vec<(Isotope, f64)>);

impl Densities {
    fn add(&mut self, i: Isotope, n: f64) {
        match self.0.iter_mut().find(|(j, _)| *j == i) {
            Some(e) => e.1 += n,
            None => self.0.push((i, n)),
        }
    }
}

/// The resolution context: one material against one library's masses.
struct Ctx<'a> {
    name: &'a str,
    masses: &'a AtomicMasses,
    folds: Vec<String>,
}

impl Ctx<'_> {
    fn mass(&self, i: Isotope) -> Result<f64, MaterialError> {
        self.masses
            .mass_u(i)
            .ok_or_else(|| MaterialError::NotInLibrary {
                material: self.name.to_string(),
                isotope: i,
            })
    }

    fn range(&self, what: &'static str, value: f64) -> Result<f64, MaterialError> {
        if (0.0..=1.0).contains(&value) {
            Ok(value)
        } else {
            Err(MaterialError::OutOfRange {
                material: self.name.to_string(),
                what,
                value,
            })
        }
    }

    /// `(isotope, atom fraction within the species)`.
    fn expand(&mut self, s: Species) -> Result<Vec<(Isotope, f64)>, MaterialError> {
        let e = match s {
            Species::Isotope(i) => {
                self.mass(i)?;
                return Ok(vec![(i, 1.0)]);
            }
            Species::Natural(e) => e,
        };
        let natural = e.natural_abundance();
        if natural.is_empty() {
            return Err(MaterialError::NoNaturalComposition(e));
        }
        let (mutual, absent): (Vec<(Isotope, f64)>, Vec<(Isotope, f64)>) = natural
            .iter()
            .copied()
            .partition(|(i, _)| self.masses.contains(*i));
        if absent.is_empty() {
            return Ok(natural);
        }
        let nat = Isotope::new(e.z(), 0).map_err(|_| MaterialError::NoNaturalComposition(e))?;
        if self.masses.contains(nat) {
            self.folds.push(format!(
                "natural {} taken as the library's {} evaluation",
                e.symbol(),
                nat
            ));
            return Ok(vec![(nat, 1.0)]);
        }
        let library = match self.masses.source() {
            MassSource::EndfAwr(lib) => lib.clone(),
            MassSource::Given(why) => format!("masses given by the caller: {why}"),
        };
        if mutual.is_empty() {
            return Err(MaterialError::CannotExpand {
                element: e,
                missing: absent.iter().map(|&(i, _)| i).collect(),
                library,
            });
        }
        let mut unfolded: Vec<Isotope> = Vec::new();
        let mut out: Vec<(Isotope, f64)> = mutual.clone();
        for &(missing, a) in &absent {
            let target = match (missing.z(), missing.a(), missing.metastable_state()) {
                (8, 17 | 18, _) => Some(isotopes::O16),
                (73, 180, 1) => Isotope::new(73, 181).ok(),
                (74, 180, _) => Isotope::new(74, 182).ok(),
                _ => None,
            }
            .filter(|t| out.iter().any(|(i, _)| i == t));
            let Some(t) = target else {
                unfolded.push(missing);
                continue;
            };
            if let Some(slot) = out.iter_mut().find(|(i, _)| *i == t) {
                slot.1 += a;
            }
            self.folds.push(format!(
                "{missing} ({a:.6} of natural {}) folded into {t}",
                e.symbol()
            ));
        }
        if !unfolded.is_empty() {
            return Err(MaterialError::CannotExpand {
                element: e,
                missing: unfolded,
                library,
            });
        }
        Ok(out)
    }

    /// Mean mass \[u\] of a set of `(isotope, fraction)` with fractions summing to 1.
    fn mean_mass(&self, set: &[(Isotope, f64)]) -> Result<f64, MaterialError> {
        let mut m = 0.0;
        for &(i, x) in set {
            m += x * self.mass(i)?;
        }
        Ok(m)
    }

    /// Atom fractions from fractions on `basis`.
    fn to_atom(
        &self,
        basis: Basis,
        set: Vec<(Isotope, f64)>,
    ) -> Result<Vec<(Isotope, f64)>, MaterialError> {
        match basis {
            Basis::Atom => Ok(set),
            Basis::Weight => {
                let mut v = Vec::with_capacity(set.len());
                for (i, w) in set {
                    v.push((i, w / self.mass(i)?));
                }
                let s: f64 = v.iter().map(|x| x.1).sum();
                Ok(v.into_iter().map(|(i, x)| (i, x / s)).collect())
            }
        }
    }

    fn uranium(&self, e: &Enrichment) -> Result<Vec<(Isotope, f64)>, MaterialError> {
        let x = self.range("enrichment", e.value().get::<ratio>())?;
        self.to_atom(
            e.basis(),
            vec![(e.fissile().isotope(), x), (isotopes::U238, 1.0 - x)],
        )
    }

    fn plutonium(&self, v: &PuVector) -> Result<Vec<(Isotope, f64)>, MaterialError> {
        let mut set = Vec::new();
        for &(i, r) in v.fractions() {
            if i.z() != 94 {
                return Err(MaterialError::NotPlutonium {
                    material: self.name.to_string(),
                    isotope: i,
                });
            }
            set.push((
                i,
                self.range("plutonium vector fraction", r.get::<ratio>())?,
            ));
        }
        check_sum(self.name, set.iter().map(|x| x.1).sum())?;
        self.to_atom(v.basis(), set)
    }

    fn heavy_metal(&self, hm: &HeavyMetal) -> Result<Vec<(Isotope, f64)>, MaterialError> {
        match hm {
            HeavyMetal::Uranium(e) => self.uranium(e),
            HeavyMetal::Plutonium(v) => self.plutonium(v),
            HeavyMetal::Mixed {
                uranium,
                plutonium,
                plutonium_fraction,
                basis,
            } => {
                let u = self.uranium(uranium)?;
                let pu = self.plutonium(plutonium)?;
                let p = self.range("plutonium fraction", plutonium_fraction.get::<ratio>())?;
                // Pu atom share: by atom it is p; by weight p/m_Pu over the sum.
                let p_atom = match basis {
                    Basis::Atom => p,
                    Basis::Weight => {
                        let (mu, mp) = (self.mean_mass(&u)?, self.mean_mass(&pu)?);
                        (p / mp) / (p / mp + (1.0 - p) / mu)
                    }
                };
                let mut out: Vec<(Isotope, f64)> = u
                    .into_iter()
                    .map(|(i, x)| (i, x * (1.0 - p_atom)))
                    .collect();
                out.extend(pu.into_iter().map(|(i, x)| (i, x * p_atom)));
                Ok(out)
            }
        }
    }

    /// Partner atoms per heavy-metal atom, by element, for a fuel family.
    fn partners(&self, form: &FuelForm) -> Result<Vec<(Element, f64)>, MaterialError> {
        let unsupported = |form: &str| MaterialError::UnsupportedFuelForm {
            material: self.name.to_string(),
            form: form.to_string(),
        };
        Ok(match form {
            FuelForm::Metallic => vec![],
            FuelForm::Oxide => vec![(Element::Oxygen, 2.0)],
            FuelForm::Carbide => vec![(Element::Carbon, 1.0)],
            FuelForm::Oxycarbide { c_to_hm, o_to_hm } => {
                let (c, o) = (c_to_hm.get::<ratio>(), o_to_hm.get::<ratio>());
                if c < 0.0 || o < 0.0 {
                    return Err(MaterialError::OutOfRange {
                        material: self.name.to_string(),
                        what: "oxycarbide C/HM or O/HM",
                        value: c.min(o),
                    });
                }
                vec![(Element::Carbon, c), (Element::Oxygen, o)]
            }
            FuelForm::Nitride => vec![(Element::Nitrogen, 1.0)],
            FuelForm::Silicide => vec![(Element::Silicon, 2.0 / 3.0)],
            FuelForm::Hydride => return Err(unsupported("Hydride")),
            FuelForm::MoltenSalt(c) => return Err(unsupported(&format!("MoltenSalt({c:?})"))),
        })
    }
}

fn check_sum(name: &str, sum: f64) -> Result<(), MaterialError> {
    if (sum - 1.0).abs() > 1.0e-6 {
        Err(MaterialError::FractionsDoNotSumToOne {
            material: name.to_string(),
            sum,
        })
    } else {
        Ok(())
    }
}

fn density_g_cc(m: &Material) -> Result<f64, MaterialError> {
    let rho = m
        .density
        .ok_or_else(|| MaterialError::MissingDensity(m.name.clone()))?
        .get::<gram_per_cubic_centimeter>();
    if rho > 0.0 && rho.is_finite() {
        Ok(rho)
    } else {
        Err(MaterialError::OutOfRange {
            material: m.name.clone(),
            what: "mass density [g/cm3]",
            value: rho,
        })
    }
}

/// Atoms per barn·cm of a set of atom fractions at mass density `rho` g/cm³.
fn number_density(rho: f64, mean_mass_u: f64) -> f64 {
    rho * AVOGADRO_PER_MOL / mean_mass_u * BARN_CM_PER_CUBIC_CM
}

impl Material {
    /// **Turn this description into atom densities** against a library's
    /// nuclides and masses (from [`super::NuclearData::atomic_masses`]).
    ///
    /// # Errors
    /// Any [`MaterialError`]: a nuclide missing from the library, a missing
    /// density, fractions that do not sum to one, an input out of range, a
    /// contaminant description that does not apply, an unsupported fuel
    /// family.
    pub fn resolve(&self, masses: &AtomicMasses) -> Result<ResolvedMaterial, MaterialError> {
        let mut ctx = Ctx {
            name: &self.name,
            masses,
            folds: Vec::new(),
        };
        let mut base = Densities::default();
        match &self.composition {
            Composition::Fuel { heavy_metal, form } => {
                let rho = density_g_cc(self)?;
                let hm = ctx.heavy_metal(heavy_metal)?;
                let mut partner_atoms = Vec::new();
                for (el, per_hm) in ctx.partners(form)? {
                    for (i, a) in ctx.expand(el.natural())? {
                        partner_atoms.push((i, per_hm * a));
                    }
                }
                let mut m_unit = ctx.mean_mass(&hm)?;
                for &(i, s) in &partner_atoms {
                    m_unit += s * ctx.mass(i)?;
                }
                let n_hm = number_density(rho, m_unit);
                for (i, f) in hm {
                    base.add(i, f * n_hm);
                }
                for (i, s) in partner_atoms {
                    base.add(i, s * n_hm);
                }
            }
            Composition::AtomDensities(entries) => {
                if entries.is_empty() {
                    return Err(MaterialError::Empty(self.name.clone()));
                }
                for &(s, d) in entries {
                    let n = to_atoms_per_barn_cm(d);
                    if n < 0.0 || !n.is_finite() {
                        return Err(MaterialError::OutOfRange {
                            material: self.name.clone(),
                            what: "atom density [/b-cm]",
                            value: n,
                        });
                    }
                    for (i, a) in ctx.expand(s)? {
                        base.add(i, a * n);
                    }
                }
            }
            Composition::Fractions { basis, parts } => {
                if parts.is_empty() {
                    return Err(MaterialError::Empty(self.name.clone()));
                }
                let rho = density_g_cc(self)?;
                let mut set: Vec<(Isotope, f64)> = Vec::new();
                let mut sum = 0.0;
                for &(s, r) in parts {
                    let x = ctx.range("fraction", r.get::<ratio>())?;
                    sum += x;
                    let iso = ctx.expand(s)?;
                    // A natural element's weight fraction splits by mass share.
                    let shares: Vec<(Isotope, f64)> = match basis {
                        Basis::Atom => iso,
                        Basis::Weight => {
                            let mbar = ctx.mean_mass(&iso)?;
                            let mut v = Vec::with_capacity(iso.len());
                            for (i, a) in iso {
                                v.push((i, a * ctx.mass(i)? / mbar));
                            }
                            v
                        }
                    };
                    set.extend(shares.into_iter().map(|(i, a)| (i, a * x)));
                }
                check_sum(&self.name, sum)?;
                let atom = ctx.to_atom(*basis, set)?;
                let n = number_density(rho, ctx.mean_mass(&atom)?);
                for (i, x) in atom {
                    base.add(i, x * n);
                }
            }
        }
        let reports = apply_contaminants(&mut ctx, &mut base, &self.contaminants)?;
        let mut total_mass = 0.0;
        for &(i, n) in &base.0 {
            total_mass += n * ctx.mass(i)?;
        }
        let total = MassDensity::new::<gram_per_cubic_centimeter>(
            total_mass / AVOGADRO_PER_MOL / BARN_CM_PER_CUBIC_CM,
        );
        Ok(ResolvedMaterial {
            name: self.name.clone(),
            nuclides: base
                .0
                .into_iter()
                .map(|(i, n)| (i, atoms_per_barn_cm(n)))
                .collect(),
            moderator: self.moderator,
            report: MaterialReport {
                given_density: match self.composition {
                    Composition::AtomDensities(_) => None,
                    _ => self.density,
                },
                total_density: total,
                contaminants: reports,
                folds: ctx.folds,
                mass_source: masses.source().clone(),
            },
        })
    }
}

/// Apply every contaminant against the base (pre-contaminant) totals.
fn apply_contaminants(
    ctx: &mut Ctx<'_>,
    base: &mut Densities,
    list: &[(Species, Contaminant)],
) -> Result<Vec<String>, MaterialError> {
    // Reference totals of the base material: atoms and mass (u·atoms/b-cm).
    let (mut n_hm, mut m_hm, mut n_all, mut m_all) = (0.0, 0.0, 0.0, 0.0);
    for &(i, n) in &base.0 {
        let m = ctx.mass(i)?;
        n_all += n;
        m_all += n * m;
        if i.is_heavy_metal() {
            n_hm += n;
            m_hm += n * m;
        }
    }
    let snapshot: Vec<(Isotope, f64)> = base.0.clone();
    let mut reports = Vec::new();
    for &(s, c) in list {
        let ppm = c.value().get::<ratio>();
        if ppm < 0.0 || !ppm.is_finite() {
            return Err(MaterialError::OutOfRange {
                material: ctx.name.to_string(),
                what: "contaminant amount",
                value: ppm,
            });
        }
        let (ref_n, ref_m) = match c.of() {
            PpmOf::HeavyMetal => {
                if n_hm == 0.0 {
                    return Err(MaterialError::NoHeavyMetal(ctx.name.to_string()));
                }
                (n_hm, m_hm)
            }
            PpmOf::WholeMaterial => (n_all, m_all),
            PpmOf::PerHostAtom => {
                if c.basis() != Basis::Atom {
                    return Err(MaterialError::PerHostAtomNeedsAtomBasis(
                        ctx.name.to_string(),
                    ));
                }
                (n_all, m_all)
            }
        };
        let iso = ctx.expand(s)?;
        let (added_atoms, added_mass) = match c.basis() {
            Basis::Atom => (ppm * ref_n, ppm * ref_n * ctx.mean_mass(&iso)?),
            Basis::Weight => (ppm * ref_m / ctx.mean_mass(&iso)?, ppm * ref_m),
        };
        match c.placement() {
            Placement::AddOn => {}
            Placement::DisplaceFertile => {
                let u238 = isotopes::U238;
                let have = snapshot
                    .iter()
                    .find(|(i, _)| *i == u238)
                    .map(|x| x.1)
                    .unwrap_or(0.0);
                if have == 0.0 {
                    return Err(MaterialError::NoFertileIsotope(ctx.name.to_string()));
                }
                let remove = match c.basis() {
                    Basis::Atom => added_atoms,
                    Basis::Weight => added_mass / ctx.mass(u238)?,
                };
                take(ctx.name, base, u238, remove)?;
            }
            Placement::DisplaceHeavyMetal => {
                if n_hm == 0.0 {
                    return Err(MaterialError::NoHeavyMetal(ctx.name.to_string()));
                }
                for &(i, n) in snapshot.iter().filter(|(i, _)| i.is_heavy_metal()) {
                    let remove = match c.basis() {
                        Basis::Atom => added_atoms * n / n_hm,
                        Basis::Weight => added_mass * n / m_hm,
                    };
                    take(ctx.name, base, i, remove)?;
                }
            }
        }
        for (i, a) in iso {
            base.add(i, a * added_atoms);
        }
        reports.push(format!(
            "{s}: {:.6e} ppm by {:?}, relative to {:?}, placed {:?} ({added_atoms:.6e} atoms/b-cm)",
            ppm * 1.0e6,
            c.basis(),
            c.of(),
            c.placement()
        ));
    }
    Ok(reports)
}

fn take(name: &str, base: &mut Densities, i: Isotope, remove: f64) -> Result<(), MaterialError> {
    let slot = base
        .0
        .iter_mut()
        .find(|(j, _)| *j == i)
        .ok_or_else(|| MaterialError::DisplacementExceedsAvailable(name.to_string()))?;
    if remove > slot.1 {
        return Err(MaterialError::DisplacementExceedsAvailable(
            name.to_string(),
        ));
    }
    slot.1 -= remove;
    Ok(())
}

/// For tests and examples: the masses of `isotopes` keyed by their GNDS name.
#[cfg(test)]
pub(crate) fn masses_for_tests(pairs: &[(Isotope, f64)]) -> AtomicMasses {
    AtomicMasses::new(
        pairs.iter().copied(),
        MassSource::Given("unit-test masses".into()),
    )
}

#[cfg(test)]
mod tests {
    use super::super::isotope::isotopes::*;
    use super::super::material::{FissileUranium, GraphiteKind};
    use super::*;
    use uom::si::f64::Ratio;
    use uom::si::ratio::{part_per_million, percent};

    /// Masses close to ENDF/B-VIII.0's (AWR x m_n), for arithmetic tests.
    fn masses() -> AtomicMasses {
        let mut v: Vec<(Isotope, f64)> = vec![
            (U234, 234.0409),
            (U235, 235.0439),
            (U238, 238.0508),
            (U233, 233.0396),
            (O16, 15.9949),
            (O17, 16.9991),
            (H1, 1.00783),
            (H2, 2.01410),
            (B10, 10.0129),
            (B11, 11.0093),
            (C12, 12.0),
            (C13, 13.0034),
            (N14, 14.0031),
            (N15, 15.0001),
            (Si28, 27.9769),
            (Si29, 28.9765),
            (Si30, 29.9738),
            (Pu239, 239.0522),
            (Pu240, 240.0538),
            (Al27, 26.9815),
            (Mn55, 54.9380),
        ];
        v.sort_by_key(|x| x.0);
        masses_for_tests(&v)
    }

    fn ppm(x: f64) -> Ratio {
        Ratio::new::<part_per_million>(x)
    }

    /// **The #937 acceptance check**: Godiva built through the fuel builder
    /// (enrichment by atom over U-235 + U-238, U-234 as an add-on atom ppm of
    /// the heavy metal, the base mass density) and through the
    /// specification's own atom densities (HEU-MET-FAST-001 as in
    /// `vv::godiva`) give the same number densities to 1e-12. Also by
    /// weight: the same material entered in weight fractions round-trips.
    #[test]
    fn godiva_both_ways_gives_identical_number_densities() {
        let m = masses();
        let (n4, n5, n8) = (4.9184e-4, 4.4994e-2, 2.4984e-3);
        let spec = Material::from_atom_densities([
            (U234, atoms_per_barn_cm(n4)),
            (U235, atoms_per_barn_cm(n5)),
            (U238, atoms_per_barn_cm(n8)),
        ])
        .resolve(&m)
        .unwrap();
        let mu = |i| m.mass_u(i).unwrap();
        let rho_base = (n5 * mu(U235) + n8 * mu(U238)) / AVOGADRO_PER_MOL / BARN_CM_PER_CUBIC_CM;
        let e = n5 / (n5 + n8);
        let c = n4 / (n5 + n8);
        let built = Material::fuel(
            HeavyMetal::Uranium(Enrichment::by_atom(
                FissileUranium::U235,
                Ratio::new::<ratio>(e),
            )),
            FuelForm::Metallic,
        )
        .density(MassDensity::new::<gram_per_cubic_centimeter>(rho_base))
        .contaminant(
            U234,
            Contaminant::ppm(
                Ratio::new::<ratio>(c),
                Basis::Atom,
                PpmOf::HeavyMetal,
                Placement::AddOn,
            ),
        )
        .resolve(&m)
        .unwrap();
        let cmp = spec.compare_number_densities(&built);
        assert!(cmp.max_relative_difference() < 1e-12, "{cmp}");
        assert!(cmp.to_string().contains("U234"));
        assert_eq!(built.report.contaminants.len(), 1);
        assert!(built.report.contaminants[0].contains("AddOn"));
        let total = built
            .report
            .total_density
            .get::<gram_per_cubic_centimeter>();
        assert!(
            total > rho_base
                && (total - spec.report.total_density.get::<gram_per_cubic_centimeter>()).abs()
                    < 1e-12
        );

        // By weight: the enrichment and ppm in weight terms reproduce it too.
        let w5 = n5 * mu(U235) / (n5 * mu(U235) + n8 * mu(U238));
        let w4 = n4 * mu(U234) / (n5 * mu(U235) + n8 * mu(U238));
        let by_w = Material::fuel(
            HeavyMetal::Uranium(Enrichment::by_weight(
                FissileUranium::U235,
                Ratio::new::<ratio>(w5),
            )),
            FuelForm::Metallic,
        )
        .density(MassDensity::new::<gram_per_cubic_centimeter>(rho_base))
        .contaminant(
            U234,
            Contaminant::ppm(
                Ratio::new::<ratio>(w4),
                Basis::Weight,
                PpmOf::HeavyMetal,
                Placement::AddOn,
            ),
        )
        .resolve(&m)
        .unwrap();
        assert!(
            spec.compare_number_densities(&by_w)
                .max_relative_difference()
                < 1e-12
        );
    }

    /// UO2 at 17 wt% (HTR-10's basis): the oxygen follows from the family at
    /// two atoms per uranium atom, folded O-18 into O-16 because this library
    /// has no O-18, and the enrichment by weight is exact.
    #[test]
    fn oxide_stoichiometry_and_natural_oxygen_fold() {
        let m = masses();
        let r = Material::fuel(
            HeavyMetal::Uranium(Enrichment::by_weight(
                FissileUranium::U235,
                Ratio::new::<percent>(17.0),
            )),
            FuelForm::Oxide,
        )
        .density(MassDensity::new::<gram_per_cubic_centimeter>(10.4))
        .resolve(&m)
        .unwrap();
        let (u5, u8) = (r.atoms_per_barn_cm(U235), r.atoms_per_barn_cm(U238));
        let o = r.atoms_per_barn_cm(O16) + r.atoms_per_barn_cm(O17);
        assert!(((o / (u5 + u8)) - 2.0).abs() < 1e-12);
        let w = u5 * 235.0439 / (u5 * 235.0439 + u8 * 238.0508);
        assert!((w - 0.17).abs() < 1e-12);
        assert!(r
            .report
            .folds
            .iter()
            .any(|f| f.contains("O18") && f.contains("O16")));
        let rho = r.report.total_density.get::<gram_per_cubic_centimeter>();
        assert!((rho - 10.4).abs() < 1e-9);
    }

    /// Each placement does what its doc says, PpmOf references the base,
    /// natural boron splits at the IUPAC abundance, and the descriptions
    /// that do not apply are errors, not guesses.
    #[test]
    fn contaminant_placements_and_references() {
        let m = masses();
        let fuel = || {
            Material::fuel(
                HeavyMetal::Uranium(Enrichment::by_atom(
                    FissileUranium::U235,
                    Ratio::new::<percent>(50.0),
                )),
                FuelForm::Metallic,
            )
            .density(MassDensity::new::<gram_per_cubic_centimeter>(19.0))
        };
        let plain = fuel().resolve(&m).unwrap();
        let hm = plain.atoms_per_barn_cm(U235) + plain.atoms_per_barn_cm(U238);

        let disp = fuel()
            .contaminant(
                U234,
                Contaminant::ppm(
                    ppm(1000.0),
                    Basis::Atom,
                    PpmOf::HeavyMetal,
                    Placement::DisplaceFertile,
                ),
            )
            .resolve(&m)
            .unwrap();
        assert!((disp.atoms_per_barn_cm(U234) - 1e-3 * hm).abs() < 1e-15);
        assert!(
            (disp.atoms_per_barn_cm(U238) - (plain.atoms_per_barn_cm(U238) - 1e-3 * hm)).abs()
                < 1e-15
        );
        assert_eq!(disp.atoms_per_barn_cm(U235), plain.atoms_per_barn_cm(U235));

        let pro = fuel()
            .contaminant(
                U234,
                Contaminant::ppm(
                    ppm(1000.0),
                    Basis::Weight,
                    PpmOf::HeavyMetal,
                    Placement::DisplaceHeavyMetal,
                ),
            )
            .resolve(&m)
            .unwrap();
        let ratio_before = plain.atoms_per_barn_cm(U235) / plain.atoms_per_barn_cm(U238);
        let ratio_after = pro.atoms_per_barn_cm(U235) / pro.atoms_per_barn_cm(U238);
        assert!((ratio_before - ratio_after).abs() < 1e-12);
        let rho_after = pro.report.total_density.get::<gram_per_cubic_centimeter>();
        assert!(
            (rho_after - 19.0).abs() < 1e-9,
            "weight displacement keeps the density: {rho_after}"
        );

        let boron = fuel()
            .contaminant(
                Element::Boron.natural(),
                Contaminant::ppm(ppm(4.0), Basis::Atom, PpmOf::HeavyMetal, Placement::AddOn),
            )
            .resolve(&m)
            .unwrap();
        let b = boron.atoms_per_barn_cm(B10) + boron.atoms_per_barn_cm(B11);
        assert!((b - 4e-6 * hm).abs() < 1e-18);
        assert!((boron.atoms_per_barn_cm(B10) / b - 0.1982).abs() < 1e-12);

        let graphite = Material::from_fractions(
            Basis::Atom,
            [(Element::Carbon.natural(), Ratio::new::<percent>(100.0))],
        )
        .density(MassDensity::new::<gram_per_cubic_centimeter>(1.73))
        .bound_in(Moderator::Graphite(GraphiteKind::ReactorPorosity10))
        .contaminant(
            Element::Boron.natural(),
            Contaminant::ppm(ppm(1.3), Basis::Atom, PpmOf::PerHostAtom, Placement::AddOn),
        );
        let g = graphite.resolve(&m).unwrap();
        let c = g.atoms_per_barn_cm(C12) + g.atoms_per_barn_cm(C13);
        let bg = g.atoms_per_barn_cm(B10) + g.atoms_per_barn_cm(B11);
        assert!((bg / c - 1.3e-6).abs() < 1e-15);
        assert_eq!(
            g.moderator,
            Some(Moderator::Graphite(GraphiteKind::ReactorPorosity10))
        );

        let err = |mat: Material| mat.resolve(&m).unwrap_err();
        assert!(matches!(
            err(graphite.clone().contaminant(
                B10,
                Contaminant::ppm(ppm(1.0), Basis::Atom, PpmOf::HeavyMetal, Placement::AddOn)
            )),
            MaterialError::NoHeavyMetal(_)
        ));
        assert!(matches!(
            err(graphite.clone().contaminant(
                B10,
                Contaminant::ppm(
                    ppm(1.0),
                    Basis::Weight,
                    PpmOf::PerHostAtom,
                    Placement::AddOn
                )
            )),
            MaterialError::PerHostAtomNeedsAtomBasis(_)
        ));
        assert!(matches!(
            err(fuel().contaminant(
                U234,
                Contaminant::ppm(
                    ppm(1e6),
                    Basis::Atom,
                    PpmOf::HeavyMetal,
                    Placement::DisplaceFertile
                )
            )),
            MaterialError::DisplacementExceedsAvailable(_)
        ));
        let pu = Material::fuel(
            HeavyMetal::Plutonium(PuVector::by_weight([
                (Pu239, Ratio::new::<percent>(94.0)),
                (Pu240, Ratio::new::<percent>(6.0)),
            ])),
            FuelForm::Metallic,
        )
        .density(MassDensity::new::<gram_per_cubic_centimeter>(19.8));
        assert!(matches!(
            err(pu.clone().contaminant(
                U234,
                Contaminant::ppm(
                    ppm(1.0),
                    Basis::Atom,
                    PpmOf::HeavyMetal,
                    Placement::DisplaceFertile
                )
            )),
            MaterialError::NoFertileIsotope(_)
        ));
        let pr = pu.resolve(&m).unwrap();
        let w = pr.atoms_per_barn_cm(Pu239) * 239.0522
            / (pr.atoms_per_barn_cm(Pu239) * 239.0522 + pr.atoms_per_barn_cm(Pu240) * 240.0538);
        assert!((w - 0.94).abs() < 1e-12);
    }

    /// The remaining paths: MOX, the other families, the refusals.
    #[test]
    fn families_mixed_fuel_and_refusals() {
        let m = masses();
        let e = Enrichment::by_weight(FissileUranium::U235, Ratio::new::<percent>(0.25));
        let pu = PuVector::by_atom([(Pu239, Ratio::new::<percent>(100.0))]);
        let rho = MassDensity::new::<gram_per_cubic_centimeter>(10.0);
        for basis in [Basis::Atom, Basis::Weight] {
            let mox = Material::fuel(
                HeavyMetal::Mixed {
                    uranium: e,
                    plutonium: pu.clone(),
                    plutonium_fraction: Ratio::new::<percent>(7.0),
                    basis,
                },
                FuelForm::Oxide,
            )
            .density(rho)
            .resolve(&m)
            .unwrap();
            let (p, u) = (
                mox.atoms_per_barn_cm(Pu239),
                mox.atoms_per_barn_cm(U235) + mox.atoms_per_barn_cm(U238),
            );
            let share = match basis {
                Basis::Atom => p / (p + u),
                Basis::Weight => {
                    p * 239.0522
                        / (p * 239.0522
                            + mox.atoms_per_barn_cm(U235) * 235.0439
                            + mox.atoms_per_barn_cm(U238) * 238.0508)
                }
            };
            assert!((share - 0.07).abs() < 1e-12, "{basis:?}: {share}");
        }
        let fuel = |form| Material::fuel(HeavyMetal::Uranium(e), form).density(rho);
        let per_hm = |form, el: &[Isotope]| {
            let r = fuel(form).resolve(&m).unwrap();
            let hm = r.atoms_per_barn_cm(U235) + r.atoms_per_barn_cm(U238);
            el.iter().map(|&i| r.atoms_per_barn_cm(i)).sum::<f64>() / hm
        };
        assert!((per_hm(FuelForm::Carbide, &[C12, C13]) - 1.0).abs() < 1e-12);
        assert!((per_hm(FuelForm::Nitride, &[N14, N15]) - 1.0).abs() < 1e-12);
        assert!((per_hm(FuelForm::Silicide, &[Si28, Si29, Si30]) - 2.0 / 3.0).abs() < 1e-12);
        let uco = FuelForm::Oxycarbide {
            c_to_hm: Ratio::new::<ratio>(0.4),
            o_to_hm: Ratio::new::<ratio>(1.5),
        };
        assert!((per_hm(uco, &[C12, C13]) - 0.4).abs() < 1e-12);
        let err = |mat: Material| mat.resolve(&m).unwrap_err();
        assert!(matches!(
            err(fuel(FuelForm::Hydride)),
            MaterialError::UnsupportedFuelForm { .. }
        ));
        assert!(matches!(
            err(fuel(FuelForm::MoltenSalt(
                super::super::material::SaltCarrier::Flibe
            ))),
            MaterialError::UnsupportedFuelForm { .. }
        ));
        assert!(matches!(
            err(fuel(FuelForm::Oxycarbide {
                c_to_hm: Ratio::new::<ratio>(-1.0),
                o_to_hm: Ratio::new::<ratio>(1.0)
            })),
            MaterialError::OutOfRange { .. }
        ));
        assert!(matches!(
            err(Material::fuel(HeavyMetal::Uranium(e), FuelForm::Metallic)),
            MaterialError::MissingDensity(_)
        ));
        let bad_e = Enrichment::by_atom(FissileUranium::U233, Ratio::new::<percent>(120.0));
        assert!(matches!(
            err(Material::fuel(HeavyMetal::Uranium(bad_e), FuelForm::Metallic).density(rho)),
            MaterialError::OutOfRange { .. }
        ));
        let not_pu = PuVector::by_atom([(U235, Ratio::new::<percent>(100.0))]);
        assert!(matches!(
            err(Material::fuel(HeavyMetal::Plutonium(not_pu), FuelForm::Metallic).density(rho)),
            MaterialError::NotPlutonium { .. }
        ));
        let short = PuVector::by_atom([(Pu239, Ratio::new::<percent>(90.0))]);
        assert!(matches!(
            err(Material::fuel(HeavyMetal::Plutonium(short), FuelForm::Metallic).density(rho)),
            MaterialError::FractionsDoNotSumToOne { .. }
        ));
        assert!(matches!(
            err(Material::from_atom_densities([(
                Pu241,
                atoms_per_barn_cm(1e-3)
            )])),
            MaterialError::NotInLibrary { .. }
        ));
        assert!(matches!(
            err(Material::from_atom_densities(
                Vec::<(Isotope, AtomDensity)>::new()
            )),
            MaterialError::Empty(_)
        ));
        assert!(matches!(
            err(Material::from_atom_densities([(
                Element::Technetium.natural(),
                atoms_per_barn_cm(1e-3)
            )])),
            MaterialError::NoNaturalComposition(_)
        ));
        assert!(matches!(
            err(Material::from_atom_densities([(
                Element::Iron.natural(),
                atoms_per_barn_cm(1e-3)
            )])),
            MaterialError::CannotExpand { .. }
        ));
        // The message names the missing isotopes, says the library lacks
        // them, and names where the library came from.
        let msg = err(Material::from_atom_densities([(
            Element::Iron.natural(),
            atoms_per_barn_cm(1e-3),
        )]))
        .to_string();
        assert!(msg.contains("not in the nuclear-data library"), "{msg}");
        assert!(msg.contains("does not contain"), "{msg}");
        assert!(msg.contains("unit-test masses"), "{msg}");
        assert!(msg.contains("Fe54"), "{msg}");
        assert!(matches!(
            err(
                Material::from_fractions(Basis::Weight, [(Al27, Ratio::new::<percent>(50.0))])
                    .density(rho)
            ),
            MaterialError::FractionsDoNotSumToOne { .. }
        ));
    }

    /// Weight fractions with a natural element split by mass share, the
    /// alloy preset resolves to its cited atom densities, and steam-table
    /// water comes out as H2O at the IF97 density and refuses steam.
    #[test]
    fn fractions_preset_and_water() {
        use uom::si::pressure::atmosphere;
        use uom::si::thermodynamic_temperature::kelvin;
        let m = masses();
        let al = Material::aluminium_1100().resolve(&m).unwrap();
        assert!((al.atoms_per_barn_cm(Al27) - 5.9699e-2).abs() < 1e-15);
        assert!(al.report.given_density.is_none());

        let w = Material::from_fractions(
            Basis::Weight,
            [
                (Species::from(Al27), Ratio::new::<percent>(99.0)),
                (Element::Silicon.natural(), Ratio::new::<percent>(1.0)),
            ],
        )
        .density(MassDensity::new::<gram_per_cubic_centimeter>(2.7))
        .resolve(&m)
        .unwrap();
        let mass = |i: Isotope| w.atoms_per_barn_cm(i) * m.mass_u(i).unwrap();
        let si = mass(Si28) + mass(Si29) + mass(Si30);
        assert!((si / (si + mass(Al27)) - 0.01).abs() < 1e-12);
        assert!((w.report.total_density.get::<gram_per_cubic_centimeter>() - 2.7).abs() < 1e-9);

        let t = ThermodynamicTemperature::new::<kelvin>(293.6);
        let water = Material::water(t, Pressure::new::<atmosphere>(1.0)).unwrap();
        assert_eq!(water.moderator(), Some(Moderator::LightWater));
        let r = water.resolve(&m).unwrap();
        let h = r.atoms_per_barn_cm(H1) + r.atoms_per_barn_cm(H2);
        let o = r.atoms_per_barn_cm(O16) + r.atoms_per_barn_cm(O17);
        assert!((h / o - 2.0).abs() < 1e-12);
        let rho = r.report.total_density.get::<gram_per_cubic_centimeter>();
        assert!(
            (rho - 0.998).abs() < 0.002,
            "IF97 liquid water near 1 atm, 293.6 K: {rho}"
        );
        let steam = Material::water(
            ThermodynamicTemperature::new::<kelvin>(500.0),
            Pressure::new::<atmosphere>(1.0),
        );
        assert!(matches!(steam, Err(MaterialError::WaterNotLiquid { .. })));
    }
}
