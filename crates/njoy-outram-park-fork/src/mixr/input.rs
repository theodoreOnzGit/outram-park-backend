// Ported from NJOY2016 `src/mixr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! MIXR card-input deck — the typed analogue of `mixr.f90`'s six input cards.
//!
//! This module models the user input MIXR reads from `nsysi`
//! (`mixr.f90:31-56`, read at `mixr.f90:99-121`) as a documented Rust struct,
//! [`MixrInput`], rather than a free-format Fortran card reader. Each field
//! cites the card and the `mixr.f90` line it comes from.
//!
//! **Units convention.** Like the rest of this crate's [`crate::endf`] model,
//! energies are in **eV** and cross sections in **barns** as plain `f64`
//! (the ENDF tabulated values are already in those units); mixing weights are
//! **dimensionless**. `uom` is deliberately *not* introduced at this boundary:
//! the mixing engine operates directly on [`crate::endf::records::Tab1`]
//! `(f64, f64)` pairs, and wrapping every tabulated point in a dimensioned
//! quantity would break that reuse and raise the mental-context load a human
//! reader carries (see the workspace "human interface layer" rule). Units are
//! instead spelled out in every doc comment.

/// One component of a mix — the Rust analogue of a single index-aligned triple
/// `(nin(i), matn(i), wtn(i))` in `mixr.f90`.
///
/// In Fortran MIXR the i-th input unit `nin(i)` (card 1, `mixr.f90:35-37,99`)
/// is read for the i-th material `matn(i)` with weight `wtn(i)` (card 3,
/// `mixr.f90:43-45,113`); the read loop `do i=1,nmat` at `mixr.f90:150-195`
/// opens `nin(i)` and locates `matn(i)` on it. This crate holds every input
/// tape fully parsed in memory, so the Fortran logical-unit number `nin(i)`
/// becomes `tape_index`: an index into the `inputs: &[Tape]` slice passed to
/// the mixing engine.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MixComponent {
    /// Index into the `inputs` slice given to the mixing engine — the analogue
    /// of the Fortran input unit number `nin(i)` (`mixr.f90:99`). The caller
    /// must supply `inputs` in the same order as the card-1 `nin` list.
    pub tape_index: usize,
    /// Material number (`MAT`) to read from that tape — `matn(i)`
    /// (`mixr.f90:113`).
    pub mat: i32,
    /// Mixing weight `w_i`, dimensionless — `wtn(i)` (`mixr.f90:113`). The
    /// output cross section is `sigma_out(E) = sum_i w_i * sigma_i(E)`
    /// (`mixr.f90:302`).
    pub weight: f64,
}

/// The full MIXR input deck (`mixr.f90:31-56`), as a typed struct.
///
/// # Card layout (`mixr.f90`)
///
/// - **Card 1 — units** (`mixr.f90:33-37`): `nout` then up to
///   `nninmx = 10` input units `nin1..ninN`. Here `nout` becomes
///   [`output_unit`](Self::output_unit) (informational only — the in-memory
///   engine returns a [`crate::endf::tape::Tape`] rather than writing a
///   physical unit), and the `nin` list is folded into
///   [`components`](Self::components) alongside card 3.
/// - **Card 2 — reaction list** (`mixr.f90:39-41`): up to `nmtmx = 20`
///   output MT numbers -> [`mt_list`](Self::mt_list).
/// - **Card 3 — material list** (`mixr.f90:43-45`): up to
///   `nmatmx = 10` `(matn, wtn)` pairs. Zipped index-for-index with the card-1
///   `nin` list into [`components`](Self::components).
/// - **Card 4 — temperature** (`mixr.f90:47-48`): `temp` in kelvin ->
///   [`temperature_kelvin`](Self::temperature_kelvin) (use `0` except for PENDF
///   tapes).
/// - **Card 5 — output material** (`mixr.f90:50-53`): `matd`, `za`, `awr` ->
///   [`output_mat`](Self::output_mat), [`output_za`](Self::output_za),
///   [`output_awr`](Self::output_awr).
/// - **Card 6 — comment** (`mixr.f90:55-56`): a <=66-char description ->
///   [`description`](Self::description).
#[derive(Debug, Clone, PartialEq)]
pub struct MixrInput {
    /// Card-1 `nout`: the Fortran output unit number. Informational only —
    /// [`crate::mixr::mix`] returns an in-memory tape (`mixr.f90:34`).
    pub output_unit: i32,
    /// The mix: one [`MixComponent`] per index-aligned `(nin(i), matn(i),
    /// wtn(i))` triple from cards 1 and 3 (`mixr.f90:99,113`).
    pub components: Vec<MixComponent>,
    /// Card-2 output MT numbers, in request order (`mixr.f90:106`). Each yields
    /// one MF=3 section on the output tape.
    pub mt_list: Vec<i32>,
    /// Card-4 temperature in **kelvin** (`mixr.f90:117`). Used only for the
    /// output MF=1/451 header record; MIXR does not itself Doppler-broaden.
    pub temperature_kelvin: f64,
    /// Card-5 output material number `matd` (`mixr.f90:118`).
    pub output_mat: i32,
    /// Card-5 `za` = 1000*Z + A of the output material (`mixr.f90:118`).
    pub output_za: f64,
    /// Card-5 `awr` = atomic weight ratio (mass / neutron mass) of the output
    /// material, dimensionless (`mixr.f90:118`).
    pub output_awr: f64,
    /// Card-6 description text, <=66 chars (`mixr.f90:120`). See the
    /// module-level note in [`crate::mixr::mix`]: this text is **not** written
    /// into the output MF=1/451 comment record, because the crate's `[f64; 6]`
    /// section-row model cannot store Hollerith characters.
    pub description: String,
}

