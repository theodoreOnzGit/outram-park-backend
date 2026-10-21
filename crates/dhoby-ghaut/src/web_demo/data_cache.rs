//! **A browser cache for processed nuclear data, shared by every demo on the
//! site** (gh:#818).
//!
//! Maintainer, 2026-10-09: "is it possible for one set of nuclear data to be
//! computed for all the simulation demos? So that if I switch tabs by
//! accident, I don't restart and have to wait 5 mins". Processing a tape
//! (RECONR, BROADR, PURR, THERMR, LEAPR) takes seconds to minutes in a
//! browser; reading its product back takes a fraction of a second. So each
//! product is kept, as the exact `f64` words of its encoding, in the
//! browser's **IndexedDB on the site's origin**: every demo page of the site,
//! a reload, and a tab the browser discarded in the background all read the
//! same store.
//!
//! This module is the track-independent half: [`Record`] (one product, with
//! the key and code version it was made under and a SHA-256 of its words),
//! the reasons a lookup misses ([`Miss`]), where a nuclide's data came from
//! ([`Source`], for the screen), and [`idb`], the IndexedDB glue (browser
//! only). What the key holds (tape content hash, processing settings,
//! processing-code version) and how a product is decoded are the caller's:
//! `examples/common/processed_cache.rs` for the Monte Carlo demos.
//!
//! **Leak Before Break** (`docs/kovan.md`): nothing here is trusted silently.
//! A record whose key, code version or checksum does not match is a miss
//! with a reason ([`Miss::note`]) that the page shows, and the data are
//! processed again. Storage that is missing or throws (a private window,
//! blocked site data) is [`Miss::Unavailable`], shown, and processing goes
//! on without the cache. A quota error on saving is [`PutError::Quota`],
//! shown, and nothing more is saved.

use sha2::{Digest, Sha256};

/// The IndexedDB database every demo page shares (same origin, same name).
pub const DB_NAME: &str = "outram-park-nuclear-data";

