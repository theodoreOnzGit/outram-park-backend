"""Remove <source> elements (decay photon/electron spectra, which the Rust
port does not carry) from an OpenMC chain file, leaving everything else as
OpenMC wrote it. Usage: python strip_sources.py in.xml out.xml"""
import sys

import lxml.etree as ET

tree = ET.parse(sys.argv[1], ET.XMLParser(remove_blank_text=True))
for src in tree.getroot().iter("source"):
    src.getparent().remove(src)
tree.write(sys.argv[2], encoding="utf-8", pretty_print=True)
