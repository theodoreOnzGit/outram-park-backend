//! In-document `/`-search for the PDF reader, off the UI thread (GH issue
//! #1199, 2026-11-04).
//!
//! # Why this exists
//!
//! The reader used to rescan the whole document on **every keystroke**,
//! synchronously on the UI thread: `page_to_stext` for every page, then a
//! substring match per line, with nothing kept between keystrokes. That
//! breaks the workspace's "no lagging" rule.
//!
//! # Measured (2026-11-04, release, `tests::bench_search_on_a_real_pdf`)
//!
//! UI-thread time per keystroke, old synchronous scan vs this worker, on
//! PDFs from the `reactor-literature` submodule:
//!
//! | document | pages | old, per keystroke | worker, 1st keystroke | worker, later | UI thread now |
//! |---|---|---|---|---|---|
//! | SAND2011-2195 (LIME manual) | 42 | 79–90 ms | 83–89 ms | 1.0–1.2 ms | < 0.01 ms |
//! | 10 CFR 50 (eCFR 2026-10-02) | 326 | 278–282 ms | 302 ms | 4.2–5.4 ms | < 0.01 ms |
//! | LA-UR-17-20093 (NJOY2016 manual) | 816 | 1883–1918 ms | 1933 ms | 7.2–12.0 ms | < 0.01 ms |
//!
//! The first keystroke still pays the one-time extraction, but on the
//! worker, with hits streaming in page by page. Every keystroke's hits were
//! asserted equal to the old scan's.
//!
//! # Design
//!
//! One worker thread per opened document ([`TextSearchWorker`]):
//!
//! - it opens its **own** [`PdfDocument`] from a copy of the bytes, because a
//!   document holds `RefCell`s and cannot be shared across threads (the same
//!   reason `kopitiam_pdf::gui_frontend::SearchWorker` gives);
//! - it extracts each page's text **once**, lazily and in page order, keeping
//!   only what a hit needs: each line's characters and each character's
//!   device-space bounding box ([`IndexedLine`]);
//! - it searches that cache for the newest query and publishes hits page by
//!   page into an `Arc<(Mutex<SearchShared>, Condvar)>`, tagged with a
//!   **generation** number. A newer query bumps the generation; the worker
//!   notices at the next page boundary and restarts, and the UI ignores any
//!   result whose generation is not the one it asked for.
//!
//! Hits come out in the same order, with the same boxes, as the old
//! synchronous scan: pages ascending, then blocks, lines and matches in
//! structured-text order. `tests::worker_hits_equal_the_old_synchronous_scan`
//! pins that.
//!
//! # Checked and not reused
//!
//! `kopitiam_pdf::gui_frontend::SearchWorker` already searches off-thread,
//! but bakes one query into one worker and re-extracts every page per query
//! (so a 300-page document is re-extracted on every keystroke, only on a
//! different thread), and its `search_page` hit geometry is not the one
//! kovan's canvas draws. The page-at-a-time worker shape is borrowed from it.

use std::sync::{Arc, Condvar, Mutex};

use eframe::egui::Pos2;
use kopitiam_pdf::mupdf::{page_to_stext, PdfDocument, StextBlock, StextOptions, StextPage};

/// One in-document search hit: a tight box in texture-pixel space (device
/// space x `scale`) on `page`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct SearchHit {
    pub(super) page: usize,
    pub(super) min: Pos2,
    pub(super) max: Pos2,
}

/// One structured-text line, reduced to what a search hit needs: its
/// characters and each character's device-space bounding box
/// `[x0, y0, x1, y1]` (the min/max over the four corners of its quad).
///
/// Taking the min/max per character first and then over a match gives the
/// same floats as taking it over all corners of the match at once, because
/// `min`/`max` are exact; so the boxes are bit-identical to the old ones.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct IndexedLine {
    chars: Vec<char>,
    boxes: Vec<[f32; 4]>,
}

/// Reduce one extracted page to its text lines, in structured-text order
/// (blocks, then lines), skipping image blocks.
pub(super) fn index_page(stext: &StextPage) -> Vec<IndexedLine> {
    let mut lines = Vec::new();
    for block in &stext.blocks {
        let StextBlock::Text(tb) = block else {
            continue;
        };
        for line in &tb.lines {
            let mut chars = Vec::with_capacity(line.chars.len());
            let mut boxes = Vec::with_capacity(line.chars.len());
            for ch in &line.chars {
                let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
                for p in [ch.quad.ul, ch.quad.ur, ch.quad.ll, ch.quad.lr] {
                    x0 = x0.min(p.x);
                    y0 = y0.min(p.y);
                    x1 = x1.max(p.x);
                    y1 = y1.max(p.y);
                }
                chars.push(ch.c);
                boxes.push([x0, y0, x1, y1]);
            }
            lines.push(IndexedLine { chars, boxes });
        }
    }
    lines
}

