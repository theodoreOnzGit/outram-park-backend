//! # DOVER
//!
//! **DOVER** — ***D**eck-based **O**pen-source **V**isualisation **E**ngine for
//! **R**eactors*.
//!
//! # Role: the low-fidelity counterpart of DHOBY GHAUT
//!
//! **DOVER is the low-fidelity equivalent of `dhoby-ghaut`** (maintainer,
//! 2026-09-25). DHOBY GHAUT is the GUI home that drives the *high-fidelity*
//! solvers; DOVER plays the same role at *low fidelity*, visualising reactors
//! from input decks. Which low-fidelity models it drives, what a deck is, and
//! whether it carries a windowing GUI are **not yet decided**.
//!
//! **Direction (2026-09-25, tentative):** input decks are perhaps TOML files,
//! read and written by a schema-checked reader, and DOVER runs steady-state
//! simulations like DWSIM as well as dynamic ones. Reuse
//! `outram-park-fork-dwsim-libs` (flowsheet, flowsheet solver, dynamics) and
//! `chem-eng-real-time-process-control-simulator` rather than duplicate them.
//! See the README.
//!
//! # STATUS: EMPTY SKELETON. Nothing is implemented.
//!
//! Created 2026-09-25 at the maintainer's direction as an empty member crate.
//! No deck format, no visualisation engine and no physics exist here, and none
//! should be inferred from the name or the role above.
//!
//! This crate has no dependencies, no public items and no behaviour. **Do not
//! describe it as providing anything**, and do not cite it as the home of any
//! capability until something has actually been written here.
//!
//! # Intended use
//!
//! Research, education, capability building and V&V only
//! (`RESPONSIBLE_USE.md` at the workspace root). Whatever this crate acquires,
//! it must not be presented as supporting nuclear facility operation, reactor
//! control, licensing or safety-critical decisions.

#![no_std]
#![forbid(unsafe_code)]

#[cfg(test)]
mod tests {
    /// The skeleton is a skeleton. This asserts only that the crate builds,
    /// links and carries its own name, which is the only claim it is entitled
    /// to make.
    #[test]
    fn the_crate_builds_and_links() {
        assert_eq!(env!("CARGO_PKG_NAME"), "dover");
    }
}
