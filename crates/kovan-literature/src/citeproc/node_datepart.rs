// Part of the kovan port of citeproc-js (GitHub #790).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/node_datepart.js
// Version:     2.4.63, commit 73bc1b44bc7d54d0bfec4e070fd27f5efe024ff9
// Copyright:   (c) 2009-2019 Frank Bennett
// Licence:     AGPL-3.0, taken from upstream's "CPAL-1.0 or AGPL-3.0-or-later"
//              (LICENSE at the commit above; see this crate's NOTICE).
// Modified:    2026-10-08, by the OUTRAM PARK contributors. This file is a
//              Rust translation (port) of the file named above, modified
//              from the original.
// No warranty: this program is distributed in the hope that it will be
//              useful, but WITHOUT ANY WARRANTY; without even the implied
//              warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR
//              PURPOSE. See the GNU Affero General Public License.

//! Port of `src/node_datepart.js`: `CSL.Node["date-part"]`.

use serde_json::Value;

use super::exec::Exec;
use super::js;
use super::obj_blob::{Blob, BlobChild, BlobContent, BlobId, BlobKind};
use super::obj_number::{new_numeric_blob, set_formatter, NumArg, NumFormatter};
use super::obj_token::Token;
use super::queue::{self, AppendArg, FormatRef, QueueId, StringParent};
use super::state::State;
use super::util_dates;
use super::util_locale::Locale;
use super::{CslResult, EngineError};

/// The closures `src/node_datepart.js` stores in `token.execs`
/// (PORTING.md §4).
#[derive(Debug, Clone, PartialEq)]
pub enum NodeDatepartExec {
    /// The one closure of `cs:date-part`: render one date part
    /// (node_datepart.js:38-305). `date_variable` is
    /// `state.build.date_variables[0]` at build time.
    Render {
        /// `date_variable`.
        date_variable: Option<String>,
    },
}

impl NodeDatepartExec {
    /// Run the closure.
    pub fn run(
        &self,
        state: &mut State,
        token: &mut Token,
        item: &Value,
        _cite_item: &Value,
    ) -> CslResult<Option<usize>> {
        match self {
            NodeDatepartExec::Render { date_variable } => {
                render(state, token, item, date_variable.as_deref())
            }
        }
    }
}

/// A JS value as `cs:date-part` handles it: the date field read from the
/// date object, or what [`format_and_strip`] returns (a string for any
/// truthy input, the falsy input otherwise).
#[derive(Debug, Clone, PartialEq)]
enum Dv {
    /// `undefined`.
    Undef,
    /// `null`.
    Null,
    /// A boolean (`false` after the year-0 and year-suffix suppressions).
    Bool(bool),
    /// A number.
    Num(f64),
    /// A string.
    Str(String),
}

impl Dv {
    fn from_value(v: Option<&Value>) -> Dv {
        match v {
            None => Dv::Undef,
            Some(Value::Null) => Dv::Null,
            Some(Value::Bool(b)) => Dv::Bool(*b),
            Some(Value::Number(n)) => Dv::Num(n.as_f64().unwrap_or(f64::NAN)),
            Some(Value::String(s)) => Dv::Str(s.clone()),
            Some(other) => Dv::Str(js::to_js_string(other)),
        }
    }

    /// The value as the JSON the `CSL.Util.Dates` formatters take.
    fn to_value(&self) -> Value {
        match self {
            Dv::Undef | Dv::Null => Value::Null,
            Dv::Bool(b) => Value::Bool(*b),
            Dv::Num(n) => {
                if n.fract() == 0.0 && n.abs() < 9.0e15 {
                    Value::from(*n as i64)
                } else {
                    serde_json::Number::from_f64(*n)
                        .map(Value::Number)
                        .unwrap_or(Value::Null)
                }
            }
            Dv::Str(s) => Value::String(s.clone()),
        }
    }

    /// JS truthiness.
    fn truthy(&self) -> bool {
        match self {
            Dv::Undef | Dv::Null => false,
            Dv::Bool(b) => *b,
            Dv::Num(n) => *n != 0.0 && !n.is_nan(),
            Dv::Str(s) => !s.is_empty(),
        }
    }

