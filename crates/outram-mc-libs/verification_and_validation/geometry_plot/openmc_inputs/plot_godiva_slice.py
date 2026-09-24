#!/usr/bin/env python3
"""Rasterise a Godiva slice with **OpenMC's own plotter** and dump the cell mask.

Reference side of the cross-code check for this crate's geometry-plot emitter
(gh:#268 acceptance item 1: "A slice of one of the repo's existing core models
... plotted this way and visually compared against the `openmc.Plot` output of
the same geometry").

`openmc --plot` needs **no cross-section library** — it only locates cells — so
this comparison runs on a machine with no nuclear data installed, which is why
it is possible here at all.

Output: `openmc_godiva_mask.txt`, one row per pixel row, `1` where OpenMC placed
a cell and `0` where it found none (outside the model). A mask rather than the
colours because OpenMC assigns cell colours arbitrarily; what both codes must
agree on is **which points are in the model**, which is the geometry question.

Usage:  python3 plot_godiva_slice.py <out_dir>

Provenance: OpenMC is MIT-licensed; run against /opt/src/openmc commit afa7a14
on 2026-09-24. Godiva radius 8.7407 cm is the ICSBEP HEU-MET-FAST-001 figure
this crate already uses.
"""
import os
import subprocess
import sys

import matplotlib.image as mpimg
import numpy as np
import openmc

R_CM = 8.7407          # ICSBEP HEU-MET-FAST-001
HALF_WIDTH_CM = 12.0   # comfortably outside the sphere, so the mask has an edge
PIXELS = 81            # odd, so a pixel sits exactly on the centre


def main(out_dir):
    os.makedirs(out_dir, exist_ok=True)
    cwd = os.getcwd()
    os.chdir(out_dir)
    try:
        m = openmc.Material(name="heu")
        m.add_nuclide("U235", 1.0)
        m.set_density("g/cm3", 18.74)
        openmc.Materials([m]).export_to_xml()

        sphere = openmc.Sphere(r=R_CM, boundary_type="vacuum")
        cell = openmc.Cell(cell_id=1, fill=m, region=-sphere)
        openmc.Geometry([cell]).export_to_xml()

        st = openmc.Settings()
        st.run_mode = "plot"
        st.particles = 100
        st.batches = 10
        st.export_to_xml()

        p = openmc.Plot()
        p.filename = "openmc_godiva"
        p.basis = "xy"
        p.origin = (0.0, 0.0, 0.0)
        p.width = (2 * HALF_WIDTH_CM, 2 * HALF_WIDTH_CM)
        p.pixels = (PIXELS, PIXELS)
        p.color_by = "cell"
        openmc.Plots([p]).export_to_xml()

        subprocess.run(["/opt/openmc/bin/openmc", "--plot"], check=True,
                       stdout=subprocess.DEVNULL)

        img = np.asarray(mpimg.imread("openmc_godiva.png"))
        # Drop any alpha channel, scale to 0..255.
        rgb = img[:, :, :3]
        if rgb.dtype.kind == "f":
            rgb = (rgb * 255.0).round().astype(int)
        # OpenMC paints "no cell here" white; every real cell gets a colour.
        mask = (~np.all(rgb == 255, axis=2)).astype(int)

        with open("openmc_godiva_mask.txt", "w") as f:
            f.write(f"# openmc {openmc.__version__} --plot, cell mask\n")
            f.write(f"# R={R_CM} cm  half_width={HALF_WIDTH_CM} cm  pixels={PIXELS}\n")
            f.write(f"# rows top-to-bottom as the PNG stores them\n")
            for row in mask:
                f.write("".join(str(v) for v in row) + "\n")
        inside = int(mask.sum())
        print(f"{PIXELS}x{PIXELS} mask written; {inside} pixels inside the model "
              f"({100.0 * inside / mask.size:.2f} %)")
    finally:
        os.chdir(cwd)


if __name__ == "__main__":
    main(sys.argv[1] if len(sys.argv) > 1 else ".")
