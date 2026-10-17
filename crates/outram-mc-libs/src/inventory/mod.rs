//! # Radionuclide inventory: what depletion hands to a source-term calculation
//!
//! **GitHub #951** (epic #925, tutorial 20 in #943). Maintainer decision,
//! 2026-10-17: the inventory type comes **from outram-mc and goes to
//! sembawang**, and there is one inventory type in the workspace.
//! `sembawang` takes a [`RadionuclideInventory`] where it used to take its own
//! `CoreInventory`. The dependency direction is `sembawang -> outram-mc-libs`;
//! this crate must never depend on `sembawang`.
//!
//! ## What it carries
//!
//! | What a source term needs | Here |
//! |---|---|
//! | Which nuclide, including the metastable state | [`Isotope`] (Z, A, m), with [`zai`] and [`za`] as integers |
//! | How much, with units | [`Amount`]: activity as `uom` [`Radioactivity`], and the atom count as `uom` [`AmountOfSubstance`] when it is known |
//! | Where in the core | [`Regions`]: the whole core, radial rings (innermost first), or named regions (components, materials) |
//! | When | [`InventoryPoint`]: step index, time, burnup, each optional |
//! | Who produced it | [`InventorySource`]: prescribed from a reference, or a named code and run |
//!
//! ## Two ways in
//!
//! - **Prescribed** ([`RadionuclideInventory::from_activities`]): a caller
//!   supplies activities by nuclide and region, typically a published table.
//!   `sembawang`'s examples work this way, and must keep working.
//! - **From depletion** ([`RadionuclideInventory::from_burnup_step`],
//!   [`RadionuclideInventory::from_burnup_result`]): the number densities of a
//!   [`BurnupStep`] become atom counts (`N * 1e24 * V`, with `N` in
//!   atoms/(barn·cm) and `V` in cm³), and activities `A = lambda * N` with
//!   `lambda` from the same [`DepletionChain`] that produced them. **Every
//!   nuclide of the step is kept**, stable ones included (at zero activity):
//!   a consumer that models fewer nuclides decides what to drop and reports
//!   it; nothing is dropped here.
//!
//! ## What it deliberately does not carry
//!
//! - **An axial mesh.** `sembawang`'s old `CoreInventory` also held `n_axial`,
//!   the number of axial nodes each ring is split into. That is a property of
//!   the temperature transient's mesh, which carries the same number, not of
//!   the inventory; the axial split itself is upstream TRISO-ATOPS's
//!   `distribute_inventory_axially`, applied by `sembawang` at the point of
//!   use.
//! - **Nuclide names in a consumer's spelling.** TRISO-ATOPS spells Ag-110m
//!   `"Ag-110m"`, OpenMC `"Ag110_m1"`. The identity is the [`Isotope`]; a
//!   consumer converts at its own boundary.
//!
//! ## Units
//!
//! This crate's transport loop is raw `f64` (see its `CLAUDE.md`, "Units").
//! This type is a public handoff between crates, not a hot loop, so it is
//! `uom` throughout, per the workspace rule for quantities crossing an API.

use uom::si::amount_of_substance::particle;
use uom::si::f64::{AmountOfSubstance, AvailableEnergy, Radioactivity, Time, Volume};
use uom::si::radioactivity::becquerel;
use uom::si::time::day;
use uom::si::volume::cubic_centimeter;

use crate::depletion::chain::DepletionChain;
use crate::depletion::operator::{BurnupResult, BurnupStep};
use crate::model::Isotope;

/// cm² per barn: a density in atoms/(barn·cm) times this is atoms/cm³.
const ATOMS_PER_CM3_PER_ATOMS_PER_BARN_CM: f64 = 1.0e24;

/// The ZAI integer `Z * 10000 + A * 10 + m` (OpenMC's and Serpent's
/// convention): U-235 is `922350`, Ag-110m `471101`.
#[must_use]
pub fn zai(isotope: Isotope) -> u32 {
    u32::from(isotope.z()) * 10_000
        + u32::from(isotope.a()) * 10
        + u32::from(isotope.metastable_state())
}

/// The ZA integer `Z * 1000 + A` (ENDF's `ZA`). It does **not** carry the
/// metastable state; use [`zai`] when that matters, as it does for Ag-110m.
#[must_use]
pub fn za(isotope: Isotope) -> u32 {
    u32::from(isotope.z()) * 1_000 + u32::from(isotope.a())
}

