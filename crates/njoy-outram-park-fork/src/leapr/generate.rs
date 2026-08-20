//! **Thermal-scattering `S(alpha, beta)` by regeneration** — the default source.
//!
//! This is the consumer surface for thermal scattering laws. Ask for a material
//! at a temperature and you get an ENDF MF=7 [`Mf7`] (or, with
//! [`thermal_scattering_tape`], the whole tape), regenerated from that
//! evaluation's own ~12 KB LEAPR card deck rather than read from its
//! multi-megabyte tape.
//!
//! ```no_run
//! use njoy_outram_park_fork::leapr::generate::{SabRequest, thermal_scattering_law};
//! use njoy_outram_park_fork::leapr::decks::SabMaterial;
//! use njoy_outram_park_fork::units::Temperature;
//! use uom::si::thermodynamic_temperature::kelvin;
//!
//! # fn demo() -> Result<(), njoy_outram_park_fork::NjoyError> {
//! // HTR-10 runs at temperatures the ENDF tape does not tabulate.
//! let law = thermal_scattering_law(&SabRequest::new(
//!     SabMaterial::CrystallineGraphite,
//!     Temperature::new::<kelvin>(523.0),
//! ))?;
//! let inelastic = law.incoherent_inelastic.as_ref().expect("MT=4");
//! println!("{} beta points", inelastic.beta.len());
//! # Ok(())
//! # }
//! ```
//!
//! # Why regeneration is the default
//!
//! Measured on 2026-08-13 against ENDF/B-VIII.0 crystalline graphite, in
//! release mode:
//!
//! - **The generated MF=7/MT=4 section is bit-identical to the official
//!   tape's.** Running the full path here — deck -> kernels -> [`endout`] ->
//!   ENDF text — and comparing the *stored* values at 296 K over the whole
//!   150 x 400 grid gives **60,000 / 60,000 values identical**, max relative
//!   deviation **0.000e0**. The 4.917e-6 residual the raw-kernel parity test
//!   reports (`tests/leapr_graphite_deck_parity.rs`) is the tape's own
//!   6-to-7-significant-figure storage round-off, and it vanishes once
//!   `endout` applies the same `sigfig` rounding NJOY applies. So the
//!   12,444-byte deck does not *approximate* the 8,730,804-byte tape for this
//!   channel; it reproduces it. (Reproduced by
//!   `examples/graphite_sab_generation.rs`.)
//! - It can generate at temperatures the tape never tabulated. Graphite is
//!   tabulated at 296/400/500/600/700/800/1000/1200/1600/2000 K; the HTR-10
//!   benchmark wants 393 K and 523 K, and interpolating a scattering law
//!   between tabulated temperatures is strictly worse than generating at the
//!   one you want.
//! - Generation costs **1.83-2.72 s per temperature**, measured over two cold-
//!   cache runs on a 12-core workstation shared with other work (1.836 / 1.830 /
//!   1.832 s at 296 / 393 / 523 K on the quieter run; 2.082 / 2.721 / 2.180 s on
//!   the busier one). Contention can only inflate a timing, so **~1.83 s is an
//!   upper bound** on the uncontended cost. A **disk-cache hit is 0.009 s** and
//!   an in-process memo hit is under a millisecond, so this is a first-use cost,
//!   not a per-query one.
//!
//! # What is and is not validated
//!
//! **This matters more than the convenience**, and it is **per material**, not
//! a property of the code path. Only crystalline graphite has been compared
//! against a reference tape:
//!
//! | Material / channel | Status (2026-08-13) |
//! |---|---|
//! | `CrystallineGraphite` MF=7/**MT=4** incoherent inelastic | **Validated** — 60,000 / 60,000 stored values bit-identical to ENDF/B-VIII.0 at 296 K. |
//! | `CrystallineGraphite` MF=7/**MT=2** coherent elastic | **Validated** — 221 / 221 Bragg grid points, max relative deviation **0.000e0** on both edge energies and `S(E)` at 296 K through this path. Across all ten temperatures, `tests/leapr_graphite_coherent_elastic_parity.rs` measures max **1.001e-13** on the raw kernel output (float round-trip noise on a 7-digit field). |
//! | `ReactorGraphite10P`, `ReactorGraphite30P` (either channel) | **Not validated.** They parse and generate through the identical path, but no parity measurement has been taken. |
//! | `HInH2O` MF=7/**MT=4** incoherent inelastic | **Not validated** — but *checked*. Regeneration at 293.6 K agrees with the published evaluation to **~0.6 %** on σ_inel over 0.0253–8 eV and **+0.09 %** on `T_eff` (1195.35 K vs 1194.3 K). That is an agreement band against this repository's own recorded measurements, not a tape diff, so [`SabRequest::validation`] still reports it unvalidated. See `tests/leapr_h2o_secondary_scatterer.rs`. |
//!
//! MT=2 matters out of proportion to its size: it is roughly 90 % of graphite's
//! thermal cross section (4.55 b coherent-elastic against 0.49 b inelastic at
//! 0.0253 eV) while being 0.4 % of the tape's bytes. Making regeneration the
//! default could not have rested on the MT=4 result alone.
//!
//! **Both channels reach that agreement only because the constant set comes
//! from the deck's declared vintage**, and they need *different* constants from
//! it: MT=4 depends on `bk` through `tev = bk*T`, MT=2 on `ev`/`amu`/`hbar`/
//! `amassn` through [`crate::leapr::vintage::PhysicalConstants::econ`]. With the
//! crate's modern constants MT=2 is 9.986e-7 off — a uniform multiplicative
//! offset, not scatter. See [`crate::leapr::vintage`].
//!
//! [`SabRequest::validation`] returns this status programmatically, per
//! channel and per material, so a caller can act on it rather than rediscover
//! it. The elastic channel is generated **by default**; omitting it would drop
//! most of graphite's thermal cross section. Use [`ElasticChannel::Omit`] only
//! if you are sourcing that channel elsewhere.
//!
//! # A physics approximation you inherit either way
//!
//! `rho(E)`, the phonon frequency spectrum, is a **deck input, not a computed
//! quantity**, so generating at a new temperature reuses the spectrum the
//! evaluator calculated at theirs. Thermal expansion and anharmonicity are
//! therefore not modelled across temperature. This is inherent to how LEAPR
//! works and the shipped ENDF tape shares it exactly — its nine
//! higher-temperature blocks are "reuse the 296 K spectrum" entries — so
//! generating at 523 K is **not** a regression against reading the tape. It is
//! still an approximation, and it is the one that limits how far from the
//! deck's own temperature range you should go.
//!
//! # Caching
//!
//! Two layers, both keyed by a hash of the full generation recipe:
//!
//! - **On disk**, through [`crate::acquire::EndfCache`] — the crate's single
//!   caching layer, with its lock / double-check / fsync / atomic-rename /
//!   SHA-256 discipline. The cached artifact is an ordinary ENDF tape, so it is
//!   byte-for-byte the same *kind* of thing a download produces.
//! - **In process**, a memo of parsed [`Mf7`] values behind an `RwLock`, so a
//!   transport code asking for the same law per-material rather than once does
//!   not re-read and re-parse a ~1 MB tape.
//!
//! **Invalidation rule: none — the key is the recipe.** The cache file name
//! embeds a SHA-256 over the deck's *content* hash, the material, the
//! temperature, the physical-constant set (both `bk` and `econ`), the
//! elastic-channel choice, the Bragg cut-off, and [`GENERATOR_REVISION`].
//! Change any of them and you get a different file; nothing is ever overwritten
//! in place, so there is no expiry to get wrong. The deck's *path* is
//! deliberately **not** in the key — the same deck in two directories shares one
//! entry — but it is recorded in the `.recipe` sidecar written beside each
//! artifact. **If you change the LEAPR kernels or `endout` in a way that alters
//! output, bump [`GENERATOR_REVISION`]** — that is the one thing the hash cannot
//! see for itself.
//!
//! # Android
//!
//! Everything here is pure CPU and pure Rust, and builds for
//! `aarch64-linux-android`. The ~1.8 s/temperature generation cost is a real
//! consideration on a phone (expect several times that on a mobile core), which
//! is exactly why the disk cache exists: pay it once per (material,
//! temperature), not per run. A device that cannot afford even that should
//! point [`SabSource::EndfTape`] at a tape instead.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock, RwLock};

