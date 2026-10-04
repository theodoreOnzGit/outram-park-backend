//! **The rung table** of the dispersion demo (gh:#530): one row per rung of
//! the dispersion lesson ladder, `deep-dives/dispersion/rungs/`. One table
//! drives both directions: the demo's "What's happening here?" opens
//! `lesson`, and each lesson page links the demo at `?rung=<name>`.
//!
//! `scripts/build-pages.sh` reads the `name:` and `lesson:` lines below (one
//! per line, as written) and fails the site build if a lesson page is missing
//! or a page links to a rung that is not here.

use dhoby_ghaut::web_demo::lesson::Rung as RungTrait;

/// One row of the table.
pub struct RungInfo {
    pub name: &'static str,
    pub title: &'static str,
    pub lesson: &'static str,
}

/// The table, in ladder order. Keep each field on its own line.
pub const TABLE: [RungInfo; 7] = [
    RungInfo {
        name: "plume",
        title: "1. The steady Gaussian plume",
        lesson: "deep-dives/dispersion/rungs/01-plume.html",
    },
    RungInfo {
        name: "sigmas",
        title: "2. Stability classes and sigma curves",
        lesson: "deep-dives/dispersion/rungs/02-sigmas.html",
    },
    RungInfo {
        name: "rise-wake",
        title: "3. Plume rise and building wake",
        lesson: "deep-dives/dispersion/rungs/03-rise-wake.html",
    },
    RungInfo {
        name: "puffs",
        title: "4. The puff train and a turning wind",
        lesson: "deep-dives/dispersion/rungs/04-puffs.html",
    },
    RungInfo {
        name: "deposition",
        title: "5. Decay in flight and deposition",
        lesson: "deep-dives/dispersion/rungs/05-deposition.html",
    },
    RungInfo {
        name: "dose",
        title: "6. Dose against distance",
        lesson: "deep-dives/dispersion/rungs/06-dose.html",
    },
    RungInfo {
        name: "capstone",
        title: "7. Capstone: HTR-10 bounding air ingress",
        lesson: "deep-dives/dispersion/rungs/07-capstone.html",
    },
];

/// A rung of the ladder (an enum, not a trait object: the workspace's Rust
/// rules).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rung {
    Plume,
    Sigmas,
    RiseWake,
    Puffs,
    Deposition,
    Dose,
    Capstone,
}

const ALL: [Rung; 7] = [
    Rung::Plume,
    Rung::Sigmas,
    Rung::RiseWake,
    Rung::Puffs,
    Rung::Deposition,
    Rung::Dose,
    Rung::Capstone,
];

impl Rung {
    fn info(self) -> &'static RungInfo {
        &TABLE[self as usize]
    }
    /// Rungs whose main view is a map (zoom moves the map); the others are
    /// plots (zoom scales their text).
    pub fn is_map(self) -> bool {
        matches!(self, Rung::Plume | Rung::Puffs | Rung::Deposition)
    }
}

impl RungTrait for Rung {
    fn all() -> &'static [Self] {
        &ALL
    }
    fn name(self) -> &'static str {
        self.info().name
    }
    fn title(self) -> &'static str {
        self.info().title
    }
    fn lesson(self) -> &'static str {
        self.info().lesson
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The enum and the table agree, names are unique, and every lesson is a
    /// page of the dispersion book's rung part.
    #[test]
    fn the_rung_table_is_consistent() {
        for (i, r) in ALL.iter().enumerate() {
            assert_eq!(*r as usize, i);
            assert!(r.lesson().starts_with("deep-dives/dispersion/rungs/"));
        }
        let mut names: Vec<&str> = TABLE.iter().map(|t| t.name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), TABLE.len());
    }
}
