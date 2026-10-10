//! **The review wizard's question set and the stamp gate** (GitHub #769;
//! wizard rules decided on #740, maintainer 2026-10-07).
//!
//! The questions live in `data/review_wizard.toml`, embedded with
//! `include_str!` ([`WIZARD_TOML`]) so desktop kovan, web-kovan (wasm) and CI
//! read the same set. Each question carries its source clauses in the
//! kovan standard corpus, exactly as recorded on #769, or names the
//! workspace rule it rests on.
//!
//! # Answers on disk
//!
//! `review.md`'s `[review.checklist]` maps a question key to an answer:
//!
//! ```toml
//! [review.checklist]
//! doc_matches_behaviour = "yes"
//! error_handling = "panics_justified: the table is compiled in and tested"
//! vv_evidence = "analytical_case"
//! ```
//!
//! An answer is an option key, or `"<option key>: <text>"` for an option
//! that `requires_text` (`Other: ____`, a justification); the text needs at
//! least two characters (#740 U3).
//!
//! # The gate
//!
//! [`stamp_gate`] is a pure function of the answers and of the context
//! ([`Applicability`]: which conditional questions apply, and what git says
//! about who wrote the tests reaching the function):
//!
//! ```text
//!   answers ─┬─ unknown / not-applicable / bad text ─────────────────> blocked_by: Invalid
//!            ├─ applicable question unanswered ──────────────────────> blocked_by: Unanswered
//!            ├─ option effect = block ───────────────────────────────> blocked_by: Answer
//!            ├─ option effect = prompt_needs_fix ────────────────────> prompts ("Mark as Needs fix instead?")
//!            ├─ option effect = flag ────────────────────────────────> flags (needs improvement; never blocks)
//!            ├─ rung4_allowed = some gate_rung4 answer (vv_evidence)
//!            │    AND a gate_rung4_author answer (vv_case_author =
//!            │    human_wrote_and_verified; added 2026-10-07)
//!            │    (~~AND no no_rung4 answer (self-check)~~ CORRECTED
//!            │     2026-10-07, maintainer on #769: independence gates
//!            │     rung 5, not rung 4)
//!            ├─ independent = independence answered, no not_independent
//!            │    answer (self-check / other): ~~may be rung 5's second
//!            │    review~~ CORRECTED 2026-10-08 (#809): one of rung 5's
//!            │    conditions; the rest (a separate organisation, a signed
//!            │    separation attestation) are registry records, judged by
//!            │    `super::ivv`, not answers
//!            └─ rung = derived_rung(answers, git): 4 when rung4_allowed AND
//!                 git shows no agent trailer on the reaching tests' commits,
//!                 else 3. The reviewer never chooses it (maintainer, #769,
//!                 2026-10-07). ~~rung = rung_4 while !rung4_allowed ->
//!                 blocked_by: Rung4NotOpen~~ CORRECTED 2026-10-07: the
//!                 `rung` question is gone, and a `rung` answer is refused
//!                 (~~`AnswerError::RungIsDerived`~~ **CORRECTED
//!                 2026-10-10** (#825): as an unknown question).
//! ```
//!
//! # The #764 placeholder keys
//!
//! #764 wrote `q1` … `q10` as placeholders. ~~They are rejected cleanly with
//! `AnswerError::LegacyPlaceholderKey`, naming the key that replaces each
//! one (`LEGACY_PLACEHOLDER_KEYS`)~~ **CORRECTED 2026-10-10** (#825): no
//! review was ever written with them, so they are now simply unknown keys
//! ([`AnswerError::UnknownQuestion`]), as is `q10` or `rung`. They are never
//! mapped: a placeholder answer (`"yes"`) is not an answer to the new
//! questions. A `review.md` holding them still parses (the entry is
//! readable); only the wizard refuses them.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use super::types::AuthorshipKind;

/// The question set, embedded at compile time (works in wasm).
pub const WIZARD_TOML: &str = include_str!("../../data/review_wizard.toml");

/// The rung a stamp gives, derived, never chosen (maintainer, #769,
/// 2026-10-07). Rung 5 is not a stamp's rung: ~~it is two independent
/// stamps~~ **CORRECTED 2026-10-08** (#809) it is IV&V, derived per function
/// by the staleness engine ([`super::ivv`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Rung {
    /// Human reviewed.
    Three,
    /// Human V&V: a qualifying V&V case written and verified by hand,
    /// without AI agents, and git agrees about the writing.
    Four,
}

impl Rung {
    pub fn as_u8(self) -> u8 {
        match self {
            Self::Three => 3,
            Self::Four => 4,
        }
    }
}

/// What git says about who wrote the tests reaching the function (the
/// commits that added them, read with [`super::types::agent_trailer`]).
/// Passed in as data.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TestAuthorship {
    /// No commit carries the agent trailer.
    Human,
    /// At least one commit does.
    Agent,
    /// Not known: no reaching test, or no commit facts.
    #[default]
    Unknown,
}

impl TestAuthorship {
    /// From the messages of the commits that added the reaching tests.
    pub fn from_messages(messages: &[String]) -> TestAuthorship {
        match super::types::authorship_from_messages(messages).map(|a| a.kind) {
            Some(AuthorshipKind::Human) => Self::Human,
            Some(_) => Self::Agent,
            None => Self::Unknown,
        }
    }
}

/// The whole question set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewWizard {
    pub version: u32,
    #[serde(rename = "question")]
    pub questions: Vec<Question>,
}

/// When a question is asked (#740: some questions appear only when they apply).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppliesWhen {
    Always,
    /// The function is a declared port (`[upstream]` in `kovan.toml`).
    Port,
    /// A physical quantity crosses the function's interface.
    PhysicalInterface,
}

/// Where a question's starting answer comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Prefill {
    /// Commit author plus agent trailer ([`independence_prefill`]).
    GitAuthorship,
    /// The agent trailer on the commits that added the tests reaching the
    /// function ([`vv_case_author_prefill`]).
    GitTestAuthorship,
}

