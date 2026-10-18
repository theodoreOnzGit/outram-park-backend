//! **The processed nuclear data every Monte Carlo demo shares** (gh:#818):
//! the cache keys, and the one place a demo builds a nuclide or a thermal
//! law from a tape, reading the product back from the cache when it is
//! there. Included by `monte_carlo_web` (every rung loader, the htr10
//! whole-core pool), `delta_tracking_web` and, since 2026-10-16 (gh:#923),
//! `nuclear_data_web`: one store for every demo in a browser, Run k_eff or
//! a single-stage rung.
//!
//! **Stages (gh:#923).** Besides each nuclide's product, the store keeps
//! its slow first stage under its own key: RECONR at 0 K
//! ([`reconstruction_key`], `process::reconstruct`, which no temperature
//! changes). A product missing from the cache is made from it by
//! `process::cross_sections_at`, with the stage read from the cache when it
//! is there. Only the nuclear-data demo saves stages
//! (`DataStore::keeping_stages`); the Monte Carlo demos save products only,
//! because RECONR at 0 K would triple their storage (TRISO: 88 MB to 307 MB,
//! measured 2026-10-16). ~~... and PURR's tables ([`urr_key`]). A product
//! missing from the cache is made from them (`Nuclide::process_from_stages`),
//! from whichever stages are cached.~~ **CHANGED 2026-10-18 (gh:#932):** PURR
//! runs inside `process::cross_sections_at`, after HEATR, so a product is no
//! longer assembled from a PURR stage. PURR tables ([`urr_key`]) remain a
//! stage of the nuclear-data demo's PURR rung alone, which also reads the
//! tables inside a Monte Carlo product when its settings are [`demo_purr`].
//!
//! **What is cached is the expensive half only**, in the exact `f64`
//! encodings gh:#786 added for relaying products between workers:
//! [`ProcessedEvaluation::to_f64s`] (RECONR + BROADR, PURR's tables with
//! HEATR's heating column, and the 0 K elastic DBRC needs) and
//! [`ThermalScattering::to_f64s`] (THERMR or LEAPR). The cheap half,
//! `Nuclide::from_processed`, is always run, from the tape the demo
//! downloads anyway.
//!
//! **The same `process` path as the CLI (gh:#932, epic #925 E).** A product
//! is njoy's `process::reconstruct` then `process::cross_sections_at` at
//! [`demo_settings`] (NJOY's defaults: PURR 20 / 64 / 10 000 after HEATR,
//! the PENDF-rounded 0 K elastic), carried over by
//! `ProcessedEvaluation::from_cross_sections`: exactly route 4,
//! `Nuclide::process_evaluation`. So [`DataStore::nuclide`] is
//! `Nuclide::from_tape_with_speed` bit for bit, processed or cached
//! (`tests::a_cached_nuclide_is_the_processed_nuclide_bit_for_bit`,
//! `tests::the_store_is_route_4_and_process_bit_for_bit`). No processing
//! logic is written here.
//!
//! ~~**Not route 4 since gh:#931.** ... The demos keep the pre-#931 chain
//! (`process_evaluation_pre_931`: PURR 20 / 16 / 2000, no HEATR), whose
//! stages this store keeps separately, until gh:#932 moves them; the keys and
//! [`CODE`] are unchanged.~~ **CHANGED 2026-10-18 (gh:#932):** moved; [`CODE`]
//! bumped, and the key names `process`'s settings.
//!
//! **The key holds everything the product depends on**: the SHA-256 of the
//! (covariance-stripped) tape bytes, the material number, the temperature
//! and the tolerances to the bit, PURR's table settings, the law's label,
//! and [`CODE`], the processing-code version. [`CODE`] is bumped whenever the
//! processing or the encoding changes what a product holds:
//! `tests::the_code_version_pins_the_processing_output` fails, and says so,
//! when the processed output of a reference tape changes.
//!
//! The browser store is `dhoby_ghaut::web_demo::data_cache::idb`; a
//! [`DataStore`] is the synchronous side a builder sees: the caller offers it
//! the records for the tape at hand before a step and saves what it
//! processed after.

// Each includer uses a different part of it.
#![allow(dead_code)]

use dhoby_ghaut::web_demo::data_cache::{sha256_hex, Miss, Record, Source};
use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::process::{self, CrossSectionSettings, PurrSettings, Reconstruction};
use njoy_outram_park_fork::purr::UrrProbabilityTables;
use outram_mc_libs::material::nuclide::Nuclide;
use std::sync::Arc;
use outram_mc_libs::material::processed::ProcessedEvaluation;
use outram_mc_libs::material::speed::SpeedTier;
use outram_mc_libs::material::thermal::ThermalScattering;
use std::collections::HashMap;

/// **The processing-code version** every cached product carries. Bump it
/// (and the fingerprint in the test below) whenever RECONR, BROADR, PURR,
/// THERMR, LEAPR or an encoding changes what a product holds; a product made
/// under another version is not used, and the page says so.
///
/// ~~`nd-2026-10-15.1`~~ `nd-2026-10-18.1` since gh:#932: products are
/// `process`'s (PURR 20 / 64 / 10 000 after HEATR, PENDF-rounded 0 K
/// elastic over the whole range), not the pre-#931 chain's.
pub const CODE: &str = "nd-2026-10-18.1";

/// Version of the key layout itself.
const KEYS: &str = "nd1";

/// **The `process` settings every demo nuclide is made with** (gh:#932):
/// NJOY's defaults (`CrossSectionSettings::default()`: PURR 20 bands, 64
/// ladders, 10 000 samples after HEATR, the 0 K elastic) at RECONR tolerance
/// `tolerance` and BROADR thinning tolerance `errthn`: exactly what
/// `Nuclide::process_evaluation(tape, mat, T, tolerance, errthn)` uses.
pub fn demo_settings(tolerance: f64, errthn: f64) -> CrossSectionSettings {
    CrossSectionSettings::default().with_reconr_tolerance(tolerance).with_broadr_tolerance(errthn)
}

