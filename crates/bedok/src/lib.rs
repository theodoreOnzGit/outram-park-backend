//! **BEDOK** — 3-D nodal-diffusion neutronics coupled to thermal hydraulics.
//!
//! A Rust translation of Than Yan Ren's (SNRSI) MATLAB implementation, ported
//! from the `main_exec_diff3d_standalone` snapshot.
//!
//! # How this crate is laid out
//!
//! **One module per `.m` file, flat, named after the original.** That is the
//! whole organising principle and it is deliberate: `th_solverxyz.rs` is
//! `th_solverxyz.m`, function for function. There is no `nodal/`, no `th/`, no
//! `coupling/` — the MATLAB has no such folders, and the point of this
//! translation is that a reader can hold the `.m` file and the `.rs` file side
//! by side.
//!
//! Two consequences worth knowing before you go looking for things:
//!
//! - **Module names are `snake_case` even where the original was not.**
//!   `makegradDxyz.m` becomes `makegrad_dxyz`, because Rust warns on
//!   non-snake-case module names. The original filename is always named in the
//!   module's own doc comment.
//! - **Three modules have no `.m` counterpart** — [`matlab`], [`types`] and
//!   [`error`]. They carry the MATLAB container semantics, the loosely-typed
//!   structs, and the `error(...)` conditions respectively. Each says so at the
//!   top. Everything else is a translation.
//!
//! # Indexing — read this before editing any module
//!
//! **The port is 0-based.** The reference's 1-based index arithmetic is
//! converted, so
//!
//! ```text
//! idx = (g-1)*es + (ix-1)*maxiy*maxiz + (iy-1)*maxiz + iz     % MATLAB
//! ```
//!
//! becomes
//!
//! ```text
//! idx =  g*es    +  ix*maxiy*maxiz    +  iy*maxiz    + iz     // Rust
//! ```
//!
//! Storage stays **column-major** in [`matlab::Array2`], [`matlab::Array3`] and
//! [`matlab::Array4`], because the reference does linear indexing into
//! multi-dimensional arrays and the layout is therefore observable.
//!
//! ## Two places the conversion is not mechanical
//!
//! Both are documented where they occur; they are the things to check first if
//! an index bug shows up.
//!
//! - **[`convert_grid3d`] used `0` as a "no material here" sentinel**, which
//!   only worked because MATLAB indices start at 1. Going 0-based makes `0` a
//!   valid index. That map is now `Option<usize>` — `None` for absent — and the
//!   two sites that tested `== 0` test `is_none()`.
//! - **[`convertindexc2d`] keeps 1-based arithmetic internally.** It maps
//!   between two index spaces whose definition — the `(2n+1)` half-index grid
//!   interleaving cell centres and edges — is stated in 1-based terms, where the
//!   offsets are load-bearing rather than incidental. It converts at the
//!   boundary instead, so callers still see 0-based indices.
//!
//! Material identifiers are a third thing worth knowing about, though it is not
//! an indexing change: `whichsigma` stores **1-based material numbers with `0`
//! meaning void**, straight from the benchmark composition CSVs. A node holding
//! material `m` reads row `m - 1` of the cross-section table. See
//! [`calcdiffvalues3d`].
//!
//! # Translation policy — no silent repairs
//!
//! The MATLAB is unfinished, and the snapshot is terminal. Defects are
//! translated **as they are**, with the defect described in the doc comment of
//! the item that carries it and a test that pins the wrong behaviour so a later
//! fix is a visible, deliberate change. Repairs belong in a separate stage with
//! before/after numbers, never mixed into translation. The reasoning is in
//! the crate README, "Translation policy": a translation carrying well-meant fixes
//! cannot be debugged against a benchmark, because a disagreement can no longer
//! be attributed.
//!
//! Defects recorded so far live in `docs/bedok-reference-defects.md` and in
//! these places in the code:
//!
//! - [`convertindexc2d`] — the mode 1 → mode 2 → mode 1 round trip is **not**
//!   the identity; the two directions disagree by an off-by-one in the forward
//!   row calculation. Found by running the tests, not by reading the code.
//! - [`handle3dcoords`] — the generic branch assigns `params.maxix` to
//!   `maxi3`.
//! - [`convert_grid3d`] — precursor indices collide for `Nc > 1`.
//! - [`geometry_ends3d`] — only the first contiguous run per grid line is
//!   found.
//! - [`convertsparsekey3d`] — the diagnostic decode is hard-coded to a
//!   17×17×19 grid.
//! - [`calc_bucklingxyz`] — the cache fingerprint is three sums and three
//!   non-zero counts, which cannot separate every distinct cross-section set;
//!   a collision silently reuses the wrong cached coefficients.
//! - [`calc_abefghxyz`] — `abefgh` loses precision to cancellation as `alpha`
//!   goes to zero, with no series fallback.
//! - [`makesigmadfxyz`] — in half-index mode the `iz` loop bound is `maxiz`
//!   where the other two axes use `m*max…`, so the upper half of the core
//!   silently gets no cross sections. Latent: every call site passes mode 1.
//! - [`makegrad_dxyz`] — a **fuelled** node outside its line's `[low, high]`
//!   bounds is skipped by the `z` pass, keeps the pre-filled identity `1`, and
//!   then has `y` and `x` accumulated on top, leaving a spurious `+1` on its
//!   diagonal. Reachable via [`geometry_ends3d`]'s first-contiguous-run
//!   limitation. Confirmed by test.
//! - [`calc_sanodalxyz`] — the same root cause with the opposite symptom: the
//!   `y`/`x` passes accumulate into a diagonal slot the `z` pass was supposed
//!   to create, so a node `z` missed **aborts** rather than computing something
//!   wrong. Confirmed by test.
//! - [`sanodaldiffusion_solverxyz`] — a nodal-update interval of **1 does not
//!   converge**, and the built-in default *is* 1 for any mesh whose three
//!   extents sum to 10 or fewer. Confirmed by test at three mesh sizes.
//! - [`diffusion_solverxyz`] — the empty-grid compaction is unreachable
//!   (`keychange` is a hard-coded `0`), and four `writematrix` CSV dumps run
//!   unconditionally on every call.
//! - Both flux solvers — a bailed-out iteration returns the **previous**
//!   pass's `k_eff` and residuals, and says nothing about having bailed. The
//!   translation adds a `Termination` value rather than reproducing the
//!   silence.
//!
//! # Two deliberate departures from the reference, both in the flux solvers
//!
//! Everything else in this crate reproduces the reference exactly. These two do
//! not, and the reasoning is recorded in `docs/bedok-reference-defects.md`
//! alongside defects D1-D7:
//!
//! - **The diagnostic CSV writes are returned, not written.** Both solvers
//!   compute the same symmetry maps the reference dumps to disk and hand them
//!   back in a `Diagnostics` struct. A library that writes files as a side
//!   effect of being called cannot be used concurrently or tested cleanly, and
//!   the physics is identical either way.
//! - **The `gmres`/`ilu` branch is not translated.** It is selected at
//!   `philenf >= 50_000_000`, which is unreachable for any runnable problem, so
//!   an ILU and a restarted GMRES written for it could never be verified
//!   against the reference. Both solvers return
//!   [`error::BedokError::IterativeSolveNotTranslated`] instead, which names
//!   the threshold.
//!
//! # Status — translation complete; verification partial
//!
//! **All 50 `.m` files are translated**: the
//! utility and indexing layer, all fourteen SANM nodal files, both flux
//! solvers, the whole thermal-hydraulics layer, **both** coupling drivers
//! (steady and transient), and five benchmark cases ([`iaea3ds`],
//! [`neacrpd1`], [`neacrpd1t`], [`neacrpa2`], [`neacrpa2t`] and
//! [`neacrpa1t`]), the critical-boron search, and the 2-D legacy case. [`iapws_if97`] is partially done — regions
//! 1, 2 and 4 with the backward and transport entry points, but **not region
//! 3**, which caps everything at 16.5292 MPa.
//!
//! **Three of the fifty do not land as modules**, and each says why in its own
//! header:
//!
//! - `main_exec_diff3d.m` and `run_neacrpd1t.m` are **scripts**, not functions.
//!   They become `examples/`, which is also the entry point the workspace's
//!   human-interface rule asks for.
//! - `plotreactor3dcolour.m` is half data preparation and half MATLAB figure
//!   emission. The first half is [`plotreactor3dcolour`]; the rendering is not
//!   translated, on the same reasoning as the CSV policy.
//!
//! One more is translated but **cannot be run**: [`geom2dxycase1`] builds a 2-D
//! case, and every solver in the snapshot is 3-D. Its own call site in
//! `main_exec_diff3d.m` is commented out.
//!
//! # Verified against the running MATLAB
//!
//! Both steady NEACRP cases reproduce the reference **exactly** — A2 at
//! `k_eff = 1.0139476080` and D1 at `0.9752848326`, with fuel temperature,
//! coolant temperature, heat flux and power identical to every printed digit.
//! Getting to that found two defects: **Z1**, a silently rounded axial mesh,
//! and **N1**, an unstable default nodal-update interval that makes A2's
//! coupled loop chaotic.
//!
//! **The transient path reproduces it too**, on all three cases — including the
//! super-prompt HZP ejection (2.1e-7 through a 67-fold excursion) and D1t over
//! its **full 20 s window** (261 steps, 2.9e-11). [`iaea3ds`] matches the
//! MATLAB as well as its published values, and [`criticalboron_xyz`] **fails
//! where the reference fails**, aborting at the same boron concentration.
//!
//! What remains outside the comparison: the T-H modules individually (they are
//! covered only transitively), `w3chf` (whose output the reference discards,
//! so there is nothing to compare), the ejection transients beyond 0.15 s, and
//! IAPWS region 3, which is not translated. See
//! `docs/bedok-reference-defects.md`.
//!
//! # Corrections are on by default — this crate is no longer bit-faithful
//!
//! With every runnable case verified against the MATLAB, **stage 2 opened on
//! 2026-08-21** and three groups of reference defects are now **corrected by
//! default**. On the cases they touch, this crate deliberately no longer
//! reproduces the MATLAB's numbers.
//!
//! | Defects | What | Switch |
//! |---|---|---|
//! | G1, G2, G3 | The diffusion face coupling and `gradterms` | [`types::Params::gradd_form`] |
//! | T5, T6 | The W-3 `K4` subcooling enthalpy | [`types::Params::w3_form`] |
//! | C2, T4 | `highy = ix` in the hottest-channel search | [`types::Params::hot_channel_search`] |
//! | T9, T13 | `fueltempavg` aliased to the Doppler temperature | [`types::Params::fueltemp_average`] |
//!
//! **To reproduce the reference, build from
//! [`types::Params::reference_faithful`]**, which turns every correction off.
//! That is what the MATLAB-parity tests do.
//!
//! What each is worth, measured: the operator correction moves NEACRP A2's
//! critical boron from 1138.8 to **1152.5 ppm** against a published 1160.6, and
//! A1's from 551.4 to **561.0** against 567.7 — closing 63% and 59% of the two
//! gaps. The CHF corrections cut A2's reported thermal margin by **18.2%**
//! (DNBR 2.55 to 2.08), both defects having overstated it. The fuel-average
//! correction moves only a reported number — D1's average fuel temperature
//! rises up to 240 K — while the Doppler temperature that drives the feedback
//! is bit-identical and `k_eff` does not move.
//!
//! Everything else in `docs/bedok-reference-defects.md` is still translated
//! as-is and pinned by a test asserting the wrong behaviour.
//!
//! **Both the steady and the transient paths now run end to end on real
//! benchmark cases.** [`iaea3ds`] matches a published `k_eff` to -1.1 pcm;
//! [`neacrpd1`] drives the coupled loop to a joint fixed point in 12 outer
//! passes; [`neacrpd1t`] marches the case-D cold-water injection through
//! [`thdiffusion_solvertimexyz`] with six delayed-neutron families. **No
//! transient result has been compared to a published curve** — the NEACRP
//! specification is not in the literature archive, so the transient tests
//! assert structure and cross-scheme agreement only.
//!
//! [`iapws_if97`] is the one module that is **not** Than Yan Ren's code: it
//! translates a third-party BSD-2-Clause implementation by Mark Mikofski, whose
//! terms are reproduced in the crate `NOTICE`. BSD-2-Clause is
//! GPL-3.0-compatible.
//!
//! The crate builds clean under clippy and rustdoc, and its 135 unit tests pass
//! (rustc 1.97.1, release profile, 2026-08-13).
//!
//! **What that does and does not establish.** The tests cover the translated
//! utility layer and pin the reference defects listed above; [`iapws_if97`]'s
//! region-1 functions are checked against the published IAPWS-IF97 verification
//! values and agree to ~3e-9 relative, which is the tabulated values' own
//! precision, and region 4's saturation line agrees with Tables 35 and 36 to
//! **1.752e-9**, with the two directions inverting each other to 4.263e-15.
//!
//! [`w3chf`]'s unit-conversion chain — seven constants with the British-unit
//! conversions folded in — is confirmed by the correlation landing at
//! 2.7-3.4 MW/m² at PWR conditions, the expected magnitude. That verifies the
//! transcription, not the correlation.
//!
//! The two flux solvers are exercised on a **uniform leaking cube** — one
//! energy group, vacuum on all six faces, an analytically known
//! `k_inf = 2.5` — where they converge to a positive, centre-peaked
//! fundamental mode below `k_inf`, and where the nodal correction moves the
//! eigenvalue -299 pcm off finite difference on a 4x4x4 mesh. That checks the
//! assembly, the factorisation and the iteration hang together and produce a
//! physically-signed answer.
//!
//! **The one benchmark comparison is [`iaea3ds`].** The IAEA 3-D PWR
//! benchmark — 17x17x19 quarter core, two groups — gives `k_eff = 1.029084`
//! against 1.029096 (PARCS) and 1.029082 (ADPRES), so **-1.1 pcm and +0.2
//! pcm**, closer than the two reference codes are to each other. Measured
//! 2026-08-18; see that module and the README for the full statement, and
//! `src/data/PROVENANCE.md` for where the two reference numbers come from.
//!
//! That is the crate's **only** validation evidence, and its scope is narrow:
//! pure neutronics, no thermal-hydraulics, no coupling, no transient, and no
//! comparison against the benchmark's published assembly powers. The coupled
//! driver [`thdiffusion_solverxyz`] is **not** shown to converge on any case —
//! read its "Verification status" before using it. Per `RESPONSIBLE_USE.md`
//! everything here remains AI-assisted draft material pending human review;
//! "the tests pass" is not the same claim as "the physics is right".
//!
//! # Provenance
//!
//! - **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
//!   Institute (SNRSI).
//! - **Permission:** given by the author for open-source release under OUTRAM
//!   PARK, with institutional approval; see the crate README, "Permission and attribution".
//! - **Licence:** GPL-3.0-only.

