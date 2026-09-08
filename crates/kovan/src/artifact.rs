//! **Artifacts** — the fine-grained research knowledge inside a paper's
//! Markdown.
//!
//! Implements §13–§20 of the Kovan redesign
//! ([GitHub issue #35](https://github.com/theodoreOnzGit/outram-park-backend/issues/35)).
//!
//! # What an artifact is
//!
//! > A **Markdown heading** immediately followed by a **fenced `toml` block
//! > containing `[kovan]`**.
//!
//! That is the whole rule (§13). There is no bespoke Markdown syntax and no
//! HTML-comment metadata, so the file stays an ordinary Markdown document that
//! GitHub, GitLab, a text editor and any coding agent can read without knowing
//! anything about Kovan.
//!
//! ````markdown
//! ## Graphite temperature assumption
//!
//! ```toml
//! [kovan]
//! id = "graphite-temperature-assumption"
//! kind = "annotation"
//! created = "2026-08-31T15:04:32+08:00"
//! modified = "2026-08-31T15:04:32+08:00"
//!
//! [source]
//! page = 87
//! region = [0.214, 0.341, 0.721, 0.508]
//!
//! [classification]
//! topics = ["htgrs/materials"]
//! ```
//!
//! Graphite temperature here appears to represent nominal operating conditions.
//! ````
//!
//! # An ordinary TOML fence stays ordinary
//!
//! §13 is explicit: "Ordinary TOML fences without `[kovan]` remain ordinary
//! code examples." A `toml` block with no `[kovan]` table is skipped in
//! silence — never a warning, never a problem. A research note that happens to
//! quote a configuration file must not become an artifact.
//!
//! # Parsing is total
//!
//! [`parse_document`] never fails as a whole. A block that *is* an artifact —
//! it has `[kovan]` — but whose metadata is malformed is reported in
//! [`ParsedDocument::problems`] while every well-formed artifact is still
//! returned. A wiki has to stay browsable when one note is broken; refusing to
//! open the whole paper because a single `created` timestamp is missing would
//! be worse than showing the rest and naming the fault.
//!
//! # The source document is implicit
//!
//! §15: an artifact does **not** repeat its paper's cite key. It lives inside
//! `papers/<citekey>/<citekey>.md`, so the containing directory already says
//! which document it belongs to. Only the *location within* that document is
//! recorded here.

use crate::entity::Classification;
use serde::{Deserialize, Serialize};

/// The Markdown heading depth that delimits one artifact from the next: a
/// single `#`.
///
/// Every artifact — the paper header included — starts at a `#` heading and
/// runs to the line before the next `#`. Deeper headings inside that span
/// are the operator's own prose structure, not boundaries (maintainer
/// direction, GH issue #35, 2026-09-08).
pub const ARTIFACT_LEVEL: u8 = 1;

/// The kinds of artifact §14 defines.
///
/// Deliberately a small vocabulary — §14: "Keep the vocabulary small until
/// dogfooding proves more types necessary." Adding a variant forces every
/// `match` to account for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactKind {
    /// The paper itself — the document's **first** artifact, whose heading
    /// is the citekey and whose body carries the BibTeX record in a
    /// ```latex fence plus whatever prose (Summary, abstract) follows.
    ///
    /// Making the header an artifact rather than a special case means one
    /// parser, one renderer and one span rule for the whole document
    /// (maintainer direction, GH issue #35, 2026-09-08). It is the one
    /// artifact [`crate::classify::delete_artifact_cascade`] refuses to
    /// delete: removing it would decapitate the paper.
    Paper,
    /// A researcher's free-standing note.
    Note,
    /// A remark anchored to a specific place in the source document.
    Annotation,
    /// A pointer to a section of the source with no text copied from it — §17,
    /// the copyright-conscious way to record that a passage matters.
    SourceReference,
    /// An equation, written as ordinary GFM math (§18).
    Formula,
    /// A table lifted from the source, body carried as CSV (§19).
    DigitisedTable,
    /// A curve read off a figure, body carried as CSV (§20).
    DigitisedGraph,
    /// A typed connector between two nodes — the artifact form of a
    /// [`crate::relation::UserRelation`]. Carries a `[relation]` table and
    /// no body (maintainer direction, GH issue #35, 2026-09-08: "I want all
    /// connectors, relationships to use the same artifact schema").
    Relation,
    /// A saved mindmap — its own metadata in the same `[kovan]`/TOML shape
    /// as every other artifact, rather than a private file format.
    Mindmap,
}

/// Errors from reading one artifact's metadata.
///
/// Every variant names the heading it occurred under, because that is how a
/// human finds the block in their editor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArtifactError {
    /// The `[kovan]` table is present but does not match the schema.
    Malformed {
        heading: String,
        line: usize,
        message: String,
    },
    /// A `[source]` anchor is self-contradictory or out of range.
    BadAnchor {
        heading: String,
        line: usize,
        message: String,
    },
}

impl std::fmt::Display for ArtifactError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Malformed {
                heading,
                line,
                message,
            } => {
                write!(f, "line {line} ({heading:?}): {message}")
            }
            Self::BadAnchor {
                heading,
                line,
                message,
            } => {
                write!(f, "line {line} ({heading:?}): {message}")
            }
        }
    }
}

impl std::error::Error for ArtifactError {}

/// A rectangle on a page, in **normalised** page coordinates (§15).
///
/// All four values are fractions of the page in `0.0..=1.0`, with the origin
/// at the top-left, ordered `[x0, y0, x1, y1]`. §15 is emphatic that these are
/// never screen pixels: a pixel rectangle is meaningless at a different zoom,
/// on a different display, or after the reader is replaced — and this project
/// is in the middle of replacing its reader (§24).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(from = "[f64; 4]", into = "[f64; 4]")]
pub struct Region {
    /// Left edge, `0.0..=1.0`.
    pub x0: f64,
    /// Top edge, `0.0..=1.0`.
    pub y0: f64,
    /// Right edge, `0.0..=1.0`.
    pub x1: f64,
    /// Bottom edge, `0.0..=1.0`.
    pub y1: f64,
}