/// How the inventory is resolved in space.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Regions {
    /// One region: the whole core.
    WholeCore,
    /// Radial rings, **innermost first**. The number of rings.
    RadialRings(usize),
    /// Named regions in order: components, materials, depletion zones.
    Named(Vec<String>),
}

impl Regions {
    /// Number of regions.
    #[must_use]
    pub fn len(&self) -> usize {
        match self {
            Regions::WholeCore => 1,
            Regions::RadialRings(n) => *n,
            Regions::Named(names) => names.len(),
        }
    }

    /// True when there are no regions at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// When the inventory holds: a step index, a time and a burnup, each optional
/// (a published equilibrium-core table states none of them).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct InventoryPoint {
    /// Depletion step index (0 = beginning of life), when it came from one.
    pub step: Option<usize>,
    /// Time since the start of irradiation.
    pub time: Option<Time>,
    /// Burnup, energy per unit initial heavy-metal mass (1 MWd/kgHM is
    /// 8.64e10 J/kg).
    pub burnup: Option<AvailableEnergy>,
}

impl InventoryPoint {
    /// No step, time or burnup stated.
    #[must_use]
    pub fn unspecified() -> Self {
        Self::default()
    }
}

/// Who produced the inventory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InventorySource {
    /// Supplied by the caller. `reference` says where the numbers come from
    /// (a citation, or "shakedown: 1 Ci per nuclide per ring").
    Prescribed {
        /// Citation or description of the numbers.
        reference: String,
    },
    /// Computed by a depletion code.
    Depletion {
        /// The code (`"outram-mc-libs"` for this crate's own depletion).
        code: String,
        /// Which run: chain, integrator, power, step; whatever identifies it.
        run: String,
    },
}

/// How much of one nuclide is in one region.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Amount {
    activity: Radioactivity,
    atoms: Option<AmountOfSubstance>,
}

impl Amount {
    /// An activity alone, as a published table gives it.
    #[must_use]
    pub fn from_activity(activity: Radioactivity) -> Self {
        Self {
            activity,
            atoms: None,
        }
    }

    /// An atom count and its activity, as depletion gives them.
    #[must_use]
    pub fn from_atoms_and_activity(atoms: AmountOfSubstance, activity: Radioactivity) -> Self {
        Self {
            activity,
            atoms: Some(atoms),
        }
    }

    /// The activity.
    #[must_use]
    pub fn activity(&self) -> Radioactivity {
        self.activity
    }

    /// The atom count, when known (`get::<particle>()` reads it as a count).
    #[must_use]
    pub fn atoms(&self) -> Option<AmountOfSubstance> {
        self.atoms
    }
}

/// One nuclide's amount in every region.
#[derive(Debug, Clone, PartialEq)]
pub struct NuclideEntry {
    isotope: Isotope,
    per_region: Vec<Amount>,
}

impl NuclideEntry {
    /// Which nuclide.
    #[must_use]
    pub fn isotope(&self) -> Isotope {
        self.isotope
    }

    /// Its [`zai`] integer.
    #[must_use]
    pub fn zai(&self) -> u32 {
        zai(self.isotope)
    }

    /// The amount in each region, in the inventory's region order.
    #[must_use]
    pub fn per_region(&self) -> &[Amount] {
        &self.per_region
    }

    /// The activity in `region`, or `None` past the last region.
    #[must_use]
    pub fn activity_in(&self, region: usize) -> Option<Radioactivity> {
        self.per_region.get(region).map(Amount::activity)
    }

    /// The activity summed over every region.
    #[must_use]
    pub fn total_activity(&self) -> Radioactivity {
        let bq: f64 = self
            .per_region
            .iter()
            .map(|a| a.activity.get::<becquerel>())
            .sum();
        Radioactivity::new::<becquerel>(bq)
    }
}