/// Every `(start_char, end_char)` half-open char range in `chars` that
/// case-insensitively matches `needle` (ASCII fold). Overlapping matches
/// are not returned — the scan advances past each hit.
pub(super) fn substr_char_ranges(chars: &[char], needle: &str) -> Vec<(usize, usize)> {
    let needle: Vec<char> = needle.chars().flat_map(|c| c.to_lowercase()).collect();
    if needle.is_empty() || needle.len() > chars.len() {
        return Vec::new();
    }
    let lower: Vec<char> = chars.iter().map(|c| c.to_ascii_lowercase()).collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i + needle.len() <= lower.len() {
        if lower[i..i + needle.len()] == needle[..] {
            out.push((i, i + needle.len()));
            i += needle.len();
        } else {
            i += 1;
        }
    }
    out
}

/// Every hit of `needle` on one indexed page, in line order, as boxes in
/// texture-pixel space (`scale` = texture pixels per device unit).
/// Word/character granularity: a hit highlights the matched text, not its
/// whole line.
pub(super) fn page_hits(
    lines: &[IndexedLine],
    page: usize,
    needle: &str,
    scale: f32,
) -> Vec<SearchHit> {
    let mut hits = Vec::new();
    for line in lines {
        for (s, e) in substr_char_ranges(&line.chars, needle) {
            let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
            for b in &line.boxes[s..e] {
                x0 = x0.min(b[0]);
                y0 = y0.min(b[1]);
                x1 = x1.max(b[2]);
                y1 = y1.max(b[3]);
            }
            hits.push(SearchHit {
                page,
                min: Pos2::new(x0 * scale, y0 * scale),
                max: Pos2::new(x1 * scale, y1 * scale),
            });
        }
    }
    hits
}

/// What the UI and the worker share, behind one mutex.
#[derive(Debug, Default)]
struct SearchShared {
    /// The query generation the UI wants. Bumped by every
    /// [`TextSearchWorker::submit`].
    generation: u64,
    /// The trimmed needle for `generation`. Empty = no search.
    needle: String,
    /// Hits found so far for `generation`, in document order.
    hits: Vec<SearchHit>,
    /// Pages searched so far for `generation` (pages `0..pages_done`).
    pages_done: usize,
    /// Set when the UI drops the worker: the thread exits at the next page.
    shutdown: bool,
}

/// What [`TextSearchWorker::poll`] hands back for the generation asked about.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct SearchProgress {
    /// Hits after the first `from` the caller already has.
    pub(super) new_hits: Vec<SearchHit>,
    /// Pages searched so far.
    pub(super) pages_done: usize,
    /// Pages in the document.
    pub(super) page_count: usize,
}

impl SearchProgress {
    /// Whether every page has been searched.
    pub(super) fn finished(&self) -> bool {
        self.pages_done >= self.page_count
    }
}

/// A background text-extraction-and-search worker for one opened document.
///
/// Dropping it tells the thread to stop at its next page boundary; the drop
/// never waits for the thread, so a slow page cannot stall the UI.
pub(super) struct TextSearchWorker {
    shared: Arc<(Mutex<SearchShared>, Condvar)>,
    page_count: usize,
}

impl TextSearchWorker {
    /// Start a worker over a private copy of the document's `bytes`, with
    /// `page_count` pages, publishing boxes at `scale` texture pixels per
    /// device unit. `None` if the thread cannot be spawned.
    pub(super) fn spawn(bytes: Vec<u8>, page_count: usize, scale: f32) -> Option<Self> {
        let shared = Arc::new((Mutex::new(SearchShared::default()), Condvar::new()));
        let thread_shared = Arc::clone(&shared);
        std::thread::Builder::new()
            .name("kovan-pdf-search".to_string())
            .spawn(move || worker_loop(bytes, page_count, scale, thread_shared))
            .ok()?;
        Some(Self { shared, page_count })
    }