impl From<[f64; 4]> for Region {
    fn from(v: [f64; 4]) -> Self {
        Self {
            x0: v[0],
            y0: v[1],
            x1: v[2],
            y1: v[3],
        }
    }
}

impl From<Region> for [f64; 4] {
    fn from(r: Region) -> Self {
        [r.x0, r.y0, r.x1, r.y1]
    }
}

impl Region {
    /// This rectangle in normalised page fractions, from a pixel rectangle
    /// on a page of size `w` x `h` pixels (§15: fractions of the page,
    /// origin top-left). Corners may be given in any order — they are
    /// sorted here.
    ///
    /// `None` for a degenerate page size (`w` or `h` non-positive) or a
    /// zero-area / out-of-range rectangle, i.e. exactly when the result
    /// would fail [`Region::is_valid`].
    ///
    /// The one place this normalisation lives: the PDF reader's crop and
    /// annotation paths and [`crate::classify`]'s legacy-section migration
    /// all go through it rather than repeating the arithmetic.
    pub fn from_pixels(min: (f32, f32), max: (f32, f32), w: f32, h: f32) -> Option<Self> {
        if w <= 0.0 || h <= 0.0 {
            return None;
        }
        let r = Self {
            x0: (min.0.min(max.0) / w) as f64,
            y0: (min.1.min(max.1) / h) as f64,
            x1: (min.0.max(max.0) / w) as f64,
            y1: (min.1.max(max.1) / h) as f64,
        };
        r.is_valid().then_some(r)
    }

    /// Whether every coordinate is in `0.0..=1.0` and the rectangle is
    /// non-degenerate (`x0 < x1`, `y0 < y1`).
    pub fn is_valid(&self) -> bool {
        let in_range = [self.x0, self.y0, self.x1, self.y1]
            .iter()
            .all(|v| v.is_finite() && (0.0..=1.0).contains(v));
        in_range && self.x0 < self.x1 && self.y0 < self.y1
    }
}

/// Where in the source document an artifact points (§15).
///
/// Three shapes are legal, and exactly one must be used:
/// a single `page`; a single `page` plus a `region` on it; or an inclusive
/// `pages = [start, end]` range.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct SourceAnchor {
    /// A single 1-based page number.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page: Option<u32>,
    /// An inclusive `[start, end]` page range (§15: "inclusive start/end").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pages: Option<[u32; 2]>,
    /// A rectangle on `page`. Meaningless without `page`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region: Option<Region>,
}

impl SourceAnchor {
    /// Check the §15 invariants.
    ///
    /// # Errors
    ///
    /// A message naming the specific violation: neither `page` nor `pages`;
    /// both at once; a `region` without a `page`; a region outside `0.0..=1.0`
    /// or with inverted edges; a page number of zero (pages are 1-based); or a
    /// range whose end precedes its start.
    pub fn validate(&self) -> Result<(), String> {
        match (self.page, self.pages) {
            (None, None) => return Err("[source] has neither `page` nor `pages`".into()),
            (Some(_), Some(_)) => {
                return Err("[source] has both `page` and `pages`; use one or the other".into())
            }
            _ => {}
        }
        if let Some(p) = self.page {
            if p == 0 {
                return Err("`page` is 0, but page numbers are 1-based".into());
            }
        }
        if let Some([start, end]) = self.pages {
            if start == 0 {
                return Err("`pages` starts at 0, but page numbers are 1-based".into());
            }
            if end < start {
                return Err(format!(
                    "`pages` range [{start}, {end}] ends before it starts"
                ));
            }
        }
        if let Some(region) = self.region {
            if self.page.is_none() {
                return Err(
                    "`region` needs a `page`; a rectangle spanning a page range is undefined"
                        .into(),
                );
            }
            if !region.is_valid() {
                return Err(format!(
                    "`region` {:?} is not a valid normalised rectangle \
                     (needs 0.0..=1.0, x0 < x1, y0 < y1)",
                    <[f64; 4]>::from(region)
                ));
            }
        }
        Ok(())
    }

    /// Whether this anchor covers 1-based `page`.
    ///
    /// This is the query §31's "Follow" synchronisation runs on every page
    /// change: when the reader shows page 87, the artifacts anchored there are
    /// the ones to highlight.
    pub fn covers_page(&self, page: u32) -> bool {
        if self.page == Some(page) {
            return true;
        }
        matches!(self.pages, Some([start, end]) if (start..=end).contains(&page))
    }

    /// The first page this anchor refers to, for ordering and for "jump to
    /// source".
    pub fn first_page(&self) -> Option<u32> {
        self.page.or(self.pages.map(|[start, _]| start))
    }
}

/// How a digitised table or graph was extracted (§19, §20).
///
/// Kovan writes this automatically; it is provenance, not a user preference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Extraction {
    /// e.g. `"pdf_native"`, `"ocr"`, `"manual_digitisation"`. Left as a free
    /// string rather than an enum: §36 defers the digitiser design, and
    /// freezing this vocabulary now would pre-empt that decision.
    pub method: String,
    /// The engine used, where one was, e.g. `"kopitiam-ocr"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engine: Option<String>,
    /// The figure's own identifier in the source, e.g. `"Fig 1."`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub figure: Option<String>,
    /// The x column's label, which is also the CSV's first header field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x_label: Option<String>,
    /// The y column's label, which is also the CSV's second header field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub y_label: Option<String>,
    /// The x-axis calibration in words, e.g.
    /// `"log scale, px 107.2 = 0.001 , px 358.7 = 1000"` — without it the
    /// numbers below cannot be re-derived from the figure, so it is
    /// provenance, not decoration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x_axis: Option<String>,
    /// The y-axis calibration in words. See [`Self::x_axis`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub y_axis: Option<String>,
    /// Who digitised it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub digitised_by: Option<String>,
    /// RFC 3339 timestamp of the digitisation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub digitised_at: Option<String>,
    /// Human-review state, e.g. `"UNREVIEWED — points not yet
    /// human-verified"`. Only a human may set this to a reviewed value —
    /// see the workspace digitiser rules.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub review: Option<String>,
}

