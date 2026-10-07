# Porting citeproc-js 2.4.63 to Rust — the rules (epic #790)

Every agent and human working on `src/citeproc/` follows this file. It exists
so that many people (and many agents, in parallel) produce **one** engine, not
several that disagree. Read it in full before touching a file here.

## 0. What "port" means here

- We translate `vendor/citeproc-js/src/*.js` (commit `73bc1b44`, citeproc-js
  2.4.63) **function by function, branch by branch**, keeping its quirks. The
  pass criterion is *citeproc-js's output*, not the CSL spec and not the
  fixture's `RESULT`: if citeproc-js does something odd, so do we.
- Do **not** redesign, simplify or "fix" upstream behaviour. If you are sure
  upstream is wrong, port it anyway and record it in the known-differences
  file only if the port's output then *differs* from citeproc-js (it should
  not).
- Do **not** consult other CSL engines (hayagriva, citeproc-rs, pandoc
  citeproc) for behaviour. The JS source is the only reference.
- Fetch the sources with `scripts/csl-reference.sh` (or see §9). They live
  in the git-ignored `vendor/`; never commit them.

## 1. Licence header (mandatory on every file)

citeproc-js is "CPAL-1.0 OR AGPL-3.0-or-later"; we take the AGPL option. Each
Rust file starts with this header, naming the JS file(s) it ports (keep the
list current as you add functions):

```text
// Part of the kovan port of citeproc-js (GitHub #790).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/<file>.js
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
```

The port never leaves an AGPL crate.

## 2. File layout: one JS file → one Rust file, same name

`src/node_text.js` → `src/citeproc/node_text.rs`. Every bundled JS file
already has its Rust file (pre-created, header only). Put a JS function in the
Rust file named after the JS file it comes from — that is how a reader finds
it. Above each ported function put a one-line doc comment naming the JS
function (`/// `CSL.Util.Dates.year.long` (util_dates.js).`).

Names: `CSL.Util.Dates.year.long` → `util_dates::year_long`;
`CSL.Engine.prototype.getTerm` → `State::get_term` (an `impl State` block in
the file the JS method lives in — Rust allows `impl State` in many files);
`CSL.NameOutput.prototype._renderOnePersonalName` → `NameOutput::render_one_personal_name`.
JS hyphenated keys become snake_case fields with a comment giving the JS key
when it is not obvious.

## 3. The JS object model in Rust

| JS | Rust |
|---|---|
| the engine (`this` in `CSL.Engine.prototype.*`, `state` in closures) | `state::State` (one struct, `&mut State` everywhere) |
| `state.opt`, `state.citation.opt`, ... (open-ended bags) | `js::Obj` (= `serde_json::Map<String, Value>`) with the typed accessors in `js.rs` |
| `state.tmp`, `state.build`, `state.fun`, `state.registry` | typed structs in `state.rs` (add the fields you need, JS name in a comment) |
| a CSL-JSON item (`Item`) and a citation item (`item`) | `serde_json::Value` (an object), passed as `&Value` |
| `CSL.Token` | `token::Token` (`strings` is a `js::Obj`, so `undefined` = key absent) |
| `CSL.Blob`, `CSL.NumericBlob` | `blob::Blob` in the arena `state.blobs`, referred to by `blob::BlobId` |
| an array mixing strings and blobs | `Vec<blob::BlobChild>` |
| `CSL.Stack` | `stack::Stack<T>` |
| a closure stored on a token (`token.execs.push(function (state, Item, item) {...})`) | a variant of that file's `...Exec` enum, carrying the captured variables **by value** (§4) |
| a condition closure (`token.tests.push(...)`, `token.test = ...`) | a variant of that file's `...Test` enum (§4) |
| `CSL.error(msg)` (throws) | `return Err(EngineError::Csl(msg))`; internal functions that can reach it return `CslResult<T>` and callers use `?` |
| `CSL.debug(msg)` | nothing (drop it) |
| `undefined` / `null` / `false` distinctions | `Option`, `Value::Null`, `false` — keep the distinction where the JS tests it (`=== undefined`, `"undefined" === typeof`) |
| JS truthiness (`if (x)`) on a `Value` | `js::truthy(&v)` — `""`, `0`, `NaN`, `null`, `false`, absent are false; `[]` and `{}` are **true** |
| `"" + x`, `String(x)` | `js::to_js_string(&v)` |
| `parseInt(x, 10)` | `js::parse_int(&str)` (returns `Option<i64>` for `NaN`) |

Rust design rules of the workspace still bind: **no `Box`, no `dyn`, no
lifetime parameters** on structs. Shared read-only data is `Arc<T>`; aliased
mutable JS objects become arena entries with an id (`BlobId`). Token lists are
built into a `Vec<Token>` and stored as `Arc<Vec<Token>>` once configured, so
running them is `let toks = state.citation.tokens.clone();` (an `Arc` clone)
followed by indexing — no borrow of `state` is held while a token runs.

## 4. Closures become enums, with the same count and order

citeproc-js builds each token's behaviour by pushing closures. The port pushes
enum variants instead:

```rust
// node_label.rs
#[derive(Debug, Clone, PartialEq)]
pub enum NodeLabelExec {
    /// The closure in CSL.Node.label.build (node_label.js:20-60).
    Render { term: String, plural: Option<i64> },
}

impl NodeLabelExec {
    pub fn run(&self, state: &mut State, token: &Token, item: &Value, cite_item: &Value)
        -> CslResult<Option<usize>> { ... }
}
```

and `token.execs.push(Exec::NodeLabel(NodeLabelExec::Render { ... }))`.
`exec.rs` holds the wrapper `Exec` (one variant per file) and dispatches; its
variants for every file are pre-declared, so you only add variants to **your
file's** enum. A JS closure that returns a jump index returns `Some(index)`.

