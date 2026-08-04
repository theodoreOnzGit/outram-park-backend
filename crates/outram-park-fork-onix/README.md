# outram-park-fork-onix

Pure-Rust depletion / burnup — an independent translation of the MIT-licensed [ONIX](https://github.com/jlanversin/ONIX) depletion code.

Solves the depletion (Bateman) equations via a CRAM (Chebyshev Rational Approximation Method) solver, producing the actinide + fission-product isotopic inventory that feeds the MSRE fission-product-migration chemistry. Data-free: consumes cross sections/decay data from `njoy-outram-park-fork`.

> **⚠️ Scaffold — no human V&V.** Port in progress under the MSRE digital-twin
> epic (`op-6w0`). Independent OUTRAM PARK fork; not affiliated with the
> upstream project. Not for nuclear facility operation, reactor control,
> safety-critical, or licensing decisions.

## Bookkeeping status

> Maintainer sign-off tracker (see the workspace `CLAUDE.md` "Bookkeeping
> pass" command). A crate is **complete** only once the maintainer has
> personally signed off on BOTH axes below.

| Axis | Status |
|---|---|
| Verification & Validation (V&V) — human-reviewed | ❌ Not yet manually checked |
| Human / user interface — human-reviewed | ❌ Not yet manually checked |

**Status: INCOMPLETE** until both axes are manually checked and cleared by the maintainer.