impl Extraction {
    /// An [`Extraction`] with only the method and engine set — the shape
    /// used by non-digitiser paths (a table lifted natively from the PDF,
    /// say) that have no plot calibration to record.
    pub fn new(method: impl Into<String>, engine: Option<String>) -> Self {
        Self {
            method: method.into(),
            engine,
            figure: None,
            x_label: None,
            y_label: None,
            x_axis: None,
            y_axis: None,
            digitised_by: None,
            digitised_at: None,
            review: None,
        }
    }
}

/// The mandatory `[kovan]` table (§14).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactMeta {
    /// Stable identifier, unique within the paper. §40: this survives a
    /// heading being reworded, which is why the heading text is not the id.
    pub id: String,
    /// What this artifact is.
    pub kind: ArtifactKind,
    /// RFC 3339 timestamp of creation, e.g. `"2026-08-31T15:04:32+08:00"`.
    /// Kept as a string: Kovan never does arithmetic on it, and adding a
    /// date/time crate to compare two values it only ever displays and sorts
    /// would not earn its place.
    pub created: String,
    /// RFC 3339 timestamp of the last modification.
    pub modified: String,
    /// RFC 3339 timestamp of human review, when a human has reviewed it (§14).
    /// Absent means unreviewed — which for a digitised dataset is the only
    /// state a machine may ever write.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reviewed: Option<String>,
}

/// The full fenced-TOML payload of one artifact.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArtifactToml {
    /// The mandatory `[kovan]` table.
    pub kovan: ArtifactMeta,
    /// Where in the source it points (§15). Absent for a free-standing note.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<SourceAnchor>,
    /// Fine-grained classification (§16). Feeds the mindmap. Omitted from
    /// the written TOML when empty, so an unclassified note stays terse.
    #[serde(default, skip_serializing_if = "Classification::is_empty")]
    pub classification: Classification,
    /// Extraction provenance (§19, §20). Digitised kinds only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extraction: Option<Extraction>,
    /// User-authored relations to other nodes, **sourced from this
    /// artifact** (op-30um.1) — one `[[relation]]` table per
    /// [`crate::relation::UserRelation`], with the `source` half of that
    /// triple implicit (it is always the artifact this TOML belongs to).
    /// Omitted from the written TOML when empty, so an artifact with no
    /// hand-drawn connections stays exactly as terse as it was before this
    /// field existed. See `crate::relation`'s module docs for why this is
    /// the relation's on-disk home rather than a separate store.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relation: Option<crate::relation::RelationRecord>,
    /// Ids of the relation artifacts in the mindmap document that name this
    /// artifact at either end — the back half of a two-way reference.
    ///
    /// The relation artifact lives in `mindmap.md` and points *out* at the
    /// paper and artifact it came from; this list points *back* at it, so a
    /// reader holding either file can find the other without scanning the
    /// library (maintainer direction, GH issue #35, 2026-09-08: "they
    /// should reference each other").
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub connections: Vec<String>,
}

/// One artifact, as found in a Markdown document.
#[derive(Debug, Clone, PartialEq)]
pub struct Artifact {
    /// The heading text, exactly as written. Display only — the identity is
    /// [`ArtifactMeta::id`], so renaming a heading does not break links (§40).
    pub heading: String,
    /// Heading depth, 1 for `#` through 6 for `######`.
    pub level: u8,
    /// 1-based line of the heading, for "jump to it in the editor".
    pub line: usize,
    /// The parsed `[kovan]` payload.
    pub toml: ArtifactToml,
    /// Everything between the metadata fence and the next heading: prose, math
    /// (§18), and the CSV fence of a digitised table or graph (§19, §20).
    /// Verbatim, including any inner fences.
    pub body: String,
}

impl Artifact {
    /// The artifact's stable id.
    pub fn id(&self) -> &str {
        &self.toml.kovan.id
    }

    /// The artifact's kind.
    pub fn kind(&self) -> ArtifactKind {
        self.toml.kovan.kind
    }

    /// Whether a human has marked this reviewed (§14).
    pub fn is_reviewed(&self) -> bool {
        self.toml.kovan.reviewed.is_some()
    }

    /// The first fenced `csv` block in the body, if any — the payload of a
    /// digitised table (§19) or graph (§20).
    ///
    /// Returned verbatim, without parsing: the CSV is the researcher's data
    /// and this module's job is to locate it, not to interpret it.
    pub fn csv_block(&self) -> Option<&str> {
        // Sentinel-delimited (the current format) and bare-fence (written
        // before GH issue #35, 2026-09-08) bodies both resolve here: the
        // sentinels sit outside the fence, so scanning for the fence finds
        // the data either way.
        let mut rest = self.body.as_str();
        while let Some(open) = rest.find("```csv") {
            let after = &rest[open + "```csv".len()..];
            let after = after.strip_prefix('\n').unwrap_or(after);
            if let Some(close) = after.find("\n```") {
                return Some(&after[..close + 1]);
            }
            rest = after;
        }
        None
    }

    /// This artifact's CSV as it should be **exported to a `.csv` file**:
    /// the header row and the data rows, with any `#` provenance comment
    /// lines dropped.
    ///
    /// Bodies written since GH issue #35 (2026-09-08) carry no comments in
    /// the first place, so this is a no-op on them; it exists so an
    /// artifact saved in the older commented form still exports cleanly
    /// (maintainer direction: "only the data within backticks, nothing
    /// more").
    pub fn csv_export(&self) -> Option<String> {
        let block = self.csv_block()?;
        let data: Vec<&str> = block
            .lines()
            .filter(|l| !l.trim_start().starts_with('#'))
            .collect();
        (!data.is_empty()).then(|| {
            let mut s = data.join("\n");
            s.push('\n');
            s
        })
    }
}