use uom::si::thermodynamic_temperature::kelvin;

use crate::acquire::EndfCache;
use crate::endf::tape::Tape;
use crate::leapr::coher::{coher_general_with_constants, coher_with_constants, GeneralCrystal};
use crate::leapr::continuous::phonon_expansion;
use crate::leapr::discrete::add_discrete_oscillators;
use crate::leapr::translation::add_translation;
use crate::leapr::deck::LeaprDeck;
use crate::leapr::decks::{embedded_deck_text, locate_deck, DeckSource, SabMaterial};
use crate::leapr::endout::{endout, ElasticOutput, LeaprOutput};
use crate::leapr::frequency::FrequencyModel;
use crate::leapr::input::ElasticOption;
use crate::leapr::vintage::PhysicalConstants;
use crate::thermr::mf7::{parse_mf7, Mf7};
use crate::units::Temperature;
use crate::NjoyError;

/// Bump this whenever a change to the LEAPR kernels or to
/// [`crate::leapr::endout`] alters generated output.
///
/// It is mixed into the cache key, so bumping it invalidates every previously
/// cached artifact without anyone having to delete files. The recipe hash sees
/// the deck and the parameters; it cannot see the code, so this constant stands
/// in for the code's identity. Leaving it stale after a physics change means
/// serving yesterday's numbers from cache — the one way this cache can lie.
/// **Revision 2 (2026-08-14)** — [`generate_tape`] now runs the translational
/// and discrete-oscillator stages (`trans`, `discre`) after the phonon
/// expansion, where it previously emitted the continuum-only law; and
/// [`crate::leapr::endout`] now writes the secondary-scatterer `B(7)..B(12)`
/// constants. The first changes `S(alpha, beta)`, `T_eff` and the Debye-Waller
/// integral for every deck with `twt > 0` or discrete oscillators (light water,
/// the hydrides, the methanes); graphite is unaffected, having neither term.
pub const GENERATOR_REVISION: u32 = 2;

/// Upper energy \[eV\] of the coherent-elastic Bragg sum handed to
/// [`crate::leapr::coher::coher`].
///
/// 5 eV is the value the graphite parity study used, and it comfortably covers
/// the thermal range where a scattering law applies (the ENDF/B-VIII.0 graphite
/// tape's retained Bragg grid tops out well below it). NJOY thins the tail
/// above the last significant edge, so raising this does not add resolution
/// where it matters.
pub const COHERENT_ELASTIC_EMAX_EV: f64 = 5.0;