/// SHA-256 of `bytes`, lower-case hex. The cache keys a tape by its content,
/// so a re-published tape with other bytes is a different key.
pub fn sha256_hex(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

/// SHA-256 of `words` as little-endian bytes: what a [`Record`] carries to
/// detect a corrupt entry.
pub fn words_sha256(words: &[f64]) -> String {
    let mut h = Sha256::new();
    let mut buf = Vec::with_capacity(8 * 4096);
    for chunk in words.chunks(4096) {
        buf.clear();
        for w in chunk {
            buf.extend_from_slice(&w.to_le_bytes());
        }
        h.update(&buf);
    }
    hex(&h.finalize())
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// One cached product.
#[derive(Clone, Debug, PartialEq)]
pub struct Record {
    /// Everything the product depends on (the caller's format).
    pub key: String,
    /// What it is, for the screen (`"U-235"`, `"graphite S(a,b)"`).
    pub label: String,
    /// The processing-code version it was made with.
    pub code: String,
    /// When it was processed, ms since 1970 (UTC).
    pub created_ms: f64,
    /// [`words_sha256`] of `words` when it was sealed.
    pub sha256: String,
    /// The product's exact encoding.
    pub words: Vec<f64>,
}

impl Record {
    /// A record of `words`, checksummed.
    pub fn seal(key: String, label: String, code: String, created_ms: f64, words: Vec<f64>) -> Self {
        let sha256 = words_sha256(&words);
        Self {
            key,
            label,
            code,
            created_ms,
            sha256,
            words,
        }
    }

    /// Bytes of the product.
    pub fn bytes(&self) -> usize {
        8 * self.words.len()
    }

    /// The words, if this record is the product asked for: the same key, the
    /// same code version, and words that still match their checksum.
    ///
    /// # Errors
    ///
    /// [`Miss::Mismatch`] for another key or code version, [`Miss::Corrupt`]
    /// for words that do not match the checksum.
    pub fn open(&self, key: &str, code: &str) -> Result<&[f64], Miss> {
        if self.key != key {
            return Err(Miss::Mismatch(format!("stored under another key ({})", self.key)));
        }
        if self.code != code {
            return Err(Miss::Mismatch(format!(
                "made by processing code {}, this page runs {code}",
                self.code
            )));
        }
        if words_sha256(&self.words) != self.sha256 {
            return Err(Miss::Corrupt(format!(
                "its {} words do not match their checksum",
                self.words.len()
            )));
        }
        Ok(&self.words)
    }
}

/// Why a product was not read from the cache.
#[derive(Clone, Debug, PartialEq)]
pub enum Miss {
    /// Never cached (or evicted by the browser, which removes whole sites).
    Absent,
    /// Cached under another key or code version.
    Mismatch(String),
    /// The words do not match their checksum.
    Corrupt(String),
    /// The words passed the checksum but could not be decoded or used.
    Unusable(String),
    /// The browser's storage is missing or refused (a private window, blocked
    /// site data).
    Unavailable(String),
}

impl Miss {
    /// What the page says, or `None` for a plain first-time miss.
    pub fn note(&self, label: &str) -> Option<String> {
        match self {
            Miss::Absent => None,
            Miss::Mismatch(why) => Some(format!("{label}: the cached data were not used ({why}); processed again.")),
            Miss::Corrupt(why) => Some(format!("{label}: the cached data are corrupt ({why}); processed again.")),
            Miss::Unusable(why) => Some(format!("{label}: the cached data could not be read ({why}); processed again.")),
            Miss::Unavailable(why) => Some(format!(
                "This browser's storage is unavailable ({why}): nuclear data are processed and not cached."
            )),
        }
    }
}

/// Where a product the demo uses came from (Leak Before Break: the page
/// shows it).
#[derive(Clone, Debug, PartialEq)]
pub enum Source {
    Processed,
    Cached { created_ms: f64, code: String },
}

impl Source {
    /// As the page prints it.
    pub fn describe(&self) -> String {
        match self {
            Source::Processed => "processed now".into(),
            Source::Cached { created_ms, code } => format!(
                "from this browser's cache, processed {}, code {code}",
                utc_label(*created_ms)
            ),
        }
    }

    /// One line for a list of products: how many were read from the cache
    /// (with the oldest processing date and the code versions) and how many
    /// were processed now.
    pub fn summarize<'a>(sources: impl IntoIterator<Item = &'a Source>) -> String {
        let (mut cached, mut fresh, mut oldest) = (0usize, 0usize, f64::INFINITY);
        let mut codes: Vec<&str> = Vec::new();
        for s in sources {
            match s {
                Source::Processed => fresh += 1,
                Source::Cached { created_ms, code } => {
                    cached += 1;
                    oldest = oldest.min(*created_ms);
                    if !codes.contains(&code.as_str()) {
                        codes.push(code);
                    }
                }
            }
        }
        match (cached, fresh) {
            (0, 0) => "no nuclear data".into(),
            (0, n) => format!("all {n} processed now"),
            (c, 0) => format!(
                "all {c} from this browser's cache (processed {} or later, code {})",
                utc_label(oldest),
                codes.join(", ")
            ),
            (c, n) => format!(
                "{c} from this browser's cache (processed {} or later, code {}), {n} processed now",
                utc_label(oldest),
                codes.join(", ")
            ),
        }
    }
}

/// `ms` since 1970 as `YYYY-MM-DD HH:MM UTC`.
pub fn utc_label(ms: f64) -> String {
    if !ms.is_finite() {
        return "at an unknown time".into();
    }
    let secs = (ms / 1000.0).floor() as i64;
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02} {:02}:{:02} UTC", rem / 3600, (rem % 3600) / 60)
}

/// What the cache holds, from its index (the page reads only the index, never
/// the products).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Summary {
    pub entries: usize,
    pub bytes: f64,
    /// Each code version present.
    pub codes: Vec<String>,
}

impl Summary {
    /// From the index rows `(code, bytes)`.
    pub fn from_rows(rows: impl IntoIterator<Item = (String, f64)>) -> Self {
        let mut s = Summary::default();
        for (code, bytes) in rows {
            s.entries += 1;
            s.bytes += bytes;
            if !s.codes.contains(&code) {
                s.codes.push(code);
            }
        }
        s
    }

    /// As the page prints it.
    pub fn describe(&self) -> String {
        if self.entries == 0 {
            return "empty".into();
        }
        format!(
            "{} product{}, {:.0} MB (code {})",
            self.entries,
            if self.entries == 1 { "" } else { "s" },
            self.bytes / 1.0e6,
            self.codes.join(", ")
        )
    }
}