- **Count and order must match**: the intermediate dump compares `execs_n`
  and `tests_n` per token with citeproc-js. Never merge two closures into one
  variant, never skip a no-op closure.
- Captured variables become variant fields, by value. A closure capturing a
  *mutable* JS object that is mutated later (rare) needs a state-side home —
  say so in a comment.
- `this` inside a closure is the token: it is the `token: &Token` parameter.
- Conditions: `token.tests` holds `Test` variants (`exec.rs`), `token.test`
  the evaluator (`Option<Test>`), the same pattern.

## 5. Strings: JS semantics

JS strings are UTF-16. `str.length`, `slice`, `substr`, `charAt`, `indexOf`
count UTF-16 code units. Use the helpers in `js.rs` (`js::len`, `js::slice`,
`js::substr`, `js::char_at`, `js::index_of`), which follow JS exactly, rather
than byte indexing (which panics on a non-ASCII boundary) or `chars()`
(which differs on astral characters).

Regular expressions: use the `regex` crate, compiled once (`std::sync::LazyLock`
or stored in state). Differences that bite:

- JS `\w`, `\d`, `\b` are **ASCII-only**; Rust's are Unicode. Write `[A-Za-z0-9_]`,
  `[0-9]`, or use `(?-u:\w)` / `(?-u:\b)` (the latter only on ASCII-safe input).
- JS `\s` includes `\u{feff}`; Rust's does not. Use `js::WS` (a class string) when it matters.
- JS `.` excludes `\r`, `\u{2028}`, `\u{2029}` as well as `\n`.
- No lookaround in `regex`: hand-write the check (comment the JS pattern).
- Replacement strings: `$1x` in Rust means group `1x`; write `${1}x`.
- `str.replace(/re/, ...)` without `g` replaces the **first** match only
  (`Regex::replace`, not `replace_all`); `str.replace("lit", ...)` replaces the
  first literal occurrence only.
- `str.split(/re/)` with capture groups **includes the captures** in the output:
  use `js::split_with_captures`.
- `toUpperCase` / `toLowerCase` → `to_uppercase` / `to_lowercase` (both full
  Unicode; matches V8 for the cases in the suite).

Collation (`localeCompare`, `Intl.Collator`): node's ICU. See §8.

## 6. Who owns what (parallel work)

Each agent owns a set of JS files (its brief says which) and may freely edit
the matching Rust files. Outside them:

- `state.rs`: add fields to the struct you need **in the block marked for your
  wave/agent** (`// ---- fields: <agent> ----`). Do not reorder or rename
  others' fields. If you need a field someone else should own, add it in your
  block with a comment.
- `exec.rs`: do not edit (all wrapper variants are pre-declared). If a file you
  own lacks a wrapper variant, add it at the end and say so in your report.
- `mod.rs`: the public API. Only the agent whose brief says so edits it.
- Another agent's file: do not edit. If you need a function that belongs
  there and does not exist yet, write a call to the name it will have and add
  a stub in **your** file under `// STUB(<owner file>): ...` with
  `todo!()`-free fallback behaviour, and list it in your report. The
  integrator removes stubs when merging.

## 7. Verification

1. **Unit tests per function, differential against citeproc-js.** For pure
   helpers (date parser, number parser, page mangler, flip-flop, text-case,
   name particles, ...) write a node script in your scratch directory that
   calls the bundled citeproc (`target/csl-reference/node_modules/citeproc/citeproc_commonjs.js`,
   which exports `CSL`) on many inputs, write the outputs as JSON to
   `tests/data/csl/units/<area>.json`, and a Rust test that replays them.
   Commit the JSON (it is citeproc-js output, ours to commit) and the
   generating script under `scripts/csl-units/` (node, run by hand).
2. **The fixture harness** `tests/citeproc_test_suite.rs`: list an area in
   `tests/data/csl/ported_areas.json` once its fixtures match; any remaining
   difference goes in `test_suite_known_differences.json` with a reason.
3. **The intermediate dump** `tests/citeproc_intermediate.rs` (#792).
4. Build and test in **release** mode only:
   `cargo test --release -p kovan-literature --lib citeproc` and
   `cargo test --release -p kovan-literature --test citeproc_test_suite -- --nocapture`.
   Also `cargo check --release -p kovan-literature --target wasm32-unknown-unknown --lib`
   must stay clean (no threads, no fs, no `Instant` in the port).
5. No `unwrap()` on input-derived data; no panics reachable from the public API.

## 8. Known hard spots (read before porting the named area)

- **Collation.** citeproc-js sorts with `String.prototype.localeCompare` /
  `Intl.Collator` (node's ICU, root collation tailored by locale). The #795
  owner decides the Rust collator (candidate: `icu_collator`, Unicode-3.0
  licence, pure Rust, wasm-clean) and records the decision in `DECISIONS.md`.
  Until then call `js::locale_compare(a, b, lang)`, the single place to change.
- `opt.sort_sep` depends on `localeCompare` (build.js).
- The date parser's `(?![0-9])` needs a hand-written matcher.
- `deleteNodeByNameAttribute` skips elements because it removes while
  iterating: reproduce this.
- APA's and Chicago's `*_sort` areas are ~68k tokens each because macros are
  inlined: keep `Token` cheap to clone.

## 9. Environment

- `vendor/citeproc-js` at `73bc1b44` (with its `locale/` submodule) and
  `vendor/csl-test-suite` at `6eefc5b0`; `target/csl-reference/node_modules/citeproc`
  (npm citeproc 2.4.63, byte-identical to a build of `src/`).
- Node 22 is installed. Use `require(process.env.CITEPROC_MODULE ||
  "<root>/target/csl-reference/node_modules/citeproc/citeproc_commonjs.js")`.