/// What a product depends on in its settings besides the tolerances, as it
/// goes in the key: PURR's bands, ladders and samples (or `none`), HEATR and
/// the 0 K elastic. ~~`purr=20,16,2000`~~, the pre-#931 chain's (gh:#932).
fn settings_tag(s: &CrossSectionSettings) -> String {
    let purr = s.purr.map_or_else(|| "none".to_string(), |p| format!("{},{},{}", p.bands, p.ladders, p.samples));
    format!("purr={purr}|heatr={}|e0k={}", u8::from(s.heatr), u8::from(s.elastic_0k))
}

/// PURR's `(bands, ladders, samples)` in [`demo_settings`] (20, 64, 10 000).
/// A Monte Carlo product holds tables at these; the nuclear-data demo's PURR
/// rung reads them from it when asked for the same settings.
pub fn demo_purr() -> (usize, usize, usize) {
    let p = PurrSettings::default();
    (p.bands, p.ladders, p.samples)
}

/// A float in a key: readable, and exact (its bits).
fn exact(x: f64) -> String {
    format!("{x}:{:016x}", x.to_bits())
}

/// The prefix of every key made from one tape (`nd1|<sha256>|`): the browser
/// fetches all of a tape's products with one range query.
pub fn tape_prefix(tape_sha: &str) -> String {
    format!("{KEYS}|{tape_sha}|")
}

/// The key of a tape's product at [`demo_settings`] (RECONR + BROADR +
/// HEATR + PURR + the 0 K elastic).
pub fn evaluation_key(tape_sha: &str, mat: i32, temp_k: f64, tolerance: f64, errthn: f64) -> String {
    format!(
        "{}pe|mat={mat}|T={}|tol={}|errthn={}|{}",
        tape_prefix(tape_sha),
        exact(temp_k),
        exact(tolerance),
        exact(errthn),
        settings_tag(&demo_settings(tolerance, errthn))
    )
}

/// The key of a thermal law: from a tape (`source` = its SHA-256) or from
/// LEAPR (`source` = `leapr-<material>`).
pub fn law_key(source: &str, mat: i32, temp_k: f64, label: &str) -> String {
    format!("{}tsl|mat={mat}|T={}|label={label}", tape_prefix(source), exact(temp_k))
}

/// The prefix of a tape's products at `temp_k`, any tolerance (gh:#923):
/// each carries PURR's tables at [`demo_purr`] (~~`URR_TABLE_SETTINGS`~~,
/// gh:#932), which the nuclear-data demo's PURR rung reads from it.
pub fn evaluation_prefix(tape_sha: &str, mat: i32, temp_k: f64) -> String {
    format!("{}pe|mat={mat}|T={}|", tape_prefix(tape_sha), exact(temp_k))
}

/// The key of a tape's RECONR product at 0 K (gh:#923): the stage every
/// temperature of a nuclide starts from, and what the nuclear-data demo's
/// RECONR rung shows. Ends in `|`, so a prefix query for it finds it alone.
pub fn reconstruction_key(tape_sha: &str, mat: i32, tolerance: f64) -> String {
    format!("{}rc0|mat={mat}|tol={}|", tape_prefix(tape_sha), exact(tolerance))
}

/// The key of a tape's PURR tables at `temp_k` with `(nbin, nladr, nsamp)`
/// (gh:#923). Ends in `|`, as [`reconstruction_key`].
pub fn urr_key(tape_sha: &str, mat: i32, temp_k: f64, settings: (usize, usize, usize)) -> String {
    let (nbin, nladr, nsamp) = settings;
    format!("{}urr|mat={mat}|T={}|purr={nbin},{nladr},{nsamp}|", tape_prefix(tape_sha), exact(temp_k))
}

/// The synchronous side of the cache that a data builder sees.
///
/// - [`DataStore::off`]: no cache (tests, and the native tools that compare
///   against fresh processing);
/// - [`DataStore::memory`]: natively, the engine remembers what it
///   processed for its lifetime (a rung switched away from and back is not
///   processed again);
/// - [`DataStore::browser`]: in a Web Worker, the caller offers the records
///   it read from IndexedDB for the tape at hand ([`DataStore::offer`]) and
///   saves what was processed ([`DataStore::take_fresh`]).
pub struct DataStore {
    /// Records may be offered and are made.
    enabled: bool,
    /// Keep made records for later lookups (native memory).
    remember: bool,
    /// Save the stages (RECONR at 0 K, PURR) as records of their own
    /// (gh:#923). Off for the Monte Carlo demos, which save products only:
    /// RECONR at 0 K is far larger than the broadened product (the TRISO
    /// rung's cache: 88 MB of products, 307 MB with its stages, measured
    /// 2026-10-16), so keeping it would triple their storage. On for the
    /// nuclear-data demo, whose rungs show the stages. Stages are read
    /// whichever way it is set.
    keep_stages: bool,
    /// The one RECONR tolerance whose stage is saved when `keep_stages` is
    /// set (gh:#923, maintainer 2026-10-17: keep only 0.001). The RECONR
    /// rung's coarser sweep steps are processed and shown but not saved:
    /// all six took 126 MB, 0.001 alone 50.3 MB (measured 2026-10-16), and
    /// only 0.001 is the tolerance every other demo builds on.
    kept_tolerance: f64,
    entries: HashMap<String, Record>,
    fresh: Vec<Record>,
    /// Per product used, in order: its label and where it came from.
    pub sources: Vec<(String, Source)>,
    /// Per stage used (RECONR at 0 K, PURR; gh:#923), in order: its label
    /// and where it came from. A product made from its stages lists them
    /// here, and itself in `sources`.
    pub stages: Vec<(String, Source)>,
    /// What the page must show (misses with a reason, storage notes).
    pub notes: Vec<String>,
    /// The tape the next products come from: its bytes' address and length,
    /// and its SHA-256.
    tape: Option<(usize, usize, String)>,
    /// The clock for a record's date (ms since 1970).
    now_ms: fn() -> f64,
}

fn wall_ms() -> f64 {
    #[cfg(target_arch = "wasm32")]
    {
        js_sys::Date::now()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0.0, |d| d.as_secs_f64() * 1000.0)
    }
}

impl Default for DataStore {
    /// What an engine holds: [`DataStore::memory`] natively,
    /// [`DataStore::browser`] in a Web Worker.
    fn default() -> Self {
        if cfg!(target_arch = "wasm32") {
            Self::browser()
        } else {
            Self::memory()
        }
    }
}