/// Which MF=7 channels to emit.
///
/// A closed enum (workspace rule: no trait objects).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ElasticChannel {
    /// **Default.** Generate MF=7/MT=2 alongside MT=4, giving a complete
    /// scattering law.
    ///
    /// For crystalline graphite this channel is validated against
    /// ENDF/B-VIII.0 (max relative deviation 9.986e-7 over 2,200 tabulated
    /// values; see the [module docs](self)). It is the default regardless of
    /// material because omitting it drops roughly 90 % of graphite's thermal
    /// cross section, which is the worse error by a wide margin.
    #[default]
    Generate,
    /// Emit MT=4 only — the validated channel — because the elastic channel is
    /// being sourced elsewhere. The resulting [`Mf7`] has
    /// `coherent_elastic == None`, and a transport code that does not notice
    /// will under-count graphite's thermal cross section badly. Choose this
    /// deliberately.
    Omit,
}

impl ElasticChannel {
    /// A short label for cache keys. Stable.
    pub const fn label(self) -> &'static str {
        match self {
            ElasticChannel::Generate => "mt2+mt4",
            ElasticChannel::Omit => "mt4",
        }
    }
}

/// Where a scattering law comes from.
///
/// A closed enum (workspace rule: no trait objects). The default is
/// regeneration; the tape is used only when a caller points at one explicitly.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum SabSource {
    /// **Default.** Regenerate from the material's LEAPR deck (located by
    /// [`crate::leapr::decks::locate_deck`]) and cache the result.
    #[default]
    RegenerateFromDeck,
    /// Read an existing ENDF thermal-scattering tape from this path.
    ///
    /// Nothing is generated and nothing is cached; the tape is parsed as-is.
    /// Its tabulated temperatures are whatever the evaluator chose, so the
    /// requested temperature must be one of them (within
    /// [`crate::thermr::mf7::temperature_match_tolerance_k`]).
    EndfTape(PathBuf),
}

/// How much confidence a given MF=7 channel has earned, in this crate, today.
///
/// Returned by [`SabRequest::validation`] so a caller can branch on validation
/// status instead of having to have read the docs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelValidation {
    /// Compared point-by-point against a published reference evaluation and
    /// found to agree within that reference's own storage precision.
    ValidatedAgainstReferenceTape,
    /// Produced by ported, unit-tested kernels, but **not** compared
    /// point-by-point against a published reference evaluation. Treat the
    /// numbers as an untrusted draft: the code path is shared with a validated
    /// case, which is evidence about the code, not about this material.
    NotValidatedAgainstReferenceTape,
    /// The channel is not present in the result at all.
    NotEmitted,
}

/// A request for a thermal scattering law.
///
/// Built with [`SabRequest::new`] (regenerate, both channels) and adjusted with
/// the `with_*` methods.
#[derive(Debug, Clone, PartialEq)]
pub struct SabRequest {
    /// Which bound-scatterer material.
    pub material: SabMaterial,
    /// The temperature to produce the law at. Regeneration accepts any positive
    /// temperature; see the `rho(E)` caveat in the [module docs](self) before
    /// straying far from the deck's own range.
    pub temperature: Temperature,
    /// Which channels to emit. Defaults to [`ElasticChannel::Generate`].
    pub elastic: ElasticChannel,
    /// Where the law comes from. Defaults to [`SabSource::RegenerateFromDeck`].
    pub source: SabSource,
}

impl SabRequest {
    /// A request for `material` at `temperature`, regenerated from its deck
    /// with both channels — the defaults.
    pub fn new(material: SabMaterial, temperature: Temperature) -> Self {
        SabRequest {
            material,
            temperature,
            elastic: ElasticChannel::default(),
            source: SabSource::default(),
        }
    }

    /// Choose which channels to emit.
    pub fn with_elastic(mut self, elastic: ElasticChannel) -> Self {
        self.elastic = elastic;
        self
    }

    /// Take the law from an existing ENDF tape instead of regenerating it.
    pub fn with_tape(mut self, path: impl Into<PathBuf>) -> Self {
        self.source = SabSource::EndfTape(path.into());
        self
    }

    /// The temperature in kelvin, as the LEAPR kernels want it.
    pub fn temperature_k(&self) -> f64 {
        self.temperature.get::<kelvin>()
    }

    /// The validation standing of each channel for *this* request, as
    /// `(inelastic MT=4, elastic MT=2)`.
    ///
    /// Validation is a property of the **material**, not of the code path: as of
    /// 2026-08-13 only [`SabMaterial::CrystallineGraphite`] has been compared
    /// point-by-point against a reference tape (both channels). The porous
    /// reactor grades run the identical code with a different deck and are
    /// reported as unvalidated, because they are.
    ///
    /// A tape source reports both channels validated in the sense that matters:
    /// they are the published evaluation itself, not something this crate
    /// computed. See the [module docs](self) for the measured figures.
    pub fn validation(&self) -> (ChannelValidation, ChannelValidation) {
        if matches!(self.source, SabSource::EndfTape(_)) {
            return (
                ChannelValidation::ValidatedAgainstReferenceTape,
                ChannelValidation::ValidatedAgainstReferenceTape,
            );
        }
        // Validation is a property of the material, not of the code path: only
        // crystalline graphite has been compared against a reference tape.
        let status = if matches!(self.material, SabMaterial::CrystallineGraphite) {
            ChannelValidation::ValidatedAgainstReferenceTape
        } else {
            ChannelValidation::NotValidatedAgainstReferenceTape
        };
        let elastic = match self.elastic {
            ElasticChannel::Omit => ChannelValidation::NotEmitted,
            ElasticChannel::Generate => status,
        };
        (status, elastic)
    }
}