    /// Ask for `needle` (trimmed by the caller), superseding any search in
    /// flight. Returns the generation to [`Self::poll`] with.
    pub(super) fn submit(&self, needle: &str) -> u64 {
        let (lock, cvar) = &*self.shared;
        let mut s = lock.lock().unwrap_or_else(|e| e.into_inner());
        s.generation += 1;
        s.needle = needle.to_string();
        s.hits.clear();
        s.pages_done = 0;
        cvar.notify_all();
        s.generation
    }

    /// Progress for `generation`, with the hits after the first `from`.
    /// `None` when the worker has moved on to a newer generation, so a stale
    /// result can never be shown against a newer query.
    pub(super) fn poll(&self, generation: u64, from: usize) -> Option<SearchProgress> {
        let (lock, _) = &*self.shared;
        let s = lock.lock().unwrap_or_else(|e| e.into_inner());
        if s.generation != generation {
            return None;
        }
        Some(SearchProgress {
            new_hits: s
                .hits
                .get(from..)
                .map(<[SearchHit]>::to_vec)
                .unwrap_or_default(),
            pages_done: s.pages_done,
            page_count: self.page_count,
        })
    }
}

impl Drop for TextSearchWorker {
    fn drop(&mut self) {
        let (lock, cvar) = &*self.shared;
        let mut s = lock.lock().unwrap_or_else(|e| e.into_inner());
        s.shutdown = true;
        cvar.notify_all();
    }
}