/// Why a [`RadionuclideInventory`] could not be built.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum InventoryError {
    /// No nuclides.
    #[error("an inventory needs at least one nuclide")]
    Empty,
    /// No regions.
    #[error("an inventory needs at least one region")]
    NoRegions,
    /// A nuclide does not give one amount per region.
    #[error("{nuclide} gives {got} amounts for {expected} regions")]
    RegionCountMismatch {
        /// The nuclide.
        nuclide: Isotope,
        /// Number of regions.
        expected: usize,
        /// Number of amounts supplied.
        got: usize,
    },
    /// A negative or non-finite activity or atom count.
    #[error("{nuclide} has a negative or non-finite amount in region {region}")]
    BadAmount {
        /// The nuclide.
        nuclide: Isotope,
        /// The region index.
        region: usize,
    },
    /// The same nuclide twice.
    #[error("{0} appears twice")]
    Duplicate(Isotope),
    /// A depletion nuclide name that is not a GNDS name (`U235`, `Ag110_m1`).
    #[error("'{0}' is not a GNDS nuclide name")]
    BadName(String),
    /// A depletion nuclide the chain does not track, so it has no decay
    /// constant.
    #[error("'{0}' is not in the depletion chain")]
    NotInChain(String),
    /// A non-positive or non-finite volume.
    #[error("the depleted volume must be positive and finite")]
    BadVolume,
}

/// **The radionuclide inventory**: nuclides by [`Isotope`], amounts by region,
/// the point it holds at, and who produced it. See the module docs.
///
/// Nuclides keep the order they were given in. That matters downstream:
/// TRISO-ATOPS's normal-operation solve needs a parent before its daughter.
#[derive(Debug, Clone, PartialEq)]
pub struct RadionuclideInventory {
    regions: Regions,
    nuclides: Vec<NuclideEntry>,
    point: InventoryPoint,
    source: InventorySource,
}

impl RadionuclideInventory {
    /// Build and validate.
    ///
    /// # Errors
    /// [`InventoryError::Empty`], [`InventoryError::NoRegions`],
    /// [`InventoryError::RegionCountMismatch`], [`InventoryError::BadAmount`]
    /// (negative or non-finite; `-0.0` is accepted as zero) and
    /// [`InventoryError::Duplicate`].
    pub fn new(
        regions: Regions,
        nuclides: Vec<(Isotope, Vec<Amount>)>,
        point: InventoryPoint,
        source: InventorySource,
    ) -> Result<Self, InventoryError> {
        if nuclides.is_empty() {
            return Err(InventoryError::Empty);
        }
        if regions.is_empty() {
            return Err(InventoryError::NoRegions);
        }
        let n = regions.len();
        let mut entries: Vec<NuclideEntry> = Vec::with_capacity(nuclides.len());
        for (isotope, per_region) in nuclides {
            if per_region.len() != n {
                return Err(InventoryError::RegionCountMismatch {
                    nuclide: isotope,
                    expected: n,
                    got: per_region.len(),
                });
            }
            let bad = |x: f64| !x.is_finite() || x < 0.0;
            if let Some(region) = per_region.iter().position(|a| {
                bad(a.activity.get::<becquerel>())
                    || a.atoms.is_some_and(|m| bad(m.get::<particle>()))
            }) {
                return Err(InventoryError::BadAmount {
                    nuclide: isotope,
                    region,
                });
            }
            if entries.iter().any(|e| e.isotope == isotope) {
                return Err(InventoryError::Duplicate(isotope));
            }
            entries.push(NuclideEntry {
                isotope,
                per_region,
            });
        }
        Ok(Self {
            regions,
            nuclides: entries,
            point,
            source,
        })
    }

    /// **Prescribed**: activities by nuclide and region, as a published table
    /// gives them.
    ///
    /// # Errors
    /// As [`Self::new`].
    pub fn from_activities(
        regions: Regions,
        activities: Vec<(Isotope, Vec<Radioactivity>)>,
        point: InventoryPoint,
        source: InventorySource,
    ) -> Result<Self, InventoryError> {
        let nuclides = activities
            .into_iter()
            .map(|(i, a)| (i, a.into_iter().map(Amount::from_activity).collect()))
            .collect();
        Self::new(regions, nuclides, point, source)
    }

