//! The rung table's track-independent half: a rung's URL name, its title and
//! its lesson page, the `?rung=` parameter, and the "What's happening here?"
//! link (epic gh:#520: "each rung links to its own page, both ways; one table
//! drives both").
//!
//! A track's rung enum implements [`Rung`]. `scripts/build-pages.sh` checks
//! that every lesson page a rung names exists and that every page's demo link
//! names a real rung, reading the `name:` and `lesson:` lines of the track's
//! rung files (see the Monte Carlo demo's `rungs.rs`).

/// Root of the backend GitHub Pages site. Absolute, so lesson links also work
/// from a native build.
pub const SITE: &str = "https://theodoreonzgit.github.io/outram-park-backend/";

/// What every rung of every track has. Implemented by a plain `enum`.
pub trait Rung: Copy + PartialEq + 'static {
    /// Every rung, in ladder order. The first is what a bare URL opens.
    fn all() -> &'static [Self];
    /// Stable short name, used in `?rung=`.
    fn name(self) -> &'static str;
    /// Title shown in the panel.
    fn title(self) -> &'static str;
    /// The lesson page, relative to [`SITE`] (e.g.
    /// `tutorials/monte-carlo/godiva.html`).
    fn lesson(self) -> &'static str;
}

/// The rung named `name`, if any.
pub fn parse<R: Rung>(name: &str) -> Option<R> {
    R::all().iter().copied().find(|r| r.name() == name)
}

/// The rung a URL query asks for (`?rung=`), else the first.
pub fn from_query<R: Rung>(query: &[(String, String)]) -> R {
    crate::web_demo::platform::query_value(query, "rung").and_then(parse).unwrap_or(R::all()[0])
}

/// The lesson page as an absolute URL.
pub fn lesson_url<R: Rung>(r: R) -> String {
    format!("{SITE}{}", r.lesson())
}

/// The "What's happening here?" link to the rung's lesson, opening in a new
/// tab.
pub fn whats_happening<R: Rung>(ui: &mut egui::Ui, r: R) {
    ui.add(egui::Hyperlink::from_label_and_url("What's happening here? (the lesson)", lesson_url(r)).open_in_new_tab(true));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy, PartialEq, Debug)]
    enum Track {
        One,
        Two,
    }
    impl Rung for Track {
        fn all() -> &'static [Self] {
            &[Track::One, Track::Two]
        }
        fn name(self) -> &'static str {
            match self {
                Track::One => "one",
                Track::Two => "two",
            }
        }
        fn title(self) -> &'static str {
            self.name()
        }
        fn lesson(self) -> &'static str {
            "tutorials/x/one.html"
        }
    }

    #[test]
    fn the_rung_comes_from_the_query_or_is_the_first() {
        let q = |k: &str, v: &str| vec![(k.to_string(), v.to_string())];
        assert_eq!(from_query::<Track>(&q("rung", "two")), Track::Two);
        assert_eq!(from_query::<Track>(&q("rung", "nope")), Track::One);
        assert_eq!(from_query::<Track>(&[]), Track::One);
        assert_eq!(lesson_url(Track::One), format!("{SITE}tutorials/x/one.html"));
    }
}

/// A row of selectable rung names; returns the rung chosen (which may be the
/// current one).
pub fn picker<R: Rung>(ui: &mut egui::Ui, current: R) -> R {
    let mut r = current;
    ui.horizontal_wrapped(|ui| {
        ui.label("Rung:");
        for &x in R::all() {
            ui.selectable_value(&mut r, x, x.name());
        }
    });
    r
}