/// Everything that determined a generated artifact's bytes — the auditable
/// record written beside it in the cache.
#[derive(Debug, Clone, PartialEq)]
pub struct GenerationRecipe {
    /// Which material.
    pub material: SabMaterial,
    /// Where the deck text came from.
    pub deck_source: DeckSource,
    /// SHA-256 of the deck text, lowercase hex.
    pub deck_sha256: String,
    /// The evaluation vintage the deck declares, rendered as `EVAL-<MON><YY>`,
    /// or `"none"` when the deck carries no such field.
    pub eval_field: String,
    /// The constant set that vintage selected.
    pub constants: PhysicalConstants,
    /// Temperature \[K\].
    pub temperature_k: f64,
    /// Which channels were emitted.
    pub elastic: ElasticChannel,
}

impl GenerationRecipe {
    /// The inputs that **determine the output bytes**, in canonical text form.
    /// This is what [`Self::key`] hashes.
    ///
    /// Deliberately verbose and stable: every line here changes the result.
    /// Note what is *absent* — [`Self::deck_source`], the path the deck was read
    /// from. Two identical decks in different directories must share a cache
    /// entry, and [`Self::deck_sha256`] already identifies the content; keying
    /// on the path would silently double the cache whenever
    /// `OUTRAM_PARK_TSL_DIR` moved. The path is still recorded, in
    /// [`Self::canonical_text`].
    ///
    /// Changing this format changes every cache key — correct, but wasteful, so
    /// do it only when adding a genuine input.
    pub fn key_text(&self) -> String {
        format!(
            "njoy-outram-park-fork leapr S(alpha,beta) generation\n\
             generator_revision = {}\n\
             material           = {}\n\
             mat                = {}\n\
             deck_sha256        = {}\n\
             evaluation         = {}\n\
             constants          = {} (bk = {:.9e} eV/K, econ = {:.9e} /eV)\n\
             temperature_k      = {:.9e}\n\
             channels           = {}\n\
             coher_emax_ev      = {:.9e}\n",
            GENERATOR_REVISION,
            self.material.label(),
            self.material.mat(),
            self.deck_sha256,
            self.eval_field,
            self.constants.label(),
            self.constants.bk_ev_per_k(),
            self.constants.econ(),
            self.temperature_k,
            self.elastic.label(),
            COHERENT_ELASTIC_EMAX_EV,
        )
    }

    /// The full audit record written to the `.recipe` sidecar beside a cached
    /// artifact: [`Self::key_text`] plus the things worth recording that do not
    /// affect the bytes (where the deck was read from, and the resulting key).
    pub fn canonical_text(&self) -> String {
        format!(
            "{}deck_source        = {}\ncache_key          = {}\n",
            self.key_text(),
            self.deck_source,
            self.key()
        )
    }

    /// The first 16 hex characters of the SHA-256 of [`Self::key_text`] — the
    /// cache key.
    ///
    /// 64 bits is far more than enough to separate the handful of artifacts one
    /// project generates, and it keeps the file name readable.
    pub fn key(&self) -> String {
        let full = sha256_hex(self.key_text().as_bytes());
        full[..16].to_string()
    }

    /// The cache file name: material, temperature and key, with an `.endf`
    /// extension because the artifact really is an ENDF tape.
    pub fn cache_file_name(&self) -> String {
        format!(
            "{}-{:.4}K-{}.endf",
            self.material.label(),
            self.temperature_k,
            self.key()
        )
    }
}

/// Lowercase-hex SHA-256, for cache keys and provenance records.
fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    use std::fmt::Write;
    let digest = Sha256::digest(bytes);
    let mut s = String::with_capacity(64);
    for b in digest {
        let _ = write!(s, "{b:02x}");
    }
    s
}