/// The worker thread: extract pages once into `cache`, search the newest
/// generation page by page, publish while that generation is still current,
/// and sleep on the condvar when there is nothing to do.
fn worker_loop(
    bytes: Vec<u8>,
    page_count: usize,
    scale: f32,
    shared: Arc<(Mutex<SearchShared>, Condvar)>,
) {
    let (lock, cvar) = &*shared;
    let doc = PdfDocument::open(bytes).ok();
    // A page that fails to extract is cached as no lines, as the old scan
    // skipped it.
    let mut cache: Vec<Option<Vec<IndexedLine>>> = vec![None; page_count];
    let mut done_gen = 0u64;
    loop {
        // Wait for a generation this thread has not finished, or shutdown.
        let (generation, needle) = {
            let mut s = lock.lock().unwrap_or_else(|e| e.into_inner());
            while !s.shutdown && s.generation == done_gen {
                s = cvar.wait(s).unwrap_or_else(|e| e.into_inner());
            }
            if s.shutdown {
                return;
            }
            (s.generation, s.needle.clone())
        };
        let mut superseded = false;
        for page in 0..page_count {
            if needle.is_empty() {
                break;
            }
            let lines = cache[page].get_or_insert_with(|| {
                doc.as_ref()
                    .and_then(|d| page_to_stext(d, page, StextOptions::default()).ok())
                    .map(|st| index_page(&st))
                    .unwrap_or_default()
            });
            let hits = page_hits(lines, page, &needle, scale);
            let mut s = lock.lock().unwrap_or_else(|e| e.into_inner());
            if s.shutdown {
                return;
            }
            if s.generation != generation {
                superseded = true;
                break;
            }
            s.hits.extend(hits);
            s.pages_done = page + 1;
        }
        if !superseded {
            let mut s = lock.lock().unwrap_or_else(|e| e.into_inner());
            if s.generation == generation {
                // An empty needle searches nothing and is finished at once.
                s.pages_done = page_count;
                done_gen = generation;
            }
        }
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    /// The search loop exactly as `PdfReaderState::recompute_search` ran it
    /// before #1199 — synchronous `page_to_stext` per page and the old
    /// per-line `line_hits` — kept verbatim as the oracle the worker must
    /// match.
    pub(crate) fn old_synchronous_scan(
        doc: &PdfDocument,
        needle: &str,
        scale: f32,
    ) -> Vec<SearchHit> {
        fn line_hits(
            line: &kopitiam_pdf::mupdf::StextLine,
            needle: &str,
            scale: f32,
        ) -> Vec<(Pos2, Pos2)> {
            let chars: Vec<char> = line.chars.iter().map(|ch| ch.c).collect();
            substr_char_ranges(&chars, needle)
                .into_iter()
                .map(|(s, e)| {
                    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
                    for ch in &line.chars[s..e] {
                        for p in [ch.quad.ul, ch.quad.ur, ch.quad.ll, ch.quad.lr] {
                            x0 = x0.min(p.x);
                            y0 = y0.min(p.y);
                            x1 = x1.max(p.x);
                            y1 = y1.max(p.y);
                        }
                    }
                    (
                        Pos2::new(x0 * scale, y0 * scale),
                        Pos2::new(x1 * scale, y1 * scale),
                    )
                })
                .collect()
        }
        let mut hits = Vec::new();
        for page in 0..doc.page_count() {
            let Ok(stext) = page_to_stext(doc, page, StextOptions::default()) else {
                continue;
            };
            for block in &stext.blocks {
                let StextBlock::Text(tb) = block else {
                    continue;
                };
                for line in &tb.lines {
                    for (min, max) in line_hits(line, needle, scale) {
                        hits.push(SearchHit { page, min, max });
                    }
                }
            }
        }
        hits
    }

    /// A PDF of `pages.len()` letter pages, page `i` carrying the lines
    /// `pages[i]` in Helvetica, one line every 20 pt from the top.
    pub(crate) fn text_pdf(pages: &[&[&str]]) -> Vec<u8> {
        use lopdf::content::{Content, Operation};
        use lopdf::{dictionary, Document, Object as L, Stream};
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let font = doc.add_object(dictionary! {
            "Type" => "Font",
            "Subtype" => "Type1",
            "BaseFont" => "Helvetica",
        });
        let resources = doc.add_object(dictionary! {
            "Font" => dictionary! { "F1" => font },
        });
        let mut kids = Vec::new();
        for lines in pages {
            let mut ops = vec![
                Operation::new("BT", vec![]),
                Operation::new("Tf", vec!["F1".into(), 12.into()]),
                Operation::new("Td", vec![72.into(), 720.into()]),
            ];
            for (k, line) in lines.iter().enumerate() {
                if k > 0 {
                    ops.push(Operation::new("Td", vec![0.into(), (-20).into()]));
                }
                ops.push(Operation::new("Tj", vec![L::string_literal(*line)]));
            }
            ops.push(Operation::new("ET", vec![]));
            let content = Content { operations: ops };
            let stream = doc.add_object(Stream::new(dictionary! {}, content.encode().unwrap()));
            let page = doc.add_object(dictionary! {
                "Type" => "Page",
                "Parent" => pages_id,
                "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
                "Contents" => stream,
                "Resources" => resources,
            });
            kids.push(L::Reference(page));
        }
        let count = kids.len() as i64;
        doc.objects.insert(
            pages_id,
            L::Dictionary(dictionary! { "Type" => "Pages", "Kids" => kids, "Count" => count }),
        );
        let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
        doc.trailer.set("Root", catalog);
        let mut out = Vec::new();
        doc.save_to(&mut out).unwrap();
        out
    }

    pub(crate) const FIXTURE: &[&[&str]] = &[
        &["Reactor kinetics", "the delayed neutron fraction"],
        &["No match on this page"],
        &["Neutron flux and neutron current", "NEUTRON in capitals"],
        &["a final neutron"],
    ];

    /// Poll `worker` for `generation` until every page is searched,
    /// collecting hits the way the reader does. Panics after 30 s.
    pub(crate) fn wait_for(worker: &TextSearchWorker, generation: u64) -> Vec<SearchHit> {
        let start = Instant::now();
        let mut hits = Vec::new();
        loop {
            let p = worker
                .poll(generation, hits.len())
                .expect("generation superseded unexpectedly");
            hits.extend(p.new_hits.iter().copied());
            if p.finished() {
                return hits;
            }
            assert!(
                start.elapsed() < Duration::from_secs(30),
                "search never finished"
            );
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    /// The fixture has text the extractor finds, so the equality test below
    /// is not vacuously comparing two empty lists.
    #[test]
    fn the_fixture_has_hits_on_three_pages() {
        let doc = PdfDocument::open(text_pdf(FIXTURE)).unwrap();
        let hits = old_synchronous_scan(&doc, "neutron", 150.0 / 72.0);
        let pages: Vec<usize> = hits.iter().map(|h| h.page).collect();
        assert_eq!(pages, vec![0, 2, 2, 2, 3], "{hits:?}");
    }

    /// Same hits, same order, bit-identical boxes as the old synchronous
    /// scan, for several needles including one with no match and one that
    /// matches inside a word.
    #[test]
    fn worker_hits_equal_the_old_synchronous_scan() {
        let bytes = text_pdf(FIXTURE);
        let doc = PdfDocument::open(bytes.clone()).unwrap();
        let scale = 150.0 / 72.0;
        let worker = TextSearchWorker::spawn(bytes, FIXTURE.len(), scale).unwrap();
        for needle in ["neutron", "Neutron flux", "on", "zzz", "e"] {
            let generation = worker.submit(needle);
            let got = wait_for(&worker, generation);
            assert_eq!(
                got,
                old_synchronous_scan(&doc, needle, scale),
                "needle {needle:?}"
            );
        }
    }

    /// A query superseded by a newer one is never reported: polling the old
    /// generation returns `None`, and the newer one's hits are exactly its
    /// own.
    #[test]
    fn a_superseded_query_is_discarded() {
        let bytes = text_pdf(FIXTURE);
        let doc = PdfDocument::open(bytes.clone()).unwrap();
        let scale = 150.0 / 72.0;
        let worker = TextSearchWorker::spawn(bytes, FIXTURE.len(), scale).unwrap();
        let old = worker.submit("e");
        let new = worker.submit("kinetics");
        assert!(new > old);
        assert!(worker.poll(old, 0).is_none());
        let got = wait_for(&worker, new);
        assert_eq!(got, old_synchronous_scan(&doc, "kinetics", scale));
        assert_eq!(got.len(), 1);
        assert!(worker.poll(old, 0).is_none());
    }

    /// An empty needle finishes at once with no hits.
    #[test]
    fn an_empty_needle_finishes_with_no_hits() {
        let worker = TextSearchWorker::spawn(text_pdf(FIXTURE), FIXTURE.len(), 1.0).unwrap();
        let generation = worker.submit("");
        assert!(wait_for(&worker, generation).is_empty());
    }

    /// Unreadable bytes give no hits rather than a hung or panicking worker,
    /// as the old scan skipped pages it could not extract.
    #[test]
    fn unreadable_bytes_finish_with_no_hits() {
        let worker = TextSearchWorker::spawn(b"not a pdf".to_vec(), 3, 1.0).unwrap();
        let generation = worker.submit("neutron");
        assert!(wait_for(&worker, generation).is_empty());
    }

    /// Timing and equality on a real document (#1199). Ignored: it needs a
    /// PDF from the `reactor-literature` submodule, named by
    /// `KOVAN_SEARCH_BENCH_PDF`, and prints rather than gates timings.
    ///
    /// Methodology: for each prefix of the query (one per keystroke, as the
    /// search box saw them), time (a) the old synchronous scan on the UI
    /// thread and (b) on the worker, the time from `submit` to the last page
    /// published. (a) is what the UI thread blocked for per keystroke before
    /// the fix; after it the UI thread only pays `submit` + one `poll` per
    /// frame, timed as (c). Asserts the final hits equal the old scan.
    #[test]
    #[ignore = "needs KOVAN_SEARCH_BENCH_PDF pointing at a real PDF"]
    fn bench_search_on_a_real_pdf() {
        let Ok(path) = std::env::var("KOVAN_SEARCH_BENCH_PDF") else {
            eprintln!("KOVAN_SEARCH_BENCH_PDF not set; skipping");
            return;
        };
        let query = std::env::var("KOVAN_SEARCH_BENCH_QUERY").unwrap_or_else(|_| "reactor".into());
        let bytes = std::fs::read(&path).unwrap();
        let doc = PdfDocument::open(bytes.clone()).unwrap();
        let pages = doc.page_count();
        let scale = 150.0 / 72.0;
        eprintln!("{path}: {pages} pages, query {query:?}");
        let worker = TextSearchWorker::spawn(bytes, pages, scale).unwrap();
        let prefixes: Vec<String> = (1..=query.chars().count())
            .map(|n| query.chars().take(n).collect())
            .collect();
        for needle in &prefixes {
            let t = Instant::now();
            let old = old_synchronous_scan(&doc, needle, scale);
            let old_ms = t.elapsed().as_secs_f64() * 1e3;

            let t = Instant::now();
            let generation = worker.submit(needle);
            let submit_ms = t.elapsed().as_secs_f64() * 1e3;
            let t = Instant::now();
            let p = worker.poll(generation, 0);
            let poll_ms = t.elapsed().as_secs_f64() * 1e3;
            assert!(p.is_some());
            let t = Instant::now();
            let new = wait_for(&worker, generation);
            let worker_ms = t.elapsed().as_secs_f64() * 1e3;
            assert_eq!(new, old, "needle {needle:?}");
            eprintln!(
                "{needle:>12?}: hits {:>5}  old UI-thread {old_ms:9.1} ms  \
                 worker {worker_ms:9.1} ms  UI submit {submit_ms:.3} ms + poll {poll_ms:.3} ms",
                new.len()
            );
        }
    }
}
