//! # What the wizard offers: reactor types by generation, with honest status
//!
//! Screen 1 of the workbench asks "what do you want to build?" and lists the
//! reactor types grouped by generation (maintainer, 2026-10-05). Most of them
//! have no model in this workspace yet, so every card carries a
//! [`Support`] level and a one-line note saying what exists. A card that
//! claimed more than the workspace holds would be the kind of overclaim
//! `RESPONSIBLE_USE.md` forbids.
//!
//! The notes record what was checked on 2026-10-05 by reading the crates
//! named in them. They are not V&V statements. **CORRECTED 2026-11-25
//! (gh:#1736):** three notes were contradicted by the code: the SFR card
//! said "no model" while EBR-II's plant model (`engine::ebr2`) and its
//! fresh-driver Monte Carlo model (`nee_soon::ebr2_fresh_driver`) exist;
//! the liquid-fuel MSR card named only Moltres while MSRE's full-core model
//! (`nee_soon::msre_n1`) and plant model (`engine::msre`) exist; and the
//! prismatic HTTR card said "awaiting reference data" while
//! `nee_soon::httr` has built the HTTR start-up core since 2026-10-16. The
//! notes below were re-checked against those crates.
//!
//! **Historical presets (gh:#1736).** MSRE and EBR-II are offered as
//! [`HistoricalPreset`]s under their types. The wizard's ten steps are
//! built on the HTR-10 model, so neither preset runs them: each states, per
//! step, which existing piece it reuses and, where nothing exists, the
//! issue that tracks it ([`HistoricalPreset::steps`]).

/// A reactor generation, the grouping of screen 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Generation {
    /// Generation II: the classic fleet.
    GenII,
    /// Generation III+ large plants.
    GenIIIPlusLarge,
    /// Generation III+ small modular reactors.
    GenIIIPlusSmr,
    /// The six Generation IV systems (MSR split by fuel form).
    GenIV,
}

impl Generation {
    /// Every generation, in the order screen 1 lists them.
    pub const ALL: [Self; 4] = [
        Self::GenII,
        Self::GenIIIPlusLarge,
        Self::GenIIIPlusSmr,
        Self::GenIV,
    ];

    /// Heading on screen 1.
    #[must_use]
    pub fn title(self) -> &'static str {
        match self {
            Self::GenII => "Generation II classics",
            Self::GenIIIPlusLarge => "Generation III+ large",
            Self::GenIIIPlusSmr => "Generation III+ SMRs",
            Self::GenIV => "Generation IV",
        }
    }
}

/// How much of a reactor type the workspace can build today.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Support {
    /// The wizard walks this type through the implemented steps.
    Wizard,
    /// The workspace has some models of this type, but the wizard does not
    /// build it yet.
    Partial,
    /// Nothing in the workspace models this type yet.
    NotYet,
}

impl Support {
    /// Short badge text.
    #[must_use]
    pub fn badge(self) -> &'static str {
        match self {
            Self::Wizard => "in the wizard",
            Self::Partial => "partial",
            Self::NotYet => "not yet",
        }
    }
}

/// A reactor type on screen 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReactorType {
    /// Pressurised water reactor.
    Pwr,
    /// Boiling water reactor.
    Bwr,
    /// AP1000-class large passive PWR.
    Ap1000,
    /// Integral PWR SMR (NuScale-class).
    IntegralPwrSmr,
    /// BWR SMR (BWRX-300-class).
    BwrSmr,
    /// Very-high-temperature / high-temperature gas-cooled reactor.
    Htgr,
    /// Sodium-cooled fast reactor.
    Sfr,
    /// Lead-cooled fast reactor.
    Lfr,
    /// Gas-cooled fast reactor.
    Gfr,
    /// Supercritical-water-cooled reactor.
    Scwr,
    /// Molten salt reactor, solid fuel (salt-cooled, e.g. FHR).
    MsrSolidFuel,
    /// Molten salt reactor, liquid (circulating) fuel.
    MsrLiquidFuel,
}

impl ReactorType {
    /// Every type, in screen-1 order.
    pub const ALL: [Self; 12] = [
        Self::Pwr,
        Self::Bwr,
        Self::Ap1000,
        Self::IntegralPwrSmr,
        Self::BwrSmr,
        Self::Htgr,
        Self::Sfr,
        Self::Lfr,
        Self::Gfr,
        Self::Scwr,
        Self::MsrSolidFuel,
        Self::MsrLiquidFuel,
    ];

