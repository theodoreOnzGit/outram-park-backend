//! **The rung table** of the TRISO-ATOPS and fuel failure demo (gh:#540, the
//! demo phase of #531): one row per rung of the boon-lay lesson ladder,
//! `tutorials/triso-atops/`. One table drives both directions: the demo's
//! "What's happening here?" opens `lesson`, and each lesson page links the
//! demo at `?rung=<name>`.
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
pub const TABLE: [RungInfo; 8] = [
    RungInfo {
        name: "triso",
        title: "1. The particle: five layers",
        lesson: "tutorials/triso-atops/triso.html",
    },
    RungInfo {
        name: "decay",
        title: "2. An atom's clock: decay by Monte Carlo",
        lesson: "tutorials/triso-atops/decay.html",
    },
    RungInfo {
        name: "walk",
        title: "3. An atom's walk: Walk-on-Spheres",
        lesson: "tutorials/triso-atops/walk.html",
    },
    RungInfo {
        name: "layers",
        title: "4. Through the layers: D(T) per layer",
        lesson: "tutorials/triso-atops/layers.html",
    },
    RungInfo {
        name: "failure",
        title: "5. When the shell breaks: fuel failure",
        lesson: "tutorials/triso-atops/failure.html",
    },
    RungInfo {
        name: "chemistry",
        title: "6. Air and steam: chemical attack",
        lesson: "tutorials/triso-atops/chemistry.html",
    },
    RungInfo {
        name: "release",
        title: "7. To the coolant: release and pools",
        lesson: "tutorials/triso-atops/coolant.html",
    },
    RungInfo {
        name: "source-term",
        title: "8. Into the source term",
        lesson: "tutorials/triso-atops/handoff.html",
    },
];

/// A rung of the ladder (an enum, not a trait object: the workspace's Rust
/// rules).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rung {
    Triso,
    Decay,
    Walk,
    Layers,
    Failure,
    Chemistry,
    Release,
    SourceTerm,
}

const ALL: [Rung; 8] = [
    Rung::Triso,
    Rung::Decay,
    Rung::Walk,
    Rung::Layers,
    Rung::Failure,
    Rung::Chemistry,
    Rung::Release,
    Rung::SourceTerm,
];

impl Rung {
    fn info(self) -> &'static RungInfo {
        &TABLE[self as usize]
    }
    /// Rungs whose main view is the particle (zoom moves the view, world unit
    /// µm); the others are plots (zoom scales their text).
    pub fn is_particle(self) -> bool {
        matches!(self, Rung::Triso | Rung::Decay | Rung::Walk)
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
    /// page of the boon-lay tutorial book.
    #[test]
    fn the_rung_table_is_consistent() {
        for (i, r) in ALL.iter().enumerate() {
            assert_eq!(*r as usize, i);
            assert!(r.lesson().starts_with("tutorials/triso-atops/"));
        }
        let mut names: Vec<&str> = TABLE.iter().map(|t| t.name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), TABLE.len());
    }
}