/// Why saving a record failed.
#[derive(Clone, Debug, PartialEq)]
pub enum PutError {
    /// The browser's storage quota for the site is full.
    Quota(String),
    Other(String),
}

impl PutError {
    /// What the page says.
    pub fn note(&self, label: &str) -> String {
        match self {
            PutError::Quota(m) => format!(
                "{label} was not cached: this browser's storage quota for the site is full ({m}). \
                 Nothing more is cached this session; clear the cache to make room."
            ),
            PutError::Other(m) => format!("{label} was not cached ({m})."),
        }
    }
}

/// **The IndexedDB glue** (browser only). GUI/platform code: it cannot run
/// natively, so no native test reaches it (the test rule's exception); the
/// pure logic above is tested natively and this is checked in headless
/// Chromium (`verification_and_validation/nuclear_data_cache_web/`).
///
/// The store is two object stores in [`DB_NAME`]: `records` (the products,
/// keyed by [`Record::key`]) and `meta` (a small index row per product, so
/// the page can show the cache size without reading products). The glue is
/// a few lines of JavaScript, built once with `new Function`, so that every
/// IndexedDB call and every exception stays inside promises this module
/// awaits: storage that throws (a private window) rejects, it never panics.
#[cfg(target_arch = "wasm32")]
pub mod idb {
    use super::{PutError, Record, Summary, DB_NAME};
    use js_sys::{Array, Float64Array, Function, Object, Promise, Reflect};
    use wasm_bindgen::{JsCast as _, JsValue};
    use wasm_bindgen_futures::JsFuture;

    const GLUE: &str = r#"
      const DB = __DB__;
      let opening = null;
      function open() {
        if (opening) return opening;
        opening = new Promise((res, rej) => {
          let r;
          try {
            if (typeof indexedDB === "undefined" || !indexedDB) throw new Error("IndexedDB is not available in this browser context");
            r = indexedDB.open(DB, 1);
          } catch (e) { rej(e); return; }
          r.onupgradeneeded = () => {
            const d = r.result;
            if (!d.objectStoreNames.contains("records")) d.createObjectStore("records", { keyPath: "key" });
            if (!d.objectStoreNames.contains("meta")) d.createObjectStore("meta", { keyPath: "key" });
          };
          r.onsuccess = () => { const d = r.result; d.onversionchange = () => { d.close(); opening = null; }; res(d); };
          r.onerror = () => rej(r.error);
          r.onblocked = () => rej(new Error("another tab of this site holds the cache open"));
        });
        opening.catch(() => { opening = null; });
        return opening;
      }
      const req = (r) => new Promise((res, rej) => { r.onsuccess = () => res(r.result); r.onerror = () => rej(r.error); });
      const done = (tx) => new Promise((res, rej) => {
        tx.oncomplete = () => res();
        tx.onerror = () => rej(tx.error);
        tx.onabort = () => rej(tx.error || new Error("transaction aborted"));
      });
      const err = (e) => ({ name: String((e && e.name) || "Error"), message: String((e && e.message) || e) });
      return {
        async getPrefix(prefix) {
          try {
            const d = await open();
            const tx = d.transaction("records", "readonly");
            return await req(tx.objectStore("records").getAll(IDBKeyRange.bound(prefix, prefix + "￿")));
          } catch (e) { throw err(e); }
        },
        async put(rec, meta) {
          try {
            const d = await open();
            const tx = d.transaction(["records", "meta"], "readwrite");
            tx.objectStore("records").put(rec);
            tx.objectStore("meta").put(meta);
            await done(tx);
          } catch (e) { throw err(e); }
        },
        async clear() {
          try {
            const d = await open();
            const tx = d.transaction(["records", "meta"], "readwrite");
            tx.objectStore("records").clear();
            tx.objectStore("meta").clear();
            await done(tx);
          } catch (e) { throw err(e); }
        },
        async metas() {
          try {
            const d = await open();
            const tx = d.transaction("meta", "readonly");
            return await req(tx.objectStore("meta").getAll());
          } catch (e) { throw err(e); }
        },
      };
    "#;

    thread_local! {
        static API: std::cell::RefCell<Option<JsValue>> = const { std::cell::RefCell::new(None) };
    }

