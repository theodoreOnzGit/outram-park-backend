//! A minimal single-line text-editing buffer shared by every tab that needs a
//! path or pattern field (repository root, in the Browser/Symbols/Literature
//! tabs).
//!
//! Deliberately tiny: append-at-end / backspace-at-end / set / clear. There is
//! no cursor-in-the-middle editing, no selection, no clipboard — KOVAN's TUI is
//! a viewer/navigator over deterministic sibling-crate output, not a text
//! editor, so this is scoped to "type a path, press Enter."

/// A single line of user-typed text (a filesystem path, in every current
/// caller). Insertion is always at the end; there is no cursor position to
/// track because nothing in this crate needs to edit the middle of a path.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TextInput {
    value: String,
}

impl TextInput {
    /// Start with `initial` already in the buffer (e.g. a sensible default
    /// root path like `"."`).
    pub fn new(initial: impl Into<String>) -> Self {
        Self {
            value: initial.into(),
        }
    }

    /// The current contents.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Append one character at the end.
    pub fn push_char(&mut self, c: char) {
        self.value.push(c);
    }

    /// Remove the last character, if any. No-op on an empty buffer.
    pub fn backspace(&mut self) {
        self.value.pop();
    }

    /// Empty the buffer.
    ///
    /// Not currently called from any tab's `handle_key` (each tab lets the
    /// user clear a field by hand with repeated [`backspace`](Self::backspace)),
    /// so this is `#[allow(dead_code)]` in a non-test build — kept because a
    /// text-input type without a way to reset it is an incomplete
    /// abstraction, and the test suites below use it to set up fixtures.
    #[allow(dead_code)]
    pub fn clear(&mut self) {
        self.value.clear();
    }

    /// Replace the buffer's contents wholesale. See [`clear`](Self::clear) for
    /// why this carries the same `#[allow(dead_code)]`.
    #[allow(dead_code)]
    pub fn set(&mut self, value: impl Into<String>) {
        self.value = value.into();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_with_the_given_initial_value() {
        let input = TextInput::new(".");
        assert_eq!(input.value(), ".");
    }

    #[test]
    fn push_char_appends_at_the_end() {
        let mut input = TextInput::new("src");
        input.push_char('/');
        input.push_char('a');
        assert_eq!(input.value(), "src/a");
    }

    #[test]
    fn backspace_removes_the_last_character() {
        let mut input = TextInput::new("abc");
        input.backspace();
        assert_eq!(input.value(), "ab");
    }

    #[test]
    fn backspace_on_empty_buffer_is_a_no_op() {
        let mut input = TextInput::default();
        input.backspace();
        assert_eq!(input.value(), "");
    }

    #[test]
    fn clear_empties_the_buffer() {
        let mut input = TextInput::new("something");
        input.clear();
        assert_eq!(input.value(), "");
    }

    #[test]
    fn set_replaces_the_whole_buffer() {
        let mut input = TextInput::new("old");
        input.set("new-value");
        assert_eq!(input.value(), "new-value");
    }
}