// --- support layer (no `.m` counterpart) ---------------------------------
pub mod driftflux6_solverstatic1d;
pub mod error;
pub mod matlab;
pub mod thdiffusion_solverxyz;
pub mod th_solvertimexyz;
pub mod th_solverxyz;
pub mod types;

// --- translated `.m` files -----------------------------------------------
pub mod calc_1sttransleakagexyz;
pub mod calc_2ndtransleakagexyz;
pub mod calc_a1234_expansionxyz;
pub mod calc_a1_expansionxyz;
pub mod calc_abefghxyz;
pub mod calc_bucklingxyz;
pub mod calc_relpower3d;
pub mod calc_sanodalxyz;
pub mod calc_transleakagexyz;
pub mod calcdiffvalues3d;
pub mod convert_grid3d;
pub mod convertindexc2d;
pub mod convertsparseformat2d;
pub mod convertsparsekey3d;
pub mod criticalboron_xyz;
pub mod diffusion_solverxyz;
pub mod fiss_src_extrapolatexyz;
pub mod fuelrodheat_1dcylnd;
pub mod fuelrodheattime_1dcylnd;
pub mod fixinfnan;
pub mod fixnegativematrix;
pub mod geom2dxycase1;
pub mod geometry_ends3d;
pub mod iapws_if97;
pub mod iaea3ds;
pub mod handle2dcoords;
pub mod handle3dcoords;
pub mod makegrad_dxyz;
pub mod makeheatlaplacian_1dcylnd;
pub mod makesigmadfxyz;
pub mod thdiffusion_solvertimexyz;
pub mod neacrpa1t;
pub mod neacrpa2;
pub mod neacrpa2t;
pub mod neacrpd1;
pub mod neacrpd1t;
pub mod pauseonnan;
pub mod plotreactor3dcolour;
pub mod sanodaldiffusion_solverxyz;
pub mod sigmavalupd3d;
pub mod sigmavalupd3d_handler;
pub mod singleflow1devap;
pub mod singleflow1devaptime;
pub mod w3chf;
pub mod w3chfhottest;

pub use error::{BedokError, Result};