    /// `"" + value`.
    fn js_string(&self) -> String {
        match self {
            Dv::Undef => "undefined".to_string(),
            Dv::Null => "null".to_string(),
            Dv::Bool(b) => b.to_string(),
            Dv::Num(n) => js::number_to_js_string(*n),
            Dv::Str(s) => s.clone(),
        }
    }

    /// `last_string_output = value`: the string of a truthy value, else the
    /// empty string (a falsy `last_string_output` is never read).
    fn js_string_if_truthy(&self) -> String {
        if self.truthy() {
            self.js_string()
        } else {
            String::new()
        }
    }

    /// `parseInt(value, 10)`; `None` is `NaN`.
    fn parse_int(&self) -> Option<i64> {
        js::parse_int(&self.js_string())
    }

    /// `value != 1` is false, i.e. `value == 1` (loose equality).
    fn loosely_one(&self) -> bool {
        match self {
            Dv::Undef | Dv::Null => false,
            Dv::Bool(b) => *b,
            Dv::Num(n) => *n == 1.0,
            Dv::Str(s) => {
                let t = js::trim(s);
                !t.is_empty() && t.parse::<f64>().map(|n| n == 1.0).unwrap_or(false)
            }
        }
    }

    /// `value === other` for a stored `years_used` entry.
    fn strict_eq(&self, other: &Value) -> bool {
        match (self, other) {
            (Dv::Undef, Value::Null) | (Dv::Null, Value::Null) => true,
            (Dv::Bool(a), Value::Bool(b)) => a == b,
            (Dv::Num(a), Value::Number(b)) => b.as_f64() == Some(*a),
            (Dv::Str(a), Value::String(b)) => a == b,
            _ => false,
        }
    }
}

/// `state.output.append(value, tokname)` (or the same on `dateput`) for a
/// value of any JS type: `undefined` appends nothing, a string or number is
/// the ordinary `append`, and a falsy non-string (`false`, `null`) makes
/// `new CSL.Blob(false, token)`, a blob with an empty children array
/// (obj_blob.js: `"string" === typeof str`, else `str ? [str] : []`).
fn append_dv(state: &mut State, q: QueueId, v: &Dv, tok: FormatRef) -> CslResult<bool> {
    match v {
        Dv::Undef => Ok(false),
        Dv::Str(s) => queue::append_simple(state, q, s.as_str(), tok),
        Dv::Num(n) => queue::append(state, q, AppendArg::Number(*n), tok, false, false, false),
        Dv::Bool(_) | Dv::Null => {
            // The part of queue.js `append` that precedes the string branch.
            if state.tmp.doing_macro_with_date {
                return Ok(false);
            }
            if state.tmp.element_trace.value().map(String::as_str) == Some("suppress-me") {
                return Ok(false);
            }
            let token = match tok {
                FormatRef::Token(t) => t,
                FormatRef::None => state_queue_empty(state, q),
                FormatRef::Name(n) => queue::get_token(state, q, &n).ok_or_else(|| {
                    EngineError::Csl(format!(
                        "CSL processor error: unknown format token name: {n}"
                    ))
                })?,
            };
            let mut token = token;
            if !token.strings.contains_key("delimiter") {
                token.set_string("delimiter", "");
            }
            let blob = Blob::new(None, Some(&token), None);
            let curr = queue::current(state, q).ok_or_else(|| {
                EngineError::Csl("Cannot read properties of undefined (reading 'push')".into())
            })?;
            let id = state.blobs.add(blob);
            state.blobs.push(curr, id)?;
            Ok(true)
        }
    }
}

fn state_queue_empty(state: &State, q: QueueId) -> Token {
    match q {
        QueueId::Output => state.output.empty.clone(),
        QueueId::Dateput => state.dateput.empty.clone(),
    }
}

/// `state.output.append(value, this)` for the part's own token. As upstream
/// does on the first `append`, give `this` an empty `delimiter` if it has none.
fn append_this(state: &mut State, q: QueueId, v: &Dv, token: &mut Token) -> CslResult<bool> {
    if !token.strings.contains_key("delimiter") {
        token.set_string("delimiter", "");
    }
    append_dv(state, q, v, FormatRef::Token(token.clone()))
}