/// The result of scanning one Markdown document.
///
/// Both fields are populated on every call — see the module docs on why
/// parsing is total.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ParsedDocument {
    /// Every well-formed artifact, in document order.
    pub artifacts: Vec<Artifact>,
    /// Blocks that declared `[kovan]` but could not be read. A block with no
    /// `[kovan]` table is an ordinary code example and never appears here.
    pub problems: Vec<ArtifactError>,
}

impl ParsedDocument {
    /// Look an artifact up by its stable id.
    pub fn get(&self, id: &str) -> Option<&Artifact> {
        self.artifacts.iter().find(|a| a.id() == id)
    }

    /// Every artifact anchored to 1-based `page` — §31's "Follow" query.
    pub fn anchored_to_page(&self, page: u32) -> Vec<&Artifact> {
        self.artifacts
            .iter()
            .filter(|a| a.toml.source.as_ref().is_some_and(|s| s.covers_page(page)))
            .collect()
    }
}

/// 1-based line number of `byte_offset` within `text`.
fn line_of(text: &str, byte_offset: usize) -> usize {
    text[..byte_offset.min(text.len())]
        .bytes()
        .filter(|b| *b == b'\n')
        .count()
        + 1
}

/// Scan a Markdown document for Kovan artifacts (§13).
///
/// Recognises a heading immediately followed by a fenced `toml` block whose
/// content parses as TOML and contains a `[kovan]` table. "Immediately" means
/// the fence is the next block in the document — blank lines between them are
/// Markdown whitespace and do not count as intervening content.
///
/// Never fails: see [`ParsedDocument`] and the module docs.
pub fn parse_document(markdown: &str) -> ParsedDocument {
    use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};

    let mut out = ParsedDocument::default();

    // (heading text, level, byte offset of the heading)
    let mut pending: Option<(String, u8, usize)> = None;
    let mut heading_text = String::new();
    let mut in_heading = false;
    let mut heading_start = 0usize;
    let mut heading_level = 1u8;

    // Set while inside a fenced `toml` block that directly follows a heading.
    let mut toml_buf: Option<String> = None;
    let mut fence_end = 0usize;

    let parser = Parser::new_ext(
        markdown,
        Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH,
    );
    for (event, range) in parser.into_offset_iter() {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                // Only a level-1 heading closes the previous artifact: `##`
                // and below are the operator's own subdivisions *inside* an
                // artifact's prose and belong to its body (maintainer
                // direction, GH issue #35, 2026-09-08 — "can be subdivided
                // by ## or ### or any markdown ... till the next artifact
                // denoted by #").
                if level as u8 == ARTIFACT_LEVEL {
                    if let Some(a) = out.artifacts.last_mut() {
                        if a.body.is_empty() && fence_end > 0 && fence_end <= range.start {
                            a.body = markdown[fence_end..range.start].trim().to_string();
                        }
                    }
                }
                in_heading = true;
                heading_text.clear();
                heading_start = range.start;
                heading_level = level as u8;
            }
            Event::Text(t) | Event::Code(t) if in_heading => heading_text.push_str(&t),
            Event::End(TagEnd::Heading(_)) => {
                in_heading = false;
                // Only a level-1 heading can open an artifact, so a `##`
                // followed by a ```toml fence stays ordinary prose.
                pending = (heading_level == ARTIFACT_LEVEL).then(|| {
                    (
                        heading_text.trim().to_string(),
                        heading_level,
                        heading_start,
                    )
                });
            }
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(lang)))
                if pending.is_some() && lang.split(',').next().unwrap_or("").trim() == "toml" =>
            {
                toml_buf = Some(String::new());
            }
            Event::Text(t) if toml_buf.is_some() => {
                if let Some(buf) = toml_buf.as_mut() {
                    buf.push_str(&t);
                }
            }
            Event::End(TagEnd::CodeBlock) => {
                if let (Some(buf), Some((heading, level, offset))) =
                    (toml_buf.take(), pending.take())
                {
                    let line = line_of(markdown, offset);
                    // §13: no `[kovan]` table means an ordinary code example.
                    // Checked structurally, not by substring match, so a
                    // string value containing "[kovan]" is not mistaken for one.
                    let is_artifact = toml::from_str::<toml::Value>(&buf)
                        .ok()
                        .and_then(|v| v.get("kovan").cloned())
                        .is_some();
                    if is_artifact {
                        match toml::from_str::<ArtifactToml>(&buf) {
                            Ok(parsed) => {
                                if let Some(anchor) = &parsed.source {
                                    if let Err(message) = anchor.validate() {
                                        out.problems.push(ArtifactError::BadAnchor {
                                            heading: heading.clone(),
                                            line,
                                            message,
                                        });
                                        continue;
                                    }
                                }
                                fence_end = range.end;
                                out.artifacts.push(Artifact {
                                    heading,
                                    level,
                                    line,
                                    toml: parsed,
                                    body: String::new(),
                                });
                            }
                            Err(e) => out.problems.push(ArtifactError::Malformed {
                                heading,
                                line,
                                message: e.to_string(),
                            }),
                        }
                    }
                }
            }
            // Any other block between a heading and a fence breaks the
            // "immediately followed by" rule (§13).
            Event::Start(Tag::Paragraph)
            | Event::Start(Tag::BlockQuote(_))
            | Event::Start(Tag::List(_))
            | Event::Start(Tag::Table(_))
                if toml_buf.is_none() =>
            {
                pending = None;
            }
            _ => {}
        }
    }

    // The final artifact's body runs to the end of the document.
    if let Some(a) = out.artifacts.last_mut() {
        if a.body.is_empty() && fence_end > 0 && fence_end <= markdown.len() {
            a.body = markdown[fence_end..].trim().to_string();
        }
    }

    out
}