    /// Card title.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Pwr => "PWR",
            Self::Bwr => "BWR",
            Self::Ap1000 => "AP1000-class",
            Self::IntegralPwrSmr => "Integral PWR SMR",
            Self::BwrSmr => "BWR SMR",
            Self::Htgr => "VHTR / HTGR",
            Self::Sfr => "SFR",
            Self::Lfr => "LFR",
            Self::Gfr => "GFR",
            Self::Scwr => "SCWR",
            Self::MsrSolidFuel => "MSR, solid fuel",
            Self::MsrLiquidFuel => "MSR, liquid fuel",
        }
    }

    /// Recipe key (`reactor = "..."`).
    #[must_use]
    pub fn key(self) -> &'static str {
        match self {
            Self::Pwr => "pwr",
            Self::Bwr => "bwr",
            Self::Ap1000 => "ap1000",
            Self::IntegralPwrSmr => "ipwr_smr",
            Self::BwrSmr => "bwr_smr",
            Self::Htgr => "htgr",
            Self::Sfr => "sfr",
            Self::Lfr => "lfr",
            Self::Gfr => "gfr",
            Self::Scwr => "scwr",
            Self::MsrSolidFuel => "msr_solid",
            Self::MsrLiquidFuel => "msr_liquid",
        }
    }

    /// The type a recipe key names.
    #[must_use]
    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|t| t.key() == key)
    }

    /// Its generation.
    #[must_use]
    pub fn generation(self) -> Generation {
        match self {
            Self::Pwr | Self::Bwr => Generation::GenII,
            Self::Ap1000 => Generation::GenIIIPlusLarge,
            Self::IntegralPwrSmr | Self::BwrSmr => Generation::GenIIIPlusSmr,
            _ => Generation::GenIV,
        }
    }

    /// What the workspace can do with it today.
    #[must_use]
    pub fn support(self) -> Support {
        match self {
            Self::Htgr => Support::Wizard,
            Self::Pwr | Self::MsrSolidFuel | Self::MsrLiquidFuel | Self::Sfr => Support::Partial,
            _ => Support::NotYet,
        }
    }

    /// One line on what exists, read from the workspace on 2026-10-05 and
    /// re-checked for the SFR, liquid-fuel MSR and HTGR cards on 2026-11-25
    /// (gh:#1736).
    #[must_use]
    pub fn note(self) -> &'static str {
        match self {
            Self::Htgr => {
                "Basic -> pebble bed (HTR-10) runs every step (Steps 7–10 simplified, each panel says how); prismatic: nee_soon::httr \
                 builds the HTTR start-up core, not yet in the wizard"
            }
            Self::Pwr => {
                "outram-mc-libs has the ICSBEP LCT-008 lattice; no plant model in the wizard"
            }
            Self::MsrSolidFuel => {
                "fhr_sim_v2 (digital-twin engine) is a low-fidelity FHR; no high-fidelity model"
            }
            Self::MsrLiquidFuel => {
                "MSRE preset: nee_soon::msre_n1 full core + engine::msre plant; outram-park-fork-moltres has circulating-fuel \
                 diffusion + precursor drift"
            }
            Self::Sfr => "EBR-II preset: engine::ebr2 plant (SHRT-17/45R) + nee_soon::ebr2_fresh_driver subassembly; no whole core",
            _ => "no model in the workspace yet",
        }
    }
}

/// HTGR sub-types offered under Basic mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HtgrCore {
    /// Prismatic block core; preset HTTR.
    Prismatic,
    /// Pebble bed; preset HTR-10.
    PebbleBed,
}

impl HtgrCore {
    /// Card title.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Prismatic => "Prismatic (HTTR preset)",
            Self::PebbleBed => "Pebble bed (HTR-10 preset)",
        }
    }

    /// Whether the preset exists yet.
    #[must_use]
    pub fn available(self) -> bool {
        matches!(self, Self::PebbleBed)
    }

    /// Why a preset is unavailable, or what it is.
    #[must_use]
    pub fn note(self) -> &'static str {
        match self {
            Self::Prismatic => {
                "nee_soon::httr builds the IRPhEP HTTR start-up core (every TRISO, R1's dimensions); the \
                 wizard's steps are not built for it yet"
            }
            Self::PebbleBed => {
                "nee_soon::htr10_rmc, the HTR-10 model of the RMC code-to-code record"
            }
        }
    }
}

/// A historical reactor offered as a preset beside the wizard (gh:#1736).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoricalPreset {
    /// The Molten-Salt Reactor Experiment (ORNL, 1965-69).
    Msre,
    /// Experimental Breeder Reactor II (ANL), SHRT-17 / SHRT-45R.
    Ebr2,
}

/// Whether a preset's step exists in the workspace.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresetStatus {
    /// Built: `reuses` names the piece.
    Available,
    /// Not modelled; the issue tracks it.
    NotModelled { issue: u32 },
}

/// One step of a preset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PresetStep {
    pub name: &'static str,
    /// The existing piece the step reuses, or what is missing.
    pub reuses: &'static str,
    pub status: PresetStatus,
}

impl HistoricalPreset {
    /// Every preset.
    pub const ALL: [Self; 2] = [Self::Msre, Self::Ebr2];