/// `state.locale[state.opt.lang]`.
fn current_locale(state: &State) -> CslResult<&Locale> {
    let lang = js::get_string(&state.opt, "lang").unwrap_or_default();
    state.locale.get(&lang).ok_or_else(|| {
        EngineError::Csl("Cannot read properties of undefined (reading 'noun-genders')".into())
    })
}

fn type_error_not_function(name: &str, form: &str) -> EngineError {
    EngineError::Csl(format!(
        "TypeError: CSL.Util.Dates.{name}[\"{form}\"] is not a function"
    ))
}

/// `"" + CSL.Util.Dates[name][form](state, val, gender, default_locale)`.
///
/// The formatters return `undefined` in places (month names without a
/// term, a non-four-digit `short` year); `"" + undefined` is `"undefined"`,
/// as upstream.
fn call_dates_formatter(
    state: &mut State,
    name: &str,
    form: &str,
    val: &Value,
    gender: Option<&str>,
    default_locale: bool,
) -> CslResult<String> {
    let undef = |o: Option<String>| o.unwrap_or_else(|| "undefined".to_string());
    match (name, form) {
        ("year", "long") => Ok(util_dates::year_long(val)),
        ("year", "short") => Ok(undef(util_dates::year_short(val)?)),
        ("year", "numeric") => Ok(util_dates::year_numeric(val)),
        // `imperial(state, num, end)` receives the *gender* as `end`.
        ("year", "imperial") => {
            state.year_imperial(val, gender.map(|g| !g.is_empty()).unwrap_or(false))
        }
        ("month", "numeric") => Ok(js::to_js_string(&util_dates::month_numeric(val))),
        ("month", "numeric-leading-zeros") => Ok(util_dates::month_numeric_leading_zeros(val)),
        ("month", "long") => Ok(undef(util_dates::month_long(
            state,
            val,
            gender,
            default_locale,
        ))),
        ("month", "short") => Ok(undef(util_dates::month_short(
            state,
            val,
            gender,
            default_locale,
        ))),
        ("day", "numeric") | ("day", "long") => util_dates::day_numeric(val),
        ("day", "numeric-leading-zeros") => Ok(util_dates::day_numeric_leading_zeros(val)),
        ("day", "ordinal") => util_dates::day_ordinal(state, val, gender),
        _ => Err(type_error_not_function(name, form)),
    }
}

/// `formatAndStrip.call(token, myform, gender, val)` inside
/// `CSL.Node["date-part"].build`.
fn format_and_strip(
    state: &mut State,
    token: &Token,
    myform: &str,
    gender: Option<&str>,
    val: Dv,
) -> CslResult<Dv> {
    if !val.truthy() {
        return Ok(val);
    }
    let name = token.string("name");
    let default_locale = js::truthy_opt(token.extra.get("default_locale"));
    let mut out = call_dates_formatter(
        state,
        &name,
        myform,
        &val.to_value(),
        gender,
        default_locale,
    )?;
    if name == "month" {
        if state.tmp.strip_periods != 0 {
            out = out.replace('.', "");
        } else {
            for d in &token.decorations {
                if d.name == "@strip-periods" && d.value == "true" {
                    out = out.replace('.', "");
                    break;
                }
            }
        }
    }
    Ok(Dv::Str(out))
}

/// `state.getTerm(name)` as the date-part closure calls it, as a value
/// that is `false` when the term is absent or empty.
fn term_or_false(state: &mut State, name: &str) -> CslResult<Option<String>> {
    Ok(state
        .get_term(name, None, None, None, None, false)?
        .filter(|t| !t.is_empty()))
}

/// `STUB(registry.rs)`: `state.registry.registry[Item.id]` and its
/// `disambig.year_suffix`. `None` when the registry has no entry for the
/// item (the registry is the engine agent's, and does not exist yet).
// PORT-LATER(w2-engine): registry.js — `state.registry.registry[Item.id].disambig.year_suffix`.
fn registry_year_suffix(_state: &State, _id: &str) -> Option<Value> {
    None
}