    fn api() -> Result<JsValue, String> {
        API.with(|a| {
            if let Some(v) = a.borrow().as_ref() {
                return Ok(v.clone());
            }
            let src = GLUE.replace("__DB__", &format!("{DB_NAME:?}"));
            let v = Function::new_no_args(&src)
                .call0(&JsValue::NULL)
                .map_err(|e| format!("the cache glue did not start: {e:?}"))?;
            *a.borrow_mut() = Some(v.clone());
            Ok(v)
        })
    }

    /// `(name, message)` of a rejection.
    fn reason(e: &JsValue) -> (String, String) {
        let s = |k: &str| Reflect::get(e, &k.into()).ok().and_then(|v| v.as_string()).unwrap_or_default();
        let (n, m) = (s("name"), s("message"));
        if n.is_empty() && m.is_empty() {
            ("Error".into(), format!("{e:?}"))
        } else {
            (n, m)
        }
    }

    async fn call(method: &str, args: &[JsValue]) -> Result<JsValue, (String, String)> {
        let api = api().map_err(|m| ("Error".to_string(), m))?;
        let f: Function = Reflect::get(&api, &method.into())
            .ok()
            .and_then(|f| f.dyn_into().ok())
            .ok_or_else(|| ("Error".to_string(), format!("no cache method {method}")))?;
        let arr = Array::new();
        for a in args {
            arr.push(a);
        }
        let p: Promise = f
            .apply(&api, &arr)
            .map_err(|e| reason(&e))?
            .dyn_into()
            .map_err(|_| ("Error".to_string(), "not a promise".to_string()))?;
        JsFuture::from(p).await.map_err(|e| reason(&e))
    }

    fn set(o: &Object, k: &str, v: impl Into<JsValue>) {
        let _ = Reflect::set(o, &k.into(), &v.into());
    }

    fn record_from(v: &JsValue) -> Option<Record> {
        let s = |k: &str| Reflect::get(v, &k.into()).ok().and_then(|x| x.as_string());
        let words = Reflect::get(v, &"words".into()).ok()?.dyn_into::<Float64Array>().ok()?.to_vec();
        Some(Record {
            key: s("key")?,
            label: s("label").unwrap_or_default(),
            code: s("code")?,
            created_ms: Reflect::get(v, &"created_ms".into()).ok()?.as_f64()?,
            sha256: s("sha256")?,
            words,
        })
    }

    /// Every record whose key starts with `prefix`. A row that is not a
    /// record is returned as `Err` in the list, so the caller shows it.
    ///
    /// # Errors
    ///
    /// The storage is unavailable or refused the read.
    pub async fn get_prefix(prefix: &str) -> Result<Vec<Result<Record, String>>, String> {
        let v = call("getPrefix", &[prefix.into()]).await.map_err(|(n, m)| format!("{n}: {m}"))?;
        let arr: Array = v.dyn_into().map_err(|_| "the cache returned something other than a list".to_string())?;
        Ok(arr
            .iter()
            .map(|row| {
                record_from(&row).ok_or_else(|| {
                    let k = Reflect::get(&row, &"key".into()).ok().and_then(|x| x.as_string()).unwrap_or_default();
                    format!("a malformed entry ({k})")
                })
            })
            .collect())
    }

    /// Save `r` (and its index row).
    ///
    /// # Errors
    ///
    /// [`PutError::Quota`] when the site's quota is full, else
    /// [`PutError::Other`].
    pub async fn put(r: &Record) -> Result<(), PutError> {
        let rec = Object::new();
        set(&rec, "key", r.key.as_str());
        set(&rec, "label", r.label.as_str());
        set(&rec, "code", r.code.as_str());
        set(&rec, "created_ms", r.created_ms);
        set(&rec, "sha256", r.sha256.as_str());
        set(&rec, "words", Float64Array::from(r.words.as_slice()));
        let meta = Object::new();
        set(&meta, "key", r.key.as_str());
        set(&meta, "label", r.label.as_str());
        set(&meta, "code", r.code.as_str());
        set(&meta, "created_ms", r.created_ms);
        set(&meta, "bytes", r.bytes() as f64);
        call("put", &[rec.into(), meta.into()]).await.map(|_| ()).map_err(|(n, m)| {
            if n == "QuotaExceededError" || m.to_lowercase().contains("quota") {
                PutError::Quota(format!("{n}: {m}"))
            } else {
                PutError::Other(format!("{n}: {m}"))
            }
        })
    }

