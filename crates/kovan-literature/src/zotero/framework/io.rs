// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translate, https://github.com/zotero/translate
//   (commit e0fe482b8a07): src/translation/translate.js
//   `Zotero.Translate.IO.String` :2847-2970 (`read` :2887, `write` :2933,
//   `init` :2948).
// Copyright (c) Corporation for Digital Scholarship, Vienna, Virginia, USA.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! Translator input and output: `Zotero.read` and `Zotero.write` over a
//! string (`Zotero.Translate.IO.String`).
//!
//! Positions count `char`s, where upstream counts UTF-16 code units; the two
//! agree except on astral-plane characters (see [`super::js`]).

/// The input of an import translator (`Zotero.read`).
#[derive(Debug, Clone)]
pub struct ImportInput {
    text: Vec<char>,
    bytes_read: usize,
    /// `_noCR`: set once a search for `\r` has failed.
    no_cr: bool,
}

impl ImportInput {
    /// Input over a string (`new Zotero.Translate.IO.String(string)`).
    pub fn new(text: &str) -> Self {
        ImportInput {
            text: text.chars().collect(),
            bytes_read: 0,
            no_cr: false,
        }
    }

    /// `init()`: rewind to the start (the framework does this between
    /// `detectImport` and `doImport`, :2948-2950).
    pub fn rewind(&mut self) {
        self.bytes_read = 0;
        self.no_cr = false;
    }

    /// The whole text.
    pub fn text(&self) -> String {
        self.text.iter().collect()
    }

    /// The number of characters consumed so far (`bytesRead`).
    pub fn position(&self) -> usize {
        self.bytes_read
    }

    /// `Zotero.read(n)` (:2899-2903): the next `n` characters, or `None`
    /// (JavaScript `false`) at the end. Note upstream's `false` for `n = 0`
    /// only at the end: `read(0)` mid-input is `Some("")`, which is falsy.
    pub fn read_chars(&mut self, n: usize) -> Option<String> {
        if self.bytes_read >= self.text.len() {
            return None;
        }
        let start = self.bytes_read;
        self.bytes_read += n;
        let end = self.bytes_read.min(self.text.len());
        Some(self.text[start..end].iter().collect())
    }

    /// `Zotero.read()` (:2904-2930): the next line without its terminator, or
    /// `None` at the end.
    ///
    /// Ported with upstream's quirk: when the rest of the input has no `\n`
    /// but has a `\r`, the line returned ends one character before the `\r`
    /// (`substr(oldPointer, crIndex-oldPointer-1)`, :2924), dropping that
    /// character.
    pub fn read_line(&mut self) -> Option<String> {
        let len = self.text.len();
        if self.bytes_read >= len {
            return None;
        }
        let old = self.bytes_read;
        if let Some(lf_rel) = self.text[old..].iter().position(|&c| c == '\n') {
            let mut lf = old + lf_rel;
            self.bytes_read = lf + 1;
            // `this.string.substr(lfIndex-1, 1) === "\r"`; at lfIndex 0,
            // substr(-1, 1) is the last character of the string.
            let prev = if lf == 0 {
                self.text.last()
            } else {
                self.text.get(lf - 1)
            };
            if len > lf && prev == Some(&'\r') {
                lf = lf.saturating_sub(1);
                if lf < old {
                    // substr with a negative length is "".
                    return Some(String::new());
                }
            }
            return Some(self.text[old..lf].iter().collect());
        }
        if !self.no_cr {
            match self.text[old..].iter().position(|&c| c == '\r') {
                None => self.no_cr = true,
                Some(cr_rel) => {
                    let cr = old + cr_rel;
                    self.bytes_read = cr + 1;
                    let n = cr as isize - old as isize - 1;
                    let end = if n <= 0 { old } else { old + n as usize };
                    return Some(self.text[old..end].iter().collect());
                }
            }
        }
        self.bytes_read = len;
        Some(self.text[old..].iter().collect())
    }
}

/// The output of an export translator (`Zotero.write`).
#[derive(Debug, Clone, Default)]
pub struct ExportOutput {
    text: String,
}

impl ExportOutput {
    /// Empty output.
    pub fn new() -> Self {
        ExportOutput::default()
    }

    /// `Zotero.write(data)` (:2933).
    pub fn write(&mut self, data: &str) {
        self.text.push_str(data);
    }

    /// What has been written.
    pub fn as_str(&self) -> &str {
        &self.text
    }

    /// The written text, consuming the output.
    pub fn into_string(self) -> String {
        self.text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_line_handles_lf_crlf_and_the_cr_quirk() {
        let mut i = ImportInput::new("a\r\nbc\nd");
        assert_eq!(i.read_line().as_deref(), Some("a"));
        assert_eq!(i.read_line().as_deref(), Some("bc"));
        assert_eq!(i.read_line().as_deref(), Some("d"));
        assert_eq!(i.read_line(), None);
        // Old-Mac line endings: upstream drops the character before each \r.
        let mut i = ImportInput::new("ab\rcd\ref");
        assert_eq!(i.read_line().as_deref(), Some("a"));
        assert_eq!(i.read_line().as_deref(), Some("c"));
        assert_eq!(i.read_line().as_deref(), Some("ef"));
        assert_eq!(i.read_line(), None);
    }

    #[test]
    fn read_chars_and_rewind() {
        let mut i = ImportInput::new("abc");
        assert_eq!(i.read_chars(2).as_deref(), Some("ab"));
        assert_eq!(i.read_chars(2).as_deref(), Some("c"));
        assert_eq!(i.read_chars(2), None);
        i.rewind();
        assert_eq!(i.read_chars(1).as_deref(), Some("a"));
    }
}