impl DataStore {
    /// Where each product of the last load came from, and what to show,
    /// taken (the next load starts a fresh list).
    pub fn take_report(&mut self) -> (Vec<(String, Source)>, Vec<String>) {
        self.stages.clear();
        (std::mem::take(&mut self.sources), std::mem::take(&mut self.notes))
    }

    /// Where each stage of the last request came from (gh:#923), taken.
    pub fn take_stages(&mut self) -> Vec<(String, Source)> {
        std::mem::take(&mut self.stages)
    }

    fn with(enabled: bool, remember: bool) -> Self {
        Self {
            enabled,
            remember,
            keep_stages: false,
            kept_tolerance: f64::NAN,
            entries: HashMap::new(),
            fresh: Vec::new(),
            sources: Vec::new(),
            stages: Vec::new(),
            notes: Vec::new(),
            tape: None,
            now_ms: wall_ms,
        }
    }
    /// No cache: every product is processed, nothing is kept.
    pub fn off() -> Self {
        Self::with(false, false)
    }
    /// Natively: remember every product for the store's life.
    pub fn memory() -> Self {
        Self::with(true, true)
    }
    /// In the browser: products offered from IndexedDB, made ones handed
    /// back to be saved.
    pub fn browser() -> Self {
        Self::with(true, false)
    }

    /// This store saves stages as records of their own (the nuclear-data
    /// demo; see `keep_stages`), RECONR's only at `reconstruction_tolerance`
    /// (see `kept_tolerance`).
    pub fn keeping_stages(mut self, reconstruction_tolerance: f64) -> Self {
        self.keep_stages = true;
        self.kept_tolerance = reconstruction_tolerance;
        self
    }

    /// Whether this store reads and makes records.
    pub fn enabled(&self) -> bool {
        self.enabled
    }

    /// What this store holds in memory (natively, the engine's cache).
    pub fn summary(&self) -> dhoby_ghaut::web_demo::data_cache::Summary {
        dhoby_ghaut::web_demo::data_cache::Summary::from_rows(
            self.entries.values().map(|r| (r.code.clone(), r.bytes() as f64)),
        )
    }

