//! # KEMBANGAN
//!
//! **K**inetics, **E**lasticity and **M**aterials **B**ehaviour for **A**ging
//! in **N**uclear Environments through **G**enerative **A**I and
//! **N**umerical modelling.
//!
//! Materials damage under irradiation: how a material changes when
//! particles hit it. Research, education and V&V only.
//!
//! **What exists (2026-11-22, gh:#1699):** [`rustbca`], the copy of RustBCA
//! (binary-collision Monte Carlo for ions and the recoils they knock on),
//! moved here from `boon-lay`, which still re-exports it as
//! `boon_lay::rustbca`. **Not implemented:** dpa, gas production and
//! property changes.
//!
//! **Dependency direction:** kembangan serves `boon-lay` and other crates;
//! `punggol`, `outram-mc-libs`, `njoy-outram-park-fork` and `petir` serve
//! kembangan. None of them may depend on it.

// `no_std` + `alloc` without the default `std` feature. Unit tests always
// build with `std`.
#![cfg_attr(not(any(feature = "std", test)), no_std)]
// As in boon-lay: the std-gated parts of RustBCA leave items unused without
// `std`.
#![cfg_attr(not(feature = "std"), allow(dead_code, unused_imports, unused_variables, unused_mut, irrefutable_let_patterns))]

// `alloc` by name (a std crate does not put `alloc` in its extern prelude on
// its own); RustBCA's files import `alloc` items by name.
extern crate alloc;
extern crate anyhow_nostd as anyhow;

/// What this crate is for, in one line (the placeholder-crate convention,
/// kept for the README and the test below).
pub const SCOPE: &str = "materials damage under irradiation: binary-collision cascades now; dpa, gas production and property changes planned";

/// **A copy of RustBCA, not kembangan's own code** — the binary-collision
/// Monte Carlo for ions and the recoils they knock on (fission fragments,
/// alphas, recoil knock-ons), copied from `lcpp-org/RustBCA` (`a356280d`,
/// v3.0.0 plus 17 commits, gh:#1426; GPL-3.0, Jon Drobny and Davide Curreli)
/// and made `no_std` + `alloc`. Moved from `boon-lay` on 2026-11-22
/// (gh:#1699); `boon_lay::rustbca` re-exports it. The module docs carry the
/// attribution and every local change; fragment results are illustrative
/// until V&V'd.
pub mod rustbca;

#[cfg(test)]
mod tests {
    #[test]
    fn scope_names_materials_damage() {
        assert!(super::SCOPE.contains("materials damage"));
    }
}
