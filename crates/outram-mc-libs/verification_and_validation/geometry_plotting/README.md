# Geometry plotting: the Rust port vs `openmc --plot`, pixel for pixel

V&V record for `outram_mc_libs::geometry::plot` (gh:#268): the port of
OpenMC's plotter, `src/plot.cpp`, to Rust with a native PNG writer.

## Result

**17 reference images, 241 041 pixels, 0 differing pixels.** Exact RGB
equality on every slice plot *and* every ray-traced plot, measured 2026-09-25
on x86_64 Linux (glibc), OpenMC 0.16.1-dev25 at commit `d7d3284a1`.

| Image | Kind | Pixels | Differing |
|---|---|---|---|
| `godiva_xy_cell` | slice, cell | 81 x 81 | 0 |
| `lattice_xy_cell` | slice, cell, 3x3 lattice | 160 x 160 | 0 |
| `lattice_xy_material` | slice, material | 160 x 160 | 0 |
| `lattice_xz_cell` | slice, x-z basis, off-axis origin | 64 x 220 | 0 |
| `lattice_yz_material` | slice, y-z basis | 100 x 100 | 0 |
| `lattice_xy_level0` | slice, `level = 0` (root cells) | 160 x 160 | 0 |
| `lattice_xy_meshlines` | slice, regular-mesh lines | 160 x 160 | 0 |
| `overlap_cell_show` | slice, overlaps shown (default red) | 100 x 100 | 0 |
| `overlap_material_show` | slice, material, overlap + background colours | 100 x 100 | 0 |
| `mask_cell` | slice, mask with mask background, user colour | 100 x 100 | 0 |
| `mask_material` | slice, material mask (white), void cell | 100 x 100 | 0 |
| `level1_cell` | slice, `level` deeper than the model | 100 x 100 | 0 |
| `sphere_wireframe` | wireframe ray trace, perspective | 120 x 100 | 0 |
| `sphere_solid` | solid (Phong) ray trace | 120 x 100 | 0 |
| `lattice_wireframe` | wireframe, thickness 2, partly transparent | 120 x 100 | 0 |
| `lattice_solid` | solid, moved light, diffuse fraction 0.3 | 120 x 100 | 0 |
| `lattice_wireframe_ortho` | wireframe, orthographic, cell colouring | 100 x 100 | 0 |

The ray traces were expected to disagree on a few pixels — shading passes
through `tan`, `exp` and a truncation to a byte, and the two tracers relocate
differently after a crossing — and the pass criterion was written to allow a
recorded count. They do not disagree at all on these cases, so the recorded
count, and the gate, is 0 on Linux/glibc (see "Scope of the result").

## Methodology

**Reference side.** [`openmc_inputs/make_references.py`](openmc_inputs/make_references.py)
builds four small CSG models with OpenMC's Python API, writes one `plots.xml`
per model, runs `/home/teddy0/Documents/research/openmcbin/bin/openmc --plot`
(built with libpng) and copies the PNGs into [`openmc_reference/`](openmc_reference/).
`openmc --plot` needs no cross-section library; it only locates cells and
traces rays.

- **(a) Godiva** — one sphere, r = 8.7407 cm (ICSBEP HEU-MET-FAST-001).
- **(b) Pin lattice** — a 3x3 `RectLattice`, pitch 1.26 cm, of fuel pins
  (fuel r 0.4096, clad r 0.475) round a guide tube (r 0.56 / 0.60), z in
  [-10, 10], inside a water reflector to +/-3 cm built from four
  intersection-only slabs. Three universes, eleven cells, three materials.
- **(c) Overlap / mask** — two spheres of r = 2 at x = -1 and x = +1 that
  **overlap on purpose**, a void sphere, and a box. Exercises material vs cell
  colouring, `show_overlaps`, `overlap_color`, `background`, masks with and
  without a mask background, a user colour, a void cell under material
  colouring (drawn white), and a `level` deeper than the model (all
  background).
- **(d) Ray traces** — a sphere in a box (simple) and the pin lattice
  (lattice): wireframe with partly transparent domains, solid with the light
  at the camera and elsewhere, and one orthographic wireframe.

**Our side.** `tests/geometry_plot_openmc_parity.rs` rebuilds the same models
with this crate's types and renders the same plots.

Two things decide whether default colours can match, and both are respected
rather than worked around:

1. **Index order.** OpenMC's Python API writes `geometry.xml` sorted by cell
   id and the C++ reads cells in document order, so a cell's index — which
   picks its default colour, and its first-match order inside a universe — is
   the rank of its id. Materials are indexed in `openmc.Materials` order. The
   Rust models use the same orders.
2. **One plotter seed per run.** `model::plotter_seed` starts at 1 and every
   plot in a `plots.xml` draws its default colours from it in turn
   (`src/plot.cpp:174`, `:580-596`). The test replays the plots of each
   `plots.xml` in order through one `&mut u64`, so the fifth plot's colours
   depend on the first four exactly as upstream's do.

**Comparison.** Both PNGs are decoded and compared RGB triple by RGB triple.
RGB and not a cell mask: whether the colour stream is ported correctly is part
of what is being checked. Our PNG decoder (`geometry::plot::decode_png`) was
cross-checked independently with Pillow: identical verdict on all 17 images
(and the images are not trivial — up to 192 distinct colours in a solid ray
trace).

**Pass criteria, fixed before the comparison ran.** Slices: 0 differing
pixels. Ray traces: at most a recorded count, each explained. Both met with 0.

### Negative controls — the harness can fail

`harness_detects_small_perturbations` makes the smallest change of each kind
and requires a difference. Measured:

| Perturbation | Differing pixels |
|---|---|
| plotter seed 2 instead of 1 (Godiva) | 2733 |
| slice origin moved half a pixel, 0.02 cm (lattice) | 892 |
| guide tube moved to a corner tile (lattice) | 1985 |
| diffuse fraction 0.31 instead of 0.30 (lit lattice) | 2231 |

## Scope of the result

- **Platform.** The references were produced against glibc's `libm`, and the
  ray-traced shading goes through `tan` and `exp`. On another platform's
  `libm` a last-bit difference can flip a byte; the test allows 60 pixels
  (0.5 %) off Linux/glibc. **That allowance is not a measurement** — no
  non-glibc host has run this comparison.
- **What the ray tracer does differently, by design** (documented in
  `src/geometry/plot/raytrace.rs`): union/complement regions can produce an
  extra wireframe edge (this crate's cell distance is the nearest bounding
  surface, not upstream's `distance_complex` walk); relocation after a crossing
  is from the root, not from the crossed level; a void material is treated as
  transparent where upstream indexes its tables with -1. None of the cases here
  exercises these, which is why they agree exactly; they are stated, not
  measured.
- **Slices:** one known divergence — an overlap directly above a hole in a
  lower universe is not reported, because `Geometry::locate` returns no
  partial path. Not exercised.

## Not ported, and why

- **Voxel plots** — HDF5 volumes, not images.
- **JPEG** — lossy; it blurs exactly the boundaries a geometry plot exists to
  show and makes pixel comparison meaningless. OpenMC itself writes only PNG
  or PPM. PNG is sufficient.
- **`plots.xml` parsing and the `openmc_*` C API** — this crate reads no XML;
  plots are Rust values.
- **Mesh lines for non-regular meshes** — the crate has only `RegularMesh`.

## Dependency

PNG is written by `geometry::plot::image`: chunk framing, CRC-32 and the five
row filters in-crate, DEFLATE from `miniz_oxide`. `miniz_oxide` is pure Rust
and was **already** in this crate's dependency tree (a non-optional dependency
of `njoy-outram-park-fork`, which builds it for Android/Termux and wasm32), so
naming it directly adds no crate to the build.

## Files

| Path | What |
|---|---|
| `openmc_inputs/make_references.py` | reference driver (new work, GPL-3.0-only) |
| `openmc_inputs/LICENSE.openmc` | OpenMC's MIT notice |
| `openmc_reference/*.png` | what `openmc --plot` wrote |
| `outram/*.png` | what this crate wrote (`OUTRAM_PLOT_OUT=<dir> cargo test --release -p outram-mc-libs --test geometry_plot_openmc_parity`) |

The earlier record for the matplotlib-script path, a Godiva cell mask, is
[`../geometry_plot/geometry_plot_2026_09_24.md`](../geometry_plot/geometry_plot_2026_09_24.md).

A worked example on a real model — the HTR-10 explicit-TRISO core — is in
`crates/nee_soon/verification_and_validation/htr10_geometry_images/`.

## Provenance

| Item | Value |
|---|---|
| Reference code | OpenMC 0.16.1-dev25, commit `d7d3284a1b13d7cae020e0d8fea9f1e60d91b18b` (MIT), Release build, PNG support yes |
| Python API | `openmc` 0.16.1.dev25+gd7d3284a1, in `/home/teddy0/Documents/research/openmc-py-env` |
| Input deck | written for this record (no upstream deck): `openmc_inputs/make_references.py`, embedded below |
| Cross-section library | none — `--plot` does not load nuclear data (materials name H1 only so the XML is valid) |
| Particles / batches | not applicable (`run_mode = "plot"`; the `Settings` values are placeholders) |
| Date run | 2026-09-25 |
| Rust side | branch `develop` + this change; `cargo test --release -p outram-mc-libs --test geometry_plot_openmc_parity` |

### Driver script (verbatim)

```python
#!/usr/bin/env python3
"""Reference images for the outram-mc-libs geometry-plotting port (gh:#268).

Builds four small CSG models with OpenMC's Python API, writes one plots.xml
per model, runs ``openmc --plot`` and copies every PNG OpenMC writes into
``<out_dir>``. The Rust side (``tests/geometry_plot_openmc_parity.rs``)
rebuilds the SAME models cell-for-cell and compares its PNGs against these
pixel for pixel.

Nothing here needs a cross-section library: ``openmc --plot`` only locates
cells and traces rays.

Two properties of OpenMC the Rust mirror has to respect, stated here because
they decide the colours:

* **Cell and material index order.** OpenMC's Python API writes
  ``geometry.xml`` sorted by cell id, and the C++ reads cells in document
  order, so the cell *index* (which picks the default colour and the
  first-match order inside a universe) is the rank of the cell id. Materials
  are written in the order of the ``openmc.Materials`` list.
* **One plotter seed per run.** ``model::plotter_seed`` starts at 1 and every
  plot in a plots.xml draws its default colours from it in turn, so the
  second plot's colours depend on how many cells the first one coloured.
  Each case below therefore puts its plots in ONE plots.xml, in a fixed
  order, and the Rust test replays them in that order with one seed.

Usage:  python3 make_references.py <out_dir> [openmc_executable]

Provenance: OpenMC 0.16.1-dev25, commit d7d3284a1 (MIT, notice in
LICENSE.openmc), built with libpng; run 2026-09-25. This driver is new work,
GPL-3.0-only like the rest of the crate.
"""
import os
import shutil
import subprocess
import sys

import openmc

OPENMC = "/home/teddy0/Documents/research/openmcbin/bin/openmc"


def _mat(mid, name, density):
    m = openmc.Material(material_id=mid, name=name)
    m.add_nuclide("H1", 1.0)
    m.set_density("g/cm3", density)
    return m


def _settings(entropy_mesh=None):
    st = openmc.Settings()
    st.run_mode = "plot"
    st.particles = 100
    st.batches = 10
    if entropy_mesh is not None:
        st.entropy_mesh = entropy_mesh
    return st


def _run(case_dir, materials, geometry, settings, plots, out_dir, exe):
    os.makedirs(case_dir, exist_ok=True)
    cwd = os.getcwd()
    os.chdir(case_dir)
    try:
        materials.export_to_xml()
        geometry.export_to_xml()
        settings.export_to_xml()
        openmc.Plots(plots).export_to_xml()
        subprocess.run([exe, "--plot"], check=True, stdout=subprocess.DEVNULL)
        for p in plots:
            src = p.filename + ".png"
            shutil.copy(src, os.path.join(out_dir, src))
            print("wrote", src)
    finally:
        os.chdir(cwd)


# --------------------------------------------------------------------------
# (a) Godiva: one sphere, ICSBEP HEU-MET-FAST-001 radius.
# --------------------------------------------------------------------------
def godiva(work, out_dir, exe):
    heu = _mat(1, "heu", 18.74)
    s = openmc.Sphere(surface_id=1, r=8.7407, boundary_type="vacuum")
    c = openmc.Cell(cell_id=1, fill=heu, region=-s)
    geom = openmc.Geometry([c])

    p = openmc.SlicePlot(plot_id=1)
    p.filename = "godiva_xy_cell"
    p.basis = "xy"
    p.origin = (0.0, 0.0, 0.0)
    p.width = (24.0, 24.0)
    p.pixels = (81, 81)
    p.color_by = "cell"
    _run(os.path.join(work, "godiva"), openmc.Materials([heu]), geom,
         _settings(), [p], out_dir, exe)


# --------------------------------------------------------------------------
# (b) 3x3 pin lattice (8 fuel pins round a guide tube) in a water reflector
#     built from four intersection-only slabs.
# --------------------------------------------------------------------------
def pin_lattice_model():
    fuel = _mat(1, "fuel", 10.4)
    clad = _mat(2, "clad", 6.55)
    water = _mat(3, "water", 1.0)
    mats = openmc.Materials([fuel, clad, water])

    fuel_or = openmc.ZCylinder(surface_id=1, r=0.4096)
    clad_or = openmc.ZCylinder(surface_id=2, r=0.475)
    gt_ir = openmc.ZCylinder(surface_id=3, r=0.56)
    gt_or = openmc.ZCylinder(surface_id=4, r=0.60)
    xl = openmc.XPlane(surface_id=5, x0=-1.89)
    xr = openmc.XPlane(surface_id=6, x0=1.89)
    yl = openmc.YPlane(surface_id=7, y0=-1.89)
    yr = openmc.YPlane(surface_id=8, y0=1.89)
    zb = openmc.ZPlane(surface_id=9, z0=-10.0, boundary_type="vacuum")
    zt = openmc.ZPlane(surface_id=10, z0=10.0, boundary_type="vacuum")
    xo_l = openmc.XPlane(surface_id=11, x0=-3.0, boundary_type="vacuum")
    xo_r = openmc.XPlane(surface_id=12, x0=3.0, boundary_type="vacuum")
    yo_l = openmc.YPlane(surface_id=13, y0=-3.0, boundary_type="vacuum")
    yo_r = openmc.YPlane(surface_id=14, y0=3.0, boundary_type="vacuum")

    pin = openmc.Universe(universe_id=1, cells=[
        openmc.Cell(cell_id=1, fill=fuel, region=-fuel_or),
        openmc.Cell(cell_id=2, fill=clad, region=+fuel_or & -clad_or),
        openmc.Cell(cell_id=3, fill=water, region=+clad_or),
    ])
    gt = openmc.Universe(universe_id=2, cells=[
        openmc.Cell(cell_id=4, fill=water, region=-gt_ir),
        openmc.Cell(cell_id=5, fill=clad, region=+gt_ir & -gt_or),
        openmc.Cell(cell_id=6, fill=water, region=+gt_or),
    ])
    lat = openmc.RectLattice(lattice_id=10)
    lat.lower_left = (-1.89, -1.89)
    lat.pitch = (1.26, 1.26)
    lat.universes = [[pin, pin, pin], [pin, gt, pin], [pin, pin, pin]]

    zslab = +zb & -zt
    root = openmc.Universe(universe_id=0, cells=[
        openmc.Cell(cell_id=7, fill=lat, region=+xl & -xr & +yl & -yr & zslab),
        openmc.Cell(cell_id=8, fill=water, region=+xr & -xo_r & +yo_l & -yo_r & zslab),
        openmc.Cell(cell_id=9, fill=water, region=+xo_l & -xl & +yo_l & -yo_r & zslab),
        openmc.Cell(cell_id=10, fill=water, region=+xl & -xr & +yr & -yo_r & zslab),
        openmc.Cell(cell_id=11, fill=water, region=+xl & -xr & +yo_l & -yl & zslab),
    ])
    return mats, openmc.Geometry(root), (fuel, clad, water)


def pin_lattice(work, out_dir, exe):
    mats, geom, _ = pin_lattice_model()

    mesh = openmc.RegularMesh(mesh_id=1)
    mesh.lower_left = (-3.0, -3.0, -10.0)
    mesh.upper_right = (3.0, 3.0, 10.0)
    mesh.dimension = (4, 4, 1)

    plots = []

    def slice_plot(pid, name, basis, origin, width, pixels, color_by):
        p = openmc.SlicePlot(plot_id=pid)
        p.filename = name
        p.basis = basis
        p.origin = origin
        p.width = width
        p.pixels = pixels
        p.color_by = color_by
        plots.append(p)
        return p

    slice_plot(1, "lattice_xy_cell", "xy", (0.0, 0.0, 0.0), (6.4, 6.4), (160, 160), "cell")
    slice_plot(2, "lattice_xy_material", "xy", (0.0, 0.0, 0.0), (6.4, 6.4), (160, 160), "material")
    slice_plot(3, "lattice_xz_cell", "xz", (0.0, 0.2, 0.0), (6.4, 22.0), (64, 220), "cell")
    slice_plot(4, "lattice_yz_material", "yz", (0.1, 0.0, 5.0), (6.4, 6.4), (100, 100), "material")
    p = slice_plot(5, "lattice_xy_level0", "xy", (0.0, 0.0, 0.0), (6.4, 6.4), (160, 160), "cell")
    p.level = 0
    p = slice_plot(6, "lattice_xy_meshlines", "xy", (0.0, 0.0, 0.0), (6.4, 6.4), (160, 160), "material")
    p.meshlines = {"type": "entropy", "linewidth": 1, "color": (0, 0, 0)}

    _run(os.path.join(work, "lattice"), mats, geom, _settings(entropy_mesh=mesh),
         plots, out_dir, exe)


# --------------------------------------------------------------------------
# (c) Material vs cell colouring, masks, overlaps, a void cell, background.
#     Cells 1 and 2 are two spheres that OVERLAP on purpose.
# --------------------------------------------------------------------------
def overlap_mask(work, out_dir, exe):
    a = _mat(1, "a", 1.0)
    b = _mat(2, "b", 2.0)
    c = _mat(3, "c", 3.0)
    mats = openmc.Materials([a, b, c])

    s1 = openmc.Sphere(surface_id=1, x0=-1.0, r=2.0)
    s2 = openmc.Sphere(surface_id=2, x0=1.0, r=2.0)
    s3 = openmc.Sphere(surface_id=3, y0=3.0, r=0.5)
    box = [
        openmc.XPlane(surface_id=4, x0=-4.0, boundary_type="vacuum"),
        openmc.XPlane(surface_id=5, x0=4.0, boundary_type="vacuum"),
        openmc.YPlane(surface_id=6, y0=-4.0, boundary_type="vacuum"),
        openmc.YPlane(surface_id=7, y0=4.0, boundary_type="vacuum"),
        openmc.ZPlane(surface_id=8, z0=-4.0, boundary_type="vacuum"),
        openmc.ZPlane(surface_id=9, z0=4.0, boundary_type="vacuum"),
    ]
    inbox = +box[0] & -box[1] & +box[2] & -box[3] & +box[4] & -box[5]
    geom = openmc.Geometry([
        openmc.Cell(cell_id=1, fill=a, region=-s1),
        openmc.Cell(cell_id=2, fill=b, region=-s2),
        openmc.Cell(cell_id=3, fill=c, region=+s1 & +s2 & +s3 & inbox),
        openmc.Cell(cell_id=4, fill=None, region=-s3),
    ])

    plots = []

    def slice_plot(pid, name, color_by):
        p = openmc.SlicePlot(plot_id=pid)
        p.filename = name
        p.basis = "xy"
        p.origin = (0.0, 0.0, 0.0)
        p.width = (10.0, 10.0)
        p.pixels = (100, 100)
        p.color_by = color_by
        plots.append(p)
        return p

    p = slice_plot(1, "overlap_cell_show", "cell")
    p.show_overlaps = True
    p = slice_plot(2, "overlap_material_show", "material")
    p.show_overlaps = True
    p.overlap_color = (0, 255, 0)
    p.background = (10, 20, 30)
    p = slice_plot(3, "mask_cell", "cell")
    p.mask_components = [geom.get_all_cells()[2]]
    p.mask_background = (40, 40, 40)
    p.colors = {geom.get_all_cells()[3]: (200, 200, 0)}
    p = slice_plot(4, "mask_material", "material")
    p.mask_components = [c]
    p = slice_plot(5, "level1_cell", "cell")
    p.level = 1

    _run(os.path.join(work, "overlap"), mats, geom, _settings(), plots, out_dir, exe)


# --------------------------------------------------------------------------
# Ray-traced plots: a sphere in a box (simple) and the pin lattice (lattice).
# --------------------------------------------------------------------------
def sphere_in_box_model():
    a = _mat(1, "ball", 1.0)
    b = _mat(2, "box", 2.0)
    mats = openmc.Materials([a, b])
    s = openmc.Sphere(surface_id=1, r=3.0)
    box = [
        openmc.XPlane(surface_id=2, x0=-5.0, boundary_type="vacuum"),
        openmc.XPlane(surface_id=3, x0=5.0, boundary_type="vacuum"),
        openmc.YPlane(surface_id=4, y0=-5.0, boundary_type="vacuum"),
        openmc.YPlane(surface_id=5, y0=5.0, boundary_type="vacuum"),
        openmc.ZPlane(surface_id=6, z0=-5.0, boundary_type="vacuum"),
        openmc.ZPlane(surface_id=7, z0=5.0, boundary_type="vacuum"),
    ]
    inbox = +box[0] & -box[1] & +box[2] & -box[3] & +box[4] & -box[5]
    geom = openmc.Geometry([
        openmc.Cell(cell_id=1, fill=a, region=-s),
        openmc.Cell(cell_id=2, fill=b, region=+s & inbox),
    ])
    return mats, geom, (a, b)


def raytrace(work, out_dir, exe):
    mats, geom, (ball, boxm) = sphere_in_box_model()
    plots = []

    w = openmc.WireframeRayTracePlot(plot_id=1)
    w.filename = "sphere_wireframe"
    w.pixels = (120, 100)
    w.camera_position = (20.0, 15.0, 10.0)
    w.look_at = (0.0, 0.0, 0.0)
    w.horizontal_field_of_view = 45.0
    w.color_by = "material"
    w.colors = {boxm: (80, 160, 220)}
    w.xs = {boxm: 0.05}
    plots.append(w)

    s = openmc.SolidRayTracePlot(plot_id=2)
    s.filename = "sphere_solid"
    s.pixels = (120, 100)
    s.camera_position = (20.0, 15.0, 10.0)
    s.look_at = (0.0, 0.0, 0.0)
    s.horizontal_field_of_view = 45.0
    s.color_by = "material"
    s.opaque_domains = [ball]
    plots.append(s)

    _run(os.path.join(work, "raytrace_sphere"), mats, geom, _settings(), plots, out_dir, exe)

    mats, geom, (fuel, clad, water) = pin_lattice_model()
    plots = []

    w = openmc.WireframeRayTracePlot(plot_id=1)
    w.filename = "lattice_wireframe"
    w.pixels = (120, 100)
    w.camera_position = (12.0, 9.0, 18.0)
    w.look_at = (0.0, 0.0, 5.0)
    w.horizontal_field_of_view = 50.0
    w.color_by = "material"
    w.colors = {water: (120, 170, 255), clad: (150, 150, 150)}
    w.xs = {water: 0.02, clad: 0.5}
    w.wireframe_thickness = 2
    plots.append(w)

    s = openmc.SolidRayTracePlot(plot_id=2)
    s.filename = "lattice_solid"
    s.pixels = (120, 100)
    s.camera_position = (12.0, 9.0, 18.0)
    s.look_at = (0.0, 0.0, 5.0)
    s.horizontal_field_of_view = 50.0
    s.color_by = "material"
    s.opaque_domains = [fuel, clad]
    s.light_position = (-10.0, 20.0, 30.0)
    s.diffuse_fraction = 0.3
    plots.append(s)

    o = openmc.WireframeRayTracePlot(plot_id=3)
    o.filename = "lattice_wireframe_ortho"
    o.pixels = (100, 100)
    o.camera_position = (0.0, -30.0, 0.0)
    o.look_at = (0.0, 0.0, 0.0)
    o.orthographic_width = 8.0
    o.color_by = "cell"
    plots.append(o)

    _run(os.path.join(work, "raytrace_lattice"), mats, geom, _settings(), plots, out_dir, exe)


def main(out_dir, exe):
    out_dir = os.path.abspath(out_dir)
    os.makedirs(out_dir, exist_ok=True)
    work = os.path.join(out_dir, "_work")
    godiva(work, out_dir, exe)
    pin_lattice(work, out_dir, exe)
    overlap_mask(work, out_dir, exe)
    raytrace(work, out_dir, exe)
    shutil.rmtree(work)


if __name__ == "__main__":
    main(sys.argv[1] if len(sys.argv) > 1 else ".",
         sys.argv[2] if len(sys.argv) > 2 else OPENMC)
```
