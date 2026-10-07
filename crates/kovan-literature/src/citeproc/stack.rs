// Part of the kovan port of citeproc-js (GitHub #790).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/stack.js
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

//! `CSL.Stack`: a stack whose `tip` is the last element.
//!
//! JS quirks kept: `push(val)` without `literal` pushes `""` when `val` is
//! falsy, and `tip` after the last `pop` is `{}` (here: `None`). The falsy
//! rule needs to know what "falsy" means for `T`, hence [`JsFalsy`]; a stack
//! whose values are never falsy-substituted uses `push_literal`.

/// What a falsy value of `T` is replaced with on a non-literal push (JS
/// pushes `""`, so the replacement is `T`'s "empty string" value).
pub trait JsFalsy: Sized {
    /// JS truthiness of the value.
    fn is_truthy(&self) -> bool;
    /// The value JS's `""` stands for in this stack.
    fn empty_string() -> Self;
}

impl JsFalsy for String {
    fn is_truthy(&self) -> bool {
        !self.is_empty()
    }
    fn empty_string() -> Self {
        String::new()
    }
}

impl JsFalsy for serde_json::Value {
    fn is_truthy(&self) -> bool {
        crate::citeproc::js::truthy(self)
    }
    fn empty_string() -> Self {
        serde_json::Value::String(String::new())
    }
}

impl JsFalsy for i64 {
    fn is_truthy(&self) -> bool {
        *self != 0
    }
    fn empty_string() -> Self {
        0
    }
}

impl JsFalsy for bool {
    fn is_truthy(&self) -> bool {
        *self
    }
    fn empty_string() -> Self {
        false
    }
}

/// `CSL.Stack`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Stack<T> {
    /// `mystack`.
    pub mystack: Vec<T>,
}

impl<T: Clone> Stack<T> {
    /// `new CSL.Stack()`: empty.
    pub fn new() -> Self {
        Stack { mystack: Vec::new() }
    }

    /// `new CSL.Stack(val, CSL.LITERAL)` (or with a truthy `val`): one element.
    pub fn with(val: T) -> Self {
        Stack { mystack: vec![val] }
    }

    /// `push(val, true)`: push as is.
    pub fn push_literal(&mut self, val: T) {
        self.mystack.push(val);
    }

    /// `replace(val, true)`. Upstream throws on an empty stack; so do we.
    pub fn replace_literal(&mut self, val: T) -> Result<(), crate::citeproc::EngineError> {
        match self.mystack.last_mut() {
            Some(top) => {
                *top = val;
                Ok(())
            }
            None => Err(crate::citeproc::EngineError::Csl(
                "Internal CSL processor error: attempt to replace nonexistent stack item".into(),
            )),
        }
    }

    /// `pop()`.
    pub fn pop(&mut self) -> Option<T> {
        self.mystack.pop()
    }

    /// `tip` / `value()`: the last element (`None` where JS has `{}` or
    /// `undefined`).
    pub fn tip(&self) -> Option<&T> {
        self.mystack.last()
    }

    /// Mutable `tip`.
    pub fn tip_mut(&mut self) -> Option<&mut T> {
        self.mystack.last_mut()
    }

    /// `value()`.
    pub fn value(&self) -> Option<&T> {
        self.mystack.last()
    }

    /// `length()`.
    pub fn len(&self) -> usize {
        self.mystack.len()
    }

    /// `mystack.length === 0`.
    pub fn is_empty(&self) -> bool {
        self.mystack.is_empty()
    }

    /// `clear()`.
    pub fn clear(&mut self) {
        self.mystack.clear();
    }
}

impl<T: Clone + JsFalsy> Stack<T> {
    /// `new CSL.Stack(val)` without `literal`: a falsy `val` gives an empty
    /// stack (JS: `if (literal || val)`).
    pub fn with_nonliteral(val: T) -> Self {
        if val.is_truthy() {
            Stack { mystack: vec![val] }
        } else {
            Stack::new()
        }
    }

    /// `push(val)` without `literal`: a falsy `val` is pushed as `""`.
    pub fn push(&mut self, val: T) {
        if val.is_truthy() {
            self.mystack.push(val);
        } else {
            self.mystack.push(T::empty_string());
        }
    }

    /// `replace(val)` without `literal`.
    pub fn replace(&mut self, val: T) -> Result<(), crate::citeproc::EngineError> {
        let v = if val.is_truthy() { val } else { T::empty_string() };
        self.replace_literal(v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stack_follows_csl_stack() {
        let mut s: Stack<String> = Stack::with_nonliteral(String::new());
        assert!(s.is_empty());
        s.push("a".into());
        s.push(String::new());
        assert_eq!(s.len(), 2);
        assert_eq!(s.tip().map(String::as_str), Some(""));
        s.replace("b".into()).unwrap();
        assert_eq!(s.pop().as_deref(), Some("b"));
        assert_eq!(s.tip().map(String::as_str), Some("a"));
        s.clear();
        assert!(s.replace("x".into()).is_err());
        let lit: Stack<i64> = Stack::with(0);
        assert_eq!(lit.len(), 1);
    }
}
