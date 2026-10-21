// Part of the kovan port of citeproc-js (GitHub #790).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/disambig_names.js (CSL.Registry.NameReg)
// Version:     2.4.63, commit 73bc1b44bc7d54d0bfec4e070fd27f5efe024ff9
// Copyright:   (c) 2009-2019 Frank Bennett
// Licence:     AGPL-3.0, taken from upstream's "CPAL-1.0 or AGPL-3.0-or-later"
//              (LICENSE at the commit above; see this crate's NOTICE).
// Modified:    2026-10-08, by the OUTRAM PARK contributors. This file is a
//              Rust translation (port) of the files named above, modified
//              from the original.
// No warranty: this program is distributed in the hope that it will be
//              useful, but WITHOUT ANY WARRANTY; without even the implied
//              warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR
//              PURPOSE. See the GNU Affero General Public License.

//! `CSL.Registry.NameReg`: which names (family name, initials form, full
//! given name) have been seen on which items, so that the disambiguator and
//! the name renderer know when a family name or a set of initials is shared.
//!
//! The JS object is a closure bundle: `addname`, `evalname` and `delitems`
//! share the variables `pkey`, `ikey` and `skey`. Here [`NameReg`] holds the
//! data and the three operations are free functions over the state
//! ([`addname`], [`evalname`], [`delitems`]), each computing its own keys with
//! [`set_keys`].
//!
//! Upstream calls `state.nameOutput.getName(nameobj, "locale-translit", true)`
//! and `CSL.Util.Names.initializeWith(state, skey, "%s")`: [`registry_name`] and
//! [`initialize_with`] call the names port (util_names_render.rs, util_names.rs).

use std::collections::BTreeMap;
use std::sync::LazyLock;

use regex::Regex;
use serde_json::Value;

use super::js;
use super::state::State;
use super::CslResult;

/// `namereg[pkey].ikey[ikey].skey[skey]`: `{items}`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SKeyEntry {
    /// `items`.
    pub items: Vec<String>,
}

/// `namereg[pkey].ikey[ikey]`: `{count, skey, items}`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct IKeyEntry {
    /// `count`: the number of distinct `skey`s.
    pub count: i64,
    /// `skey`.
    pub skey: BTreeMap<String, SKeyEntry>,
    /// `items`.
    pub items: Vec<String>,
}

/// `namereg[pkey]`: `{count, ikey, items}`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PKeyEntry {
    /// `count`: the number of distinct `ikey`s.
    pub count: i64,
    /// `ikey`.
    pub ikey: BTreeMap<String, IKeyEntry>,
    /// `items`.
    pub items: Vec<String>,
}

/// `new CSL.Registry.NameReg(state)`.
///
/// `nameindpkeys` (upstream keeps it "for restoring state following
/// preview") and `itemkeyreg` are write-only or never used upstream and are
/// not kept.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct NameReg {
    /// `namereg`.
    pub namereg: BTreeMap<String, PKeyEntry>,
    /// `nameind`: by item id, the `pkey::ikey::skey` strings it registered,
    /// in registration order.
    pub nameind: BTreeMap<String, Vec<String>>,
}

static PERIODS: LazyLock<Regex> = LazyLock::new(|| rx(r"\."));
static WS_RUN: LazyLock<Regex> = LazyLock::new(|| rx(&format!("[{}]+", js::WS)));
static WS_END: LazyLock<Regex> = LazyLock::new(|| rx(&format!("[{}]+$", js::WS)));
/// `/[,\!]* ([^,]+)$/`.
static LOWER_SUFFIX: LazyLock<Regex> = LazyLock::new(|| rx(r"[,!]* ([^,]+)$"));
/// `/[,\!]* [^,]+$/`.
static LOWER_SUFFIX_CUT: LazyLock<Regex> = LazyLock::new(|| rx(r"[,!]* [^,]+$"));

fn rx(src: &str) -> Regex {
    #[allow(clippy::expect_used)]
    Regex::new(src).expect("static regex")
}

/// `strip_periods(str)`: periods to spaces, runs of white space to one,
/// trailing white space dropped. A falsy `str` is `""`.
fn strip_periods(s: &str) -> String {
    let a = PERIODS.replace_all(s, " ");
    let b = WS_RUN.replace_all(&a, " ");
    WS_END.replace(&b, "").into_owned()
}

/// A string-valued property of a name object (`undefined` and other falsy
/// values read as `""`; JS would pass them to `strip_periods`, which
/// replaces falsy with `""`).
fn name_part(nameobj: &Value, key: &str) -> String {
    match nameobj.get(key) {
        Some(v) if js::truthy(v) => js::to_js_string(v),
        _ => String::new(),
    }
}