/// Render `heading`/`toml`/`body` as the Markdown block §13 defines:
/// heading, immediately followed by a fenced `toml` block, followed by the
/// body. The exact counterpart to [`parse_document`] — text produced here
/// re-parses to an equivalent [`Artifact`] (see the round-trip test below).
///
/// `level` is the heading depth, 1 for `#` through 6 for `######` — same
/// meaning as [`Artifact::level`].
///
/// # Errors
///
/// Only if `toml`'s own TOML serialisation fails, which cannot happen for
/// its field types (see [`ArtifactToml`]'s fields) — the `Result` spares
/// callers an `unwrap`.
/// Wrap `csv_data` — the header row plus data rows, and nothing else — as a
/// sentinel-delimited fenced CSV body ready for
/// [`render_artifact_block`].
///
/// The provenance that used to sit in `#` comments inside the fence belongs
/// in the artifact's `[extraction]` table instead (see [`Extraction`]), so
/// what lands between the backticks is exactly what a spreadsheet would
/// read.
pub fn render_csv_body(csv_data: &str) -> String {
    format!("```csv\n{}\n```\n", csv_data.trim_end())
}

/// Wrap `latex` — a BibTeX record or a formula — as a fenced ```latex
/// block, the schema's third block type alongside ```toml (metadata) and
/// ```csv (data).
pub fn render_latex_body(latex: &str) -> String {
    format!("```latex\n{}\n```\n", latex.trim_end())
}

pub fn render_artifact_block(
    level: u8,
    heading: &str,
    payload: &ArtifactToml,
    body: &str,
) -> Result<String, String> {
    let level = level.clamp(1, 6);
    let hashes = "#".repeat(level as usize);
    let toml_text = toml::to_string_pretty(payload).map_err(|e| e.to_string())?;
    let mut out = format!("{hashes} {heading}\n\n```toml\n{toml_text}```\n");
    let body = body.trim();
    if !body.is_empty() {
        out.push('\n');
        out.push_str(body);
        out.push('\n');
    }
    Ok(out)
}

