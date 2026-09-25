//! # DOVER
//!
//! **DOVER** — ***D**eck-based **O**pen-source **V**isualisation **E**ngine for
//! **R**eactors*.
//!
//! # STATUS: EMPTY SKELETON. The scope is not yet decided. Nothing is implemented.
//!
//! Created 2026-09-25 at the maintainer's direction as an empty member crate.
//! The name points towards visualisation driven by input decks, but **the
//! scope is still to be decided**: no deck format, no visualisation engine and
//! no physics exist here, and none should be inferred from the name.
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