    /// Forget every product held in memory.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.fresh.clear();
    }

    /// The products of the next step come from this tape: hash it. Returns
    /// its SHA-256. The loaders call this once per tape (they need the hash
    /// to query the browser's store); the builder's step then passes the
    /// same slice, and the hash is reused rather than computed again.
    pub fn begin_tape(&mut self, bytes: &[u8]) -> String {
        let sha = sha256_hex(bytes);
        self.tape = Some((bytes.as_ptr() as usize, bytes.len(), sha.clone()));
        sha
    }

    /// The SHA-256 of `bytes`: the memo when it is for this very slice (same
    /// address and length, so the same live bytes), else computed.
    pub fn tape_sha(&mut self, bytes: &[u8]) -> String {
        self.sha_of(bytes)
    }

    fn sha_of(&mut self, bytes: &[u8]) -> String {
        match &self.tape {
            Some((at, n, sha)) if *at == bytes.as_ptr() as usize && *n == bytes.len() => sha.clone(),
            _ => self.begin_tape(bytes),
        }
    }

    /// Records read from the browser's store for the tape at hand.
    pub fn offer(&mut self, records: Vec<Record>) {
        for r in records {
            self.entries.insert(r.key.clone(), r);
        }
    }

    /// The records processed since the last call, to be saved.
    pub fn take_fresh(&mut self) -> Vec<Record> {
        std::mem::take(&mut self.fresh)
    }

    /// Forget what was offered (the browser store drops them after a step,
    /// so a product is not held twice in a worker's memory).
    pub fn forget_offered(&mut self) {
        if !self.remember {
            self.entries.clear();
        }
    }

    /// A note the page shows (and the console).
    pub fn note(&mut self, line: String) {
        log::warn!("nuclear-data cache: {line}");
        self.notes.push(line);
    }

    /// The cached words under `key`, if usable; a miss with a reason is
    /// noted.
    fn lookup(&mut self, key: &str, label: &str) -> Option<(Vec<f64>, Source)> {
        if !self.enabled {
            return None;
        }
        let r = self.entries.get(key)?;
        match r.open(key, CODE) {
            Ok(w) => Some((
                w.to_vec(),
                Source::Cached {
                    created_ms: r.created_ms,
                    code: r.code.clone(),
                },
            )),
            Err(m) => {
                self.entries.remove(key);
                if let Some(n) = m.note(label) {
                    self.note(n);
                }
                None
            }
        }
    }

    /// Keep a product just processed.
    fn made(&mut self, key: String, label: &str, words: Vec<f64>) {
        if !self.enabled {
            return;
        }
        let r = Record::seal(key.clone(), label.to_string(), CODE.to_string(), (self.now_ms)(), words);
        if self.remember {
            self.entries.insert(key, r.clone());
        }
        self.fresh.push(r);
    }

    /// The product of `tape` (whose bytes are `bytes`) at [`demo_settings`],
    /// from the cache or processed now: `Nuclide::process_evaluation(tape,
    /// mat, temp_k, tolerance, errthn)` bit for bit (gh:#932), with RECONR at
    /// 0 K read from the cache when it is there.
    ///
    /// # Errors
    ///
    /// Processing failed, or `mat` is not the tape's one material.
    #[allow(clippy::too_many_arguments)]
    pub fn evaluation(
        &mut self,
        bytes: &[u8],
        tape: &Tape,
        mat: i32,
        temp_k: f64,
        tolerance: f64,
        errthn: f64,
        label: &str,
    ) -> Result<ProcessedEvaluation, String> {
        if !self.enabled {
            self.sources.push((label.to_string(), Source::Processed));
            return Nuclide::process_evaluation(tape, mat, temp_k, tolerance, errthn)
                .map_err(|e| format!("{label}: {e}"));
        }
        let key = evaluation_key(&self.sha_of(bytes), mat, temp_k, tolerance, errthn);
        if let Some((w, src)) = self.lookup(&key, label) {
            match ProcessedEvaluation::from_f64s(&w) {
                Ok(p) if p.temp_k.to_bits() == temp_k.to_bits() => {
                    self.sources.push((label.to_string(), src));
                    return Ok(p);
                }
                Ok(p) => self.note(
                    Miss::Unusable(format!("processed at {} K, not {temp_k} K", p.temp_k))
                        .note(label)
                        .unwrap_or_default(),
                ),
                Err(e) => self.note(Miss::Unusable(e.to_string()).note(label).unwrap_or_default()),
            }
        }
        // A miss: `process::reconstruct` (from the cache when it is there:
        // another temperature, another demo) then `process::cross_sections_at`,
        // which is `Nuclide::process_evaluation` with RECONR kept (gh:#932).
        let sha = self.sha_of(bytes);
        let t0 = (self.now_ms)();
        let first = self.stages.len();
        let settings = demo_settings(tolerance, errthn);
        let recon0 = self.reconstruction(&sha, tape, mat, tolerance, label)?;
        let t = uom::si::f64::ThermodynamicTemperature::new::<uom::si::thermodynamic_temperature::kelvin>(temp_k);
        let xs = process::cross_sections_at(&recon0, t, &settings).map_err(|e| format!("{label}: {e}"))?;
        let p = ProcessedEvaluation::from_cross_sections(xs);
        let read: Vec<&str> = self.stages[first..]
            .iter()
            .filter(|(_, s)| matches!(s, Source::Cached { .. }))
            .map(|_| "RECONR")
            .collect();
        log::info!(
            "nuclear-data cache: {label} made in {:.1} s{}",
            ((self.now_ms)() - t0) / 1000.0,
            if read.is_empty() { String::new() } else { format!(" ({} read from this browser's cache)", read.join(" and ")) }
        );
        self.made(key, label, p.to_f64s());
        self.sources.push((label.to_string(), Source::Processed));
        Ok(p)
    }

    /// RECONR at 0 K of the tape whose SHA-256 is `tape_sha`
    /// (`process::reconstruct` at RECONR tolerance `tolerance`; ~~`Nuclide::reconstruct_0k`~~,
    /// the same RECONR call, gh:#932), from the cache or processed now
    /// (gh:#923). Stored in the [`ProcessedEvaluation`] encoding, with no 0 K
    /// elastic, no URR tables and `temp_k` 0; a cached one is rebuilt as the
    /// `Reconstruction` `process::reconstruct` returns (the tape, its MAT,
    /// RECONR's result to the bit, the tolerance), so
    /// `process::cross_sections_at` gives the same product either way.
    ///
    /// # Errors
    ///
    /// RECONR failed, or `mat` is not the tape's one material.
    pub fn reconstruction(&mut self, tape_sha: &str, tape: &Tape, mat: i32, tolerance: f64, label: &str) -> Result<Reconstruction, String> {
        let on_tape = process::single_material(tape).map_err(|e| format!("{label}: {e}"))?;
        if on_tape != mat {
            return Err(format!("{label}: asked for MAT {mat}, but the tape holds MAT {on_tape}"));
        }
        let key = reconstruction_key(tape_sha, mat, tolerance);
        let what = format!("{label} (RECONR)");
        if let Some((w, src)) = self.lookup(&key, &what) {
            match ProcessedEvaluation::from_f64s(&w) {
                Ok(p) => {
                    self.stages.push((what, src));
                    return Ok(Reconstruction { tape: Arc::new(tape.clone()), mat, reconr: p.recon, reconr_tolerance: tolerance });
                }
                Err(e) => self.note(Miss::Unusable(e.to_string()).note(&what).unwrap_or_default()),
            }
        }
        let recon = process::reconstruct(tape, &CrossSectionSettings::default().with_reconr_tolerance(tolerance))
            .map_err(|e| format!("{label}: {e}"))?;
        if self.enabled && self.keep_stages && tolerance.to_bits() == self.kept_tolerance.to_bits() {
            let words =
                ProcessedEvaluation { recon: recon.reconr.clone(), elastic_0k: Vec::new(), urr: None, temp_k: 0.0 }.to_f64s();
            self.made(key, &what, words);
        }
        self.stages.push((what, Source::Processed));
        Ok(recon)
    }

    /// PURR's tables at `temp_k` with `settings` `(nbin, nladr, nsamp)` of the
    /// tape whose SHA-256 is `tape_sha`, from the cache or processed now
    /// (gh:#923): `UrrProbabilityTables::from_endf`. Only the nuclear-data
    /// demo's PURR rung asks for them since gh:#932 (a product's PURR runs
    /// inside `process::cross_sections_at`). At [`demo_purr`] they are read
    /// from a Monte Carlo product at `temp_k` when one is held; those carry
    /// HEATR's heating column, which `from_endf` leaves zero, and every other
    /// word is the same (PURR's ladders do not depend on HEATR; gh:#931
    /// measured 520 heating words as the only change on U-234).
    /// Stored as `[0]` for none, else `1` then `UrrProbabilityTables::to_f64s`.
    ///
    /// # Errors
    ///
    /// PURR failed.
    pub fn urr_tables(
        &mut self,
        tape_sha: &str,
        tape: &Tape,
        mat: i32,
        temp_k: f64,
        settings: (usize, usize, usize),
        label: &str,
    ) -> Result<Option<UrrProbabilityTables>, String> {
        let key = urr_key(tape_sha, mat, temp_k, settings);
        let what = format!("{label} (PURR)");
        if let Some((w, src)) = self.lookup(&key, &what) {
            let decoded = match w.split_first() {
                Some((&0.0, [])) => Ok(None),
                Some((&1.0, rest)) => UrrProbabilityTables::from_f64s(rest).map(Some).map_err(|e| e.to_string()),
                _ => Err("not a PURR record".to_string()),
            };
            match decoded {
                Ok(t) => {
                    self.stages.push((what, src));
                    return Ok(t);
                }
                Err(e) => self.note(Miss::Unusable(e).note(&what).unwrap_or_default()),
            }
        }
        // A product at this temperature holds these very tables.
        if settings == demo_purr() && self.enabled {
            let prefix = evaluation_prefix(tape_sha, mat, temp_k);
            let mut keys: Vec<String> = self.entries.keys().filter(|k| k.starts_with(&prefix)).cloned().collect();
            keys.sort();
            for k in keys {
                if let Some((w, src)) = self.lookup(&k, &what) {
                    if let Ok(p) = ProcessedEvaluation::from_f64s(&w) {
                        if p.temp_k.to_bits() == temp_k.to_bits() {
                            self.stages.push((what, src));
                            return Ok(p.urr);
                        }
                    }
                }
            }
        }
        let t = UrrProbabilityTables::from_endf(tape, mat, temp_k, settings.0, settings.1, settings.2)
            .map_err(|e| format!("{label}: {e}"))?;
        if self.enabled && self.keep_stages {
            let words = match &t {
                None => vec![0.0],
                Some(t) => std::iter::once(1.0).chain(t.to_f64s()).collect(),
            };
            self.made(key, &what, words);
        }
        self.stages.push((what, Source::Processed));
        Ok(t)
    }

    /// A nuclide at a [`SpeedTier`]: `Nuclide::from_tape_with_speed` (route 4,
    /// njoy's `process`, again since gh:#932; ~~`pre931_nuclide`~~), with its
    /// expensive half from the cache when it is there.
    ///
    /// # Errors
    ///
    /// Processing or assembly failed.
    #[allow(clippy::too_many_arguments)]
    pub fn nuclide(
        &mut self,
        bytes: &[u8],
        tape: &Tape,
        mat: i32,
        name: &str,
        temp_k: f64,
        speed: SpeedTier,
        label: &str,
    ) -> Result<Nuclide, String> {
        let tol = speed.data_tolerance();
        let p = self.evaluation(bytes, tape, mat, temp_k, tol, tol, label)?;
        Ok(Nuclide::from_processed(tape, mat, name, temp_k, p)
            .map_err(|e| format!("{label}: {e}"))?
            .with_speed(speed))
    }

    /// A thermal law from its tape (`ThermalScattering::from_tape`), from
    /// the cache when it is there.
    ///
    /// # Errors
    ///
    /// THERMR failed.
    pub fn thermal(
        &mut self,
        bytes: &[u8],
        tape: &Tape,
        mat: i32,
        temp_k: f64,
        name: &str,
        label: &str,
    ) -> Result<ThermalScattering, String> {
        let source = if self.enabled { self.sha_of(bytes) } else { String::new() };
        self.law(&law_key(&source, mat, temp_k, name), label, || {
            ThermalScattering::from_tape(tape, mat, temp_k, name).map_err(|e| format!("{label}: {e}"))
        })
    }

    /// A thermal law under `key`, from the cache or `make`.
    ///
    /// # Errors
    ///
    /// `make` failed.
    pub fn law(
        &mut self,
        key: &str,
        label: &str,
        make: impl FnOnce() -> Result<ThermalScattering, String>,
    ) -> Result<ThermalScattering, String> {
        if let Some((w, src)) = self.lookup(key, label) {
            match ThermalScattering::from_f64s(&w) {
                Ok(t) => {
                    self.sources.push((label.to_string(), src));
                    return Ok(t);
                }
                Err(e) => self.note(Miss::Unusable(e.to_string()).note(label).unwrap_or_default()),
            }
        }
        let t = make()?;
        self.made(key.to_string(), label, t.to_f64s());
        self.sources.push((label.to_string(), Source::Processed));
        Ok(t)
    }
}

