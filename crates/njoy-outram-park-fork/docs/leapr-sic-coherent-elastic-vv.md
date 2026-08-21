# SiC coherent elastic — V&V and the ENDF/B-VIII.0 residual

Status: **AI-assisted draft, no human review.** Per `RESPONSIBLE_USE.md` the
findings below are untrusted until a human checks them. The central claim is
about a *published evaluation*, so it needs that check before it is cited
anywhere.

Tracked as GitHub issue
[#28](https://github.com/theodoreOnzGit/outram-park-backend/issues/28) and
kopi-beans bead `op-4daf`.

## What is being verified

`src/leapr/coher/general.rs` implements the generalized coherent-elastic
formulation of Zhu (2014) — a reciprocal-lattice sum over an arbitrary crystal
— and `src/leapr/coher/crystals.rs` supplies the 3C-SiC structure that the
distributed LEAPR decks do not carry (they set `iel = 0`, and their
coherent-elastic section came from a separate in-house routine; see
`reference-data/endf/tsl-SiinSiC.readme`).

The oracle is `reference-data/endf/tsl-SiinSiC.endf` /
`tsl-CinSiC.endf`, MF=7/MT=2, whose two copies are byte-identical apart from
the header — coherent elastic is a property of the 3C-SiC lattice, not of
either sublattice.

## Result 1 — the thermal-point cross section

Measured at 0.0253 eV, barn per principal atom, 300 K tape versus a 296 K
regeneration:

| Channel | Regenerated | Tape | Δ |
|---|---|---|---|
| elastic (both materials) | 2.85169 | 2.94078 | −3.03 % |
| inelastic (Si in SiC) | 0.066150 | 0.066150 | 0.0000 % |

The inelastic channel is exact at all eight declared temperatures (306,033
points, worst 0.0000 %) — `tests/leapr_temperature_block_selection.rs`. That
rules out deck parsing, the `alpha`/`beta` grids, the phonon expansion and the
`T_eff` machinery as sources of the elastic residual.

## Result 2 — root cause of the −3.03 %

A single cumulative number cannot separate "slightly wrong everywhere" from
"structurally wrong in one specific way". The edge-by-edge comparison in
`tests/leapr_sic_coherent_elastic_oracle.rs`
(`published_sic_bragg_pattern_matches_an_invalid_centring_not_zinc_blende`)
does separate them, using the one currency that is independent of lattice
constant, physical-constant vintage and Debye-Waller treatment: **which
reflections are extinguished**.

Every SiC Bragg edge, published or regenerated, sits at `E = n E_1` with
`n = h^2 + k^2 + l^2` on the conventional cell's simple-cubic reciprocal
lattice (`E_1 = 1.066463` meV, the `(100)` spacing of `a = 4.379` A), so `n`
keys the two tables together.

Measured 2026-08-21, `n` up to 100:

| Basis tried | reflections live in both | extinction mismatches |
|---|---|---|
| zinc-blende — what this crate ships | 35 | **25** |
| "edge-centred" — see below | 60 | **0** |

The published tape carries a **live** `(100)` reflection, and live `(210)`,
`(300)`/`(221)`, `(320)`, `(410)`/`(322)`, … — every mixed-parity reflection
with **odd** `h+k+l`. Zinc-blende extinguishes all of those exactly. The tape
extinguishes every mixed-parity reflection with **even** `h+k+l`.

That selection rule — live iff `h+k+l` is odd, or `h`, `k`, `l` share a parity
— is the signature of the structure factor

$$S = 1 + e^{i \pi h} + e^{i \pi k} + e^{i \pi l},$$

whose modulus squared is 16, 4, 0, 4 for zero, one, two, three odd indices,
against the fcc sum's 16, 0, 0, 16. `S` is what you get from centring
translations `(1/2,0,0)`, `(0,1/2,0)`, `(0,0,1/2)` — "half along each axis" —
in place of the fcc translations `(0,1/2,1/2)`, `(1/2,0,1/2)`, `(1/2,1/2,0)` —
"half along each *pair* of axes".

**That basis is not a possible crystal.** A set of centring translations must
be closed under addition modulo the lattice for the atoms it generates to be
symmetry-equivalent, and this one is not:
`(1/2,0,0) + (0,1/2,0) = (1/2,1/2,0)`, which is not a member. Pinned by
`the_edge_centred_basis_is_not_a_valid_centring_but_the_face_centred_one_is`.

Feeding that basis through this crate's own general path, with both sides
folded by the **same** Debye-Waller factor (`4W' = 5.977 /eV`, fitted to the
published tape's own Bragg pattern, so the basis is the only thing that
differs):

| Basis | `sigma(0.0253 eV)` | vs tape 2.94078 b |
|---|---|---|
| zinc-blende — what this crate ships | 2.83347 b | −3.65 % |
| edge-centred | 2.91289 b | −0.95 % |

Swapping in the basis the extinction pattern implies therefore recovers about
**2.7 of the 3.65 percentage points**. Reproduced by
`swapping_in_the_tapes_implied_basis_recovers_most_of_the_thermal_point_gap`.

(The −3.03 % headline in Result 1 is the *shipped* pipeline at 296 K using its
own `W'`, not this common-`W'` comparison; the two differ because the fitted
tape `W'` is ~10 % larger. Both are stated where they are measured.)

### Interpretation

The residual is not a defect in this port. The evaluation's coherent-elastic
section behaves as though the fcc centring vectors were transcribed with one
half-step each instead of two. This crate therefore **keeps the physically
correct zinc-blende structure and deliberately does not reproduce the tape**.

This is a finding about a published ENDF/B-VIII.0 evaluation. It is recorded
here, pinned by tests, and flagged for human review; it has not been reported
upstream, and it should not be until a human has checked it.

## Result 3 — the ~1 % that remains

After the basis is accounted for, the residual sits entirely in the weak
`|b_Si − b_C|^2` difference reflections (all-even `h k l` with
`h+k+l = 2 mod 4`). The model runs high there by +6.1 % at `n = 20`, rising
monotonically to +23.5 % at `n = 100`.

Growth with `tau^2`, confined to the difference reflections, is the signature
of a **per-atom-type** Debye-Waller factor in the evaluation —
`F = sum_j b_j exp(-W_j tau^2) exp(i tau . r_j)`, Zhu's "exact Debye-Waller"
option — against the single **compound** coefficient this crate uses (Zhu's
"cubic approximation", `crystals::debye_waller_decks`). A difference
reflection is exquisitely sensitive to that, since it subtracts two nearly
equal terms; the sum reflections are not, and they agree.

Separately, the Debye-Waller coefficients themselves differ: fitting a shared
slope to each side's own Bragg pattern gives `4W' = 5.977 /eV` for the tape at
300 K against 5.382 /eV for this crate at 296 K — about 10 % apart after
temperature scaling. Not yet root-caused.

Neither effect is large at the thermal point, because the difference
reflections are weak.

## What is still open

- Confirming Result 2 against the Zhu & Hawari ICNC 2015 paper, which
  describes the in-house routine that produced MT=2. No freely available copy
  has been located, and it is **not** in `crates/kovan-literature`. The 2014
  MS thesis (the primary source actually used) is:
  `crates/kovan-literature/open/theses/zhu2014thermal.{json,pdf}`.
- Implementing the per-atom-type Debye-Waller option, which would let this
  crate reproduce Result 3's difference reflections.
- Human review of everything above.
