"""Godiva (ICSBEP HEU-MET-FAST-001) in OpenMC, on the NJOY2016 ACE set.

Geometry and atom densities taken from this repo's own
examples/godiva_keff_endf_local.rs so the two codes run the SAME model:
bare HEU sphere, r = 8.7407 cm, vacuum boundary, 293.6 K.
"""
import openmc

fuel = openmc.Material(name="Godiva HEU")
fuel.add_nuclide("U234", 4.9184e-4, "ao")
fuel.add_nuclide("U235", 4.4994e-2, "ao")
fuel.add_nuclide("U238", 2.4984e-3, "ao")
fuel.set_density("sum")
openmc.Materials([fuel]).export_to_xml()

sph = openmc.Sphere(r=8.7407, boundary_type="vacuum")
cell = openmc.Cell(fill=fuel, region=-sph)
openmc.Geometry([cell]).export_to_xml()

s = openmc.Settings()
s.particles = 10000
s.inactive = 50
s.batches = 200
s.temperature = {"default": 293.6, "method": "nearest", "tolerance": 10.0}
s.source = openmc.IndependentSource(
    space=openmc.stats.Point((0.0, 0.0, 0.0)))
s.output = {"tallies": False}
s.export_to_xml()
