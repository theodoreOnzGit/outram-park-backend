# OpenMC inputs and reference output for the depletion-chain XML read path

Snapshot for gh:#270's *"Depletion chain XML | read"* row. Committed, not cited,
per the workspace rule that the reference side of a code-to-code comparison must
be reproducible by a reader.

## Provenance

* **Project** — OpenMC, <https://github.com/openmc-dev/openmc>
* **Author / organisation** — the OpenMC development team (openmc-dev)
* **Version** — `0.1.dev1+gafa7a14ac`, i.e. commit **`afa7a14`**, as the
  generated `.txt` headers record. From the local checkout at `/opt/src/openmc`.
* **Date accessed** — 2026-09-23
* **Licence** — MIT; the upstream notice is `LICENSE.openmc` in this folder.
  Compatible with this workspace's GPL-3.0-only.

## Contents

| File | What it is | Processing |
|---|---|---|
| `chain_simple.xml` | `examples/pincell_depletion/chain_simple.xml` — the 9-nuclide notebook chain | copied verbatim |
| `chain_simple_decay.xml` | `tests/chain_simple_decay.xml` — the same chain plus decay photon/electron sources and two metastable states | copied verbatim |
| `chain_ni.xml` | `tests/chain_ni.xml` — 21 nuclides with `(n,2n)`, `(n,p)`, `(n,a)` and targetless decays | copied verbatim |
| `dump_chain.py` | the driver: `openmc.deplete.Chain.from_xml` -> a flat, bit-exact text dump | new work (GPL-3.0, this workspace) |
| `openmc_chain_*.txt` | the generated reference dumps | produced by `dump_chain.py` from the `.xml` beside it |
| `LICENSE.openmc` | upstream MIT notice | copied verbatim |

## Reproducing

```bash
PYTHONPATH=/opt/src/openmc /opt/ompy/bin/python \
    dump_chain.py chain_simple.xml openmc_chain_simple.txt
```

No cross-section library is needed — `Chain.from_xml` reads XML only, which is
why this comparison runs where a continuous-energy one cannot.

## What consumes it

`crates/njoy-outram-park-fork/tests/depletion_chain_xml_vs_openmc.rs` builds the
same flat dump from this workspace's reader and diffs the two line by line. The
results are written up in `../chain_xml_read_2026_09_23.md`.
