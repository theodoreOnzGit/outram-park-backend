//! `kovan-cli cost` — estimate how many tokens a file would cost an agent to
//! read whole (GitHub issue #32: "kovan-cli should import the token savings
//! features of kopitiam-cli, by wiring the api in directly").
//!
//! Wired directly to [`kopitiam_tokenizer::estimate_tokens`] /
//! [`kopitiam_tokenizer::estimate_tokens_by_line`] — a dependency-free,
//! per-Unicode-script character-weighted estimate (see that crate's own
//! module doc for the accuracy model: roughly ±25-30% against a real
//! GPT-2/Qwen-family BPE tokenizer on ordinary text). This is materially
//! better than [`kovan_semantics::agent_docs::estimated_tokens`]'s `bytes/4`
//! heuristic, which is left as-is — it only feeds `agent-docs-gen --budget`'s
//! own accounting and is not this command's concern.
//!
//! Deliberately named `cost`, not `tokens` — that name is already
//! [`super::tokens`]'s per-commit API-usage accounting (`kovan-metrics`), an
//! unrelated concept this command must not collide with.

use std::path::PathBuf;

use kopitiam_tokenizer::{estimate_tokens, estimate_tokens_by_line};

/// Read `path` and print its estimated token cost — the whole-file total by
/// default, or a per-line breakdown with `by_line`.
///
/// # Errors
///
/// A message if `path` cannot be read (missing, a directory, or not valid
/// UTF-8 — the estimator operates on `&str`, so a binary file is reported
/// rather than guessed at).
pub fn run(path: PathBuf, by_line: bool) -> Result<(), String> {
    let text = std::fs::read_to_string(&path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    if by_line {
        for line in estimate_tokens_by_line(&text) {
            println!("{}: {}", line.line, line.tokens);
        }
    }
    println!("{}: ~{} estimated tokens", path.display(), estimate_tokens(&text));
    Ok(())
}
