//! Convenience re-exports — the intended entry point.
//!
//! ```
//! use outram_park_digital_twin_engine::prelude::*;
//! ```
//!
//! **A public item reachable only by its full module path is not considered
//! exposed.** If you add one on the critical path, add it here in the same
//! change.
//!
//! That rule is not decoration. Across seven crates dogfooded in gh #58, every
//! usability finding had the same shape — the type was exported, the way in was
//! not shown — and the two worst were preludes: one that imported *nothing*
//! (an empty scope, no error, every later type failing to resolve with a
//! message pointing at the wrong place), and one that exported a `todo!()`
//! which panics.
//!
//! ## Headless and ASCII come first
//!
//! ```
//! use outram_park_digital_twin_engine::prelude::*;
//!
//! // Any simulator implementing HeadlessModel runs with no GUI:
//! //   let trace = run(&mut model, controls, HeadlessRun::default());
//!
//! // And any schematic geometry renders to a terminal-readable grid:
//! let mut c = AsciiCanvas::new(20, 6, (0.0, 0.0), (10.0, 10.0));
//! c.rect(1.0, 1.0, 9.0, 5.0, Some("CORE"));
//! c.arrow(5.0, 2.0, 5.0, 4.0); // y grows DOWN, so this flows downward
//! assert!(c.render().contains('v'));
//! ```

// Headless execution — required of every egui simulator here (gh #150).
pub use crate::headless::{assert_deterministic, run, run_csv, HeadlessModel, HeadlessRun};

// Schematic rendering to a character grid, so a diagram is checkable by a test
// or an agent rather than only by eye.
pub use crate::ascii::AsciiCanvas;

// The app scaffold's shared-state and GUI plumbing.
pub use crate::app_scaffold::{PanelSet, SharedState};