/// The 0-based, end-exclusive line range of `artifact`'s whole Markdown
/// block in `md` — its heading line through everything up to (but not
/// including) the next Markdown heading of depth `<= artifact.level`, or the
/// end of the document.
///
/// This is the span [`render_artifact_block`]'s output occupies once
/// inserted, so it is what an in-place edit (`crate::classify::
/// replace_artifact_body`) and a "which block is this line in?" hit test
/// operate on.
pub fn block_span(md: &str, artifact: &Artifact) -> std::ops::Range<usize> {
    let start = artifact.line.saturating_sub(1);
    let lines: Vec<&str> = md.lines().collect();
    let mut end = lines.len();
    // Fence tracking is not optional here. A `#` at the start of a line
    // inside a ```csv or ```toml block is data or a TOML comment, not a
    // heading — and a digitised artifact's CSV used to begin with
    // `# kovan digitiser dataset (schema v1)`. Without this, deleting such
    // an artifact truncated its span at that line and left the rest of the
    // block orphaned in the document: the "delete annotation isn't working
    // properly" report (GH issue #35, 2026-09-08).
    let mut in_fence = false;
    for (i, line) in lines.iter().enumerate().skip(start + 1) {
        if line.trim_start().starts_with("```") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        let hashes = line.chars().take_while(|c| *c == '#').count();
        if hashes >= 1 && hashes <= artifact.level as usize && line.chars().nth(hashes) == Some(' ')
        {
            end = i;
            break;
        }
    }
    start..end
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------
    // Sentinel-delimited CSV bodies (GH issue #35, 2026-09-08).
    // -----------------------------------------------------------------

    #[test]
    fn render_csv_body_is_a_bare_fence_with_only_the_data_inside() {
        let body = render_csv_body("time (s),power (%)\n0,1\n");
        let lines: Vec<&str> = body.lines().collect();
        assert_eq!(lines[0], "```csv");
        assert_eq!(lines[1], "time (s),power (%)");
        assert_eq!(lines[2], "0,1");
        assert_eq!(lines[3], "```");
        // No HTML sentinels: the `#` heading delimits the artifact and the
        // fence language identifies the block (GH issue #35, 2026-09-08).
        assert!(!body.contains("<!--"));
    }

    #[test]
    fn a_sentinel_wrapped_body_round_trips_through_the_parser() {
        let toml = ArtifactToml {
            kovan: ArtifactMeta {
                id: "fig-1".into(),
                kind: ArtifactKind::DigitisedGraph,
                created: "t".into(),
                modified: "t".into(),
                reviewed: None,
            },
            source: None,
            classification: Classification::default(),
            extraction: Some(Extraction::new("manual_digitisation", None)),
            relation: None,
            connections: Vec::new(),
        };
        let md =
            render_artifact_block(ARTIFACT_LEVEL, "Fig 1.", &toml, &render_csv_body("a,b\n1,2\n"))
                .unwrap();
        let doc = parse_document(&md);
        let art = doc.get("fig-1").expect("artifact parses");
        assert_eq!(art.csv_block().unwrap().trim(), "a,b\n1,2");
        assert_eq!(art.csv_export().unwrap(), "a,b\n1,2\n");
        assert!(!art.body.contains("<!--"));
    }

    #[test]
    fn csv_export_strips_the_comment_lines_of_an_older_body() {
        // Bodies written before this format carried `#` provenance inside
        // the fence; exporting one must still yield data only.
        let art = Artifact {
            heading: "Fig 1.".into(),
            level: ARTIFACT_LEVEL,
            line: 1,
            toml: ArtifactToml {
                kovan: ArtifactMeta {
                    id: "fig-1".into(),
                    kind: ArtifactKind::DigitisedGraph,
                    created: "t".into(),
                    modified: "t".into(),
                    reviewed: None,
                },
                source: None,
                classification: Classification::default(),
                extraction: None,
                relation: None,
            connections: Vec::new(),
            },
            body: "```csv\n# kovan digitiser dataset (schema v1)\n# figure: Fig 1.\nx,y\n1,2\n```\n".into(),
        };
        assert_eq!(art.csv_export().unwrap(), "x,y\n1,2\n");
    }

    /// The annotation example from §13, verbatim.
    const ANNOTATION: &str = r#"
# Graphite temperature assumption

```toml
[kovan]
id = "graphite-temperature-assumption"
kind = "annotation"
created = "2026-08-31T15:04:32+08:00"
modified = "2026-08-31T15:04:32+08:00"

[source]
page = 87
region = [0.214, 0.341, 0.721, 0.508]

[classification]
topics = ["htgrs/materials"]
projects = ["outram-park"]
```

Graphite temperature here appears to represent nominal operating conditions.
"#;

    #[test]
    fn parses_the_annotation_example_from_the_issue() {
        let doc = parse_document(ANNOTATION);
        assert!(doc.problems.is_empty(), "{:?}", doc.problems);
        assert_eq!(doc.artifacts.len(), 1);

        let a = &doc.artifacts[0];
        assert_eq!(a.heading, "Graphite temperature assumption");
        assert_eq!(a.level, ARTIFACT_LEVEL);
        assert_eq!(a.id(), "graphite-temperature-assumption");
        assert_eq!(a.kind(), ArtifactKind::Annotation);
        assert!(!a.is_reviewed());

        let anchor = a.toml.source.as_ref().unwrap();
        assert_eq!(anchor.page, Some(87));
        let r = anchor.region.unwrap();
        assert!((r.x0 - 0.214).abs() < 1e-12 && (r.y1 - 0.508).abs() < 1e-12);

        assert_eq!(a.toml.classification.topics, vec!["htgrs/materials"]);
        assert_eq!(a.toml.classification.projects, vec!["outram-park"]);
        assert!(
            a.body.contains("nominal operating conditions"),
            "{:?}",
            a.body
        );
    }

    #[test]
    fn an_ordinary_toml_fence_is_not_an_artifact_and_is_not_a_problem() {
        // §13: "Ordinary TOML fences without [kovan] remain ordinary code
        // examples." Silently skipped — not a warning.
        let md = r#"
## How to configure the solver

```toml
[solver]
tolerance = 1e-8
```
"#;
        let doc = parse_document(md);
        assert!(doc.artifacts.is_empty());
        assert!(doc.problems.is_empty(), "{:?}", doc.problems);
    }

    #[test]
    fn a_string_value_mentioning_kovan_does_not_make_an_artifact() {
        // The [kovan] check is structural, not a substring match.
        let md = r#"
# Example

```toml
note = "the header looks like [kovan] but is not one"
```
"#;
        let doc = parse_document(md);
        assert!(doc.artifacts.is_empty(), "{:?}", doc.artifacts);
        assert!(doc.problems.is_empty(), "{:?}", doc.problems);
    }

    #[test]
    fn a_paragraph_between_heading_and_fence_breaks_the_immediately_followed_rule() {
        let md = r#"
# Not an artifact

Some prose sits in between.

```toml
[kovan]
id = "x"
kind = "note"
created = "2026-08-31T00:00:00+08:00"
modified = "2026-08-31T00:00:00+08:00"
```
"#;
        let doc = parse_document(md);
        assert!(doc.artifacts.is_empty(), "{:?}", doc.artifacts);
    }

    #[test]
    fn blank_lines_between_heading_and_fence_are_fine() {
        // Blank lines are Markdown whitespace, not intervening content.
        let md = "# Heading\n\n\n\n```toml\n[kovan]\nid = \"x\"\nkind = \"note\"\ncreated = \"c\"\nmodified = \"m\"\n```\n";
        let doc = parse_document(md);
        assert_eq!(doc.artifacts.len(), 1, "{:?}", doc.problems);
    }

    #[test]
    fn several_artifacts_are_returned_in_document_order_with_their_own_bodies() {
        let md = r#"
# First

```toml
[kovan]
id = "first"
kind = "note"
created = "c"
modified = "m"
```

body of first

# Second

```toml
[kovan]
id = "second"
kind = "formula"
created = "c"
modified = "m"
```

body of second
"#;
        let doc = parse_document(md);
        assert_eq!(doc.artifacts.len(), 2, "{:?}", doc.problems);
        assert_eq!(doc.artifacts[0].id(), "first");
        assert_eq!(doc.artifacts[1].id(), "second");
        assert_eq!(doc.artifacts[0].body.trim(), "body of first");
        assert_eq!(doc.artifacts[1].body.trim(), "body of second");
        assert_eq!(doc.get("second").unwrap().kind(), ArtifactKind::Formula);
    }

    #[test]
    fn one_malformed_artifact_does_not_lose_the_others() {
        // A wiki must stay browsable when one note is broken.
        let md = r#"
# Broken

```toml
[kovan]
id = "broken"
```

# Fine

```toml
[kovan]
id = "fine"
kind = "note"
created = "c"
modified = "m"
```
"#;
        let doc = parse_document(md);
        assert_eq!(doc.artifacts.len(), 1);
        assert_eq!(doc.artifacts[0].id(), "fine");
        assert_eq!(doc.problems.len(), 1);
        match &doc.problems[0] {
            ArtifactError::Malformed { heading, .. } => assert_eq!(heading, "Broken"),
            other => panic!("expected Malformed, got {other:?}"),
        }
    }

    #[test]
    fn digitised_table_example_from_the_issue_keeps_its_csv_and_provenance() {
        let md = r#"
# Table 4.4 — Core component materials

```toml
[kovan]
id = "table-4-4"
kind = "digitised_table"
created = "2026-08-31T15:20:11+08:00"
modified = "2026-08-31T15:23:54+08:00"
reviewed = "2026-08-31T15:23:54+08:00"

[source]
page = 87
region = [0.126, 0.308, 0.874, 0.681]

[classification]
topics = ["htgrs/materials", "htgrs/materials/graphite-properties"]

[extraction]
method = "pdf_native"
```

```csv
component,density_g_cm3,material,temperature_K
fuel,1.75,graphite matrix,900
reflector,1.76,graphite,900
```
"#;
        let doc = parse_document(md);
        assert!(doc.problems.is_empty(), "{:?}", doc.problems);
        let a = &doc.artifacts[0];
        assert_eq!(a.kind(), ArtifactKind::DigitisedTable);
        assert!(a.is_reviewed());
        assert_eq!(a.toml.extraction.as_ref().unwrap().method, "pdf_native");
        assert_eq!(a.toml.classification.topics.len(), 2);

        let csv = a.csv_block().expect("csv block");
        assert!(csv.starts_with("component,density_g_cm3"), "{csv:?}");
        assert!(csv.contains("reflector,1.76,graphite,900"), "{csv:?}");
        assert!(!csv.contains("```"), "{csv:?}");
    }

    #[test]
    fn source_reference_needs_no_body_at_all() {
        // §17: classify a section without reproducing any of its text.
        let md = r#"
# Coupled neutronics methodology

```toml
[kovan]
id = "coupled-neutronics-methodology"
kind = "source_reference"
created = "c"
modified = "m"

[source]
pages = [42, 48]

[classification]
topics = ["htgrs/neutronics"]
```
"#;
        let doc = parse_document(md);
        let a = &doc.artifacts[0];
        assert_eq!(a.kind(), ArtifactKind::SourceReference);
        assert_eq!(a.body, "");
        assert_eq!(a.toml.source.as_ref().unwrap().pages, Some([42, 48]));
        assert!(a.csv_block().is_none());
    }

    // -------------------------------------------------------------------
    // Anchors — §15
    // -------------------------------------------------------------------

    fn anchor(
        page: Option<u32>,
        pages: Option<[u32; 2]>,
        region: Option<[f64; 4]>,
    ) -> SourceAnchor {
        SourceAnchor {
            page,
            pages,
            region: region.map(Region::from),
        }
    }

    #[test]
    fn a_valid_anchor_is_one_page_a_page_plus_region_or_a_range() {
        assert!(anchor(Some(87), None, None).validate().is_ok());
        assert!(anchor(Some(87), None, Some([0.1, 0.2, 0.8, 0.9]))
            .validate()
            .is_ok());
        assert!(anchor(None, Some([42, 48]), None).validate().is_ok());
        // §15's range is inclusive, so a single-page range is legal.
        assert!(anchor(None, Some([42, 42]), None).validate().is_ok());
    }

    #[test]
    fn contradictory_and_out_of_range_anchors_are_rejected() {
        assert!(
            anchor(None, None, None).validate().is_err(),
            "neither page nor pages"
        );
        assert!(
            anchor(Some(1), Some([1, 2]), None).validate().is_err(),
            "both"
        );
        assert!(
            anchor(Some(0), None, None).validate().is_err(),
            "pages are 1-based"
        );
        assert!(
            anchor(None, Some([0, 2]), None).validate().is_err(),
            "pages are 1-based"
        );
        assert!(
            anchor(None, Some([9, 2]), None).validate().is_err(),
            "end before start"
        );
        assert!(
            anchor(None, Some([1, 2]), Some([0.1, 0.2, 0.8, 0.9]))
                .validate()
                .is_err(),
            "region without page"
        );
    }

    #[test]
    fn a_region_must_be_normalised_and_non_degenerate() {
        // §15: values are fractions of the page, never screen pixels.
        assert!(anchor(Some(1), None, Some([0.0, 0.0, 1.0, 1.0]))
            .validate()
            .is_ok());
        assert!(
            anchor(Some(1), None, Some([12.0, 30.0, 87.0, 68.0]))
                .validate()
                .is_err(),
            "pixels"
        );
        assert!(
            anchor(Some(1), None, Some([-0.1, 0.2, 0.8, 0.9]))
                .validate()
                .is_err(),
            "negative"
        );
        assert!(
            anchor(Some(1), None, Some([0.8, 0.2, 0.1, 0.9]))
                .validate()
                .is_err(),
            "x inverted"
        );
        assert!(
            anchor(Some(1), None, Some([0.1, 0.9, 0.8, 0.2]))
                .validate()
                .is_err(),
            "y inverted"
        );
        assert!(
            anchor(Some(1), None, Some([0.5, 0.2, 0.5, 0.9]))
                .validate()
                .is_err(),
            "zero width"
        );
    }

    #[test]
    fn a_bad_anchor_is_reported_and_the_artifact_is_not_returned() {
        let md = r#"
# Bad anchor

```toml
[kovan]
id = "bad"
kind = "annotation"
created = "c"
modified = "m"

[source]
page = 87
region = [12.0, 30.0, 87.0, 68.0]
```
"#;
        let doc = parse_document(md);
        assert!(doc.artifacts.is_empty());
        assert_eq!(doc.problems.len(), 1);
        assert!(
            matches!(doc.problems[0], ArtifactError::BadAnchor { .. }),
            "{:?}",
            doc.problems
        );
    }

    #[test]
    fn covers_page_answers_the_follow_query() {
        // §31: when the reader shows page 87, which artifacts live there?
        let single = anchor(Some(87), None, None);
        assert!(single.covers_page(87));
        assert!(!single.covers_page(86));

        let range = anchor(None, Some([42, 48]), None);
        assert!(range.covers_page(42) && range.covers_page(45) && range.covers_page(48));
        assert!(!range.covers_page(41) && !range.covers_page(49));

        assert_eq!(single.first_page(), Some(87));
        assert_eq!(range.first_page(), Some(42));
        assert_eq!(anchor(None, None, None).first_page(), None);
    }

    #[test]
    fn anchored_to_page_selects_across_a_document() {
        let md = r#"
# On 87

```toml
[kovan]
id = "on-87"
kind = "note"
created = "c"
modified = "m"

[source]
page = 87
```

# Spanning 42 to 48

```toml
[kovan]
id = "spanning"
kind = "source_reference"
created = "c"
modified = "m"

[source]
pages = [42, 48]
```

# Unanchored

```toml
[kovan]
id = "floating"
kind = "note"
created = "c"
modified = "m"
```
"#;
        let doc = parse_document(md);
        assert_eq!(doc.artifacts.len(), 3, "{:?}", doc.problems);

        let on87: Vec<_> = doc.anchored_to_page(87).iter().map(|a| a.id()).collect();
        assert_eq!(on87, vec!["on-87"]);

        let on45: Vec<_> = doc.anchored_to_page(45).iter().map(|a| a.id()).collect();
        assert_eq!(on45, vec!["spanning"]);

        // An artifact with no [source] is anchored to nothing.
        assert!(doc.anchored_to_page(1).is_empty());
    }

    #[test]
    fn region_round_trips_through_its_four_element_array_form() {
        let md = "[kovan]\nid = \"x\"\nkind = \"note\"\ncreated = \"c\"\nmodified = \"m\"\n\n[source]\npage = 5\nregion = [0.1, 0.2, 0.3, 0.4]\n";
        let parsed: ArtifactToml = toml::from_str(md).unwrap();
        let text = toml::to_string_pretty(&parsed).unwrap();
        // Serialised as a 4-element array (whitespace is toml's business),
        // and the values survive the trip unchanged.
        let back: ArtifactToml = toml::from_str(&text).unwrap();
        assert_eq!(back, parsed);
        let r = back.source.unwrap().region.unwrap();
        assert_eq!(<[f64; 4]>::from(r), [0.1, 0.2, 0.3, 0.4]);
        // An artifact with no classification does not carry an empty table.
        assert!(!text.contains("[classification]"), "{text}");
    }

    #[test]
    fn line_numbers_point_at_the_heading() {
        let md = "intro\n\n# Second line block\n\n```toml\n[kovan]\nid = \"x\"\nkind = \"note\"\ncreated = \"c\"\nmodified = \"m\"\n```\n";
        let doc = parse_document(md);
        assert_eq!(doc.artifacts[0].line, 3);
    }

    #[test]
    fn an_empty_document_yields_nothing_and_does_not_panic() {
        assert_eq!(parse_document(""), ParsedDocument::default());
        assert_eq!(
            parse_document("# Just a title\n\nprose only\n"),
            ParsedDocument::default()
        );
    }

    #[test]
    fn block_span_covers_the_heading_through_the_body() {
        // `##` subheadings belong to the artifact's prose; only the next
        // `#` ends the span (GH issue #35, 2026-09-08).
        let md = "# First\n\n```toml\n[kovan]\nid = \"first\"\nkind = \"note\"\ncreated = \"c\"\nmodified = \"m\"\n```\n\nbody one\n\n## a subheading\n\nstill body one\n\n# Second\n\nbody two\n";
        let doc = parse_document(md);
        let a = doc.get("first").unwrap();
        let span = block_span(md, a);
        assert_eq!(span.start, a.line - 1);
        assert_eq!(md.lines().nth(span.end), Some("# Second"));
        // The subheading is inside the span, not a boundary.
        assert!(md.lines().take(span.end).any(|l| l == "## a subheading"));
    }

    #[test]
    fn block_span_of_the_last_block_runs_to_eof() {
        let md = "# Only\n\n```toml\n[kovan]\nid = \"only\"\nkind = \"note\"\ncreated = \"c\"\nmodified = \"m\"\n```\n\ntail\n";
        let doc = parse_document(md);
        let a = doc.get("only").unwrap();
        assert_eq!(block_span(md, a).end, md.lines().count());
    }

    #[test]
    fn render_artifact_block_round_trips_through_parse_document() {
        let payload = ArtifactToml {
            kovan: ArtifactMeta {
                id: "coupled-neutronics-methodology".to_string(),
                kind: ArtifactKind::SourceReference,
                created: "2026-08-31T15:10:12+08:00".to_string(),
                modified: "2026-08-31T15:10:12+08:00".to_string(),
                reviewed: None,
            },
            source: Some(SourceAnchor {
                page: None,
                pages: Some([42, 48]),
                region: None,
            }),
            classification: Classification {
                topics: vec!["htgrs/neutronics".to_string()],
                projects: vec![],
            },
            extraction: None,
            relation: None,
            connections: Vec::new(),
        };
        let rendered =
            render_artifact_block(ARTIFACT_LEVEL, "Coupled neutronics methodology", &payload, "").unwrap();

        let doc = parse_document(&rendered);
        assert!(doc.problems.is_empty(), "{:?}", doc.problems);
        assert_eq!(doc.artifacts.len(), 1);
        let a = &doc.artifacts[0];
        assert_eq!(a.heading, "Coupled neutronics methodology");
        assert_eq!(a.level, ARTIFACT_LEVEL);
        assert_eq!(a.toml, payload);
    }

    #[test]
    fn render_artifact_block_keeps_a_non_empty_body() {
        let payload = ArtifactToml {
            kovan: ArtifactMeta {
                id: "a-note".to_string(),
                kind: ArtifactKind::Note,
                created: "c".to_string(),
                modified: "m".to_string(),
                reviewed: None,
            },
            source: None,
            classification: Classification::default(),
            extraction: None,
            relation: None,
            connections: Vec::new(),
        };
        let rendered =
            render_artifact_block(ARTIFACT_LEVEL, "A note", &payload, "Some prose about it.").unwrap();
        let doc = parse_document(&rendered);
        assert_eq!(doc.artifacts[0].body, "Some prose about it.");
    }
}
