//! **The processed nuclear data every Monte Carlo demo shares** (gh:#818):
//! the cache keys, and the one place a demo builds a nuclide or a thermal
//! law from a tape, reading the product back from the cache when it is
//! there. Included by `monte_carlo_web` (every rung loader, the htr10
//! whole-core pool) and `delta_tracking_web`.
//!
//! **What is cached is the expensive half only**, in the exact `f64`
//! encodings gh:#786 added for relaying products between workers:
//! [`ProcessedEvaluation::to_f64s`] (RECONR + BROADR + PURR, and the 0 K
//! elastic grid DBRC needs) and [`ThermalScattering::to_f64s`] (THERMR or
//! LEAPR). The cheap half, `Nuclide::from_processed`, is always run, from the
//! tape the demo downloads anyway. `Nuclide::from_tape_with_speed` is exactly
//! `process_evaluation` (at the tier's tolerance for RECONR and BROADR) then
//! `from_processed` then `with_speed`, which is what [`DataStore::nuclide`]
//! does, so a nuclide from the cache is the nuclide from the tape, bit for
//! bit; `tests::a_cached_nuclide_is_the_processed_nuclide_bit_for_bit` pins
//! it.
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
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::material::processed::ProcessedEvaluation;
use outram_mc_libs::material::speed::SpeedTier;
use outram_mc_libs::material::thermal::ThermalScattering;
use std::collections::HashMap;

/// **The processing-code version** every cached product carries. Bump it
/// (and the fingerprint in the test below) whenever RECONR, BROADR, PURR,
/// THERMR, LEAPR or an encoding changes what a product holds; a product made
/// under another version is not used, and the page says so.
pub const CODE: &str = "nd-2026-10-09.1";

/// Version of the key layout itself.
const KEYS: &str = "nd1";

/// PURR's table settings in `Nuclide::process_evaluation`
/// (`UrrProbabilityTables::from_endf(tape, mat, temp_k, 20, 16, 2000)`).
const PURR: &str = "purr=20,16,2000";

/// A float in a key: readable, and exact (its bits).
fn exact(x: f64) -> String {
    format!("{x}:{:016x}", x.to_bits())
}

/// The prefix of every key made from one tape (`nd1|<sha256>|`): the browser
/// fetches all of a tape's products with one range query.
pub fn tape_prefix(tape_sha: &str) -> String {
    format!("{KEYS}|{tape_sha}|")
}

/// The key of a tape's RECONR + BROADR + PURR product.
pub fn evaluation_key(tape_sha: &str, mat: i32, temp_k: f64, tolerance: f64, errthn: f64) -> String {
    format!(
        "{}pe|mat={mat}|T={}|tol={}|errthn={}|{PURR}",
        tape_prefix(tape_sha),
        exact(temp_k),
        exact(tolerance),
        exact(errthn)
    )
}

/// The key of a thermal law: from a tape (`source` = its SHA-256) or from
/// LEAPR (`source` = `leapr-<material>`).
pub fn law_key(source: &str, mat: i32, temp_k: f64, label: &str) -> String {
    format!("{}tsl|mat={mat}|T={}|label={label}", tape_prefix(source), exact(temp_k))
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
    entries: HashMap<String, Record>,
    fresh: Vec<Record>,
    /// Per product used, in order: its label and where it came from.
    pub sources: Vec<(String, Source)>,
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
        (std::mem::take(&mut self.sources), std::mem::take(&mut self.notes))
    }

    fn with(enabled: bool, remember: bool) -> Self {
        Self {
            enabled,
            remember,
            entries: HashMap::new(),
            fresh: Vec::new(),
            sources: Vec::new(),
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

    /// The RECONR + BROADR + PURR product of `tape` (whose bytes are
    /// `bytes`), from the cache or processed now.
    ///
    /// # Errors
    ///
    /// Processing failed.
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
        let p = Nuclide::process_evaluation(tape, mat, temp_k, tolerance, errthn)
            .map_err(|e| format!("{label}: {e}"))?;
        self.made(key, label, p.to_f64s());
        self.sources.push((label.to_string(), Source::Processed));
        Ok(p)
    }

    /// A nuclide at a [`SpeedTier`]: `Nuclide::from_tape_with_speed`, with
    /// its expensive half from the cache when it is there.
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
            eprintln!("U-234 {speed:?}: {} values identical; record {} words", want.len(), records[0].words.len());
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

    /// **The code version pins the processing output.** He-4 and U-234 at
    /// 300.15 K, tolerance 1e-3, are processed and their encodings hashed;
    /// the hashes are recorded here with [`CODE`]. If the processing or the
    /// encoding changes these products, this test fails: bump [`CODE`] (so
    /// browsers stop using products made by the old code) and record the new
    /// hashes. Laws are not fingerprinted here (THERMR/LEAPR take tens of
    /// seconds each); a change to them must bump [`CODE`] by hand.
    #[test]
    fn the_code_version_pins_the_processing_output() {
        let pinned = [
            ("n-002_He_004-ENDF8.0.endf", "7fa436eca26cf2f0bf2aff4f71f640c531b2fe8f8df08cb0f97d1386a8ebd699"),
            ("n-092_U_234-ENDF8.0.endf", "0198a48acff9ccc2bb05f37290cbe0c51dc0f69b610d62911cd2cd6d1f1e1902"),
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
        assert!(k.starts_with("nd1|abc|pe|mat=9228|T=300.15:") && k.ends_with(PURR), "{k}");
        assert!(law_key("leapr-UInUO2", 0, 300.15, "U_UO2").starts_with(&tape_prefix("leapr-UInUO2")));
    }
}