    /// Remove every cached product.
    ///
    /// # Errors
    ///
    /// The storage is unavailable or refused.
    pub async fn clear() -> Result<(), String> {
        call("clear", &[]).await.map(|_| ()).map_err(|(n, m)| format!("{n}: {m}"))
    }

    /// What the cache holds, from its index.
    ///
    /// # Errors
    ///
    /// The storage is unavailable or refused.
    pub async fn summary() -> Result<Summary, String> {
        let v = call("metas", &[]).await.map_err(|(n, m)| format!("{n}: {m}"))?;
        let arr: Array = v.dyn_into().map_err(|_| "the cache index is not a list".to_string())?;
        Ok(Summary::from_rows(arr.iter().map(|row| {
            let code = Reflect::get(&row, &"code".into()).ok().and_then(|x| x.as_string()).unwrap_or_default();
            let bytes = Reflect::get(&row, &"bytes".into()).ok().and_then(|x| x.as_f64()).unwrap_or(0.0);
            (code, bytes)
        })))
    }

    /// Now, ms since 1970.
    pub fn now_ms() -> f64 {
        js_sys::Date::now()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A record opens only under its own key and code, and only while its
    /// words match their checksum; every miss says why; dates, sources and
    /// summaries print as the page shows them.
    #[test]
    fn a_record_opens_only_when_it_is_what_was_asked_for() {
        let words = vec![786.0, 1.0, f64::NAN, -0.0, 1.0e-300, f64::INFINITY];
        let r = Record::seal("k|1".into(), "U-235".into(), "c1".into(), 0.0, words.clone());
        assert_eq!(r.bytes(), 48);
        let got = r.open("k|1", "c1").expect("open");
        assert_eq!(
            got.iter().map(|x| x.to_bits()).collect::<Vec<_>>(),
            words.iter().map(|x| x.to_bits()).collect::<Vec<_>>()
        );
        assert!(matches!(r.open("k|2", "c1"), Err(Miss::Mismatch(_))));
        assert!(matches!(r.open("k|1", "c2"), Err(Miss::Mismatch(m)) if m.contains("c1")));
        let mut bad = r.clone();
        bad.words[1] = 1.0000000000000002;
        assert!(matches!(bad.open("k|1", "c1"), Err(Miss::Corrupt(_))));
        assert!(Miss::Absent.note("U-235").is_none());
        for m in [
            Miss::Mismatch("x".into()),
            Miss::Corrupt("x".into()),
            Miss::Unusable("x".into()),
        ] {
            assert!(m.note("U-235").is_some_and(|n| n.contains("U-235") && n.contains("processed again")));
        }
        assert!(Miss::Unavailable("SecurityError".into())
            .note("U-235")
            .is_some_and(|n| n.contains("not cached")));
        assert_eq!(utc_label(0.0), "1970-01-01 00:00 UTC");
        assert_eq!(utc_label(1_791_514_800_000.0), "2026-10-09 03:00 UTC");
        assert_eq!(utc_label(951_782_400_000.0), "2000-02-29 00:00 UTC");
        assert_eq!(Source::Processed.describe(), "processed now");
        let c = Source::Cached {
            created_ms: 1_791_514_800_000.0,
            code: "nd-1".into(),
        };
        assert_eq!(
            c.describe(),
            "from this browser's cache, processed 2026-10-09 03:00 UTC, code nd-1"
        );
        assert_eq!(Source::summarize([&c, &c]), "all 2 from this browser's cache (processed 2026-10-09 03:00 UTC or later, code nd-1)");
        assert!(Source::summarize([&c, &Source::Processed]).ends_with("1 processed now"));
        assert_eq!(Source::summarize([&Source::Processed]), "all 1 processed now");
        let s = Summary::from_rows([("a".to_string(), 2.0e6), ("a".to_string(), 1.0e6)]);
        assert_eq!(s.describe(), "2 products, 3 MB (code a)");
        assert_eq!(Summary::default().describe(), "empty");
        assert!(PutError::Quota("QuotaExceededError".into()).note("U-238").contains("quota"));
        assert!(PutError::Other("x".into()).note("U-238").contains("not cached"));
        assert_eq!(sha256_hex(b"abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        assert_ne!(words_sha256(&[0.0]), words_sha256(&[-0.0]));
    }
}
