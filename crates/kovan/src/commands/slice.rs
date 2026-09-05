//! `kovan-cli slice <file> <start> <end>` — print one line range instead of
//! the whole file, the third leg of the token-frugal `cost -> outline ->
//! slice` loop (GitHub issue #32).
//!
//! No dependency at all: this is a plain line-indexed read, deliberately kept
//! that simple rather than reusing any parsing/search machinery it doesn't
//! need.

use std::path::PathBuf;

/// Print lines `start..=end` (1-based, inclusive) of `path`, each prefixed
/// with its line number. Out-of-range bounds are clamped to the file's
/// actual line count rather than erroring — asking for more than a short
/// file has is a common, harmless case (an agent guessing at a range).
///
/// # Errors
///
/// A message if `path` cannot be read, or if `start > end`.
pub fn run(path: PathBuf, start: usize, end: usize) -> Result<(), String> {
    if start == 0 || start > end {
        return Err(format!(
            "invalid range {start}..={end} — start must be >= 1 and <= end"
        ));
    }
    let text = std::fs::read_to_string(&path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let lines: Vec<&str> = text.lines().collect();
    let last = end.min(lines.len());
    if start > lines.len() {
        println!(
            "{}: has only {} line(s) — nothing in range {start}..={end}",
            path.display(),
            lines.len()
        );
        return Ok(());
    }
    for (i, line) in lines[start - 1..last].iter().enumerate() {
        println!("{}: {line}", start + i);
    }
    Ok(())
}
