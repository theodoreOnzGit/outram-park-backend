# How to read this deep dive

> **Research, education and V&V only.** Nothing here is for reactor
> operation, licensing, safety-critical decisions or emergency response. See
> [`RESPONSIBLE_USE.md`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/RESPONSIBLE_USE.md).

> **Review status: AI-assisted first draft (2026-10-03), not yet
> human-reviewed.** Every claim below was checked against the code at the
> commit this page was built from, but a person has not yet signed it off.
> Treat it as a draft until a review stamp appears here.

This is the **extended** deep dive into
[`boon-lay`](https://github.com/theodoreOnzGit/outram-park-backend/tree/@@COMMIT@@/crates/boon-lay):
how the crate models a TRISO fuel particle, how it decides when one fails,
and how fission products leave it on their way to an offsite source term. It
is written for someone who wants to read the code, change it, or check it.

## What you will find on each page

- **The problem first.** Each chapter opens with the physical question it
  answers, then shows how the code answers it.
- **Code anchors.** A sentence that makes a claim about the code links to the
  exact lines, pinned to the build commit, so the link cannot drift. Snippets
  are pulled from the real source files when the site is built, never copied.
- **API links.** Names in the API reference link to the rustdoc beside this
  book (`../../api/boon_lay/`).
- **Numbers with their source.** Every V&V number names the file that records
  it. A number without a source is not stated.

## The names matter in this crate

Two names are kept apart deliberately, and this deep dive follows the crate's
rule:

- **PANAMA-I** means the 1990 Jülich report (Verfondern & Nabielek,
  HTA-IB-03/90) and the results printed in it.
- **boon-lay fuel failure** means this crate's own code, written from the
  report's equations, and every number that code computes. The PANAMA source
  code was never available to this project.

The rule is stated in full in
[`src/fuel_failure/mod.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/fuel_failure/mod.rs#L24-L46).

Likewise, **TRISO-ATOPS** is INL's MIT-licensed Python code; the
`triso_atops_fork` module is a Rust port of it, verified against it code to
code.

## Something doesn't tally?

If a page says something the code does not do, that is a defect in the page.
Please [open an issue](https://github.com/theodoreOnzGit/outram-park-backend/issues/new)
and name the page and the line. Tracking issue for this track:
[#517](https://github.com/theodoreOnzGit/outram-park-backend/issues/517).