    /// **From depletion**: one recorded step of an outram-mc burnup run, for a
    /// depleted volume `volume`, as one named region `label`.
    ///
    /// Atoms are `N * 1e24 * V` (`N` in atoms/(barn·cm), `V` in cm³);
    /// activities are `lambda * atoms` with `lambda` from `chain`, so a stable
    /// nuclide carries its atoms at zero activity. The point is the step's
    /// index and time; burnup is not recorded by [`BurnupStep`] and is left
    /// unset. The source is `Depletion { code: "outram-mc-libs", run }`.
    ///
    /// To hand a one-material depletion to a ring-resolved consumer, state the
    /// mapping with [`Self::with_regions`].
    ///
    /// # Errors
    /// [`InventoryError::BadVolume`], [`InventoryError::BadName`],
    /// [`InventoryError::NotInChain`], and as [`Self::new`] (a negative
    /// density is refused, not clipped).
    pub fn from_burnup_step(
        step: &BurnupStep,
        chain: &DepletionChain,
        volume: Volume,
        label: &str,
        run: &str,
    ) -> Result<Self, InventoryError> {
        let v_cm3 = volume.get::<cubic_centimeter>();
        if !(v_cm3.is_finite() && v_cm3 > 0.0) {
            return Err(InventoryError::BadVolume);
        }
        let mut nuclides = Vec::with_capacity(step.densities.len());
        for (name, density) in &step.densities {
            let isotope =
                Isotope::from_name(name).map_err(|_| InventoryError::BadName(name.clone()))?;
            let lambda = chain
                .decay_constant_of(name)
                .ok_or_else(|| InventoryError::NotInChain(name.clone()))?;
            let atoms = density * ATOMS_PER_CM3_PER_ATOMS_PER_BARN_CM * v_cm3;
            let amount = Amount::from_atoms_and_activity(
                AmountOfSubstance::new::<particle>(atoms),
                Radioactivity::new::<becquerel>(lambda * atoms),
            );
            nuclides.push((isotope, vec![amount]));
        }
        Self::new(
            Regions::Named(vec![label.to_string()]),
            nuclides,
            InventoryPoint {
                step: Some(step.step),
                time: Some(Time::new::<day>(step.time_days)),
                burnup: None,
            },
            InventorySource::Depletion {
                code: "outram-mc-libs".to_string(),
                run: run.to_string(),
            },
        )
    }

    /// [`Self::from_burnup_step`] for every recorded step of `result`, in
    /// order (`[0]` is beginning of life).
    ///
    /// # Errors
    /// As [`Self::from_burnup_step`], for the first step that fails.
    pub fn from_burnup_result(
        result: &BurnupResult,
        chain: &DepletionChain,
        volume: Volume,
        label: &str,
        run: &str,
    ) -> Result<Vec<Self>, InventoryError> {
        result
            .steps
            .iter()
            .map(|s| Self::from_burnup_step(s, chain, volume, label, run))
            .collect()
    }

    /// The same amounts under a different region description, for a caller
    /// stating how its regions map onto a consumer's (one depleted material as
    /// one radial ring, say). Only the description changes.
    ///
    /// # Errors
    /// [`InventoryError::NoRegions`], or
    /// [`InventoryError::RegionCountMismatch`] (against the first nuclide)
    /// when the region count changes.
    pub fn with_regions(mut self, regions: Regions) -> Result<Self, InventoryError> {
        if regions.is_empty() {
            return Err(InventoryError::NoRegions);
        }
        if regions.len() != self.regions.len() {
            return Err(InventoryError::RegionCountMismatch {
                nuclide: self.nuclides[0].isotope,
                expected: regions.len(),
                got: self.regions.len(),
            });
        }
        self.regions = regions;
        Ok(self)
    }

    /// How the inventory is resolved in space.
    #[must_use]
    pub fn regions(&self) -> &Regions {
        &self.regions
    }

    /// Every nuclide, in the order given.
    #[must_use]
    pub fn nuclides(&self) -> &[NuclideEntry] {
        &self.nuclides
    }

    /// One nuclide's entry, if present.
    #[must_use]
    pub fn get(&self, isotope: Isotope) -> Option<&NuclideEntry> {
        self.nuclides.iter().find(|e| e.isotope == isotope)
    }

    /// When the inventory holds.
    #[must_use]
    pub fn point(&self) -> &InventoryPoint {
        &self.point
    }