/// `state.nameOutput.getName(nameobj, "locale-translit", true).name`
/// (util_names_render.js:858). The caller's name is not mutated here (upstream
/// deletes `family`/`given` from a literal name in place: candidate C35).
fn registry_name(state: &State, nameobj: &Value) -> CslResult<Value> {
    let mut n = nameobj.clone();
    let got = state.name_output_get_name(&mut n, "locale-translit", true, None)?;
    Ok(Value::Object(got.name.unwrap_or_default()))
}

/// `CSL.Util.Names.initializeWith(state, name, terminator)` (util_names.js:42).
fn initialize_with(state: &State, name: &str, terminator: &str) -> CslResult<String> {
    Ok(super::util_names::initialize_with(state, name, terminator, false))
}

/// `set_keys(state, itemid, nameobj)`: the `(pkey, ikey, skey)` of a name.
fn set_keys(state: &State, itemid: &str, nameobj: &Value) -> CslResult<(String, String, String)> {
    let mut pkey = strip_periods(&name_part(nameobj, "family"));

    if js::get_str(&state.opt, "demote-non-dropping-particle") == Some("never")
        && js::truthy_opt(nameobj.get("non-dropping-particle"))
        && js::truthy_opt(nameobj.get("family"))
    {
        pkey = format!("{pkey} {}", name_part(nameobj, "non-dropping-particle"));
    }

    let mut skey = strip_periods(&name_part(nameobj, "given"));
    // Drop lowercase suffixes (such as et al.) from given name field
    // for disambiguation purposes.
    if let Some(m) = LOWER_SUFFIX.captures(&skey) {
        let tail = m.get(1).map(|g| g.as_str()).unwrap_or("");
        if tail == tail.to_lowercase() {
            skey = LOWER_SUFFIX_CUT.replace(&skey, "").into_owned();
        }
    }
    // The %s terminator enables normal initialization behavior
    // with non-Byzantine names.
    let ikey = initialize_with(state, &skey, "%s")?;
    if js::get_str(&state.citation.opt, "givenname-disambiguation-rule") == Some("by-cite") {
        pkey = format!("{itemid}{pkey}");
    }
    Ok((pkey, ikey, skey))
}

/// `namereg.evalname(item_id, nameobj, namenum, request_base, form,
/// initials)`: the given-name level (0 short form, 1 initials, 2 full) a name
/// needs. `None` is upstream's fall-through `undefined`. `initials` is
/// `Some(..)` when the name is rendered with `initialize-with` (a string in
/// JS).
pub fn evalname(
    state: &mut State,
    item_id: &str,
    nameobj: &Value,
    namenum: i64,
    request_base: i64,
    form: Option<&str>,
    initials: Option<&str>,
) -> CslResult<Option<i64>> {
    // XXX THIS CAN NO LONGER HAPPEN
    if js::slice(&state.tmp.area, 0, Some(12)) == "bibliography" && form.is_none_or(str::is_empty) {
        return Ok(Some(if initials.is_some() { 1 } else { 2 }));
    }
    let nameobj = registry_name(state, nameobj)?;
    let (pkey, ikey, _skey) = set_keys(state, item_id, &nameobj)?;
    //
    // possible options are:
    //
    // <option disambiguate-add-givenname value="true"/> (a)
    // <option disambiguate-add-givenname value="all-names"/> (a)
    // <option disambiguate-add-givenname value="all-names-with-initials"/> (b)
    // <option disambiguate-add-givenname value="primary-name"/> (d)
    // <option disambiguate-add-givenname value="primary-name-with-initials"/> (e)
    // <option disambiguate-add-givenname value="by-cite"/> (g)
    //
    let mut param: i64 = 2;
    let dagopt = js::truthy_opt(state.opt.get("disambiguate-add-givenname"));
    let gdropt_orig: Option<String> = state
        .citation
        .opt
        .get("givenname-disambiguation-rule")
        .filter(|v| js::truthy(v))
        .map(js::to_js_string);
    let gdropt: Option<String> = if gdropt_orig.as_deref() == Some("by-cite") {
        Some("all-names".to_string())
    } else {
        gdropt_orig.clone()
    };
    //
    // set initial value
    //
    if form == Some("short") {
        param = 0;
    } else if initials.is_some() {
        param = 1;
    }
    //
    // give literals a pass
    let reg = &state.registry.namereg.namereg;
    let Some(p) = reg.get(&pkey).filter(|p| p.ikey.contains_key(&ikey)) else {
        return Ok(Some(param));
    };
    //
    // adjust value upward if appropriate -- only if running
    // a non-names-global disambiguation strategy
    //
    if gdropt_orig.as_deref() == Some("by-cite") && param <= request_base {
        //param = request_base;
        return Ok(Some(request_base));
    }
    if !dagopt {
        return Ok(Some(param));
    }
    if gdropt
        .as_deref()
        .map(|g| js::slice(g, 0, Some(12)) == "primary-name")
        .unwrap_or(false)
        && namenum > 0
    {
        return Ok(Some(param));
    }
    //
    // the last composite condition is for backward compatibility
    //
    let g = gdropt.as_deref();
    if g.is_none() || g == Some("all-names") || g == Some("primary-name") {
        if p.count > 1 {
            param = 1;
        }
        let ikey_count = p.ikey.get(&ikey).map(|i| i.count).unwrap_or(0);
        if ikey_count > 1 || (p.count > 1 && initials.is_none()) {
            param = 2;
        }
    } else if g == Some("all-names-with-initials") || g == Some("primary-name-with-initials") {
        if p.count > 1 {
            param = 1;
        } else {
            param = 0;
        }
    }
    if !state.registry.registry.contains_key(item_id) {
        if form == Some("short") {
            return Ok(Some(0));
        } else if initials.is_some() {
            return Ok(Some(1));
        }
        Ok(None)
    } else {
        Ok(Some(param))
    }
}