/// Build the LEAPR output for one temperature and write it as an ENDF MF=7
/// tape, with no caching.
///
/// This is the missing half of NJOY's `leapr` driver for the single-scatterer,
/// continuous-spectrum case: it composes
/// [`FrequencyModel::start`] -> [`phonon_expansion`] ->
/// [`coher`](crate::leapr::coher::coher) -> [`endout`], including the `dwpix`
/// and `tempf` conversions `endout` expects (`leapr.f90:717, 3035`) which no
/// other code path performs.
///
/// The physical constants come from the deck's own declared vintage
/// ([`LeaprDeck::constants`]) and are threaded into **both** channels — `bk`
/// into `tev` for the inelastic law, and the whole set into `econ` for the
/// Bragg edge energies. That is what makes the result reproduce the published
/// tape instead of missing it by ~100x the storage precision (inelastic) or by
/// a uniform ~1e-6 offset (elastic). See [`crate::leapr::vintage`].
///
/// # Errors
///
/// - [`NjoyError::NotPorted`] if the deck uses a LEAPR feature the port does not
///   implement ([`LeaprDeck::unsupported_features`]). Silently generating
///   something plausible for an unsupported deck would be the worst outcome
///   available, so this refuses.
/// - [`NjoyError::EndfParse`] for a non-positive or non-finite temperature.
pub fn generate_tape(
    deck: &LeaprDeck,
    temperature: Temperature,
    elastic: ElasticChannel,
) -> Result<Tape, NjoyError> {
    let unsupported = deck.unsupported_features();
    if !unsupported.is_empty() {
        log::warn!(
            "LEAPR deck '{}' uses features this port does not implement: {:?}",
            deck.title,
            unsupported
        );
        return Err(NjoyError::NotPorted(
            "LEAPR deck uses features this port does not implement \
             (see LeaprDeck::unsupported_features)",
        ));
    }

    let temperature_k = temperature.get::<kelvin>();
    let input = deck.input_at_temperature(0, temperature_k)?;

    let freq = FrequencyModel::start(
        &input.continuous.rho,
        input.continuous.delta_ev,
        input.tev(),
        input.continuous.tbeta,
    );
    // The scattering law is built in the same three stages, in the same order,
    // as the Fortran temperature loop (leapr.f90:376-384):
    //
    //     call contin  ->  call trans (if twt > 0)  ->  call discre (if nd > 0)
    //
    // Each stage convolves its term into `ssm` AND advances the Debye-Waller
    // integral / effective temperature. Running only `contin` is correct for a
    // pure solid-type moderator (graphite: twt = 0, nd = 0) and badly wrong for
    // a molecular liquid — light water carries both a translational term and two
    // discrete oscillators (the H2O bend and stretch), which between them supply
    // most of its bound-atom zero-point motion. Omitting them left T_eff at
    // 482 K against the evaluation's 1194 K, and sigma_inel at +73 % (1 eV) to
    // +48 % (8 eV) above the published values, rising rather than relaxing onto
    // the free-atom limit. Bead op-ziux.
    let mut ssm = phonon_expansion(&input, &freq);

    // `dwpix`/`tempf` start as `contin` leaves them (leapr.f90:715-716) and are
    // then advanced in place by the later stages, exactly as the Fortran globals
    // are. `dwpix` is kept in raw LEAPR units until after the last stage.
    let mut dwpix = freq.f0;
    let mut tempf = freq.tbar * temperature_k;

    // `trans` (leapr.f90:844-1007, guard at 379). Updates `tempf` only.
    if input.continuous.twt > 0.0 {
        add_translation(&mut ssm, &input, &freq, &mut tempf);
    }

    // `discre` (leapr.f90:1320-1661, guard at 382). Updates both `dwpix` and
    // `tempf`; a no-op when the deck declares no oscillators.
    add_discrete_oscillators(&mut ssm, &input, &mut dwpix, &mut tempf);

    // `endout` wants the Debye-Waller integral already divided by awr*T*k_B
    // (leapr.f90:3035) and the effective temperature in kelvin, not as a ratio.
    let bk = input.constants.bk_ev_per_k();
    let dwpix = dwpix / (deck.awr * temperature_k * bk);

    // The Debye-Waller coefficient MF=7/MT=2 is written with. Normally the
    // deck's own (the principal scatterer's), but a compound Bragg channel
    // computed through the generalized path uses the *universal* coefficient —
    // see `compound_debye_waller`.
    let mut dwpix_elastic = dwpix;

    let elastic_output = match (elastic, deck.iel) {
        (ElasticChannel::Omit, _) => ElasticOutput::None,
        // `iel = 0` means "no built-in lattice", which is how every evaluation
        // produced with modified LEAPR reaches us — the crystal structure lived
        // in a separate input file that the distributed deck does not carry.
        // Consult the crystal catalogue before giving up on the channel.
        (ElasticChannel::Generate, ElasticOption::None) => {
            match GeneralCrystal::for_material(deck.mat, deck.za) {
                Some(crystal) => {
                    dwpix_elastic = compound_debye_waller(crystal, deck, temperature_k, dwpix)?;
                    ElasticOutput::Coherent(coher_general_with_constants(
                        &crystal.structure(),
                        deck.npr as usize,
                        COHERENT_ELASTIC_EMAX_EV,
                        input.constants,
                    ))
                }
                None => ElasticOutput::None,
            }
        }
        (ElasticChannel::Generate, ElasticOption::Incoherent) => {
            // `endout` can write the LTHR=2 Debye-Waller section, but the bound
            // cross section `sb` it needs is a LEAPR quantity no code path here
            // computes. Refusing beats inventing a plausible number.
            return Err(NjoyError::NotPorted(
                "incoherent-elastic (iel < 0) output needs the bound cross section \
                 `sb`, which this port does not compute — request ElasticChannel::Omit \
                 or supply the elastic channel from a tape",
            ));
        }
        (ElasticChannel::Generate, iel) => {
            let lattice = iel
                .coherent_lattice()
                .expect("every remaining ElasticOption variant maps to a coherent lattice");
            // The deck's own vintage, not the crate default: `econ` scales every
            // Bragg edge energy, so the constant set decides whether MT=2 is
            // 1e-6-close to the published tape or bit-exact against it.
            ElasticOutput::Coherent(coher_with_constants(
                lattice,
                deck.npr as usize,
                COHERENT_ELASTIC_EMAX_EV,
                input.constants,
            ))
        }
    };

    let out = LeaprOutput {
        mat: deck.mat,
        za: deck.za,
        awr: deck.awr,
        lat: if deck.lat { 1 } else { 0 },
        isym: deck.isabt,
        ilog: deck.ilog != 0,
        smin: deck.smin,
        alpha: deck.alpha.clone(),
        beta: deck.beta.clone(),
        temperatures_k: vec![temperature_k],
        dwpix: vec![dwpix_elastic],
        tempf: vec![tempf],
        ssm: vec![ssm],
        ssp: None,
        npr: deck.npr,
        spr: deck.spr,
        elastic: elastic_output,
        secondary: deck.secondary_scatterer(),
        constants: input.constants,
    };

    Ok(endout(&out))
}