/// What choosing an option does to the stamp.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Effect {
    None,
    /// "Mark as Needs fix instead?"; the stamp stays allowed.
    PromptNeedsFix,
    /// The stamp is refused.
    Block,
    /// Needs improvement; never blocks.
    Flag,
    /// Qualifying V&V evidence: half of opening rung 4.
    GateRung4,
    /// The V&V case was written and verified by hand, without AI agents:
    /// the other half (maintainer, #769, 2026-10-07).
    GateRung4Author,
    /// The reviewer is not independent of the code: the stamp still counts
    /// at rung 3 or 4, but cannot be ~~rung 5's independent second review~~
    /// (**CORRECTED 2026-10-08**, #809) rung 5's independent V&V case.
    /// ~~`NoRung4`: rung 4 is closed whatever else is answered~~
    /// **CORRECTED 2026-10-07** (maintainer, #769). `no_rung4` still reads.
    #[serde(alias = "no_rung4")]
    NotIndependent,
}

/// A UI action an option offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OptionAction {
    /// "Document it now": open kvim at the doc comment with a deviation
    /// template (#740).
    DocumentDeviation,
}

/// One question.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Question {
    pub key: String,
    pub text: String,
    pub applies_when: AppliesWhen,
    /// The workspace rule the question rests on, where #769 records one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_rule: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prefill: Option<Prefill>,
    #[serde(rename = "option")]
    pub options: Vec<WizardOption>,
    #[serde(rename = "source", default)]
    pub sources: Vec<Source>,
}

/// One option of a question.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WizardOption {
    pub key: String,
    pub label: String,
    pub effect: Effect,
    #[serde(default)]
    pub requires_text: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action: Option<OptionAction>,
}

/// A clause in the kovan standard corpus that justifies a question (#769).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    /// The standard-corpus id (`kovan::corpus::LITERATURE`).
    pub document: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub section: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page: Option<String>,
    /// Quoted wording, only where #769 records it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quote: Option<String>,
}

/// Why the question set itself is not acceptable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WizardError {
    Toml(String),
    /// A key that is not lowercase `[a-z0-9_]+` (the answer format splits
    /// on `:`).
    BadKey(String),
    DuplicateQuestion(String),
    DuplicateOption {
        question: String,
        option: String,
    },
    /// A question without an `other` option that requires text (#740 U3).
    NoOther(String),
    /// Empty question text or option label.
    EmptyText {
        question: String,
        option: Option<String>,
    },
    /// A question with neither a source nor a workspace rule (#740: such a
    /// question is dropped or marked as a workspace rule).
    Unsourced(String),
    /// A source with an empty document id, or with neither section nor page.
    BadSource {
        question: String,
    },
    /// No option with `gate_rung4` or none with `gate_rung4_author`: rung 4
    /// could never be derived. (~~`NoRungQuestion`~~ CORRECTED 2026-10-07:
    /// the rung is no longer a question.)
    NoRung4Gate,
}

impl std::fmt::Display for WizardError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Toml(e) => write!(f, "review_wizard.toml: {e}"),
            Self::BadKey(k) => write!(f, "key {k:?} is not lowercase [a-z0-9_]+"),
            Self::DuplicateQuestion(q) => write!(f, "question {q:?} appears twice"),
            Self::DuplicateOption { question, option } => {
                write!(f, "{question}: option {option:?} appears twice")
            }
            Self::NoOther(q) => write!(f, "{q}: no `other` option requiring text"),
            Self::EmptyText { question, option } => match option {
                Some(o) => write!(f, "{question}.{o}: empty label"),
                None => write!(f, "{question}: empty question text"),
            },
            Self::Unsourced(q) => write!(f, "{q}: neither a source nor a workspace rule"),
            Self::BadSource { question } => {
                write!(
                    f,
                    "{question}: a source without a document, section or page"
                )
            }
            Self::NoRung4Gate => write!(f, "no gate_rung4 and gate_rung4_author options: rung 4 cannot be derived"),
        }
    }
}

impl std::error::Error for WizardError {}

/// Which conditional questions apply to the function under review.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Applicability {
    pub is_port: bool,
    pub physical_interface: bool,
    /// Git's view of who wrote the reaching tests (for the derived rung).
    pub tests: TestAuthorship,
}

impl Applicability {
    pub fn applies(self, when: AppliesWhen) -> bool {
        match when {
            AppliesWhen::Always => true,
            AppliesWhen::Port => self.is_port,
            AppliesWhen::PhysicalInterface => self.physical_interface,
        }
    }
}

/// Why one checklist answer is not acceptable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnswerError {
    /// No such question (#764's `q1` … `q10` placeholders and a `rung`
    /// answer included: the rung is derived, never chosen).
    UnknownQuestion(String),
    /// Answered, but the question does not apply (e.g. upstream fidelity for
    /// a function that is not a port).
    NotApplicable(String),
    UnknownOption {
        question: String,
        option: String,
    },
    /// The option needs text of at least 2 characters.
    TextRequired {
        question: String,
        option: String,
    },
    /// Text given for an option that takes none.
    UnexpectedText {
        question: String,
        option: String,
    },
}

impl std::fmt::Display for AnswerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownQuestion(q) => write!(f, "no question {q:?}"),
            Self::NotApplicable(q) => write!(f, "question {q} does not apply to this function"),
            Self::UnknownOption { question, option } => {
                write!(f, "{question}: no option {option:?}")
            }
            Self::TextRequired { question, option } => {
                write!(
                    f,
                    "{question}: {option} needs at least 2 characters of text"
                )
            }
            Self::UnexpectedText { question, option } => {
                write!(f, "{question}: {option} takes no text")
            }
        }
    }
}

impl std::error::Error for AnswerError {}

/// A chosen option.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Choice {
    pub question: String,
    pub option: String,
}