    /// Card title.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Msre => "MSRE (ORNL)",
            Self::Ebr2 => "EBR-II (ANL)",
        }
    }

    /// The reactor type it sits under.
    #[must_use]
    pub fn reactor_type(self) -> ReactorType {
        match self {
            Self::Msre => ReactorType::MsrLiquidFuel,
            Self::Ebr2 => ReactorType::Sfr,
        }
    }

    /// The preset under `t`, if any.
    #[must_use]
    pub fn for_type(t: ReactorType) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.reactor_type() == t)
    }

    /// Per step: what it reuses, or what is not modelled and its issue.
    #[must_use]
    pub fn steps(self) -> [PresetStep; 6] {
        let ok = |name, reuses| PresetStep { name, reuses, status: PresetStatus::Available };
        let no = |name, reuses, issue| PresetStep { name, reuses, status: PresetStatus::NotModelled { issue } };
        match self {
            Self::Msre => [
                ok("Prefill", "nee_soon::msre_n1::Study::default(): TM-0728 / TM-0730 dimensions at temperature, the whole shield, the U-234/U-236 salt"),
                ok("Geometry review", "drawn live on this page from the assembled CSG (workbench::presets::msre_slices: the whole model with the full shield, mid-height x-y, the vessel), gh:#1752; record images in nee_soon/verification_and_validation/msre_n1_first_criticality/images/"),
                ok("k_eff", "the N-1 first-criticality record: 1.016515 ± 0.000329 on six seeds, +1571 pcm against k_ref (msre_n1_first_criticality/README.md)"),
                ok("Temperature coefficients", "the N-3 split (fuel -3.79 ± 0.37, graphite -3.26 ± 0.33 x1e-5 /F), live through msre::twin::MsreSplit"),
                ok("Plant", "engine::msre (TM-1070 circulating kinetics, feedback, loops), on the twin template as msre::twin::MsreTwin"),
                no("Meshes, MGXS, coupled multiphysics", "not built: Moltres drift is not a coupled mesh model (decision on gh:#1743)", 1743),
            ],
            Self::Ebr2 => [
                ok("Prefill", "nee_soon::ebr2_fresh_driver: one MARK-II fresh driver subassembly, every number cited"),
                ok("Geometry review", "drawn from the assembled CSG: nee_soon/verification_and_validation/ebr2_fresh_driver/ (xy, xz, pin)"),
                ok("k_inf (one subassembly)", "k_inf of the fresh driver (ebr2_fresh_driver/README.md); no measured k_inf exists to compare against"),
                no("k_eff, whole core", "no whole-core Monte Carlo model of EBR-II yet", 1313),
                ok("Plant", "engine::ebr2 (SHRT-17 / SHRT-45R lumped model, Argonne VARIANT feedback), on the twin template as ebr2::twin::Ebr2Twin"),
                no("Meshes, coupled multiphysics", "not built: a bedok nodal backend is the candidate", 1747),
            ],
        }
    }
}

/// Basic or Advanced construction (wizard Q1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    /// Start from a pre-built model and modify.
    Basic,
    /// Build from scratch; more customisable.
    Advanced,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_reactor_key_round_trips_and_every_generation_has_a_type() {
        for t in ReactorType::ALL {
            assert_eq!(ReactorType::from_key(t.key()), Some(t));
        }
        for g in Generation::ALL {
            assert!(
                ReactorType::ALL.iter().any(|t| t.generation() == g),
                "{g:?} is empty"
            );
        }
    }

    /// The catalogue must not claim more than the workspace has: exactly one
    /// type is in the wizard, and only the pebble-bed preset exists.
    #[test]
    fn only_the_htgr_pebble_bed_is_offered_as_buildable() {
        let wizard: Vec<_> = ReactorType::ALL
            .into_iter()
            .filter(|t| t.support() == Support::Wizard)
            .collect();
        assert_eq!(wizard, vec![ReactorType::Htgr]);
        assert!(HtgrCore::PebbleBed.available());
        assert!(!HtgrCore::Prismatic.available());
    }

    /// gh:#1736: MSRE and EBR-II sit under their types, every step names
    /// what it reuses, and every missing step names its issue; the SFR card
    /// is no longer "no model".
    #[test]
    fn the_historical_presets_state_every_step() {
        assert_eq!(HistoricalPreset::for_type(ReactorType::MsrLiquidFuel), Some(HistoricalPreset::Msre));
        assert_eq!(HistoricalPreset::for_type(ReactorType::Sfr), Some(HistoricalPreset::Ebr2));
        assert_eq!(HistoricalPreset::for_type(ReactorType::Pwr), None);
        for p in HistoricalPreset::ALL {
            assert!(!p.label().is_empty());
            assert_eq!(p.reactor_type().support(), Support::Partial, "{p:?} is not a wizard path");
            for s in p.steps() {
                assert!(!s.name.is_empty() && !s.reuses.is_empty());
                if let PresetStatus::NotModelled { issue } = s.status {
                    assert!(issue > 0);
                }
            }
        }
        let missing: Vec<u32> = HistoricalPreset::Ebr2
            .steps()
            .iter()
            .filter_map(|s| match s.status {
                PresetStatus::NotModelled { issue } => Some(issue),
                PresetStatus::Available => None,
            })
            .collect();
        assert_eq!(missing, [1313, 1747]);
        assert!(ReactorType::Sfr.note().starts_with("EBR-II preset"));
        assert!(ReactorType::MsrLiquidFuel.note().starts_with("MSRE preset"));
        assert!(!HtgrCore::Prismatic.note().contains("awaiting"));
    }
}