/// The LEAPR Debye-Waller coefficient `W'(T)` \[1/eV\] for one deck at one
/// temperature — `dwpix` in `leapr.f90`, already divided by `awr * T * k_B`
/// (`leapr.f90:3035`) so it is in the form [`endout`] wants.
///
/// This is the same quantity [`generate_tape`] computes on its way to a tape;
/// it is factored out here because the **compound** coefficient a generalized
/// coherent-elastic section needs is a weighted average over several decks
/// (see [`compound_debye_waller`]), and computing it must not require
/// generating each of their tapes.
///
/// # Errors
/// [`NjoyError::NotPorted`] if the deck uses an unimplemented LEAPR feature, or
/// [`NjoyError::EndfParse`] for a bad temperature — the same conditions
/// [`generate_tape`] refuses on.
pub fn debye_waller_coefficient(deck: &LeaprDeck, temperature_k: f64) -> Result<f64, NjoyError> {
    let input = deck.input_at_temperature(0, temperature_k)?;
    let freq = FrequencyModel::start(
        &input.continuous.rho,
        input.continuous.delta_ev,
        input.tev(),
        input.continuous.tbeta,
    );
    let mut dwpix = freq.f0;
    if !input.oscillators.is_empty() {
        // Discrete oscillators advance `dwpix` inside `discre`, which also
        // convolves them into `S(alpha, beta)`; there is no cheaper way to get
        // the one without the other, so pay for the full expansion.
        let mut ssm = phonon_expansion(&input, &freq);
        let mut tempf = freq.tbar * temperature_k;
        add_discrete_oscillators(&mut ssm, &input, &mut dwpix, &mut tempf);
    }
    Ok(dwpix / (deck.awr * temperature_k * input.constants.bk_ev_per_k()))
}

/// The **universal (compound) Debye-Waller coefficient** `W'(T)` \[1/eV\] for a
/// catalogued crystal, under Zhu's cubic approximation.
///
/// Zhu (2014) Eq. (3.3): a compound's Bragg channel has one Debye-Waller
/// coefficient, not one per sublattice —
/// `W'_tot = sum_n (atomic fraction)_n * W'_n` — where each `W'_n` comes from
/// that atom type's own mass and partial phonon spectrum. Each LEAPR deck
/// carries exactly one such spectrum, so the compound coefficient is assembled
/// by running [`debye_waller_coefficient`] over the decks
/// [`GeneralCrystal::debye_waller_decks`] names.
///
/// `own_dwpix` is the coefficient already computed for `deck` itself; it is
/// reused rather than recomputed, so generating a two-sublattice compound costs
/// one extra Debye-Waller integral, not two.
///
/// **This is what makes MF=7/MT=2 identical for every material of the same
/// compound**, which is the behaviour the published ENDF/B-VIII.0 SiC
/// evaluations show (MAT 43 and MAT 44 carry byte-identical MT=2 sections).
///
/// # Errors
/// Propagates from [`debye_waller_coefficient`] for any partner deck. A partner
/// deck that is not embedded in this crate is an
/// [`NjoyError::NotPorted`] — silently falling back to the single-sublattice
/// coefficient would produce a subtly wrong, plausible-looking tape.
fn compound_debye_waller(
    crystal: GeneralCrystal,
    deck: &LeaprDeck,
    temperature_k: f64,
    own_dwpix: f64,
) -> Result<f64, NjoyError> {
    let mut total = 0.0;
    for &(material, fraction) in crystal.debye_waller_decks() {
        let w = if material.mat() == deck.mat {
            own_dwpix
        } else {
            let text = embedded_deck_text(material).ok_or(NjoyError::NotPorted(
                "generalized coherent elastic needs the partner sublattice's LEAPR deck for the \
                 compound Debye-Waller coefficient, and it is not embedded in this crate",
            ))?;
            debye_waller_coefficient(&LeaprDeck::parse(text)?, temperature_k)?
        };
        total += fraction * w;
    }
    Ok(total)
}

/// The in-process memo of parsed laws, keyed by [`GenerationRecipe::key`].
///
/// `Arc<Mf7>` because the law is read-only once built and is shared across a
/// simulation (workspace rule: `Arc<T>` for read-after-construction data). The
/// map lives for the life of the process; a handful of materials at a handful
/// of temperatures is on the order of a megabyte each, which is the point —
/// re-parsing that per query is what this avoids.
static MEMO: OnceLock<RwLock<HashMap<String, Arc<Mf7>>>> = OnceLock::new();

/// Get the memo, initialising it on first use.
fn memo() -> &'static RwLock<HashMap<String, Arc<Mf7>>> {
    MEMO.get_or_init(|| RwLock::new(HashMap::new()))
}

/// Resolve a thermal scattering law for `request` — **regenerating from the
/// LEAPR deck by default**, and reading a tape only when asked to.
///
/// Cached twice over (in process, then on disk); see the [module docs](self)
/// for the caching contract and its invalidation rule, and for what is and is
/// not validated in the result.
///
/// # Errors
///
/// - [`NjoyError::Download`] if the deck cannot be located — the message names
///   the paths tried and the environment variable to set.
/// - [`NjoyError::NotPorted`] if the deck needs an unported LEAPR feature.
/// - [`NjoyError::Io`] / [`NjoyError::EndfParse`] from reading or parsing.
pub fn thermal_scattering_law(request: &SabRequest) -> Result<Arc<Mf7>, NjoyError> {
    match &request.source {
        SabSource::EndfTape(path) => {
            let tape = Tape::read(std::fs::File::open(path)?)?;
            Ok(Arc::new(parse_mf7(&tape, request.material.mat())?))
        }
        SabSource::RegenerateFromDeck => regenerate_cached(request),
    }
}

/// The whole ENDF **tape** for a request, where
/// [`thermal_scattering_law`] gives just the parsed MF=7.
///
/// Same sources, caching, errors and validation standing as
/// [`thermal_scattering_law`] — this is the identical artifact, handed over one
/// step earlier. Use it when a consumer wants to drive several `from_tape`
/// constructors off one tape (the inelastic channel plus whichever elastic law
/// the evaluation carries) instead of re-reading the file per channel; that is
/// exactly what `outram-mc-libs`' `ThermalScattering::from_leapr` does.
///
/// # Errors
///
/// - [`NjoyError::Download`] if the deck cannot be located.
/// - [`NjoyError::NotPorted`] if the deck needs an unported LEAPR feature.
/// - [`NjoyError::Io`] / [`NjoyError::EndfParse`] from reading or parsing.
pub fn thermal_scattering_tape(request: &SabRequest) -> Result<Tape, NjoyError> {
    match &request.source {
        SabSource::EndfTape(path) => Ok(Tape::read(std::fs::File::open(path)?)?),
        SabSource::RegenerateFromDeck => {
            let (path, _key) = cached_tape_path(request)?;
            Ok(Tape::read(std::fs::File::open(&path)?)?)
        }
    }
}