/// Why the stamp is refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GateReason {
    /// An answer whose effect is `block`.
    Answer(Choice),
    /// An applicable question with no answer.
    Unanswered(String),
    Invalid(AnswerError),
}

/// What the answers allow.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GateResult {
    /// Empty = the stamp may be given.
    pub blocked_by: Vec<GateReason>,
    /// Answers that ask "Mark as Needs fix instead?".
    pub prompts: Vec<Choice>,
    /// Needs-improvement flags (never block).
    pub flags: Vec<Choice>,
    /// A `gate_rung4` answer and a `gate_rung4_author` answer (~~and no
    /// `no_rung4` answer~~ CORRECTED 2026-10-07: independence does not close
    /// rung 4; the hand-written V&V case opens it with the evidence).
    pub rung4_allowed: bool,
    /// `independence` is answered and no answer is `not_independent`: one of
    /// rung 5's conditions (~~this stamp may be the independent second review
    /// for rung 5~~ **CORRECTED 2026-10-08**, #809: rung 5 is IV&V,
    /// [`super::ivv`]).
    pub independent: bool,
    /// The rung the stamp gives ([`derived_rung`]).
    pub rung: Rung,
}

impl Default for Rung {
    fn default() -> Self {
        Rung::Three
    }
}

impl GateResult {
    pub fn stampable(&self) -> bool {
        self.blocked_by.is_empty()
    }
}

/// Split an on-disk answer into its option key and optional text.
pub fn parse_answer(raw: &str) -> (&str, Option<&str>) {
    match raw.split_once(':') {
        Some((k, t)) => (k.trim(), Some(t.trim())),
        None => (raw.trim(), None),
    }
}

/// The on-disk form of an answer (inverse of [`parse_answer`]).
pub fn format_answer(option: &str, text: Option<&str>) -> String {
    match text {
        Some(t) => format!("{option}: {}", t.trim()),
        None => option.to_string(),
    }
}

fn is_key(k: &str) -> bool {
    !k.is_empty()
        && k.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

impl ReviewWizard {
    /// Parse and validate a question set.
    pub fn parse(text: &str) -> Result<Self, WizardError> {
        let w: Self = toml::from_str(text).map_err(|e| WizardError::Toml(e.to_string()))?;
        w.validate()?;
        Ok(w)
    }

    /// The embedded set ([`WIZARD_TOML`]), parsed once.
    ///
    /// Panics if the embedded file is invalid: it is compiled into the
    /// binary, and `embedded_question_set_is_valid` fails first, so this
    /// cannot reach a user.
    pub fn embedded() -> &'static Self {
        static W: OnceLock<ReviewWizard> = OnceLock::new();
        W.get_or_init(|| {
            Self::parse(WIZARD_TOML).unwrap_or_else(|e| panic!("embedded review wizard: {e}"))
        })
    }

    /// Unique keys, an `other` option requiring text on every question,
    /// non-empty text and labels, a source or workspace rule per question,
    /// and the rung question.
    pub fn validate(&self) -> Result<(), WizardError> {
        let mut seen = BTreeSet::new();
        for q in &self.questions {
            if !is_key(&q.key) {
                return Err(WizardError::BadKey(q.key.clone()));
            }
            if !seen.insert(q.key.as_str()) {
                return Err(WizardError::DuplicateQuestion(q.key.clone()));
            }
            if q.text.trim().is_empty() {
                return Err(WizardError::EmptyText {
                    question: q.key.clone(),
                    option: None,
                });
            }
            let mut opts = BTreeSet::new();
            for o in &q.options {
                if !is_key(&o.key) {
                    return Err(WizardError::BadKey(format!("{}.{}", q.key, o.key)));
                }
                if !opts.insert(o.key.as_str()) {
                    return Err(WizardError::DuplicateOption {
                        question: q.key.clone(),
                        option: o.key.clone(),
                    });
                }
                if o.label.trim().is_empty() {
                    return Err(WizardError::EmptyText {
                        question: q.key.clone(),
                        option: Some(o.key.clone()),
                    });
                }
            }
            if !q
                .options
                .iter()
                .any(|o| o.key == "other" && o.requires_text)
            {
                return Err(WizardError::NoOther(q.key.clone()));
            }
            if q.sources.is_empty() && q.workspace_rule.is_none() {
                return Err(WizardError::Unsourced(q.key.clone()));
            }
            for s in &q.sources {
                if s.document.trim().is_empty() || (s.section.is_none() && s.page.is_none()) {
                    return Err(WizardError::BadSource {
                        question: q.key.clone(),
                    });
                }
            }
        }
        let has = |e: Effect| {
            self.questions
                .iter()
                .any(|q| q.options.iter().any(|o| o.effect == e))
        };
        (has(Effect::GateRung4) && has(Effect::GateRung4Author))
            .then_some(())
            .ok_or(WizardError::NoRung4Gate)
    }

    pub fn question(&self, key: &str) -> Option<&Question> {
        self.questions.iter().find(|q| q.key == key)
    }

    /// The questions asked for this function, in order.
    pub fn applicable(&self, ctx: Applicability) -> impl Iterator<Item = &Question> {
        self.questions
            .iter()
            .filter(move |q| ctx.applies(q.applies_when))
    }

    /// Every standard-corpus id cited (for the corpus check in `kovan`).
    pub fn cited_documents(&self) -> BTreeSet<&str> {
        self.questions
            .iter()
            .flat_map(|q| q.sources.iter().map(|s| s.document.as_str()))
            .collect()
    }

    /// Check one answer; on success, the option chosen.
    pub fn check_answer(
        &self,
        question: &str,
        raw: &str,
        ctx: Applicability,
    ) -> Result<&WizardOption, AnswerError> {
        let Some(q) = self.question(question) else {
            return Err(AnswerError::UnknownQuestion(question.to_string()));
        };
        if !ctx.applies(q.applies_when) {
            return Err(AnswerError::NotApplicable(question.to_string()));
        }
        let (key, text) = parse_answer(raw);
        let choice = || (question.to_string(), key.to_string());
        let Some(o) = q.option(key) else {
            let (question, option) = choice();
            return Err(AnswerError::UnknownOption { question, option });
        };
        match (o.requires_text, text) {
            (true, Some(t)) if t.chars().count() >= 2 => Ok(o),
            (true, _) => {
                let (question, option) = choice();
                Err(AnswerError::TextRequired { question, option })
            }
            (false, Some(_)) => {
                let (question, option) = choice();
                Err(AnswerError::UnexpectedText { question, option })
            }
            (false, None) => Ok(o),
        }
    }

    /// The stamp gate over this question set (see the module docs).
    pub fn stamp_gate(&self, answers: &BTreeMap<String, String>, ctx: Applicability) -> GateResult {
        let mut g = GateResult::default();
        let mut opens = false;
        let mut opens_author = false;
        let mut not_independent = false;
        for (q, raw) in answers {
            let o = match self.check_answer(q, raw, ctx) {
                Ok(o) => o,
                Err(e) => {
                    g.blocked_by.push(GateReason::Invalid(e));
                    continue;
                }
            };
            let c = Choice {
                question: q.clone(),
                option: o.key.clone(),
            };
            match o.effect {
                Effect::None => {}
                Effect::Block => g.blocked_by.push(GateReason::Answer(c)),
                Effect::PromptNeedsFix => g.prompts.push(c),
                Effect::Flag => g.flags.push(c),
                Effect::GateRung4 => opens = true,
                Effect::GateRung4Author => opens_author = true,
                Effect::NotIndependent => not_independent = true,
            }
        }
        for q in self.applicable(ctx) {
            if !answers.contains_key(&q.key) {
                g.blocked_by.push(GateReason::Unanswered(q.key.clone()));
            }
        }
        g.rung4_allowed = opens && opens_author;
        g.independent = answers.contains_key("independence") && !not_independent;
        g.rung = rung_from(g.rung4_allowed, ctx.tests);
        g
    }

    /// Starting answers for a re-review (#740: "re-reviews start from the
    /// previous answers"): the previous answers that are still valid for
    /// this question set and applicability. Anything else is dropped, so
    /// the reviewer is asked again. ~~The rung is never carried over: it is
    /// chosen afresh after the gate is known.~~ **CORRECTED 2026-10-07**: the
    /// rung is not an answer at all; it is derived ([`derived_rung`]).
    pub fn prefill(
        &self,
        previous: &BTreeMap<String, String>,
        ctx: Applicability,
    ) -> BTreeMap<String, String> {
        previous
            .iter()
            .filter(|(q, raw)| self.check_answer(q, raw, ctx).is_ok())
            .map(|(q, raw)| (q.clone(), raw.clone()))
            .collect()
    }
}