impl MixrInput {
    /// Build a [`MixrInput`] straight from the six MIXR cards, mirroring the
    /// Fortran read order (`mixr.f90:99-121`).
    ///
    /// - `nout` — card 1 output unit (`mixr.f90:34`).
    /// - `nin` — card 1 input unit list (`mixr.f90:35-37`). Element `i` becomes
    ///   `MixComponent { tape_index: i, .. }`.
    /// - `mt_list` — card 2 output MT numbers (`mixr.f90:40`).
    /// - `mat_weights` — card 3 `(matn, wtn)` pairs (`mixr.f90:44-45`), zipped
    ///   index-for-index with `nin`.
    /// - `temperature_kelvin` — card 4 (`mixr.f90:48`).
    /// - `output_mat`, `output_za`, `output_awr` — card 5 (`mixr.f90:52-53`).
    /// - `description` — card 6 (`mixr.f90:56`).
    ///
    /// The `nin` and `mat_weights` lists are zipped to the shorter of the two,
    /// exactly reproducing the Fortran alignment where component `i` reads
    /// `matn(i)` from `nin(i)`. Trailing zero-padded card entries (which
    /// Fortran drops via the `nnin`/`nmat` scans at `mixr.f90:100-116`) should
    /// simply be omitted from these Rust vectors.
    ///
    /// # Example
    /// ```
    /// use njoy_outram_park_fork::mixr::MixrInput;
    /// // Mix MAT 125 (from input tape 0) and MAT 128 (from input tape 1)
    /// // with weights 0.7 and 0.3, producing MT=1 and MT=2 on output MAT 9999.
    /// let input = MixrInput::from_cards(
    ///     20,                       // nout
    ///     &[0, 1],                  // nin list -> tape indices
    ///     vec![1, 2],               // mt list
    ///     &[(125, 0.7), (128, 0.3)],// (matn, wtn) pairs
    ///     0.0,                      // temperature (K)
    ///     9999, 1001.0, 0.9992,     // matd, za, awr
    ///     "element mix",            // comment
    /// );
    /// assert_eq!(input.components.len(), 2);
    /// assert_eq!(input.components[1].tape_index, 1);
    /// assert_eq!(input.components[1].mat, 128);
    /// ```
    #[allow(clippy::too_many_arguments)]
    pub fn from_cards(
        nout: i32,
        nin: &[usize],
        mt_list: Vec<i32>,
        mat_weights: &[(i32, f64)],
        temperature_kelvin: f64,
        output_mat: i32,
        output_za: f64,
        output_awr: f64,
        description: impl Into<String>,
    ) -> Self {
        let components = nin
            .iter()
            .zip(mat_weights.iter())
            .map(|(&tape_index, &(mat, weight))| MixComponent { tape_index, mat, weight })
            .collect();
        MixrInput {
            output_unit: nout,
            components,
            mt_list,
            temperature_kelvin,
            output_mat,
            output_za,
            output_awr,
            description: description.into(),
        }
    }
}