/// Produce (or reuse) the on-disk cached tape for a regeneration request, and
/// return its path together with the recipe key the memo is stored under.
///
/// Split out of [`regenerate_cached`] so [`thermal_scattering_tape`] can share
/// the identical deck-location, recipe and cache logic — the two entry points
/// must never disagree about what artifact a request names.
fn cached_tape_path(request: &SabRequest) -> Result<(std::path::PathBuf, String), NjoyError> {
    let located = locate_deck(request.material)?;
    let deck = LeaprDeck::parse(&located.text)?;

    let recipe = GenerationRecipe {
        material: request.material,
        deck_source: located.source.clone(),
        deck_sha256: sha256_hex(located.text.as_bytes()),
        eval_field: deck
            .evaluation_date()
            .map(|d| d.to_string())
            .unwrap_or_else(|| "none".to_string()),
        constants: deck.constants(),
        temperature_k: request.temperature_k(),
        elastic: request.elastic,
    };
    let key = recipe.key();

    // On-disk cache, through the crate's one caching layer.
    let cache = EndfCache::new()?;
    let path = cache
        .dir()
        .join("leapr-generated")
        .join(recipe.cache_file_name());
    let path = cache.get_or_produce(path, || {
        let tape = generate_tape(&deck, request.temperature, request.elastic)?;
        let mut bytes: Vec<u8> = Vec::new();
        tape.write(&mut bytes)?;
        crate::acquire::validate_endf(&bytes, "generated LEAPR MF=7 tape")?;
        Ok(bytes)
    })?;

    // Provenance sidecar, best-effort: the full recipe in human-readable form,
    // so a cached artifact can be audited without re-deriving how it was made.
    let sidecar = path.with_extension("recipe");
    if !sidecar.exists() {
        let _ = std::fs::write(&sidecar, recipe.canonical_text());
    }

    Ok((path, key))
}