impl Question {
    pub fn option(&self, key: &str) -> Option<&WizardOption> {
        self.options.iter().find(|o| o.key == key)
    }
}

fn rung_from(rung4_allowed: bool, tests: TestAuthorship) -> Rung {
    if rung4_allowed && tests == TestAuthorship::Human {
        Rung::Four
    } else {
        Rung::Three
    }
}

/// The rung a stamp gives, derived (maintainer, #769, 2026-10-07: the user
/// never chooses it): **4** when `vv_evidence` is a qualifying answer AND
/// `vv_case_author = human_wrote_and_verified` AND git shows no agent
/// trailer on the commits that added the tests reaching the function;
/// otherwise **3** (a half-done V&V case gives 3). Pure: the answers and
/// git's [`TestAuthorship`] in, the rung out.
pub fn derived_rung(answers: &BTreeMap<String, String>, tests: TestAuthorship) -> Rung {
    let w = ReviewWizard::embedded();
    let mut opens = false;
    let mut opens_author = false;
    for (q, raw) in answers {
        let Some(question) = w.question(q) else { continue };
        let (key, _) = parse_answer(raw);
        match question.option(key).map(|o| o.effect) {
            Some(Effect::GateRung4) => opens = true,
            Some(Effect::GateRung4Author) => opens_author = true,
            _ => {}
        }
    }
    rung_from(opens && opens_author, tests)
}

/// [`ReviewWizard::stamp_gate`] over the embedded question set.
pub fn stamp_gate(answers: &BTreeMap<String, String>, ctx: Applicability) -> GateResult {
    ReviewWizard::embedded().stamp_gate(answers, ctx)
}

/// The pre-filled answer to `vv_case_author` from the messages of the
/// commits that added the tests reaching the function (maintainer, #769,
/// 2026-10-07): any agent trailer -> `agent_wrote_or_cowrote`; none ->
/// `human_wrote_and_verified`, which the reviewer still confirms (git sees
/// only who wrote the case, not that its result was verified by hand). No
/// commits known -> no pre-fill.
pub fn vv_case_author_prefill(test_commit_messages: &[String]) -> Option<&'static str> {
    super::types::authorship_from_messages(test_commit_messages).map(|a| match a.kind {
        AuthorshipKind::Human => "human_wrote_and_verified",
        AuthorshipKind::Agent | AuthorshipKind::Mixed => "agent_wrote_or_cowrote",
    })
}