/// **The browser side of a load** (wasm only): read every cached product of
/// a tape into `store` before its step, and save what the step processed
/// after. Every failure becomes a note in `store`; none stops the load.
/// Platform code: no native test reaches it (the rule's exception); it is
/// checked in headless Chromium (`verification_and_validation/
/// nuclear_data_cache_web/`).
#[cfg(target_arch = "wasm32")]
pub mod web {
    use super::{tape_prefix, DataStore};
    use dhoby_ghaut::web_demo::data_cache::{idb, Miss, PutError};

    /// Whether this session may still write (a quota error stops writes; an
    /// unavailable store stops reads and writes).
    #[derive(Clone, Copy, Debug, Default, PartialEq)]
    pub enum Health {
        #[default]
        Ok,
        Full,
        Unavailable,
    }

    /// Offer the cached products of the tape `bytes` (hashing it).
    pub async fn before_step(store: &mut DataStore, bytes: &[u8], health: &mut Health) {
        let sha = store.begin_tape(bytes);
        before_prefix(store, &tape_prefix(&sha), health).await;
    }

    /// Offer every cached product whose key starts with `prefix` (a LEAPR
    /// law has no tape; its keys start `nd1|leapr-<label>|`).
    pub async fn before_prefix(store: &mut DataStore, prefix: &str, health: &mut Health) {
        if *health == Health::Unavailable || !store.enabled() {
            return;
        }
        match idb::get_prefix(prefix).await {
            Ok(rows) => {
                let mut good = Vec::new();
                for r in rows {
                    match r {
                        Ok(r) => good.push(r),
                        Err(e) => store.note(format!("a cache entry was skipped: {e}")),
                    }
                }
                store.offer(good);
            }
            Err(e) => {
                *health = Health::Unavailable;
                if let Some(n) = Miss::Unavailable(e).note("") {
                    store.note(n);
                }
            }
        }
    }

