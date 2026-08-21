//! `kovan-cli discover` — enumerate files under a root, honouring `.gitignore`
//! (via [`kovan_discovery::discover`] / [`kovan_discovery::discover_kind`]).

use std::path::PathBuf;

use super::KindArg;

/// Print one discovered path per line, sorted (see [`kovan_discovery::discover`]
/// for the determinism guarantee). `kind` restricts the result to a single
/// [`super::KindArg`]; `None` returns every non-ignored file.
pub fn run(root: PathBuf, kind: Option<KindArg>) {
    let files = match kind {
        Some(k) => kovan_discovery::discover_kind(&root, k.into()),
        None => kovan_discovery::discover(&root, &[]),
    };
    for f in files {
        println!("{}", f.display());
    }
}
