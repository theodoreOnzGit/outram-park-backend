//! The top bar's three major tabs and the tabs under each (maintainer,
//! 2026-10-12).
//!
//! ```text
//!  top row:    [ Mindmap ]            [ Code Map ]            [ Code Review ]
//!  second row:  Home, Wiki, Save       Go to Code Review       PDF Reader,
//!               Repository, Digitiser,  (a button)             Bibliography,
//!               Table Digitiser, PDF                           Back to map
//!               Reader, Kvim Editor,                           (a button, to
//!               Bibliography                                   the Code Map)
//! ```
//!
//! A major tab's own page is the one it is named after; clicking it goes
//! there. The PDF Reader and the Bibliography are under two major tabs: they
//! are the same pages, and the major tab they were opened from stays
//! selected. Home is kept under Mindmap (not in the maintainer's list): it
//! is where a Kovan folder is opened or created, and other pages point the
//! user to it.
//!
//! Pages also change outside the bar (opening a paper, Back and Forward), so
//! the selected major tab is reconciled each frame with [`Group::after`]:
//! kept when the page is one of its own, else the page's own major tab.

use super::View;

/// A major tab of the top row.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum Group {
    #[default]
    Mindmap,
    CodeMap,
    CodeReview,
}

impl Group {
    pub(super) const ALL: [Group; 3] = [Group::Mindmap, Group::CodeMap, Group::CodeReview];

    pub(super) fn label(self) -> &'static str {
        match self {
            Group::Mindmap => "Mindmap",
            Group::CodeMap => "Code Map",
            Group::CodeReview => "Code Review",
        }
    }

    /// The page the major tab is named after.
    pub(super) fn main(self) -> View {
        match self {
            Group::Mindmap => View::Mindmap,
            Group::CodeMap => View::CodeMap,
            Group::CodeReview => View::CodeReview,
        }
    }

    /// The tabs of the second row, in order, with their labels.
    pub(super) fn sub_tabs(self) -> &'static [(View, &'static str)] {
        match self {
            Group::Mindmap => &[
                (View::Home, "Home"),
                (View::Wiki, "Wiki"),
                // "Save Repository", not "Advanced Git" (op-wqaw): most users
                // should not need Git vocabulary as the primary frame.
                (View::AdvancedGit, "Save Repository"),
                (View::Digitiser, "Digitiser"),
                (View::TableDigitiser, "Table Digitiser"),
                (View::PdfReader, "PDF Reader"),
                (View::KvimEditor, "Kvim Editor"),
                (View::Bibliography, "Bibliography"),
            ],
            // Its second row is one button, "Go to Code Review".
            Group::CodeMap => &[],
            Group::CodeReview => &[
                (View::PdfReader, "PDF Reader"),
                (View::Bibliography, "Bibliography"),
            ],
        }
    }

    /// Whether `view` is shown under this major tab.
    pub(super) fn holds(self, view: View) -> bool {
        self.main() == view
            || self.sub_tabs().iter().any(|(v, _)| *v == view)
            // The form between cropping a figure and digitising it.
            || (self == Group::Mindmap && view == View::PlotSetup)
    }

    /// The major tab selected once the app shows `view`: this one when it
    /// holds the page, else the first that does.
    pub(super) fn after(self, view: View) -> Group {
        if self.holds(view) {
            self
        } else {
            Group::ALL.into_iter().find(|g| g.holds(view)).unwrap_or(self)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EVERY_VIEW: [View; 12] = [
        View::Home,
        View::Wiki,
        View::Digitiser,
        View::PdfReader,
        View::KvimEditor,
        View::Bibliography,
        View::TableDigitiser,
        View::PlotSetup,
        View::Mindmap,
        View::AdvancedGit,
        View::CodeMap,
        View::CodeReview,
    ];

    /// The layout the maintainer asked for (2026-10-12): Mindmap holds the
    /// literature pages, Code Map holds only itself, Code Review holds only
    /// itself, the PDF Reader and the Bibliography. Every page is under
    /// some major tab, so none is unreachable.
    #[test]
    fn the_three_major_tabs_hold_the_pages_asked_for() {
        let labels = |g: Group| g.sub_tabs().iter().map(|(_, l)| *l).collect::<Vec<_>>();
        assert_eq!(
            labels(Group::Mindmap),
            [
                "Home",
                "Wiki",
                "Save Repository",
                "Digitiser",
                "Table Digitiser",
                "PDF Reader",
                "Kvim Editor",
                "Bibliography"
            ]
        );
        assert!(labels(Group::CodeMap).is_empty());
        assert_eq!(labels(Group::CodeReview), ["PDF Reader", "Bibliography"]);
        for v in EVERY_VIEW {
            assert!(Group::ALL.iter().any(|g| g.holds(v)), "{v:?} is under no major tab");
        }
        assert!(!Group::CodeMap.holds(View::PdfReader) && !Group::CodeReview.holds(View::Wiki));
    }

    /// A page shared by two major tabs keeps the one it was opened from; a
    /// page the selected tab does not hold moves to its own.
    #[test]
    fn the_major_tab_follows_the_page() {
        assert_eq!(Group::CodeReview.after(View::PdfReader), Group::CodeReview);
        assert_eq!(Group::Mindmap.after(View::PdfReader), Group::Mindmap);
        assert_eq!(Group::CodeMap.after(View::PdfReader), Group::Mindmap);
        assert_eq!(Group::CodeReview.after(View::Wiki), Group::Mindmap);
        assert_eq!(Group::Mindmap.after(View::CodeReview), Group::CodeReview);
        assert_eq!(Group::CodeReview.after(View::CodeMap), Group::CodeMap);
        assert_eq!(Group::CodeMap.after(View::PlotSetup), Group::Mindmap);
    }
}
