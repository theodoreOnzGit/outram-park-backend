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
//! named in them. They are not V&V statements.

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
            Self::Pwr | Self::MsrSolidFuel | Self::MsrLiquidFuel => Support::Partial,
            _ => Support::NotYet,
        }
    }

    /// One line on what exists, read from the workspace on 2026-10-05.
    #[must_use]
    pub fn note(self) -> &'static str {
        match self {
            Self::Htgr => {
                "Basic -> pebble bed (HTR-10) runs every step (Steps 7–10 simplified, each panel says how); prismatic (HTTR) awaits \
                 its reference literature"
            }
            Self::Pwr => {
                "outram-mc-libs has the ICSBEP LCT-008 lattice; no plant model in the wizard"
            }
            Self::MsrSolidFuel => {
                "fhr_sim_v2 (digital-twin engine) is a low-fidelity FHR; no high-fidelity model"
            }
            Self::MsrLiquidFuel => {
                "outram-park-fork-moltres has circulating-fuel diffusion + precursor drift"
            }
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
                "awaiting reference data: the maintainer adds the HTTR literature to \
                 kovan first; no dimensions are invented meanwhile"
            }
            Self::PebbleBed => {
                "nee_soon::htr10_rmc, the HTR-10 model of the RMC code-to-code record"
            }
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
}
