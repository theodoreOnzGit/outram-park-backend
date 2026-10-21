# Depletion chain XML — read path, verified against the transcription it replaces

**Date:** 2026-09-23
**Issue:** [gh:#270](https://github.com/theodoreOnzGit/outram-park-backend/issues/270),
scope table row *"Depletion chain XML | read | Feeds `outram-mc-libs`'
`depletion/chain.rs`"*.
**Code under test:**
`crates/njoy-outram-park-fork/src/hdf5/depletion_chain_xml.rs` (the codec),
`crates/njoy-outram-park-fork/src/hdf5/xml_scan.rs` (the element scanner),
`crates/outram-mc-libs/src/depletion/chain.rs`
(`DepletionChain::from_chain_xml`, the consumer mapping).
**Tests:** `njoy-outram-park-fork --lib hdf5::depletion_chain_xml` (6),
`hdf5::xml_scan` (4), and
`outram-mc-libs --test depletion_chain_xml_vs_transcription` (5).

## Why this existed to be done

`outram-mc-libs`' depletion chain carried its reference chain as a **hand
transcription** of OpenMC's `examples/pincell_depletion/chain_simple.xml`,
written out as Rust literals in `DepletionChain::simple()`. The module's own
"Known limitations" note said why: the ENDF/B-VIII decay-data crates keep
`SerdeNuclideData`'s `reaction` field private, so the neutron-reaction
**targets** — which nuclide a capture or an `(n,2n)` produces — could not be read
from them.

That made the transcription the single source of those targets, and **nothing
checked it**. A transcription nobody compares against its original is an
assumption wearing a data table's clothes.

## Methodology

1. **Read upstream first**, per the workspace porting rule. The format's
   specification is `openmc/deplete/nuclide.py`'s `Nuclide.from_xml` and
   `FissionYieldDistribution.from_xml_element` (`/opt/src/openmc`, commit
   `afa7a14`, MIT). Every default and sentinel in the reader is taken from
   there rather than inferred from the example files:

   | Field | Upstream rule |
   |---|---|
   | `half_life` absent | stable; `decay_energy` read only when it is present, default `0.0` |
   | `target` absent, or `"nothing"` (any case) | no in-chain daughter |
   | `<decay>` `branching_ratio` | **no default** — a missing one is an error, because defaulting it would state a decay rate the file does not |
   | `<reaction>` `Q` / `branching_ratio` | `0.0` / `1.0` |
   | `type="fission"` | target forced to `None` even when the file names one |
   | `<neutron_fission_yields parent="X">` | borrow X's yields |
   | `<source>` with empty `<parameters>` | skipped |

   Reading upstream is what produced the third row. A first-principles reader
   would have defaulted a missing decay branching ratio to `1.0`, which is
   wrong and silent.

2. **Parse the reference chain and compare it, exactly, to the transcription.**
   `f64` for `f64`, no tolerance. Both sides descend from the same decimal
   literals and Rust's parser is correctly rounded, so `"2.36520E+04"` in the
   file and `2.36520e4` in the source must agree bit for bit or one of them is
   wrong.

3. **Compare the burnup matrices too**, at non-zero rates on every channel
   present, because the matrix is what the CRAM solver integrates and record
   equality that did not survive into it would be worth little.

4. **Record what the mapping cannot carry, as a value rather than as prose.**
   `ReactionKind` has three variants; the chain format keys ~70 reaction types.

## Results

### The reference chain reads back exactly

`tests/reference_data/openmc_chain_simple.xml` (committed verbatim, MIT,
provenance in that folder's `README.md`) parses to a `DepletionChain`
**exactly equal** to `DepletionChain::simple()`:

| quantity | value |
|---|---|
| nuclides, in order | 9 — `I135 Xe135 Xe136 Cs135 Gd157 Gd156 U234 U235 U238` |
| decay branches | 2 |
| reaction channels | 7 |
| fission yields | 18 (3 fissionable nuclides x 6 products) |
| unmodelled channels | **0** |
| burnup-matrix entries compared | 81, all identical; 20+ non-zero |

So the hand transcription **was** faithful. That is the less interesting of the
two possible outcomes and it is still worth having, because it was previously
unknown: the claim has moved from "presumed correct" to "checked, and re-checked
on every test run".

The committed copy is separately asserted **byte-identical** to the OpenMC
checkout's file, so it cannot drift from its origin without a failure.

### Cross-code: OpenMC's own parser, on the same files

`openmc.deplete.Chain.from_xml` is the format's reference implementation and
needs no cross-section library, so — unlike almost every comparison in this
crate — it could be **run here** rather than cited from a stored number.
`openmc_inputs/dump_chain.py` walks OpenMC's parsed `Chain` and writes one flat
record per nuclide, decay branch, reaction channel, fission yield and decay
source, **every float as its IEEE-754 bit pattern**: Python's `repr` and Rust's
`Display` both round-trip but spell the same value differently
(`6.14271e-05` vs `0.0000614271`), so comparing bits takes formatting out of
the comparison entirely. `tests/depletion_chain_xml_vs_openmc.rs` builds the
same dump from this reader and diffs line for line, with no tolerance.

**Measured 2026-09-24, OpenMC `0.1.dev1+gafa7a14ac`:**

| file | nuclides | records | disagreements |
|---|---|---|---|
| `chain_simple.xml` | 9 | 36 | **0** |
| `chain_simple_decay.xml` | 11 | 43 | **0** |
| `chain_ni.xml` | 21 | 97 | **0** |

176 records over 41 nuclides, exactly equal. What that covers: half-lives and
the stable case, `decay_energy` and its `0.0` default, the `"nothing"` sentinel
*and* genuinely targetless decays, branching ratios and their defaults, reaction
Q values, fission's forced-`None` target, multi-product yields, and which
particles each nuclide sources.

### The format's awkward corners, exercised

The real files carry several things a reader written from the example alone
would break on. All are covered:

| corner | where it appears | handled |
|---|---|---|
| `type=" beta"` — leading space in an attribute | `chain_simple.xml`, Xe-135 | kept verbatim; documented to compare trimmed |
| stray whitespace before `>` | `chain_simple.xml`, Gd-157 | yes |
| `target="Nothing"` sentinel | `chain_simple.xml`, Gd-157 | mapped to `None`, case-insensitively |
| a `<decay>` with **no** target | `chain_ni.xml`, Fe-55 `ec/beta+` | mapped to `None` |
| a reaction with no target | `chain_ni.xml`, `(n,2n)`/`(n,p)`/`(n,a)` | `None` |
| `<source>` decay-radiation spectra | `chain_ni.xml`, `chain_simple_decay.xml` | **read and kept**, not dropped |
| a `<source>` with empty `<parameters>` | upstream skips it | skipped |
| yields borrowed via `parent=` | full ENDF chains | resolved, in either file order |
| several `<fission_yields>` energies | full ENDF chains | all read, sorted by energy |

Three real files parse (counts printed by the tests): `chain_simple.xml` 9
nuclides, `chain_simple_decay.xml` 11, `chain_ni.xml` 21.

### What the consumer cannot carry — measured, not asserted

`(n,gamma)`, `(n,2n)` and `fission` are the three channels
`DepletionChain::build_matrix` has rate fields for. Everything else is left out
of the matrix **and reported** through `DepletionChain::unmodelled_channels()`.
On `chain_ni.xml`: 21 nuclides, four reaction types in the file, and **18
unmodelled channels, every one `(n,a)` or `(n,p)`**. The count is pinned by a
test so it cannot grow unnoticed.

A channel whose `branching_ratio` is not 1 is also reported rather than applied:
`NeutronReaction` has no branching field, so putting the full rate on one
product would create atoms the file did not specify. Verified on a synthetic
Ag-109 `(n,gamma)` split 0.955 / 0.045 — both branches reported, neither
applied.

### Refusals, and why each is a refusal

A permissive reader would turn each of these into a wrong answer far from its
cause. Every one is an error:

* a count attribute (`decay_modes`, `reactions`) disagreeing with the children
  found — these are redundant in the file and **not read by upstream**, which
  makes them a free consistency check; all three real files pass it;
* `<products>` and `<data>` of different length (a permissive zip drops yields);
* `<products>` with no `<data>` at all (would leave `NaN` yields that surface
  later as a `NaN` inventory);
* an `<energies>` grid disagreeing with the `<fission_yields>` sets — to `1e-9`
  **relative**, not exactly, and reading upstream is why. Its writer
  (`FissionYieldDistribution.to_xml_element`) emits only the
  `<fission_yields energy=...>` attribute and **no `<energies>` element at
  all**, so that grid exists only in hand-written and legacy files, where the
  same physical energy can be spelled two ways in the two places. An exact
  comparison would have refused a legitimate file over a decimal spelling;
  `1e-9` relative still fails a missing set, an extra one, or a different
  energy. (This check was written exact first and loosened after reading the
  writer.)
* the same nuclide twice (would make the matrix depend on file order);
* a `parent=` borrow naming an absent nuclide, or one with no yields;
* a nuclide carrying fission **yields** with no `"fission"` channel, which makes
  the yields unreachable by any code;
* an element the reader does not recognise;
* **asking for a fission energy the file does not tabulate** — no interpolation,
  no nearest-neighbour. Taking a neighbouring point silently is the
  substitution that produced four separate wrong reference values in this
  workspace's V&V history.

**And one case deliberately NOT refused**, after it was first written as an
error and then reconsidered: a `"fission"` channel with **no** yields. That is a
legitimate chain — fission then acts as pure removal, which is what a chain
written to deplete actinides without tracking fission products says, and
`build_matrix` already treats an empty yield list exactly that way. Refusing it
would have rejected a file OpenMC accepts and models correctly. The inverse
(yields with no fission channel) stays an error, because nothing could ever
reach those yields.

## A defect found on the way, and fixed

The element scanner was private to `hdf5/cross_sections_xml.rs`. Sharing it
exposed that it held the document as `Vec<char>` and converted a char index back
to a byte offset with `chars[..i].iter().map(char::len_utf8).sum()` **on every
`<` in the document** — making a scan `O(n^2)` in document length. Invisible on
`cross_sections.xml` (a few hundred lines) and fatal on a chain file.

Measured on synthetic chain files, both versions counting the same elements
(`rustc -O`, this container):

| document | elements | byte-indexed (now) | char-indexed (before) | ratio |
|---|---|---|---|---|
| 0.24 MB | 4,001 | 117 µs | 236 ms | 2,017x |
| 0.98 MB | 16,001 | 634 µs | 3.96 s | 6,252x |
| 3.94 MB | 64,001 | 1.94 ms | 65.7 s | 33,842x |
| 9.92 MB | 160,001 | 5.00 ms | 411.6 s | 82,330x |

The ratio quadruples for each quadrupling of size — the signature of the
quadratic term, not a constant-factor difference. A 10 MB chain file, which is
the order of the full ENDF/B-VIII chain, took **6.9 minutes** to scan and now
takes 5 ms. The scanner now lives in `hdf5/xml_scan.rs`, shared by both readers,
and its extraction also tightened the closing-tag match so `</nuclide_extra>`
can no longer be read as closing `<nuclide>`.

## Documentation corrected in the same change

Per the workspace rule that a doc claim the code contradicts is a defect:
`crates/outram-mc-libs/src/depletion/chain.rs`' "Known limitations" item 1 said
neutron-reaction targets "come from the hardcoded `chain_simple.xml`
transcription instead". Its first sentence still holds — the decay libs' field
is still private, verified 2026-09-23 — but its conclusion no longer does. The
paragraph is struck through and corrected in place rather than deleted, so a
reader can see what changed and when.

## Limitations of this record

1. **No full ENDF/B-VIII chain file was available on this machine** to parse.
   `find` over `/opt/src/openmc` and `/home/user` turned up no
   `chain_endfb80_*.xml`. The scale evidence above is therefore on *synthetic*
   documents of the right size and element density, not on the real file. The
   `parent=` borrow and the multi-energy yield paths are exercised on synthetic
   cases for the same reason. **Not re-checked against a real full chain.**
2. **This is a read path only.** No chain writer exists, and #270 does not ask
   for one.
3. **Decay radiation sources are parsed but have no consumer.** They are kept on
   the codec's records, not mapped into `NuclideData`, because
   `build_matrix` assembles number-density rates and has nowhere to put a photon
   spectrum. A decay-heat or shielding consumer would read them from
   `DepletionChainXml` directly.
4. ~~**Agreement with the transcription is not agreement with OpenMC's own
   parse.** Both sides here are ours. What is verified is that the reader and
   the transcription agree, and that the reader follows the rules
   `nuclide.py` states. Running OpenMC's `Chain.from_xml` on the same file and
   diffing the two structures would be a stronger check and has **not** been
   done.~~ **CORRECTED 2026-09-24 — this was written before the check was
   attempted, and it has since been done.** See "Cross-code" above: 176 records
   over 41 nuclides, three files, **0 disagreements** against
   `openmc.deplete.Chain.from_xml`. The concern the paragraph raised was the
   right one; it is now answered rather than outstanding. What remains true is
   the narrower point: the agreement is on **parsing**, not physics — neither
   side is checked against the evaluated data the chain summarises.