    /// Who produced it.
    #[must_use]
    pub fn source(&self) -> &InventorySource {
        &self.source
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::depletion::integrators::Integrator;
    use crate::depletion::operator::{deplete_with, BurnupSettings};
    use crate::model::isotopes::{Cs137, U235};
    use uom::si::radioactivity::curie;

    fn ci(x: f64) -> Radioactivity {
        Radioactivity::new::<curie>(x)
    }

    fn prescribed() -> RadionuclideInventory {
        RadionuclideInventory::from_activities(
            Regions::RadialRings(2),
            vec![
                (Cs137, vec![ci(1.0), ci(3.0)]),
                (
                    Isotope::metastable(47, 110, 1).unwrap(),
                    vec![ci(0.5), ci(0.0)],
                ),
            ],
            InventoryPoint::unspecified(),
            InventorySource::Prescribed {
                reference: "test".into(),
            },
        )
        .unwrap()
    }

    /// ZAI carries the isomeric state and ZA does not; a prescribed inventory
    /// keeps order, regions, activities and provenance exactly as given.
    #[test]
    fn a_prescribed_inventory_keeps_what_it_was_given() {
        assert_eq!(zai(U235), 922_350);
        let ag110m = Isotope::metastable(47, 110, 1).unwrap();
        assert_eq!(zai(ag110m), 471_101);
        assert_eq!(za(ag110m), 47_110);
        let inv = prescribed();
        assert_eq!(inv.regions(), &Regions::RadialRings(2));
        assert_eq!(inv.nuclides()[0].isotope(), Cs137);
        assert_eq!(inv.nuclides()[1].zai(), 471_101);
        assert_eq!(inv.get(Cs137).unwrap().activity_in(1), Some(ci(3.0)));
        assert_eq!(inv.get(Cs137).unwrap().activity_in(2), None);
        assert!((inv.get(Cs137).unwrap().total_activity().get::<curie>() - 4.0).abs() < 1e-12);
        assert!(inv.get(Cs137).unwrap().per_region()[0].atoms().is_none());
        assert!(inv.get(U235).is_none());
        assert_eq!(inv.point(), &InventoryPoint::unspecified());
        assert_eq!(
            inv.source(),
            &InventorySource::Prescribed {
                reference: "test".into()
            }
        );
        assert_eq!(Regions::WholeCore.len(), 1);
        assert_eq!(Regions::Named(vec!["a".into(), "b".into()]).len(), 2);
    }

    /// Every malformed input is an error, never a panic or a silent fix.
    #[test]
    fn malformed_inventories_are_refused() {
        let src = || InventorySource::Prescribed {
            reference: "t".into(),
        };
        let p = InventoryPoint::unspecified;
        assert_eq!(
            RadionuclideInventory::from_activities(Regions::WholeCore, vec![], p(), src()),
            Err(InventoryError::Empty)
        );
        assert_eq!(
            RadionuclideInventory::from_activities(
                Regions::RadialRings(0),
                vec![(Cs137, vec![])],
                p(),
                src()
            ),
            Err(InventoryError::NoRegions)
        );
        assert!(matches!(
            RadionuclideInventory::from_activities(
                Regions::RadialRings(3),
                vec![(Cs137, vec![ci(1.0); 2])],
                p(),
                src()
            ),
            Err(InventoryError::RegionCountMismatch {
                expected: 3,
                got: 2,
                ..
            })
        ));
        assert!(matches!(
            RadionuclideInventory::from_activities(
                Regions::RadialRings(2),
                vec![(Cs137, vec![ci(1.0), ci(-1.0)])],
                p(),
                src()
            ),
            Err(InventoryError::BadAmount { region: 1, .. })
        ));
        assert!(matches!(
            RadionuclideInventory::from_activities(
                Regions::WholeCore,
                vec![(Cs137, vec![ci(f64::NAN)])],
                p(),
                src()
            ),
            Err(InventoryError::BadAmount { region: 0, .. })
        ));
        assert_eq!(
            RadionuclideInventory::from_activities(
                Regions::WholeCore,
                vec![(Cs137, vec![ci(1.0)]), (Cs137, vec![ci(2.0)])],
                p(),
                src()
            ),
            Err(InventoryError::Duplicate(Cs137))
        );
        assert!(prescribed().with_regions(Regions::WholeCore).is_err());
        assert_eq!(
            prescribed().with_regions(Regions::RadialRings(0)),
            Err(InventoryError::NoRegions)
        );
        let relabelled = prescribed()
            .with_regions(Regions::Named(vec!["in".into(), "out".into()]))
            .unwrap();
        assert_eq!(relabelled.nuclides(), prescribed().nuclides());
    }

    /// **Conversion from depletion, checked against a hand calculation.**
    ///
    /// Methodology: deplete the `chain_simple` U-235/U-238 pin (Predictor,
    /// 2 x 5 d at 1 MW in 1000 cm³) and convert every step. For I-135 at end
    /// of life the atom count must be `N * 1e24 * V` and the activity
    /// `ln 2 / 23652 s` times it (`chain_simple.xml`'s half-life), both to
    /// round-off; stable U-235 must carry atoms at zero activity; order, step
    /// index, time and provenance must come through.
    ///
    /// Result (2026-10-17): all hold, relative error below 1e-15.
    #[test]
    fn a_burnup_run_converts_with_atoms_activity_time_and_provenance() {
        let chain = DepletionChain::simple();
        let settings = BurnupSettings {
            power_watts: 1.0e6,
            fuel_volume_cm3: 1000.0,
            step_days: 5.0,
            n_steps: 2,
            ..BurnupSettings::default()
        };
        let n_u = 0.0232_f64;
        let initial = vec![
            ("U235".to_string(), 0.0425 * n_u),
            ("U238".to_string(), 0.9575 * n_u),
        ];
        let result = deplete_with(Integrator::Predictor, &chain, &initial, &settings);
        let v = Volume::new::<cubic_centimeter>(1000.0);
        let all = RadionuclideInventory::from_burnup_result(&result, &chain, v, "pin", "predictor")
            .unwrap();
        assert_eq!(all.len(), 3);
        let eol = &all[2];
        assert_eq!(eol.point().step, Some(2));
        assert!((eol.point().time.unwrap().get::<day>() - 10.0).abs() < 1e-12);
        assert_eq!(eol.regions(), &Regions::Named(vec!["pin".into()]));
        assert_eq!(
            eol.source(),
            &InventorySource::Depletion {
                code: "outram-mc-libs".into(),
                run: "predictor".into()
            }
        );
        let names: Vec<String> = eol.nuclides().iter().map(|e| e.isotope().name()).collect();
        assert_eq!(names, chain.nuclide_names());

        let i135 = eol
            .get(Isotope::new(53, 135).unwrap())
            .unwrap()
            .per_region()[0];
        let n = result.eol().density("I135") * 1.0e24 * 1000.0;
        let atoms = i135.atoms().unwrap().get::<particle>();
        assert!(n > 0.0 && ((atoms - n) / n).abs() < 1e-15, "{atoms} vs {n}");
        let a_hand = std::f64::consts::LN_2 / 2.36520e4 * n;
        let a = i135.activity().get::<becquerel>();
        assert!(((a - a_hand) / a_hand).abs() < 1e-15, "{a} vs {a_hand}");

        let u235 = eol.get(U235).unwrap().per_region()[0];
        assert!(u235.atoms().unwrap().get::<particle>() > 0.0);
        assert_eq!(u235.activity().get::<becquerel>(), 0.0);

        let one_ring = eol.clone().with_regions(Regions::RadialRings(1)).unwrap();
        assert_eq!(one_ring.regions(), &Regions::RadialRings(1));
    }

    /// The conversion refuses what it cannot convert rather than guessing.
    #[test]
    fn a_burnup_step_the_chain_cannot_explain_is_refused() {
        let chain = DepletionChain::simple();
        let step = |name: &str, d: f64| BurnupStep {
            step: 0,
            time_days: 0.0,
            flux: 0.0,
            k_inf: 0.0,
            densities: vec![(name.to_string(), d)],
        };
        let v = Volume::new::<cubic_centimeter>(1.0);
        let conv = |s: &BurnupStep, v: Volume| {
            RadionuclideInventory::from_burnup_step(s, &chain, v, "x", "y")
        };
        assert_eq!(
            conv(&step("Nothing", 1.0), v),
            Err(InventoryError::BadName("Nothing".into()))
        );
        assert_eq!(
            conv(&step("Cs137", 1.0), v),
            Err(InventoryError::NotInChain("Cs137".into()))
        );
        assert!(matches!(
            conv(&step("Xe135", -1.0), v),
            Err(InventoryError::BadAmount { .. })
        ));
        assert_eq!(
            conv(&step("Xe135", 1.0), Volume::new::<cubic_centimeter>(0.0)),
            Err(InventoryError::BadVolume)
        );
        assert!(conv(&step("Xe135", 1.0), v).is_ok());
    }
}
