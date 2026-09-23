#!/usr/bin/env python3
"""Dump a depletion chain **as OpenMC's own parser sees it**, for cross-code
comparison against OUTRAM PARK's reader (gh:#270).

This is the reference side of
`crates/njoy-outram-park-fork/tests/depletion_chain_xml_vs_openmc.rs`. It uses
`openmc.deplete.Chain.from_xml` and nothing else -- no nuclear data library is
needed, which is why this comparison can run where a continuous-energy one
cannot.

Output is a `|`-separated line format, deliberately not JSON, so the Rust side
needs no deserialiser dependency. **Every float is emitted as its IEEE-754
bit pattern in decimal**, via `struct.pack`, because Python's `repr` and Rust's
`Display` both round-trip but do not agree on spelling (`6.14271e-05` vs
`0.0000614271`); comparing bits removes formatting from the comparison
entirely.

Record types, one per line:

    nuc|<name>|<half_life bits or ->|<decay_energy bits>
    dec|<name>|<i>|<type>|<target or ->|<branching_ratio bits>
    rxn|<name>|<i>|<type>|<target or ->|<Q bits>|<branching_ratio bits>
    fy |<name>|<energy bits>|<product>|<yield bits>
    src|<name>|<particle>

`nuc` records are in the chain's own order (which is file order); `dec` and
`rxn` keep their index so order is compared too; `fy` and `src` are sorted, as
neither side promises an order for them.

Usage:  python3 dump_chain.py <chain.xml> <out.txt>

Provenance: OpenMC is MIT-licensed. Run against /opt/src/openmc commit afa7a14
on 2026-09-23; see ../chain_xml_read_2026_09_23.md for the results.
"""
import struct
import sys

import openmc.deplete


def bits(x):
    """IEEE-754 bit pattern of a float, as a decimal integer."""
    return str(struct.unpack("<Q", struct.pack("<d", float(x)))[0])


def main(path, out):
    chain = openmc.deplete.Chain.from_xml(path)
    lines = []
    for nuc in chain.nuclides:
        hl = "-" if nuc.half_life is None else bits(nuc.half_life)
        lines.append(f"nuc|{nuc.name}|{hl}|{bits(nuc.decay_energy)}")
        for i, d in enumerate(nuc.decay_modes):
            target = "-" if d.target is None else d.target
            lines.append(f"dec|{nuc.name}|{i}|{d.type}|{target}|{bits(d.branching_ratio)}")
        for i, r in enumerate(nuc.reactions):
            target = "-" if r.target is None else r.target
            lines.append(
                f"rxn|{nuc.name}|{i}|{r.type}|{target}|{bits(r.Q)}|{bits(r.branching_ratio)}"
            )
        fy = []
        if nuc.yield_data is not None:
            for energy, products in nuc.yield_data.items():
                for product, value in products.items():
                    fy.append(f"fy|{nuc.name}|{bits(energy)}|{product}|{bits(value)}")
        lines.extend(sorted(fy))
        lines.extend(f"src|{nuc.name}|{p}" for p in sorted(nuc.sources))

    header = [
        f"# openmc {openmc.__version__} deplete.Chain.from_xml",
        f"# source {path}",
        f"# {len(chain.nuclides)} nuclides",
    ]
    with open(out, "w") as f:
        f.write("\n".join(header + lines) + "\n")
    print(f"{path}: {len(chain.nuclides)} nuclides, {len(lines)} records -> {out}")


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