/// The closure of `CSL.Node["date-part"].build` (node_datepart.js:38-305).
fn render(
    state: &mut State,
    token: &mut Token,
    item: &Value,
    date_variable: Option<&str>,
) -> CslResult<Option<usize>> {
    if !js::truthy(&state.tmp.date_object) {
        return Ok(None);
    }
    state.tmp.probably_rendered_something = true;

    let name = token.string("name");
    // `last_string_output`: only its truthiness and its string form are read.
    let mut last_string_output = String::new();
    // `first_date` is assigned `true` at the top of the closure and is never
    // set to `false`.
    let first_date = true;
    state.tmp.donesies.push(name.clone());

    let date_object = state.tmp.date_object.clone();
    let field = |k: &str| Dv::from_value(date_object.get(k));

    // Render literal only when year is included in date output
    if js::truthy_opt(date_object.get("literal")) && name == "year" {
        let lit = js::to_js_string(date_object.get("literal").unwrap_or(&Value::Null));
        last_string_output = lit.clone();
        append_this(state, QueueId::Output, &Dv::Str(lit), token)?;
    }

    let mut value = field(&name);
    let mut value_end = field(&format!("{name}_end"));
    if name == "year" && value == Dv::Num(0.0) && !state.tmp.suppress_decorations {
        value = Dv::Bool(false);
    }
    let real = !state.tmp.suppress_decorations;
    let have_collapsed = state.tmp.have_collapsed;
    let collapse = state
        .area_ref(&state.tmp.area)
        .opt
        .get("collapse")
        .cloned()
        .unwrap_or(Value::Null);
    let invoked =
        collapse.as_str() == Some("year-suffix") || collapse.as_str() == Some("year-suffix-ranged");
    let precondition = js::truthy_opt(state.opt.get("disambiguate-add-year-suffix"));
    if real && precondition && invoked {
        state.tmp.years_used.push(value.to_value());
        let known_year = state.tmp.last_years_used.len() >= state.tmp.years_used.len();
        if known_year && have_collapsed {
            let idx = state.tmp.years_used.len() - 1;
            if state
                .tmp
                .last_years_used
                .get(idx)
                .map(|v| value.strict_eq(v))
                .unwrap_or(value == Dv::Undef)
            {
                value = Dv::Bool(false);
            }
        }
    }

    if value != Dv::Undef {
        let mut bc: Option<String> = None;
        let mut ad: Option<String> = None;
        if name == "year" {
            if let Some(v) = value.parse_int() {
                if v < 500 && v > 0 {
                    ad = term_or_false(state, "ad")?;
                }
                if v < 0 {
                    bc = term_or_false(state, "bc")?;
                    value = Dv::Num((v * -1) as f64);
                }
            }
            if value_end.truthy() {
                if let Some(v) = value_end.parse_int() {
                    // `ad_end` and `bc_end` are computed upstream and never read.
                    if v < 500 && v > 0 {
                        let _ad_end = term_or_false(state, "ad")?;
                    }
                    if v < 0 {
                        let _bc_end = term_or_false(state, "bc")?;
                        value_end = Dv::Num((v * -1) as f64);
                    }
                }
            }
        }

        // For gendered locales
        let mut monthnameid = field("month").js_string();
        while js::len(&monthnameid) < 2 {
            monthnameid = format!("0{monthnameid}");
        }
        let monthnameid = format!("month-{monthnameid}");
        let (gender, limit_day_ordinals) = {
            let loc = current_locale(state)?;
            (
                loc.noun_genders
                    .get(&monthnameid)
                    .filter(|g| js::truthy(g))
                    .map(js::to_js_string),
                js::truthy_opt(loc.opts.get("limit-day-ordinals-to-day-1")),
            )
        };
        if js::truthy_opt(token.strings.get("form")) {
            let myform = token.string("form");
            let mut myform_use = myform.clone();
            let mut myform_end = myform.clone();
            if name == "day" && myform == "ordinal" && limit_day_ordinals {
                if !value.loosely_one() {
                    myform_use = "numeric".to_string();
                }
                if !value_end.loosely_one() {
                    myform_end = "numeric".to_string();
                }
            }
            value = format_and_strip(state, token, &myform_use, gender.as_deref(), value)?;
            value_end = format_and_strip(state, token, &myform_end, gender.as_deref(), value_end)?;
        }
        queue::open_level(state, QueueId::Output, FormatRef::Name("empty".into()))?;
        if !state.tmp.date_collapse_at.is_empty() {
            let ready = state
                .tmp
                .date_collapse_at
                .iter()
                .all(|it| state.tmp.donesies.contains(it));
            if ready {
                if value_end.js_string() != "0" {
                    // (`first_date` is already true.)
                    // OK! So if the actual data has no month, day or season,
                    // and we reach this block, then we can combine the dates
                    // to a string, run minimial-two, and output the trailing
                    // year right here. No impact on other functionality.
                    let yrf = state.opt.get("year-range-format").cloned();
                    let yrf_set = js::truthy_opt(yrf.as_ref())
                        && yrf.as_ref().and_then(Value::as_str) != Some("expanded");
                    if yrf_set
                        && !js::truthy_opt(date_object.get("day"))
                        && !js::truthy_opt(date_object.get("month"))
                        && !js::truthy_opt(date_object.get("season"))
                        && name == "year"
                        && value.truthy()
                        && value_end.truthy()
                    {
                        // second argument adjusts collapse as required for years
                        // See OSCOLA section 1.3.2
                        let joined = format!("{}-{}", value.js_string(), value_end.js_string());
                        let mangled = state.fun.year_mangler.mangle(&joined, true)?;
                        let range_delimiter = state
                            .get_term("year-range-delimiter", None, None, None, None, false)?
                            .unwrap_or_else(|| "undefined".to_string());
                        let at = js::index_of(&mangled, &range_delimiter, 0);
                        value_end = Dv::Str(js::slice(&mangled, at + 1, None));
                    }
                    // (`last_string_output = value_end` is overwritten below.)
                    append_this(state, QueueId::Dateput, &value_end, token)?;
                    if first_date {
                        clear_first_prefix(state)?;
                    }
                }
                last_string_output = value.js_string_if_truthy();
                append_this(state, QueueId::Output, &value, token)?;
                let curr = queue::current(state, QueueId::Output).ok_or_else(no_level)?;
                if let Some(BlobChild::Blob(b)) = last_child(state, curr) {
                    state.blobs.get_mut(b).set_string("suffix", "");
                }

                match token
                    .strings
                    .get("range-delimiter")
                    .filter(|v| js::truthy(v))
                {
                    Some(rd) => {
                        let rd = js::to_js_string(rd);
                        queue::append_simple(state, QueueId::Output, rd, FormatRef::None)?;
                    }
                    None => {
                        let t = state.get_term(
                            "year-range-delimiter",
                            None,
                            None,
                            None,
                            None,
                            false,
                        )?;
                        match t {
                            Some(t) => {
                                queue::append_simple(state, QueueId::Output, t, "empty")?;
                            }
                            None => {}
                        }
                    }
                }
                queue::close_level(state, QueueId::Dateput, None)?;
                let dcurr = queue::current(state, QueueId::Dateput).ok_or_else(no_level)?;
                // `curr.blobs = curr.blobs.concat(dcurr)`: dcurr is the queue's
                // root array here, so its elements are appended.
                let extra: Vec<BlobChild> = if state.blobs.get(dcurr).kind == BlobKind::RootArray {
                    match &state.blobs.get(dcurr).blobs {
                        BlobContent::List(l) => l.clone(),
                        BlobContent::Text(_) => Vec::new(),
                    }
                } else {
                    vec![BlobChild::Blob(dcurr)]
                };
                if let BlobContent::List(l) = &mut state.blobs.get_mut(curr).blobs {
                    l.extend(extra);
                }
                // This may leave the stack pointer on a lower level.
                // It's not a problem because the stack will be clobbered
                // when the queue is initialized by the next cs:date node.
                let pending = queue::queue_children(state, QueueId::Dateput);
                queue::string(state, QueueId::Dateput, &pending, StringParent::None)?;
                let date_token = state.tmp.date_token.clone().ok_or_else(|| {
                    EngineError::Csl(
                        "Cannot read properties of undefined (reading 'strings')".into(),
                    )
                })?;
                queue::open_level(state, QueueId::Dateput, FormatRef::Token(date_token))?;
                state.tmp.date_collapse_at = Vec::new();
            } else {
                last_string_output = value.js_string_if_truthy();
                append_this(state, QueueId::Output, &value, token)?;
                if state.tmp.date_collapse_at.contains(&name) {
                    //
                    // Use ghost dateput queue
                    //
                    if value_end.js_string() != "0" {
                        //
                        // XXXXX: It's a workaround.  It's ugly.
                        // There's another one above.
                        //
                        queue::open_level(
                            state,
                            QueueId::Dateput,
                            FormatRef::Name("empty".into()),
                        )?;
                        last_string_output = value_end.js_string_if_truthy();
                        append_this(state, QueueId::Dateput, &value_end, token)?;
                        if first_date {
                            clear_first_prefix(state)?;
                        }
                        if let Some(bc) = &bc {
                            last_string_output = bc.clone();
                            queue::append_simple(
                                state,
                                QueueId::Dateput,
                                bc.as_str(),
                                FormatRef::None,
                            )?;
                        }
                        if let Some(ad) = &ad {
                            last_string_output = ad.clone();
                            queue::append_simple(
                                state,
                                QueueId::Dateput,
                                ad.as_str(),
                                FormatRef::None,
                            )?;
                        }
                        queue::close_level(state, QueueId::Dateput, None)?;
                    }
                }
            }
        } else {
            last_string_output = value.js_string_if_truthy();
            append_this(state, QueueId::Output, &value, token)?;
        }

        if let Some(bc) = &bc {
            last_string_output = bc.clone();
            queue::append_simple(state, QueueId::Output, bc.as_str(), FormatRef::None)?;
        }
        if let Some(ad) = &ad {
            last_string_output = ad.clone();
            queue::append_simple(state, QueueId::Output, ad.as_str(), FormatRef::None)?;
        }
        queue::close_level(state, QueueId::Output, None)?;
    } else if name == "month" {
        // XXX The simpler solution here will be to
        // directly install season and season_end on
        // month, with a value of 13, 14, 15, 16, or
        // (to allow correct ranging with Down Under
        // dates) 17 or 18.  That will allow ranging
        // to take place in the normal way.  With this
        // "approach", it doesn't.
        //
        // No value for this target variable
        //
        if js::truthy_opt(date_object.get("season")) {
            let season = js::to_js_string(date_object.get("season").unwrap_or(&Value::Null));
            // `value.match(/^[1-4]$/)`
            let is_season_number = matches!(season.as_str(), "1" | "2" | "3" | "4");
            value = Dv::Str(season.clone());
            if is_season_number {
                if let Some(g) = state.tmp.group_context.tip_mut() {
                    g.variable_success = true;
                }
                last_string_output = "winter".to_string();
                let t =
                    state.get_term(&format!("season-0{season}"), None, None, None, None, false)?;
                if let Some(t) = t {
                    append_this(state, QueueId::Output, &Dv::Str(t), token)?;
                }
            } else if !season.is_empty() {
                last_string_output = season.clone();
                append_this(state, QueueId::Output, &Dv::Str(season), token)?;
            }
        }
    }
    state.tmp.value = Vec::new();
    let date_var_truthy = date_variable
        .map(|v| js::truthy_opt(item.get(v)))
        .unwrap_or(false);
    if date_var_truthy
        && (value.truthy() || state.tmp.have_collapsed)
        && !js::truthy_opt(state.opt.get("has_year_suffix"))
        && name == "year"
        && !state.tmp.just_looking
    {
        let id = item.get("id").map(js::to_js_string).unwrap_or_default();
        let ys = registry_year_suffix(state, &id);
        if let Some(ys) = ys.filter(|v| *v != Value::Bool(false)) {
            if !state.tmp.has_done_year_suffix {
                state.tmp.has_done_year_suffix = true;
                last_string_output = "x".to_string();
                let num = match js::parse_int_value(&ys) {
                    Some(n) => NumArg::Number(n),
                    None => NumArg::Text("NaN".to_string()),
                };
                // first argument is for number particle [a-zA-Z], never present on dates
                let number = new_numeric_blob(state, None, num, Some(token), Some(&id))?;
                let build_area = state.build.area.clone();
                let layout_delimiter = state
                    .area_ref(&build_area)
                    .opt
                    .get("layout_delimiter")
                    .cloned();
                match &layout_delimiter {
                    Some(v) => {
                        token.extra.insert("successor_prefix".into(), v.clone());
                        token.extra.insert("splice_prefix".into(), v.clone());
                    }
                    None => {
                        token.extra.remove("successor_prefix");
                        token.extra.remove("splice_prefix");
                    }
                }
                set_formatter(state, number, NumFormatter::suffixator(None))?;
                let area_name = state.tmp.area.clone();
                let area_opt = state.area_ref(&area_name).opt.clone();
                if area_opt.get("collapse").and_then(Value::as_str) == Some("year-suffix-ranged") {
                    let rp = state.get_term(
                        "citation-range-delimiter",
                        None,
                        None,
                        None,
                        None,
                        false,
                    )?;
                    state.blobs.get_mut(number).range_prefix = rp;
                }
                let sp = if js::truthy_opt(area_opt.get("cite_group_delimiter")) {
                    area_opt.get("cite_group_delimiter").map(js::to_js_string)
                } else if js::truthy_opt(area_opt.get("year-suffix-delimiter")) {
                    area_opt.get("year-suffix-delimiter").map(js::to_js_string)
                } else {
                    area_opt.get("layout_delimiter").map(js::to_js_string)
                };
                let nb = state.blobs.get_mut(number);
                nb.successor_prefix = sp;
                nb.ugly_delimiter_suppress_hack = true;
                queue::append(
                    state,
                    QueueId::Output,
                    AppendArg::Blob(number),
                    FormatRef::Name("literal".into()),
                    false,
                    false,
                    false,
                )?;
            }
        }
    }
    let condition_set = state
        .tmp
        .group_context
        .tip()
        .map(|g| g.condition.is_some())
        .unwrap_or(false);
    if !last_string_output.is_empty() && !condition_set {
        // `last_string_output.match(/[0-9]$/)`
        state.tmp.just_did_number = last_string_output
            .chars()
            .last()
            .map(|c| c.is_ascii_digit())
            .unwrap_or(false);
        let tip = queue::current(state, QueueId::Output);
        if let Some(tip) = tip {
            if !state.blobs.get(tip).string("suffix").is_empty() {
                state.tmp.just_did_number = false;
            }
        }
    }
    Ok(None)
}