/// The regeneration path of [`thermal_scattering_law`]: memo, then disk cache,
/// then generate.
fn regenerate_cached(request: &SabRequest) -> Result<Arc<Mf7>, NjoyError> {
    // 1. In-process memo, keyed by the same recipe the disk cache uses. Keyed
    //    lookup needs the recipe, which `cached_tape_path` builds — but building
    //    it is cheap next to a parse, and doing it there keeps the two caches
    //    from ever disagreeing about a request's identity.
    let (path, key) = cached_tape_path(request)?;
    if let Ok(guard) = memo().read() {
        if let Some(hit) = guard.get(&key) {
            return Ok(Arc::clone(hit));
        }
    }

    // 2. Parse the cached tape.
    let tape = Tape::read(std::fs::File::open(&path)?)?;
    let law = Arc::new(parse_mf7(&tape, request.material.mat())?);

    if let Ok(mut guard) = memo().write() {
        guard.insert(key, Arc::clone(&law));
    }
    Ok(law)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(k: f64) -> Temperature {
        Temperature::new::<kelvin>(k)
    }

    /// The defaults are the ones the maintainer asked for: regenerate, both
    /// channels.
    ///
    /// **Methodology.** Build a bare [`SabRequest`] and assert the source and
    /// channel defaults, since "generation is the default" is a policy claim
    /// that should fail loudly if someone flips it.
    /// **Pass criterion:** `SabSource::RegenerateFromDeck` and
    /// `ElasticChannel::Generate`.
    ///
    /// **Result (2026-08-13):** holds.
    #[test]
    fn generation_is_the_default_source() {
        let r = SabRequest::new(SabMaterial::CrystallineGraphite, t(523.0));
        assert_eq!(r.source, SabSource::RegenerateFromDeck);
        assert_eq!(r.elastic, ElasticChannel::Generate);
        assert_eq!(r.temperature_k(), 523.0);

        let tape_req = r.clone().with_tape("/tmp/does-not-exist.endf");
        assert!(matches!(tape_req.source, SabSource::EndfTape(_)));
    }

    /// The per-channel validation status is reported honestly, and differs
    /// between the generated and tape sources.
    ///
    /// **Methodology.** The elastic channel is roughly 90 % of graphite's
    /// thermal cross section, so whether it has been measured must be reachable
    /// from the API rather than only from prose. Assert the standing of each
    /// (material, channel) pair.
    /// **Pass criterion:** crystalline graphite reports both channels validated
    /// (MT=4 by `tests/leapr_graphite_deck_parity.rs`, MT=2 by
    /// `tests/leapr_graphite_coherent_elastic_parity.rs`); the porous grades
    /// report neither; omitting the elastic channel reports it not emitted; a
    /// tape source reports both validated.
    ///
    /// **Result (2026-08-13):** holds. This is the tripwire that keeps the
    /// claim current — extend it, do not relax it, when a new material is
    /// measured.
    #[test]
    fn validation_status_is_reported_per_channel() {
        let gen = SabRequest::new(SabMaterial::CrystallineGraphite, t(296.0));
        assert_eq!(
            gen.validation(),
            (
                ChannelValidation::ValidatedAgainstReferenceTape,
                ChannelValidation::ValidatedAgainstReferenceTape
            ),
            "crystalline graphite: both channels measured against ENDF/B-VIII.0"
        );

        // The porous grades run the same code with a different deck and have
        // never been measured. Saying so is the whole point of this accessor.
        for m in [
            SabMaterial::ReactorGraphite10P,
            SabMaterial::ReactorGraphite30P,
        ] {
            assert_eq!(
                SabRequest::new(m, t(296.0)).validation(),
                (
                    ChannelValidation::NotValidatedAgainstReferenceTape,
                    ChannelValidation::NotValidatedAgainstReferenceTape
                ),
                "{}: no parity measurement has been taken",
                m.label()
            );
        }

        let no_elastic = gen.clone().with_elastic(ElasticChannel::Omit);
        assert_eq!(
            no_elastic.validation().1,
            ChannelValidation::NotEmitted,
            "omitting MT=2 must be visible in the status"
        );

        let from_tape = gen.clone().with_tape("/tmp/x.endf");
        assert_eq!(
            from_tape.validation(),
            (
                ChannelValidation::ValidatedAgainstReferenceTape,
                ChannelValidation::ValidatedAgainstReferenceTape
            )
        );
    }

    /// The cache key changes when — and only when — something that changes the
    /// output bytes changes.
    ///
    /// **Methodology.** Build a baseline recipe and perturb, one at a time: the
    /// temperature, the constant set, the channel choice, the deck hash, and the
    /// material. Each must produce a different key; an unchanged recipe must
    /// produce the same key twice (determinism). Also check the file name embeds
    /// the material and temperature so a cache directory is human-readable.
    /// **Pass criterion:** 5 distinct keys plus the baseline, all different;
    /// repeated hashing is stable.
    ///
    /// **Result (2026-08-13):** holds. This is the whole invalidation rule —
    /// there is no expiry or mtime check, so a key collision would be the only
    /// way to serve a stale artifact.
    #[test]
    fn cache_key_covers_every_input_that_changes_the_bytes() {
        let base = GenerationRecipe {
            material: SabMaterial::CrystallineGraphite,
            deck_source: DeckSource::Embedded,
            deck_sha256: "a".repeat(64),
            eval_field: "EVAL-SEP17".to_string(),
            constants: PhysicalConstants::Njoy2016Legacy,
            temperature_k: 296.0,
            elastic: ElasticChannel::Generate,
        };
        assert_eq!(base.key(), base.key(), "hashing must be deterministic");

        let mut variants = vec![base.key()];
        let mut push = |r: GenerationRecipe| variants.push(r.key());

        push(GenerationRecipe {
            temperature_k: 523.0,
            ..base.clone()
        });
        push(GenerationRecipe {
            constants: PhysicalConstants::Codata2018,
            ..base.clone()
        });
        push(GenerationRecipe {
            elastic: ElasticChannel::Omit,
            ..base.clone()
        });
        push(GenerationRecipe {
            deck_sha256: "b".repeat(64),
            ..base.clone()
        });
        push(GenerationRecipe {
            material: SabMaterial::ReactorGraphite30P,
            ..base.clone()
        });

        let mut uniq = variants.clone();
        uniq.sort();
        uniq.dedup();
        assert_eq!(
            uniq.len(),
            variants.len(),
            "every recipe input must move the cache key: {variants:?}"
        );

        // ...and the deck's *path* must NOT move it. The same deck read from two
        // directories has to share one cache entry, or moving OUTRAM_PARK_TSL_DIR
        // silently doubles the cache.
        let elsewhere = GenerationRecipe {
            deck_source: DeckSource::File(PathBuf::from("/somewhere/else/x.leapr")),
            ..base.clone()
        };
        assert_eq!(
            elsewhere.key(),
            base.key(),
            "deck_source must not be part of the cache key"
        );
        // It is still recorded in the audit sidecar, though.
        assert!(elsewhere
            .canonical_text()
            .contains("/somewhere/else/x.leapr"));

        let name = base.cache_file_name();
        assert!(name.starts_with("crystalline-graphite-296.0000K-"));
        assert!(name.ends_with(".endf"));
    }

    /// The canonical recipe text records everything an auditor needs, including
    /// the constant that makes the numbers reproducible.
    ///
    /// **Methodology.** Assert the presence of the fields the workspace
    /// data-provenance rule asks for: material, deck identity (hash), the
    /// evaluation's own vintage field, and the physical constant actually used.
    /// **Pass criterion:** each appears in the text.
    ///
    /// **Result (2026-08-13):** holds; the text is what is written to the
    /// `.recipe` sidecar beside each cached artifact.
    #[test]
    fn recipe_text_is_an_audit_record() {
        let r = GenerationRecipe {
            material: SabMaterial::CrystallineGraphite,
            deck_source: DeckSource::File(PathBuf::from("/x/tsl-crystalline-graphite.leapr")),
            deck_sha256: "c".repeat(64),
            eval_field: "EVAL-SEP17".to_string(),
            constants: PhysicalConstants::Njoy2016Legacy,
            temperature_k: 393.0,
            elastic: ElasticChannel::Generate,
        };
        let text = r.canonical_text();
        assert!(text.contains("crystalline-graphite"));
        assert!(text.contains("mat                = 30"));
        assert!(text.contains(&"c".repeat(64)));
        assert!(text.contains("EVAL-SEP17"));
        assert!(text.contains("njoy2016-legacy"));
        assert!(
            text.contains("8.617385000e-5"),
            "the bk actually used: {text}"
        );
        assert!(text.contains(&format!("generator_revision = {GENERATOR_REVISION}")));
    }
}
