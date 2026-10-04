//! **The rung table**: the one place that says which rungs this demo has,
//! what each is called in a URL, which lesson page explains it, and how its
//! neutrons are animated (gh:#520, #521).
//!
//! A rung is one step of the Monte Carlo tutorial ladder (epic #520): Godiva
//! first (a bare fast sphere), the TRISO pebble at the top. The demo is ONE
//! app; `?rung=<name>&mode=<watch|run>` picks what it opens on (natively:
//! `--rung <name> --mode <watch|run>`).
//!
//! **This table drives the links both ways.** The side panel's "What's
//! happening here?" opens [`RungInfo::lesson`]; the lesson pages link back to
//! `demos/monte-carlo/?rung=<name>`. `scripts/build-pages.sh` reads the
//! `name:` and `lesson:` lines of this file and fails the site build if a
//! lesson page is missing or a page links to a rung this table does not have.
//! Keep each on one line, in that form.

/// The rungs, in ladder order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rung {
    /// Rung 1: a bare sphere of highly enriched uranium (ICSBEP HEU-MET-FAST-001).
    Godiva,
    /// The hook (rung 6 when the ladder gets there): a 2D TRISO fuel pebble.
    Triso,
}

/// What the demo shows for a rung.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Illustration: neutrons animated one at a time (and, for Godiva, whole
    /// generations of the fission source).
    Watch,
    /// True Monte Carlo: a real power iteration, one console line per generation.
    Run,
}

impl Mode {
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))] // the browser URL only
    pub fn name(self) -> &'static str {
        match self {
            Mode::Watch => "watch",
            Mode::Run => "run",
        }
    }
    pub fn parse(s: &str) -> Option<Mode> {
        match s {
            "watch" => Some(Mode::Watch),
            "run" => Some(Mode::Run),
            _ => None,
        }
    }
}

/// The energy range a rung's neutrons mostly live in. It sets the default of
/// the animation-speed slider; see [`Spectrum::default_speed_at_1ev`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Spectrum {
    /// Moderated: neutrons slow down to thermal energies (TRISO; later HST-009,
    /// LCT-008, HTR-10).
    Thermal,
    /// Unmoderated: neutrons stay near their ~MeV birth energies (Godiva; later
    /// Jemima).
    Fast,
}

impl Spectrum {
    /// Default of the "speed at 1 eV" slider \[cm of flight per second\].
    ///
    /// **One unit for every rung, a default per spectrum** (maintainer
    /// decision, 2026-10-04): the slider always means the animated speed of a
    /// 1 eV neutron, so rungs stay comparable, and each spectrum's default
    /// animates a TYPICAL neutron of that spectrum at about 7 cm/s:
    ///
    /// - thermal rungs: **7 cm/s at 1 eV** (a slowing-down neutron in a
    ///   moderator; a fully thermal 0.0253 eV neutron then crawls at 1.1 cm/s
    ///   and a 2 MeV birth flies at ~9900 cm/s);
    /// - fast rungs: **0.007 cm/s at 1 eV**, which is **7 cm/s at 1 MeV**,
    ///   about where a Godiva neutron spends its life.
    ///
    /// The speed itself scales with the neutron's real speed, so every rung
    /// shows neutrons slowing down (see [`crate::anim::animated_speed`]).
    pub fn default_speed_at_1ev(self) -> f64 {
        match self {
            Spectrum::Thermal => 7.0,
            Spectrum::Fast => 0.007,
        }
    }

    /// The characteristic energy shown beside the slider \[eV\], and its name.
    pub fn characteristic(self) -> (f64, &'static str) {
        match self {
            Spectrum::Thermal => (0.0253, "0.0253 eV (thermal)"),
            Spectrum::Fast => (1.0e6, "1 MeV (fast)"),
        }
    }
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
    pub spectrum: Spectrum,
    /// Whether the rung has a Run k_eff mode.
    pub has_run: bool,
}

/// Root of the backend GitHub Pages site. Absolute so the "What's happening
/// here?" link also works from the native build.
pub const SITE: &str = "https://theodoreonzgit.github.io/outram-park-backend/";

pub const RUNGS: [RungInfo; 2] = [
    RungInfo {
        rung: Rung::Godiva,
        name: "godiva",
        title: "Godiva: a bare uranium sphere",
        lesson: "tutorials/monte-carlo/godiva.html",
        spectrum: Spectrum::Fast,
        has_run: true,
    },
    RungInfo {
        rung: Rung::Triso,
        name: "triso",
        title: "TRISO pebble: one neutron at a time",
        // No tutorial page for this rung yet (it is rung 6); the deep dive
        // explains the code it runs.
        lesson: "deep-dives/monte-carlo/index.html",
        spectrum: Spectrum::Thermal,
        has_run: false,
    },
];

impl Rung {
    pub fn info(self) -> &'static RungInfo {
        RUNGS.iter().find(|r| r.rung == self).expect("every rung is in RUNGS")
    }
    pub fn parse(s: &str) -> Option<Rung> {
        RUNGS.iter().find(|r| r.name == s).map(|r| r.rung)
    }
    /// The lesson page, as an absolute URL.
    pub fn lesson_url(self) -> String {
        format!("{SITE}{}", self.info().lesson)
    }
}

/// What the app opens on: from the page URL (`?rung=…&mode=…`) in the
/// browser, or `--rung …` / `--mode …` natively. No rung given opens Godiva
/// in Watch mode, the first rung; a rung without a Run mode ignores
/// `mode=run`. The old `demos/triso-pebble/` URL redirects to `?rung=triso`.
pub fn start(query: &[(String, String)]) -> (Rung, Mode) {
    let get = |k: &str| query.iter().find(|(a, _)| a == k).map(|(_, v)| v.as_str());
    let rung = get("rung").and_then(Rung::parse).unwrap_or(Rung::Godiva);
    let mode = get("mode").and_then(Mode::parse).unwrap_or(Mode::Watch);
    let mode = if rung.info().has_run { mode } else { Mode::Watch };
    (rung, mode)
}
