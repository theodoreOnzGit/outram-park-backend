// Part of the kovan port of citeproc-js (GitHub #790).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/obj_token.js
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

//! `CSL.Token`: one compiled step of a style (PORTING.md §3, §4).
//!
//! `build.js` turns each CSL element into a START and an END token (or one
//! SINGLETON), and the element's attributes push behaviour onto the token as
//! closures. Here the closures are [`Exec`] / [`Test`] enum values.
//!
//! JS tokens also carry ad-hoc properties set by particular builders
//! (`postponed_macro`, `dateparts`, `juris`, `and_term`, ...). The common ones
//! are fields below; a builder that needs another adds a field (with the JS
//! name in a comment) or, for rarely-read data, puts it in [`Token::extra`].

use serde_json::Value;

use super::exec::{Exec, Test};
use super::js::Obj;

/// `CSL.START`, `CSL.END`, `CSL.SINGLETON` (0, 1, 2 in citeproc-js).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TokenType {
    /// `CSL.START = 0`.
    #[default]
    Start,
    /// `CSL.END = 1`.
    End,
    /// `CSL.SINGLETON = 2`.
    Singleton,
}

impl TokenType {
    /// The integer citeproc-js uses (and the intermediate dump records).
    pub fn as_js(self) -> i64 {
        match self {
            TokenType::Start => 0,
            TokenType::End => 1,
            TokenType::Singleton => 2,
        }
    }
}

/// One formatting decoration, `["@font-style", "italic"]`: an attribute name
/// and its value, in `CSL.FORMAT_KEY_SEQUENCE` order on a token. A few
/// internal decorations carry a third element (`@showid`'s node id).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decoration {
    /// The attribute, with its `@`: `"@font-style"`, `"@quotes"`, ...
    pub name: String,
    /// The value: `"italic"`, `"true"`, ...
    pub value: String,
    /// A third array element, where upstream pushes one.
    pub extra: Option<String>,
}

impl Decoration {
    /// `[name, value]`.
    pub fn new(name: &str, value: &str) -> Decoration {
        Decoration {
            name: name.to_string(),
            value: value.to_string(),
            extra: None,
        }
    }
}

/// `CSL.Token`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Token {
    /// `name`: the CSL element (`"text"`, `"group"`, ...), or an internal
    /// name (`"output"`, `"empty"`).
    pub name: String,
    /// `tokentype`.
    pub tokentype: TokenType,
    /// `strings`: prefix, suffix, delimiter, form, text-case, ... An absent
    /// key is JS `undefined`. `Token::new` sets `prefix` and `suffix` to `""`
    /// and leaves `delimiter` absent, as the JS constructor does.
    pub strings: Obj,
    /// `decorations`.
    pub decorations: Vec<Decoration>,
    /// `variables`.
    pub variables: Vec<String>,
    /// `execs`: the token's behaviour, run in order by `CSL.tokenExec`.
    pub execs: Vec<Exec>,
    /// `tests`: condition closures (if / else-if / substitute / group).
    pub tests: Vec<Test>,
    /// Whether the JS `tests` property exists (`if (!this.tests) this.tests =
    /// []` ran, or it was assigned). Upstream creates the array lazily, so a
    /// token can have `tests: []` (the intermediate dump records `tests_n: 0`)
    /// or no `tests` at all. Set by every code path that creates it
    /// (attributes.js handlers, `if_start.tests = [...]`, `Conditions.Engine.addTest`).
    pub tests_defined: bool,
    /// `test`: the evaluator combining `tests` (`match="any|all|none"`).
    pub test: Option<Test>,
    /// `next`: the index of the following token (set by configure).
    pub next: Option<usize>,
    /// `succeed`: the jump target when `test` is true.
    pub succeed: Option<usize>,
    /// `fail`: the jump target when `test` is false.
    pub fail: Option<usize>,
    /// `postponed_macro`: the macro name a `<text macro="..">` expands.
    pub postponed_macro: Option<String>,
    /// Every other data property a builder sets on the token (`dateparts`,
    /// `juris`, `cslid`, `hasVariable`, ...), keyed by its JS name. The
    /// intermediate dump includes these.
    pub extra: Obj,
}

impl Token {
    /// `new CSL.Token(name, tokentype)`.
    pub fn new(name: &str, tokentype: TokenType) -> Token {
        let mut strings = Obj::new();
        strings.insert("prefix".into(), Value::String(String::new()));
        strings.insert("suffix".into(), Value::String(String::new()));
        Token {
            name: name.to_string(),
            tokentype,
            strings,
            ..Token::default()
        }
    }

    /// `CSL.Util.cloneToken(token)`. Note upstream copies `next`, `succeed`,
    /// `fail` and the other ad-hoc properties **not at all**: only name,
    /// tokentype, strings, decorations, variables, execs and tests.
    pub fn clone_token(&self) -> Token {
        let mut t = Token::new(&self.name, self.tokentype);
        for (k, v) in &self.strings {
            t.strings.insert(k.clone(), v.clone());
        }
        t.decorations = self.decorations.clone();
        t.variables = self.variables.clone();
        t.execs = self.execs.clone();
        t.tests = self.tests.clone();
        // `if (token.tests) newtok.tests = token.tests.slice()`
        t.tests_defined = self.tests_defined;
        t
    }

    /// `token.strings[key]` as a string, `""` when absent (the common
    /// `prefix`/`suffix` read).
    pub fn string(&self, key: &str) -> String {
        self.strings
            .get(key)
            .map(super::js::to_js_string)
            .unwrap_or_default()
    }

    /// `token.strings[key]` as a string, `None` when `undefined`.
    pub fn string_opt(&self, key: &str) -> Option<String> {
        self.strings.get(key).map(super::js::to_js_string)
    }

    /// `token.strings[key] = value` for a string value.
    pub fn set_string(&mut self, key: &str, value: &str) {
        self.strings
            .insert(key.to_string(), Value::String(value.to_string()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_token_has_empty_affixes_and_no_delimiter() {
        let t = Token::new("text", TokenType::Singleton);
        assert_eq!(t.string("prefix"), "");
        assert_eq!(t.string_opt("delimiter"), None);
        assert_eq!(t.tokentype.as_js(), 2);
        let mut u = t.clone_token();
        u.next = Some(3);
        u.set_string("delimiter", ", ");
        let v = u.clone_token();
        assert_eq!(v.next, None, "cloneToken does not copy jump indices");
        assert_eq!(v.string("delimiter"), ", ");
    }
}