/// `namereg.delitems(ids)`: unregister the names of the items `ids`. Returns
/// the ids of other items using the same names; upstream's `ret` is never
/// filled in, so this is always empty.
pub fn delitems(state: &mut State, ids: &[String]) -> CslResult<BTreeMap<String, bool>> {
    // ret carries the IDs of other items using this name.
    let ret = BTreeMap::new();
    for id in ids {
        let Some(fullkeys) = state.registry.namereg.nameind.get(id).cloned() else {
            continue;
        };
        for fullkey in fullkeys {
            let key: Vec<&str> = fullkey.split("::").collect();
            let pkey = key.first().copied().unwrap_or("").to_string();
            let ikey = key.get(1).copied().unwrap_or("").to_string();
            let skey = key.get(2).copied().unwrap_or("").to_string();
            // Skip names that have been deleted already.
            // Needed to clear integration DisambiguateAddGivenname1.txt
            // and integration DisambiguateAddGivenname2.txt
            if !state.registry.namereg.namereg.contains_key(&pkey) {
                continue;
            }
            // This was really, really unperceptive. They key elements
            // have absolutely nothing to do with whether there was ever
            // a registration at each key level.
            let has_skey = !skey.is_empty()
                && state
                    .registry
                    .namereg
                    .namereg
                    .get(&pkey)
                    .and_then(|p| p.ikey.get(&ikey))
                    .map(|i| i.skey.contains_key(&skey))
                    .unwrap_or(false);
            if has_skey {
                let mut tainted: Vec<String> = Vec::new();
                if let Some(p) = state.registry.namereg.namereg.get_mut(&pkey) {
                    if let Some(i) = p.ikey.get_mut(&ikey) {
                        let emptied = if let Some(s) = i.skey.get_mut(&skey) {
                            if let Some(posb) = s.items.iter().position(|x| x == id) {
                                s.items.remove(posb);
                            }
                            s.items.is_empty()
                        } else {
                            false
                        };
                        if emptied {
                            i.skey.remove(&skey);
                            i.count -= 1;
                            if i.count < 2 {
                                tainted.extend(i.items.iter().cloned());
                            }
                        }
                    }
                }
                for t in tainted {
                    state.tmp.tainted_item_ids.insert(t, true);
                }
            }
            if !ikey.is_empty()
                && state
                    .registry
                    .namereg
                    .namereg
                    .get(&pkey)
                    .map(|p| p.ikey.contains_key(&ikey))
                    .unwrap_or(false)
            {
                let mut tainted: Vec<String> = Vec::new();
                if let Some(p) = state.registry.namereg.namereg.get_mut(&pkey) {
                    let emptied = if let Some(i) = p.ikey.get_mut(&ikey) {
                        if let Some(posb) = i.items.iter().position(|x| x == id) {
                            i.items.remove(posb);
                        }
                        i.items.is_empty()
                    } else {
                        false
                    };
                    if emptied {
                        p.ikey.remove(&ikey);
                        p.count -= 1;
                        if p.count < 2 {
                            tainted.extend(p.items.iter().cloned());
                        }
                    }
                }
                for t in tainted {
                    state.tmp.tainted_item_ids.insert(t, true);
                }
            }
            if !pkey.is_empty() {
                let mut drop_p = false;
                if let Some(p) = state.registry.namereg.namereg.get_mut(&pkey) {
                    if let Some(posb) = p.items.iter().position(|x| x == id) {
                        p.items.remove(posb);
                    }
                    drop_p = p.items.len() < 2;
                }
                if drop_p {
                    state.registry.namereg.namereg.remove(&pkey);
                }
            }
            if let Some(list) = state.registry.namereg.nameind.get_mut(id) {
                list.retain(|k| *k != fullkey);
            }
        }
        state.registry.namereg.nameind.remove(id);
    }
    Ok(ret)
}