    /// Save what the step processed; drop what was offered.
    pub async fn after_step(store: &mut DataStore, health: &mut Health) {
        let fresh = store.take_fresh();
        store.forget_offered();
        if *health != Health::Ok {
            return;
        }
        for r in fresh {
            match idb::put(&r).await {
                Ok(()) => log::info!("nuclear-data cache: saved {} ({:.1} MB)", r.label, r.bytes() as f64 / 1e6),
                Err(e) => {
                    store.note(e.note(&r.label));
                    *health = match e {
                        PutError::Quota(_) => Health::Full,
                        PutError::Other(_) => Health::Unavailable,
                    };
                    return;
                }
            }
        }
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use njoy_outram_park_fork::reference_data::reference_endf;
    use outram_mc_libs::material::nuclide::MicroXS;

    fn tape_bytes(file: &str) -> Option<Vec<u8>> {
        let path = reference_endf(file)?;
        let raw = std::fs::read(path).expect("read");
        Some(crate::tapes::strip_covariances(&raw))
    }

    fn read(bytes: &[u8]) -> (Tape, i32) {
        let t = Tape::read(std::io::Cursor::new(bytes)).expect("tape");
        let mat = *t.materials().first().expect("material");
        (t, mat)
    }

    fn bits(x: &MicroXS) -> [u64; 9] {
        [
            x.total.to_bits(),
            x.elastic.to_bits(),
            x.fission.to_bits(),
            x.absorption.to_bits(),
            x.inelastic.to_bits(),
            x.n2n.to_bits(),
            x.n3n.to_bits(),
            x.mt5.to_bits(),
            x.nu_fission.to_bits(),
        ]
    }

    /// Every compared value of two nuclides, as bits: cross sections at
    /// 1300+ energies, URR band samples, the speed tier, and draws of the
    /// fission energy.
    fn fingerprint(n: &Nuclide, temp_k: f64) -> Vec<u64> {
        let mut v = Vec::new();
        let mut es: Vec<f64> = (0..=1200)
            .map(|i| 1.0e-5 * (2.0e7_f64 / 1.0e-5).powf(f64::from(i) / 1200.0))
            .collect();
        if let Some((lo, hi)) = n.urr_range_ev() {
            es.extend((0..=100).map(|i| lo + (hi - lo) * f64::from(i) / 100.0));
        }
        for &e in &es {
            v.extend(bits(&n.xs_at_energy(e, temp_k)));
            v.push(n.total_upper_bound(e, temp_k).to_bits());
            for xi in [0.05, 0.5, 0.95] {
                v.extend(format!("{:?}", n.sample_urr(e, xi)).bytes().map(u64::from));
            }
        }
        let mut s = 17u64;
        for &e in &[0.0253, 1.0e3, 1.0e6] {
            for _ in 0..32 {
                v.push(n.sample_fission_energy(e, &mut s).to_bits());
            }
        }
        v.push(u64::from(n.has_dbrc()));
        v.push(u64::from(n.has_urr_probability_tables()));
        v.push(format!("{:?}", n.speed()).len() as u64);
        v
    }

    /// **A cached nuclide is the processed nuclide, bit for bit** (gh:#818).
    ///
    /// Methodology: U-234 (fissile, with an unresolved range, so PURR tables
    /// go through the cache) at 300.15 K, at both tiers the demos use
    /// (`Fast`, tolerance 1e-3; `VeryFast`, 1e-2), three ways:
    /// `Nuclide::from_tape_with_speed` (the route every demo took before the
    /// cache); a [`DataStore`] processing it (and making a record); and a
    /// second store offered that record, as the browser does after reading
    /// IndexedDB. Pass: every compared value identical (no tolerance), the
    /// second store reports the product as cached and processes nothing.
    /// Then a corrupt record, a record of another code version, and a record
    /// whose words are not a product are each refused with a note, and the
    /// nuclide is processed again, still identical.
    ///
    /// Results, 2026-10-09 (release, i9-13900K): identical at both tiers,
    /// 106 395 compared values each; the record is 378 436 words at `Fast`
    /// and 165 470 at `VeryFast`; all three refused records were noted and
    /// reprocessed to the same nuclide.
    ///
    /// Re-run 2026-10-18 on the `process` path (gh:#932; the direct nuclide is
    /// again `from_tape_with_speed`, route 4, where gh:#931 had compared
    /// against the pre-#931 chain): identical at both tiers, 106 654
    /// compared values each; the record is 379 422 words at `Fast` and
    /// 166 454 at `VeryFast` (larger: the 0 K elastic is kept over the whole
    /// range); the three refused records as before.
    #[test]
    fn a_cached_nuclide_is_the_processed_nuclide_bit_for_bit() {
        let Some(bytes) = tape_bytes("n-092_U_234-ENDF8.0.endf") else {
            eprintln!("SKIP: U-234 not in reference-data/endf/");
            return;
        };
        let (tape, mat) = read(&bytes);
        let temp = 300.15;
        for speed in [SpeedTier::Fast, SpeedTier::VeryFast] {
            let direct = Nuclide::from_tape_with_speed(&tape, mat, "U234", temp, speed).expect("direct");
            let want = fingerprint(&direct, temp);
            let mut first = DataStore::browser();
            first.begin_tape(&bytes);
            let made = first.nuclide(&bytes, &tape, mat, "U234", temp, speed, "U-234").expect("made");
            assert_eq!(fingerprint(&made, temp), want, "{speed:?}: processed through the store");
            assert_eq!(first.sources[0].1, Source::Processed);
            // The product alone: a Monte Carlo store keeps no stages (gh:#923).
            let records = first.take_fresh();
            assert_eq!(records.len(), 1);
            assert!(records[0].key.starts_with(&tape_prefix(&sha256_hex(&bytes))));
            // As IndexedDB hands it back.
            let mut second = DataStore::browser();
            second.begin_tape(&bytes);
            second.offer(records.clone());
            let cached = second.nuclide(&bytes, &tape, mat, "U234", temp, speed, "U-234").expect("cached");
            assert_eq!(fingerprint(&cached, temp), want, "{speed:?}: from the cache");
            assert!(matches!(second.sources[0].1, Source::Cached { .. }));
            assert!(second.take_fresh().is_empty(), "nothing processed on a hit");
            assert!(second.notes.is_empty());
            eprintln!("U-234 {speed:?}: {} values identical; product record {} words", want.len(), records[0].words.len());
            // Refused records: corrupt, another code, not a product.
            let mut corrupt = records[0].clone();
            corrupt.words[10] += 1.0;
            let mut other = records[0].clone();
            other.code = "nd-0".into();
            let junk = Record::seal(records[0].key.clone(), "U-234".into(), CODE.into(), 0.0, vec![1.0, 2.0]);
            for (bad, why) in [(corrupt, "corrupt"), (other, "not used"), (junk, "could not be read")] {
                let mut s = DataStore::browser();
                s.begin_tape(&bytes);
                s.offer(vec![bad]);
                let n = s.nuclide(&bytes, &tape, mat, "U234", temp, speed, "U-234").expect("again");
                assert_eq!(fingerprint(&n, temp), want, "{speed:?}: reprocessed after '{why}'");
                assert_eq!(s.sources[0].1, Source::Processed);
                assert!(s.notes.iter().any(|n| n.contains(why) && n.contains("processed again")), "{:?}", s.notes);
                assert_eq!(s.take_fresh().len(), 1, "the good product replaces the bad one");
            }
        }
        // A store that is off makes nothing; one in memory remembers.
        let mut off = DataStore::off();
        off.nuclide(&bytes, &tape, mat, "U234", temp, SpeedTier::VeryFast, "U-234").expect("off");
        assert!(off.take_fresh().is_empty() && !off.enabled());
        let mut mem = DataStore::memory();
        mem.nuclide(&bytes, &tape, mat, "U234", temp, SpeedTier::VeryFast, "U-234").expect("mem");
        mem.forget_offered();
        mem.nuclide(&bytes, &tape, mat, "U234", temp, SpeedTier::VeryFast, "U-234").expect("mem");
        assert!(matches!(mem.sources[1].1, Source::Cached { .. }), "memory remembers");
    }

    /// **A nuclide made from cached stages is the processed nuclide, bit for
    /// bit** (gh:#923).
    ///
    /// Methodology: U-234 at `Fast` (tolerance 1e-3), as in the test above.
    /// A first store that keeps stages (the nuclear-data demo's kind)
    /// processes it at 300.15 K, making two records: RECONR at 0 K and the
    /// product (~~three, with PURR at 300.15 K~~: since gh:#932 PURR runs
    /// inside `process::cross_sections_at` and is not a product's stage).
    /// (a) A Monte Carlo store offered the RECONR record but not the
    /// product: its nuclide must equal `Nuclide::from_tape_with_speed` with no
    /// tolerance, RECONR read from the cache, only the product saved. (b) The
    /// same, asked for 600 K: equal to the direct nuclide at 600 K. Also the
    /// keys: no key of one kind is a prefix of another (each is fetched by a
    /// prefix query), and the evaluation key names `process`'s settings.
    ///
    /// Results, 2026-10-16 (release): (a) and (b) identical on every
    /// compared value; stage sources as stated. Re-run 2026-10-18 on the
    /// `process` path (gh:#932): (a) 106 654 and (b) 106 518 compared values
    /// identical; RECONR read from the cache in both; two records made.
    #[test]
    fn a_nuclide_made_from_cached_stages_is_the_processed_nuclide_bit_for_bit() {
        let keys = [
            reconstruction_key("ab", 9228, 1e-3),
            urr_key("ab", 9228, 300.15, demo_purr()),
            evaluation_key("ab", 9228, 300.15, 1e-3, 1e-3),
            law_key("ab", 9228, 300.15, "x"),
        ];
        for a in &keys {
            for b in &keys {
                assert!(a == b || !b.starts_with(a.as_str()), "{a} is a prefix of {b}");
            }
        }
        let Some(bytes) = tape_bytes("n-092_U_234-ENDF8.0.endf") else {
            eprintln!("SKIP: U-234 not in reference-data/endf/");
            return;
        };
        let (tape, mat) = read(&bytes);
        let speed = SpeedTier::Fast;
        let mut first = DataStore::browser().keeping_stages(speed.data_tolerance());
        first.begin_tape(&bytes);
        first.nuclide(&bytes, &tape, mat, "U234", 300.15, speed, "U-234").expect("made");
        let records = first.take_fresh();
        assert_eq!(records.len(), 2, "a store keeping stages saves RECONR and the product");
        let stages: Vec<Record> = records.iter().filter(|r| !r.key.contains("|pe|")).cloned().collect();
        assert_eq!(stages.len(), 1);
        assert!(stages[0].key.contains("|rc0|"));
        let cached = |s: &[(String, Source)], i: usize| matches!(s[i].1, Source::Cached { .. });

        // (a) RECONR from the cache.
        let want = fingerprint(&Nuclide::from_tape_with_speed(&tape, mat, "U234", 300.15, speed).expect("direct"), 300.15);
        let mut a = DataStore::browser();
        a.begin_tape(&bytes);
        a.offer(stages.clone());
        let n = a.nuclide(&bytes, &tape, mat, "U234", 300.15, speed, "U-234").expect("from stages");
        assert_eq!(fingerprint(&n, 300.15), want, "from the cached stage");
        let st = a.take_stages();
        assert!(st.len() == 1 && cached(&st, 0), "{st:?}");
        assert_eq!(a.take_fresh().len(), 1, "only the product is made");

        // (b) another temperature: RECONR from the cache.
        let want600 = fingerprint(&Nuclide::from_tape_with_speed(&tape, mat, "U234", 600.0, speed).expect("direct"), 600.0);
        let mut b = DataStore::browser();
        b.begin_tape(&bytes);
        b.offer(stages);
        let n = b.nuclide(&bytes, &tape, mat, "U234", 600.0, speed, "U-234").expect("600 K");
        assert_eq!(fingerprint(&n, 600.0), want600, "600 K from the cached reconstruction");
        let st = b.take_stages();
        assert!(st.len() == 1 && cached(&st, 0), "{st:?}");
        assert_eq!(b.take_fresh().len(), 1, "the product only");
        // A MAT that is not the tape's is refused, not processed.
        let mut c = DataStore::browser();
        assert!(c.evaluation(&bytes, &tape, mat + 1, 300.15, 1e-3, 1e-3, "U-234").is_err());
        eprintln!("U-234 from stages: {} values identical at 300.15 K, {} at 600 K", want.len(), want600.len());
    }

    /// **A cached thermal law is the processed law, bit for bit**: the
    /// ENDF/B-VIII.0 Si-in-SiC law (THERMR) at 300.15 K, processed through
    /// a store and read back from its record, has the same encoding word
    /// for word as `ThermalScattering::from_tape`. (tens of seconds)
    #[test]
    fn a_cached_law_is_the_processed_law_bit_for_bit() {
        let Some(bytes) = tape_bytes("tsl-SiinSiC.endf") else {
            eprintln!("SKIP: tsl-SiinSiC.endf not in reference-data/endf/");
            return;
        };
        let tape = Tape::read(std::io::Cursor::new(&bytes)).expect("tape");
        let direct = ThermalScattering::from_tape(&tape, 43, 300.15, "Si_SiC").expect("law");
        let mut a = DataStore::browser();
        a.begin_tape(&bytes);
        let made = a.thermal(&bytes, &tape, 43, 300.15, "Si_SiC", "Si-in-SiC").expect("made");
        let mut b = DataStore::browser();
        b.begin_tape(&bytes);
        b.offer(a.take_fresh());
        let cached = b.thermal(&bytes, &tape, 43, 300.15, "Si_SiC", "Si-in-SiC").expect("cached");
        let w = |t: &ThermalScattering| t.to_f64s().iter().map(|x| x.to_bits()).collect::<Vec<_>>();
        assert_eq!(w(&made), w(&direct));
        assert_eq!(w(&cached), w(&direct));
        assert!(matches!(b.sources[0].1, Source::Cached { .. }));
    }

    /// **The browser store is route 4 and njoy's `process`, bit for bit**
    /// (gh:#932, epic #925 E).
    ///
    /// Methodology: for each tape, the product words
    /// ([`ProcessedEvaluation::to_f64s`]) three ways at 293.6 K and the
    /// demos' tolerance 1e-3: (1) [`DataStore::evaluation`] as a browser
    /// worker runs it; (2) `Nuclide::process_evaluation` (route 4, the CLI's
    /// path); (3) `process::cross_sections` at [`demo_settings`] carried over
    /// by `ProcessedEvaluation::from_cross_sections`. Pass: all three equal
    /// word for word (bits). To show the check can fail: the pre-#931 chain
    /// (`Nuclide::process_evaluation_pre_931`, what the demos ran before
    /// gh:#932) must differ. Tapes: He-4 (no unresolved range) and U-234
    /// (an unresolved range, so PURR's tables are in the words).
    ///
    /// Results, 2026-10-18 (release, i9-13900K): He-4 3 228 words and U-234
    /// 380 182 words, identical three ways. The pre-#931 chain is another
    /// length on both (2 840 and 379 196 words: its 0 K elastic stops at
    /// 25 keV, `process`'s covers the whole range), so the check can fail.
    #[test]
    fn the_store_is_route_4_and_process_bit_for_bit() {
        use uom::si::thermodynamic_temperature::kelvin;
        let mut seen = 0;
        for file in ["n-002_He_004-ENDF8.0.endf", "n-092_U_234-ENDF8.0.endf"] {
            let Some(bytes) = tape_bytes(file) else {
                eprintln!("SKIP: {file} not in reference-data/endf/");
                continue;
            };
            seen += 1;
            let (tape, mat) = read(&bytes);
            let bits = |w: Vec<f64>| w.into_iter().map(f64::to_bits).collect::<Vec<_>>();
            let mut store = DataStore::browser();
            store.begin_tape(&bytes);
            let s = bits(store.evaluation(&bytes, &tape, mat, 293.6, 1e-3, 1e-3, file).expect("store").to_f64s());
            let r4 = bits(Nuclide::process_evaluation(&tape, mat, 293.6, 1e-3, 1e-3).expect("route 4").to_f64s());
            let t = uom::si::f64::ThermodynamicTemperature::new::<kelvin>(293.6);
            let xs = process::cross_sections(&tape, t, &demo_settings(1e-3, 1e-3)).expect("process");
            let pr = bits(ProcessedEvaluation::from_cross_sections(xs).to_f64s());
            let old = bits(Nuclide::process_evaluation_pre_931(&tape, mat, 293.6, 1e-3, 1e-3).expect("pre-931").to_f64s());
            assert_eq!(s, r4, "{file}: store vs route 4");
            assert_eq!(s, pr, "{file}: store vs process::cross_sections");
            let differ = if old.len() == s.len() { old.iter().zip(&s).filter(|(a, b)| a != b).count() } else { usize::MAX };
            eprintln!(
                "{file}: {} words identical (store = route 4 = process); pre-#931 chain {} words, {}",
                s.len(),
                old.len(),
                if differ == usize::MAX { "another length".to_string() } else { format!("{differ} differ") }
            );
            assert_ne!(old, s, "{file}: the pre-#931 chain must differ, or this check cannot fail");
        }
        eprintln!("{seen} of 2 tapes compared");
    }

    /// **The code version pins the processing output.** He-4 and U-234 at
    /// 300.15 K, tolerance 1e-3, are processed and their encodings hashed;
    /// the hashes are recorded here with [`CODE`]. If the processing or the
    /// encoding changes these products, this test fails: bump [`CODE`] (so
    /// browsers stop using products made by the old code) and record the new
    /// hashes. Laws are not fingerprinted here (THERMR/LEAPR take tens of
    /// seconds each); a change to them must bump [`CODE`] by hand.
    ///
    /// Since gh:#932 the product is `process`'s (`Nuclide::process_evaluation`):
    /// ~~He-4 `7fa436ec…`, U-234 `41caa7d7…`~~ (the pre-#931 chain's, under
    /// `nd-2026-10-15.1`).
    #[test]
    fn the_code_version_pins_the_processing_output() {
        let pinned = [
            ("n-002_He_004-ENDF8.0.endf", "caf8db1384fef09c7e00165586a0c418373046140974a176ce831d591bf66054"),
            ("n-092_U_234-ENDF8.0.endf", "4cb073c6f323599ac5f9aa45405b0d1e712a37153f36c18a26a62562a4b2b435"), // 2026-10-18: gh:#932, process's 20/64/10 000 after HEATR
        ];
        let mut got = Vec::new();
        for (file, want) in pinned {
            let Some(bytes) = tape_bytes(file) else {
                eprintln!("SKIP: {file} not in reference-data/endf/");
                continue;
            };
            let (tape, mat) = read(&bytes);
            let p = Nuclide::process_evaluation(&tape, mat, 300.15, 1e-3, 1e-3).expect("process");
            let sha = dhoby_ghaut::web_demo::data_cache::words_sha256(&p.to_f64s());
            got.push(format!("{file}: {sha}"));
            assert_eq!(
                sha, want,
                "the processed output of {file} changed: bump processed_cache::CODE (now {CODE}) and record the new hash"
            );
        }
        eprintln!("{}", got.join("\n"));
        assert!(CODE.starts_with("nd-"));
        let k = evaluation_key("abc", 9228, 300.15, 1e-3, 1e-3);
        assert!(k.starts_with("nd1|abc|pe|mat=9228|T=300.15:") && k.ends_with("|purr=20,64,10000|heatr=1|e0k=1"), "{k}");
        assert_eq!(demo_purr(), (20, 64, 10_000));
        assert!(law_key("leapr-UInUO2", 0, 300.15, "U_UO2").starts_with(&tape_prefix("leapr-UInUO2")));
    }
}
