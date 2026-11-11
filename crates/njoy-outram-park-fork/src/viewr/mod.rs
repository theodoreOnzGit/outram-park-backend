// SPDX-License-Identifier: AGPL-3.0-only
// Translated from NJOY2016.80 `src/viewr.f90` and `src/graph.f90`.
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! `VIEWR` — render 2-D/3-D plot commands to PostScript.
//!
//! A translation of NJOY2016.80's `viewr.f90` (the command reader, `set2d`,
//! `set3d`, `erbar`, `legndb`, `tagit`, `ascalv`) and of `graph.f90` (the page,
//! window, perspective, axis, text and symbol routines and the PostScript
//! writer), routine by routine, so that the PostScript file is NJOY's line for
//! line (see `verification_and_validation/viewr/`).
//!
//! Entry points: [`viewr_postscript`] (plot-command text in, PostScript text
//! out) and [`ViewrState::viewr`] when module state must persist between calls
//! (`drawh` remembers the last line width and dash across VIEWR calls of one
//! NJOY process, so the deck driver keeps one state per deck run).
//!
//! | file | routines |
//! |---|---|
//! | `driver.rs` | `viewr` (command reader, page loop) |
//! | `plot2d.rs` | `set2d`, `erbar`, `legndb`, `tagit`, `ascalv` |
//! | `plot3d.rs` | `set3d` |
//! | `graph.rs` | `initp`, `window`, `init2/3`, `trans3`, `vect3`, `xscale`... |
//! | `axis.rs` | `axis2`, `axis3` |
//! | `curves.rs` | `grid3`, `curv2`, `curv3`, `poly2`, `hatch` |
//! | `text.rs` | `text3`, `txtlen`, `rget`, `iget`, `stripv` |
//! | `symbols.rs` | `dsym` |
//! | `ps.rs` | the `write(nps,...)` routines: `moveh`, `drawh`, `fillh`, `dchr`... |
//! | `fmt.rs` | Fortran `Fw.d`, `Iw`, `O3.3` edit descriptors |
//! | `tables.rs` | font widths and colours (generated from `graph.f90`) |
//!
//! **Upstream:** `viewr.f90`, `graph.f90` (NJOY2016.80). **Manual:**
//! LA-UR-17-20093 §VIEWR. Not reproduced: the card echo VIEWR prints on its
//! output listing.

mod axis;
mod curves;
mod driver;
mod fmt;
mod graph;
mod plot2d;
mod plot3d;
mod ps;
mod state;
mod symbols;
mod tables;
mod text;

pub use graph::Graph;
pub use plot2d::{ascalv, Curve2d};
pub use state::ViewrState;
pub use text::{iget, rget, stripv};

use crate::NjoyError;

/// VIEWR on the plot-command text `plot`: the PostScript file it writes
/// (`viewr.f90`), with fresh module state.
///
/// # Errors
/// A malformed command file or an error of a routine it calls
/// ([`ViewrState::viewr`]).
pub fn viewr_postscript(plot: &str) -> Result<String, NjoyError> {
    ViewrState::default().viewr(plot)
}

/// Run VIEWR as a bare module. Needs the deck's tape units, so it is available
/// only through the deck driver (`deck::run_deck`).
pub fn run() -> Result<(), NjoyError> {
    Err(NjoyError::NotPorted("viewr without a deck (use deck::run_deck or viewr::viewr_postscript)"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SMALL: &str = "1 2 .30  2/\n1  3/\n'<t>est plot'/\n'<s>ub title'/\n1 0 2 0  0 0/\n   1.0   100.0   1.0/\n'<e>nergy'/\n   1.0   100.0   1.0/\n'<c>ross section'/\n/\n/\n'curve'\n0/\n  1.0 2.0\n  50.0 40.0\n/\n99/\n";

    #[test]
    fn a_small_plot_makes_a_complete_postscript_file() {
        let ps = viewr_postscript(SMALL).unwrap();
        assert!(ps.starts_with("%!PS-Adobe-\n%%BoundingBox:"));
        assert!(ps.contains("%%Page:   1   1"));
        assert!(ps.contains("(T) show"));
        assert!(ps.trim_end().ends_with("%%EOF"));
        assert_eq!(ps, viewr_postscript(SMALL).unwrap());
    }

    #[test]
    fn state_persists_between_calls_as_in_the_fortran_process() {
        let mut st = ViewrState::default();
        let a = st.viewr(SMALL).unwrap();
        let b = st.viewr(SMALL).unwrap();
        // `drawh`'s saved line width and dash are remembered: the second file
        // does not repeat the setlinewidth the first needed.
        assert!(a.matches("setlinewidth").count() >= b.matches("setlinewidth").count());
    }

    #[test]
    fn a_truncated_command_file_is_an_error() {
        assert!(viewr_postscript("1 2 .3 0/\n1 3/\n").is_err());
    }
}