fn no_level() -> EngineError {
    EngineError::Csl("Cannot read properties of undefined (reading 'blobs')".into())
}

/// The last child of a list blob.
fn last_child(state: &State, id: BlobId) -> Option<BlobChild> {
    match &state.blobs.get(id).blobs {
        BlobContent::List(l) => l.last().cloned(),
        BlobContent::Text(_) => None,
    }
}

/// `blob = state.dateput.current.value().blobs[0]; if (blob) {
/// blob.strings.prefix = ""; }`.
fn clear_first_prefix(state: &mut State) -> CslResult<()> {
    let cur = queue::current(state, QueueId::Dateput).ok_or_else(no_level)?;
    let first = match &state.blobs.get(cur).blobs {
        BlobContent::List(l) => l.first().cloned(),
        BlobContent::Text(_) => None,
    };
    if let Some(BlobChild::Blob(b)) = first {
        state.blobs.get_mut(b).set_string("prefix", "");
    }
    Ok(())
}

/// `CSL.Node["date-part"].build.call(token, state, target)`.
pub fn build(
    state: &mut State,
    mut token: Token,
    target: &mut Vec<Token>,
    _real_group: Option<bool>,
) -> CslResult<()> {
    if !js::truthy_opt(token.strings.get("form")) {
        token.set_string("form", "long");
    }
    // used in node_date, to send a list of rendering date parts
    // to node_key, for dates embedded in macros.
    state.build.date_parts.push(token.string("name"));
    //
    // Set delimiter here, if poss.
    //
    let date_variable = state.build.date_variables.first().cloned();

    token
        .execs
        .push(Exec::NodeDatepart(NodeDatepartExec::Render {
            date_variable,
        }));
    target.push(token);
    Ok(())
}
