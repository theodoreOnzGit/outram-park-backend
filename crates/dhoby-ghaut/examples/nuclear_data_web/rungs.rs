//! **The rung table** of the nuclear data demo (gh:#529): which rungs exist,
//! what each is called in a URL, which lesson page explains it, and which
//! ENDF tapes it processes.
//!
//! One rung per processing step of the nuclear data track's book
//! (`crates/njoy-outram-park-fork/docs/lessons/`, published at
//! `deep-dives/nuclear-data/`). `?rung=<name>` picks what the demo opens on
//! (natively: `--rung <name>`).
//!
//! **This table drives the links both ways.** The side panel's "What's
//! happening here?" opens the rung's lesson; each lesson page links back to
//! `demos/nuclear-data/?rung=<name>`. Keep each `name:` and `lesson:` on one
//! line, in that form: `scripts/build-pages.sh` reads them and fails the site
//! build if a lesson page is missing or a page links to a rung not here.
//!
//! The table is in ladder order; [`lesson::Rung::all`] puts RECONR first, so
//! a bare URL opens the most visual rung.
//!
//! [`lesson::Rung::all`]: dhoby_ghaut::web_demo::lesson::Rung::all

/// The rungs. Numbers are the lesson rungs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rung {
    /// Rung 1: what is on an ENDF tape.
    Endf,
    /// Rung 2: RECONR builds σ(E) from resonance parameters.
    Reconr,
    /// Rung 3: BROADR smears the resonances with temperature.
    Broadr,
    /// Rung 4: PURR's probability bands in the unresolved range.
    Purr,
    /// Rung 5: THERMR's bound-atom thermal scattering.
    Thermr,
    /// Rung 7: GROUPR's group averages, with self-shielding.
    Groupr,
    /// Rung 9: ACER (recorded results only).
    Acer,
}

/// One row of the rung table.
pub struct RungInfo {
    pub rung: Rung,
    /// Stable short name, used in `?rung=`.
    pub name: &'static str,
    /// Title shown in the panel.
    pub title: &'static str,
    /// The page explaining this rung, relative to the site root.
    pub lesson: &'static str,
    /// The tapes the rung processes, from `reference-data/endf/`.
    pub tapes: &'static [&'static str],
}

/// ENDF/B-VIII.0 U-238, the nuclide most rungs use.
pub const U238: &str = "n-092_U_238.endf";
/// ENDF/B-VIII.0 thermal scattering law, crystalline graphite.
pub const GRAPHITE: &str = "tsl-crystalline-graphite.endf";
/// ENDF/B-VIII.0 thermal scattering law, hydrogen in light water.
pub const H_IN_H2O: &str = "tsl-HinH2O.endf";

pub const RUNGS: [RungInfo; 7] = [
    RungInfo {
        rung: Rung::Endf,
        name: "endf",
        title: "ENDF: what the evaluator hands us",
        lesson: "deep-dives/nuclear-data/endf.html",
        tapes: &[U238],
    },
    RungInfo {
        rung: Rung::Reconr,
        name: "reconr",
        title: "RECONR: resonance parameters to σ(E)",
        lesson: "deep-dives/nuclear-data/reconr.html",
        tapes: &[U238],
    },
    RungInfo {
        rung: Rung::Broadr,
        name: "broadr",
        title: "BROADR: what temperature does to σ(E)",
        lesson: "deep-dives/nuclear-data/broadr.html",
        tapes: &[U238],
    },
    RungInfo {
        rung: Rung::Purr,
        name: "purr",
        title: "PURR: probability bands in the unresolved range",
        lesson: "deep-dives/nuclear-data/purr.html",
        tapes: &[U238],
    },
    RungInfo {
        rung: Rung::Thermr,
        name: "thermr",
        title: "THERMR: bound-atom thermal scattering",
        lesson: "deep-dives/nuclear-data/thermr.html",
        tapes: &[GRAPHITE, H_IN_H2O],
    },
    RungInfo {
        rung: Rung::Groupr,
        name: "groupr",
        title: "GROUPR: group averages and self-shielding",
        lesson: "deep-dives/nuclear-data/groupr.html",
        tapes: &[U238],
    },
    RungInfo {
        rung: Rung::Acer,
        name: "acer",
        title: "ACER: the table a transport code reads (recorded)",
        lesson: "deep-dives/nuclear-data/acer.html",
        tapes: &[],
    },
];

impl Rung {
    pub fn info(self) -> &'static RungInfo {
        RUNGS.iter().find(|r| r.rung == self).expect("every rung is in RUNGS")
    }
}

impl dhoby_ghaut::web_demo::lesson::Rung for Rung {
    fn all() -> &'static [Self] {
        &[Rung::Reconr, Rung::Endf, Rung::Broadr, Rung::Purr, Rung::Thermr, Rung::Groupr, Rung::Acer]
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

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use dhoby_ghaut::web_demo::lesson;

    /// Every rung parses back from its name, is in `all()` exactly once, and
    /// names a lesson page that exists in the nuclear data book's sources.
    #[test]
    fn every_rung_parses_back_and_has_a_lesson_page_in_the_book() {
        let book = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../njoy-outram-park-fork/docs/lessons/src");
        assert_eq!(<Rung as lesson::Rung>::all().len(), RUNGS.len());
        for r in &RUNGS {
            assert_eq!(lesson::parse::<Rung>(r.name), Some(r.rung));
            let page = r.lesson.strip_prefix("deep-dives/nuclear-data/").expect("lesson in the nuclear data book");
            let md = book.join(page.replace(".html", ".md"));
            assert!(md.exists(), "rung {} links to {}, which has no source page {}", r.name, r.lesson, md.display());
        }
    }
}
