# Reference data for `outram-mc-libs` integration tests

Small, openly-licensed input files committed so the tests that need them run on
any checkout, with no external data tree.

## `openmc_chain_simple.xml`

* **What it is** — the 9-nuclide regression depletion chain the OpenMC
  `pincell_depletion/depletion.ipynb` notebook uses.
* **Source** — OpenMC, `examples/pincell_depletion/chain_simple.xml`.
* **Author / organisation** — the OpenMC development team (openmc-dev).
* **URL** — <https://github.com/openmc-dev/openmc>
* **Commit accessed** — `afa7a14`, from the local checkout at
  `/opt/src/openmc`, on **2026-09-23**.
* **Licence** — MIT. Compatible with this workspace's GPL-3.0-only.
* **Processing** — **none**. Copied verbatim, byte for byte.
  `tests/depletion_chain_xml_vs_transcription.rs` asserts the copy still
  matches the checkout's file when a checkout is present, so it cannot drift
  silently.
* **Why it is committed rather than cited** — it is the reference side of a
  comparison (the parsed chain against `DepletionChain::simple`'s hand
  transcription of the same file). A cited-but-absent reference is one a reader
  cannot check; the file is 2 kB.