/// `namereg.addname(item_id, nameobj, pos)`: register a name of an item.
///
/// Run ALL renderings with disambiguate-add-givenname set to a value with
/// the by-cite behaviour, and then set the names-based expanded form when the
/// final makeCitationCluster rendering is output (upstream's comment).
pub fn addname(state: &mut State, item_id: &str, nameobj: &Value, pos: i64) -> CslResult<()> {
    let nameobj = registry_name(state, nameobj)?;

    if let Some(rule) = js::get_str(&state.citation.opt, "givenname-disambiguation-rule") {
        if js::slice(rule, 0, Some(8)) == "primary-" && pos != 0 {
            return Ok(());
        }
    }

    // A hack. Safe if the name object is used only here, for disambiguation purposes.
    let (pkey, ikey, skey) = set_keys(state, item_id, &nameobj)?;
    let item = item_id.to_string();
    // pkey, ikey and skey should be stored in separate cascading objects.
    // there should also be a kkey, on each, which holds the item ids using
    // that form of the name.
    let mut tainted: Vec<String> = Vec::new();
    let nr = &mut state.registry.namereg;
    if !pkey.is_empty() {
        match nr.namereg.get_mut(&pkey) {
            None => {
                nr.namereg.insert(
                    pkey.clone(),
                    PKeyEntry {
                        count: 0,
                        ikey: BTreeMap::new(),
                        items: vec![item.clone()],
                    },
                );
            }
            Some(p) => {
                if !p.items.contains(&item) {
                    p.items.push(item.clone());
                }
            }
        }
    }
    if !pkey.is_empty() && !ikey.is_empty() {
        if let Some(p) = nr.namereg.get_mut(&pkey) {
            if !p.ikey.contains_key(&ikey) {
                p.ikey.insert(
                    ikey.clone(),
                    IKeyEntry {
                        count: 0,
                        skey: BTreeMap::new(),
                        items: vec![item.clone()],
                    },
                );
                p.count += 1;
                if p.count == 2 {
                    tainted.extend(p.items.iter().cloned());
                }
            } else if let Some(i) = p.ikey.get_mut(&ikey) {
                if !i.items.contains(&item) {
                    i.items.push(item.clone());
                }
            }
        }
    }
    if !pkey.is_empty() && !ikey.is_empty() && !skey.is_empty() {
        if let Some(i) = nr
            .namereg
            .get_mut(&pkey)
            .and_then(|p| p.ikey.get_mut(&ikey))
        {
            if !i.skey.contains_key(&skey) {
                i.skey.insert(
                    skey.clone(),
                    SKeyEntry {
                        items: vec![item.clone()],
                    },
                );
                i.count += 1;
                if i.count == 2 {
                    tainted.extend(i.items.iter().cloned());
                }
            } else if let Some(s) = i.skey.get_mut(&skey) {
                if !s.items.contains(&item) {
                    s.items.push(item.clone());
                }
            }
        }
    }
    let ind = nr.nameind.entry(item.clone()).or_default();
    if !pkey.is_empty() {
        let full = format!("{pkey}::{ikey}::{skey}");
        if !ind.contains(&full) {
            ind.push(full);
        }
    }
    for t in tainted {
        state.tmp.tainted_item_ids.insert(t, true);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_periods_and_lowercase_suffix_follow_upstream() {
        assert_eq!(strip_periods("J.R.  R.  "), "J R R");
        assert_eq!(strip_periods(""), "");
        // `/[,\!]* ([^,]+)$/`: "John et al" has the lower-case tail "et al", which is cut.
        let m = LOWER_SUFFIX
            .captures("John et al")
            .map(|c| c[1].to_string());
        assert_eq!(m.as_deref(), Some("et al"));
        assert_eq!(LOWER_SUFFIX_CUT.replace("Anna jr", "").as_ref(), "Anna");
    }

    #[test]
    fn registering_a_name_records_its_keys() {
        let mut s = State::default();
        let name = serde_json::json!({"family": "Doe", "given": "Jane"});
        assert!(addname(&mut s, "ITEM-1", &name, 0).is_ok());
        assert!(s.registry.namereg.namereg.contains_key("Doe"));
        assert_eq!(delitems(&mut s, &["ITEM-1".to_string()]).unwrap().len(), 0);
    }
}