/// The pre-filled answer to `independence` (#769, 2026-10-07: "pre-filled
/// from git (commit author plus agent trailer)"):
///
/// - every commit of the change carries the agent trailer → `someone_else`
///   (an AI agent wrote it);
/// - otherwise, if the reviewer authored any of the commits →
///   `self_check` (conservative for a mixed change);
/// - otherwise → `someone_else`.
///
/// The reviewer can change it; this is only the starting answer.
pub fn independence_prefill(reviewer_is_author: bool, authorship: AuthorshipKind) -> &'static str {
    match (authorship, reviewer_is_author) {
        (AuthorshipKind::Agent, _) => "someone_else",
        (_, true) => "self_check",
        (_, false) => "someone_else",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    /// A complete, clean answer set for a non-port function with no
    /// physical interface, at rung 3.
    fn clean() -> BTreeMap<String, String> {
        a(&[
            ("doc_matches_behaviour", "yes"),
            ("limits_and_guards", "guarded_returns_result"),
            ("error_handling", "returns_result"),
            ("numerical_hazards", "none_found"),
            ("test_reach", "reached_and_checked"),
            ("vv_evidence", "unit_tests_only"),
            ("vv_case_author", "agent_wrote_or_cowrote"),
            ("maintainability", "yes"),
            ("independence", "someone_else"),
            ("unintended_function", "no"),
            ("coding_standards", "yes"),
        ])
    }

    const NONE: Applicability = Applicability {
        is_port: false,
        physical_interface: false,
        tests: TestAuthorship::Unknown,
    };
    const BOTH: Applicability = Applicability {
        is_port: true,
        physical_interface: true,
        tests: TestAuthorship::Unknown,
    };

    /// Methodology: the embedded `data/review_wizard.toml` parses and passes
    /// [`ReviewWizard::validate`]; it has the 13 questions decided on #769
    /// (10 + independence, unintended function, coding standards), under
    /// the stable keys, each with `other` requiring text; the two
    /// conditional questions are the decided ones; and the four options
    /// added on #769 are present with their decided effects.
    ///
    /// Result (2026-10-07): passes; 13 questions, 11 always asked. Since the
    /// `vv_case_author` question and without the `rung` question
    /// (2026-10-07, later the same day): 13 questions, 11 always asked.
    #[test]
    fn embedded_question_set_is_valid() {
        let w = ReviewWizard::embedded();
        let keys: Vec<&str> = w.questions.iter().map(|q| q.key.as_str()).collect();
        assert_eq!(
            keys,
            [
                "doc_matches_behaviour",
                "upstream_fidelity",
                "limits_and_guards",
                "units_documented",
                "error_handling",
                "numerical_hazards",
                "test_reach",
                "vv_evidence",
                "vv_case_author",
                "maintainability",
                "independence",
                "unintended_function",
                "coding_standards",
            ]
        );
        assert_eq!(w.applicable(NONE).count(), 11);
        assert_eq!(w.applicable(BOTH).count(), 13);
        assert_eq!(
            w.question("upstream_fidelity").unwrap().applies_when,
            AppliesWhen::Port
        );
        assert_eq!(
            w.question("units_documented").unwrap().applies_when,
            AppliesWhen::PhysicalInterface
        );
        let eff = |q: &str, o: &str| w.question(q).unwrap().option(o).unwrap().effect;
        assert_eq!(
            eff("limits_and_guards", "guard_present_range_undocumented"),
            Effect::PromptNeedsFix
        );
        assert_eq!(
            eff("numerical_hazards", "tolerance_not_justified"),
            Effect::Block
        );
        assert_eq!(
            eff("vv_evidence", "convergence_order_study"),
            Effect::GateRung4
        );
        assert_eq!(
            eff("maintainability", "too_complex_split"),
            Effect::PromptNeedsFix
        );
        // #740's agreed blocking answers.
        for (q, o) in [
            ("doc_matches_behaviour", "no"),
            ("upstream_fidelity", "deviation_not_documented"),
            ("limits_and_guards", "guard_missing"),
            ("error_handling", "falls_back_silently"),
            ("numerical_hazards", "possible_problem"),
            ("unintended_function", "yes_undocumented"),
        ] {
            assert_eq!(eff(q, o), Effect::Block, "{q}.{o}");
        }
        assert_eq!(eff("test_reach", "no_test_reaches"), Effect::Flag);
        assert_eq!(
            eff("coding_standards", "deviates_not_justified"),
            Effect::PromptNeedsFix
        );
        assert_eq!(eff("independence", "self_check"), Effect::NotIndependent);
        assert_eq!(
            w.question("upstream_fidelity")
                .unwrap()
                .option("deviation_not_documented")
                .unwrap()
                .action,
            Some(OptionAction::DocumentDeviation)
        );
        assert_eq!(
            w.question("independence").unwrap().prefill,
            Some(Prefill::GitAuthorship)
        );
        assert_eq!(
            w.cited_documents().into_iter().collect::<Vec<_>>(),
            [
                "10cfr50",
                "doe-g-414.1-4",
                "doe-std-1172-2003",
                "nureg-br-0167",
                "nureg-km-0006"
            ]
        );
    }

    /// Methodology: each validation rule rejects a set built to break it
    /// (bad key, duplicate question, duplicate option, missing Other, Other
    /// without text, empty label, empty question text, unsourced question,
    /// source with no section/page, no way to derive rung 4 (~~no rung
    /// question~~, CORRECTED 2026-10-07), bad TOML), and a
    /// workspace-rule-only question is accepted.
    ///
    /// Result (2026-10-07): passes.
    #[test]
    fn validation_rejects_each_defect() {
        // The minimum for rung 4 to be derivable (since 2026-10-07; was a
        // `rung` question).
        let rung = r#"
[[question]]
key = "vv"
text = "V&V?"
applies_when = "always"
workspace_rule = "r"
[[question.option]]
key = "case"
label = "A case"
effect = "gate_rung4"
[[question.option]]
key = "by_hand"
label = "By hand"
effect = "gate_rung4_author"
[[question.option]]
key = "other"
label = "Other"
effect = "block"
requires_text = true
"#;
        let q = |key: &str, text: &str, opts: &str, src: &str| {
            format!(
                "version = 1\n[[question]]\nkey = \"{key}\"\ntext = \"{text}\"\napplies_when = \"always\"\n{src}\n{opts}\n{rung}"
            )
        };
        let other = "[[question.option]]\nkey = \"other\"\nlabel = \"Other\"\neffect = \"none\"\nrequires_text = true\n";
        let yes = "[[question.option]]\nkey = \"yes\"\nlabel = \"Yes\"\neffect = \"none\"\n";
        let ws = "workspace_rule = \"some rule\"";
        let good = q("x", "X?", &format!("{yes}{other}"), ws);
        assert!(ReviewWizard::parse(&good).is_ok());
        let cited = format!(
            "{}[[question.source]]\ndocument = \"nureg-br-0167\"\npage = \"5\"\n",
            q("x", "X?", &format!("{yes}{other}"), "")
        );
        // A [[question.source]] after the vv question attaches to it; the
        // `x` question is then unsourced.
        assert_eq!(
            ReviewWizard::parse(&cited),
            Err(WizardError::Unsourced("x".into()))
        );
        let err = |t: String| ReviewWizard::parse(&t).unwrap_err();
        assert_eq!(
            err(q("X", "X?", other, ws)),
            WizardError::BadKey("X".into())
        );
        assert_eq!(
            err(format!("{good}\n{}", &rung)),
            WizardError::DuplicateQuestion("vv".into())
        );
        assert!(matches!(
            err(q("x", "X?", &format!("{yes}{yes}{other}"), ws)),
            WizardError::DuplicateOption { .. }
        ));
        assert_eq!(err(q("x", "X?", yes, ws)), WizardError::NoOther("x".into()));
        assert_eq!(
            err(q(
                "x",
                "X?",
                &other.replace("requires_text = true\n", ""),
                ws
            )),
            WizardError::NoOther("x".into())
        );
        assert_eq!(
            err(q(
                "x",
                "X?",
                &format!("{}{other}", yes.replace("\"Yes\"", "\" \"")),
                ws
            )),
            WizardError::EmptyText {
                question: "x".into(),
                option: Some("yes".into())
            }
        );
        assert_eq!(
            err(q("x", " ", other, ws)),
            WizardError::EmptyText {
                question: "x".into(),
                option: None
            }
        );
        assert_eq!(
            err(q("x", "X?", other, "")),
            WizardError::Unsourced("x".into())
        );
        let nosec = q(
            "x",
            "X?",
            &format!("{other}[[question.source]]\ndocument = \"nureg-br-0167\"\n"),
            ws,
        );
        assert_eq!(
            err(nosec),
            WizardError::BadSource {
                question: "x".into()
            }
        );
        let no_rung = format!("version = 1\n[[question]]\nkey = \"x\"\ntext = \"X?\"\napplies_when = \"always\"\n{ws}\n{other}");
        assert_eq!(err(no_rung), WizardError::NoRung4Gate);
        assert!(matches!(
            err("version = 1\nquestion = 3".into()),
            WizardError::Toml(_)
        ));
        // Every error has a message.
        for e in [
            WizardError::Toml("t".into()),
            WizardError::BadKey("k".into()),
            WizardError::DuplicateQuestion("q".into()),
            WizardError::DuplicateOption {
                question: "q".into(),
                option: "o".into(),
            },
            WizardError::NoOther("q".into()),
            WizardError::EmptyText {
                question: "q".into(),
                option: None,
            },
            WizardError::EmptyText {
                question: "q".into(),
                option: Some("o".into()),
            },
            WizardError::Unsourced("q".into()),
            WizardError::BadSource {
                question: "q".into(),
            },
            WizardError::NoRung4Gate,
        ] {
            assert!(!e.to_string().is_empty());
        }
    }

    /// Methodology: a complete clean answer set stamps at rung 3; dropping
    /// an applicable question blocks (Unanswered); a port and a physical
    /// interface add their questions to what must be answered.
    ///
    /// Result (2026-10-07): passes.
    #[test]
    fn clean_answers_stamp_and_missing_ones_block() {
        let g = stamp_gate(&clean(), NONE);
        assert!(g.stampable(), "{g:?}");
        assert!(g.prompts.is_empty() && g.flags.is_empty() && !g.rung4_allowed);
        let mut m = clean();
        m.remove("error_handling");
        assert_eq!(
            stamp_gate(&m, NONE).blocked_by,
            [GateReason::Unanswered("error_handling".into())]
        );
        assert_eq!(
            stamp_gate(&clean(), BOTH).blocked_by,
            [
                GateReason::Unanswered("upstream_fidelity".into()),
                GateReason::Unanswered("units_documented".into())
            ]
        );
        let mut full = clean();
        full.insert("upstream_fidelity".into(), "deviation_documented".into());
        full.insert("units_documented".into(), "yes".into());
        assert!(stamp_gate(&full, BOTH).stampable());
    }

    /// Methodology: every option the TOML marks `block` blocks, every
    /// `prompt_needs_fix` prompts without blocking, every `flag` flags
    /// without blocking: each is substituted into the clean set in turn,
    /// driven from the data so a new option is covered automatically.
    ///
    /// Result (2026-10-07): passes (8 block options, 6 prompt, 2 flag);
    /// 7 block options since the `rung` question (whose Other blocked) was
    /// removed the same day.
    #[test]
    fn effects_block_prompt_and_flag() {
        let w = ReviewWizard::embedded();
        let mut counts = [0usize; 3];
        for q in &w.questions {
            for o in &q.options {
                let mut m = clean();
                m.insert("upstream_fidelity".into(), "matches".into());
                m.insert("units_documented".into(), "yes".into());
                let raw = format_answer(&o.key, o.requires_text.then_some("a reason"));
                m.insert(q.key.clone(), raw);
                let g = w.stamp_gate(&m, BOTH);
                let c = Choice {
                    question: q.key.clone(),
                    option: o.key.clone(),
                };
                match o.effect {
                    Effect::Block => {
                        assert_eq!(g.blocked_by, [GateReason::Answer(c)]);
                        counts[0] += 1;
                    }
                    Effect::PromptNeedsFix => {
                        assert!(g.stampable(), "{c:?}");
                        assert_eq!(g.prompts, [c]);
                        counts[1] += 1;
                    }
                    Effect::Flag => {
                        assert!(g.stampable(), "{c:?}");
                        assert_eq!(g.flags, [c]);
                        counts[2] += 1;
                    }
                    _ => {}
                }
            }
        }
        assert_eq!(counts, [7, 6, 2]);
    }

    /// Methodology: the derived rung (maintainer, #769, 2026-10-07: "the
    /// user never chooses the rung"). Each of the three qualifying V&V
    /// answers, with `vv_case_author = human_wrote_and_verified` and git
    /// showing no agent trailer, gives rung 4; the others give rung 3 and
    /// still stamp. Git saying an agent wrote the tests, or not knowing,
    /// gives rung 3. A self-check leaves rung 4 open and only marks the
    /// stamp not independent (no rung 5). A `rung` (or `q10`) answer is
    /// refused (an unknown question since #825). ~~asking for rung 4 while it is closed blocks
    /// with `Rung4NotOpen`~~ CORRECTED 2026-10-07: there is no rung answer.
    ///
    /// Result (2026-10-07): passes.
    #[test]
    fn rung4_gate() {
        let human = Applicability { tests: TestAuthorship::Human, ..NONE };
        for (vv, open) in [
            ("reference_code_to_code", true),
            ("analytical_case", true),
            ("convergence_order_study", true),
            ("unit_tests_only", false),
            ("none_yet", false),
            ("other: a benchmark", false),
        ] {
            let mut m = clean();
            m.insert("vv_evidence".into(), vv.into());
            m.insert("vv_case_author".into(), "human_wrote_and_verified".into());
            let g = stamp_gate(&m, human);
            assert_eq!(g.rung4_allowed, open, "{vv}");
            assert!(g.stampable(), "{vv}");
            let want = if open { Rung::Four } else { Rung::Three };
            assert_eq!(g.rung, want, "{vv}");
            assert_eq!(derived_rung(&m, TestAuthorship::Human), want, "{vv}");
            assert_eq!(derived_rung(&m, TestAuthorship::Agent), Rung::Three, "{vv}");
            assert_eq!(derived_rung(&m, TestAuthorship::Unknown), Rung::Three, "{vv}");
        }
        for who in ["self_check", "other: pair-programmed"] {
            let mut m = clean();
            m.insert("vv_evidence".into(), "analytical_case".into());
            m.insert("vv_case_author".into(), "human_wrote_and_verified".into());
            m.insert("independence".into(), who.into());
            let g = stamp_gate(&m, human);
            assert!(g.rung4_allowed, "independence does not close rung 4: {who}");
            assert_eq!(g.rung, Rung::Four, "{who}");
            assert!(!g.independent, "{who}");
            assert!(g.stampable(), "{who}");
        }
        for k in ["q10", "rung"] {
            let mut m = clean();
            m.insert(k.to_string(), "rung_3".into());
            assert_eq!(
                stamp_gate(&m, NONE).blocked_by,
                [GateReason::Invalid(AnswerError::UnknownQuestion(k.to_string()))]
            );
        }
        // Rung 4 needs the hand-written case too (maintainer, #769).
        for author in ["agent_wrote_or_cowrote", "other: pair-programmed with an agent"] {
            let mut m = clean();
            m.insert("vv_evidence".into(), "analytical_case".into());
            m.insert("vv_case_author".into(), author.into());
            assert!(!stamp_gate(&m, NONE).rung4_allowed, "{author}");
        }
        let msgs = |t: &[&str]| t.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(vv_case_author_prefill(&msgs(&["add test"])), Some("human_wrote_and_verified"));
        assert_eq!(
            vv_case_author_prefill(&msgs(&["add test", "x\n\nClaude-Session: https://claude.ai/code/s"])),
            Some("agent_wrote_or_cowrote")
        );
        assert_eq!(vv_case_author_prefill(&[]), None);
    }

    /// Methodology: malformed answers block as `Invalid` with a typed
    /// reason: unknown question, not-applicable question, unknown option,
    /// Other with no text and with one character, text on an option that
    /// takes none. A two-character Other is accepted (#740 U3).
    ///
    /// Result (2026-10-07): passes.
    #[test]
    fn invalid_answers_are_typed() {
        let w = ReviewWizard::embedded();
        let e = |q: &str, raw: &str, ctx| w.check_answer(q, raw, ctx).unwrap_err();
        assert_eq!(
            e("nope", "yes", NONE),
            AnswerError::UnknownQuestion("nope".into())
        );
        assert_eq!(
            e("upstream_fidelity", "matches", NONE),
            AnswerError::NotApplicable("upstream_fidelity".into())
        );
        assert_eq!(
            e("units_documented", "yes", NONE),
            AnswerError::NotApplicable("units_documented".into())
        );
        let qo = |q: &str, o: &str| (q.to_string(), o.to_string());
        let (question, option) = qo("doc_matches_behaviour", "maybe");
        assert_eq!(
            e("doc_matches_behaviour", "maybe", NONE),
            AnswerError::UnknownOption { question, option }
        );
        for raw in ["other", "other:", "other: x", "other:  y "] {
            let (question, option) = qo("doc_matches_behaviour", "other");
            assert_eq!(
                e("doc_matches_behaviour", raw, NONE),
                AnswerError::TextRequired { question, option },
                "{raw:?}"
            );
        }
        let (question, option) = qo("error_handling", "panics_justified");
        assert_eq!(
            e("error_handling", "panics_justified", NONE),
            AnswerError::TextRequired { question, option }
        );
        let (question, option) = qo("doc_matches_behaviour", "yes");
        assert_eq!(
            e("doc_matches_behaviour", "yes: really", NONE),
            AnswerError::UnexpectedText { question, option }
        );
        assert_eq!(
            w.check_answer("doc_matches_behaviour", "other: ok", NONE)
                .unwrap()
                .key,
            "other"
        );
        let mut m = clean();
        m.insert("doc_matches_behaviour".into(), "other".into());
        let g = stamp_gate(&m, NONE);
        assert!(matches!(
            g.blocked_by.as_slice(),
            [GateReason::Invalid(AnswerError::TextRequired { .. })]
        ));
        assert_eq!(parse_answer(" yes "), ("yes", None));
        assert_eq!(parse_answer("other: a: b"), ("other", Some("a: b")));
        assert_eq!(format_answer("other", Some(" why ")), "other: why");
        for err in [
            AnswerError::UnknownQuestion("q".into()),
            AnswerError::NotApplicable("q".into()),
            AnswerError::UnknownOption {
                question: "q".into(),
                option: "o".into(),
            },
            AnswerError::TextRequired {
                question: "q".into(),
                option: "o".into(),
            },
            AnswerError::UnexpectedText {
                question: "q".into(),
                option: "o".into(),
            },
        ] {
            assert!(!err.to_string().is_empty());
        }
    }

    /// Methodology: #764's placeholder keys `q1` … `q10` are unknown keys
    /// since #825 (~~rejected with `LegacyPlaceholderKey`~~): each one is
    /// refused as [`AnswerError::UnknownQuestion`], never mapped, and a
    /// placeholder `q8 = "reference_code_to_code"` never opens rung 4.
    ///
    /// Result (2026-10-10): passes.
    #[test]
    fn placeholder_keys_are_unknown_questions() {
        let w = ReviewWizard::embedded();
        for k in (1..=10).map(|i| format!("q{i}")) {
            assert_eq!(
                w.check_answer(&k, "yes", BOTH).unwrap_err(),
                AnswerError::UnknownQuestion(k.clone())
            );
        }
        let g = stamp_gate(&a(&[("q1", "yes"), ("q8", "reference_code_to_code")]), NONE);
        assert!(!g.rung4_allowed, "a placeholder q8 never opens rung 4");
        assert!(g
            .blocked_by
            .contains(&GateReason::Invalid(AnswerError::UnknownQuestion("q8".into()))));
    }

    /// Methodology: the `independence` question after GitHub #809 (rung 5 is
    /// IV&V by a technically and managerially separate organisation). Its
    /// key and option keys are unchanged (stable on disk); its text names
    /// the new rung-5 conditions; it cites NUREG/BR-0167 §3.1 p. 6 with the
    /// definition quoted exactly as the standard-corpus PDF reads; `other`
    /// still needs at least 2 characters of text and still marks the stamp
    /// not independent; and no question was added, so every answer set that
    /// stamped before still stamps (an added always-asked question would
    /// leave old stamps with an unanswered question, invalid on read).
    ///
    /// Result (2026-10-08): passes; 13 questions, as before.
    #[test]
    fn independence_question_cites_br0167_for_ivv() {
        let w = ReviewWizard::embedded();
        assert_eq!(w.questions.len(), 13);
        let q = w.question("independence").unwrap();
        let opts: Vec<&str> = q.options.iter().map(|o| o.key.as_str()).collect();
        assert_eq!(opts, ["someone_else", "self_check", "other"]);
        assert!(q.text.starts_with("Who wrote this function?"), "{}", q.text);
        assert!(q.text.contains("technically and managerially separate"), "{}", q.text);
        assert!(q.text.contains("separation attestation"), "{}", q.text);
        let br = q
            .sources
            .iter()
            .find(|s| s.document == "nureg-br-0167")
            .expect("a NUREG/BR-0167 source");
        assert_eq!((br.section.as_deref(), br.page.as_deref()), (Some("§3.1"), Some("6")));
        assert_eq!(
            br.quote.as_deref(),
            Some(
                "Independent verification and validation (IV&V) is verification and validation by an \
                 organization that is both technically and managerially separate from the organization \
                 responsible for developing the software."
            )
        );
        let (question, option) = ("independence".to_string(), "other".to_string());
        assert_eq!(
            w.check_answer("independence", "other: x", NONE),
            Err(AnswerError::TextRequired { question, option })
        );
        assert_eq!(w.check_answer("independence", "other: ok", NONE).unwrap().effect, Effect::NotIndependent);
        let mut m = clean();
        m.insert("independence".into(), "other: pair-programmed".into());
        let g = stamp_gate(&m, NONE);
        assert!(g.stampable() && !g.independent);
        assert!(stamp_gate(&clean(), NONE).independent);
    }

    /// Methodology: re-review pre-fill keeps the previous valid answers,
    /// drops invalid, legacy and no-longer-applicable ones (~~and the
    /// rung~~: the rung is no longer an answer, CORRECTED 2026-10-07);
    /// the independence pre-fill from git follows the documented table.
    ///
    /// Result (2026-10-07): passes.
    #[test]
    fn prefill_from_previous_and_git() {
        let w = ReviewWizard::embedded();
        let mut prev = clean();
        prev.insert("upstream_fidelity".into(), "matches".into());
        prev.insert("q3".into(), "yes".into());
        prev.insert("doc_matches_behaviour".into(), "other".into());
        let p = w.prefill(&prev, NONE);
        assert!(!p.contains_key("upstream_fidelity") && !p.contains_key("q3"));
        assert!(!p.contains_key("doc_matches_behaviour") && !p.contains_key("rung"));
        assert_eq!(p.len(), clean().len() - 1, "the rung is no longer an answer (2026-10-07)");
        assert!(w.prefill(&prev, BOTH).contains_key("upstream_fidelity"));
        assert_eq!(
            independence_prefill(true, AuthorshipKind::Agent),
            "someone_else"
        );
        assert_eq!(
            independence_prefill(true, AuthorshipKind::Human),
            "self_check"
        );
        assert_eq!(
            independence_prefill(true, AuthorshipKind::Mixed),
            "self_check"
        );
        assert_eq!(
            independence_prefill(false, AuthorshipKind::Human),
            "someone_else"
        );
        assert_eq!(
            independence_prefill(false, AuthorshipKind::Mixed),
            "someone_else"
        );
        let o = w
            .question("independence")
            .unwrap()
            .option(independence_prefill(true, AuthorshipKind::Human));
        assert!(o.is_some());
    }
}
