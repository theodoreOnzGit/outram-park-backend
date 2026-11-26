// Review images for Mesh Studio (gh:#1771, the drawing rule): every view is
// the real page drawing the ASSEMBLED mesh the worker built, with its colour
// bar, scale bar and report. Re-run after any change to the meshes:
//
//   crates/dhoby-ghaut/web/mesh_studio/build.sh /tmp/mesh-dist
//   python3 -m http.server -d /tmp/mesh-dist 8765 &
//   node crates/dhoby-ghaut/verification_and_validation/mesh_studio_gh1771/render.mjs \
//        http://localhost:8765 crates/dhoby-ghaut/verification_and_validation/mesh_studio_gh1771
//
// Needs Playwright with Chromium. Prints each page title (shape, tet source
// and cell count) so a run can be compared with the README's table. An
// optional third argument, a regular expression, renders only the views
// whose names match (e.g. `_graded_` for the gh:#1825 set).
import { chromium } from "playwright";
const [base, out, only] = process.argv.slice(2);
const pick = only ? new RegExp(only) : null;
// Since gh:#1796 the page's default tet source is TetGen, so every view
// without `tets=` shows TetGen's tetrahedra; `tets=carved` asks for the
// structured carve-and-split route the first set (gh:#1771) showed.
const T = "tets=tetgen";
const views = [
  // The gh:#1771 set, now on the default (TetGen) tets.
  ["pipe_cross_layers", "shape=pipe&colour=layer&axis=z"],
  ["pipe_axial_layers", "shape=pipe&colour=layer&axis=x"],
  ["pipe_cross_nonorth", "shape=pipe&colour=nonorth&axis=z"],
  ["fuel_channel_cross_layers", "shape=fuel-channel&colour=layer&axis=z"],
  ["pipe_wall_layers_zoom", "shape=pipe&colour=layer&axis=z&at=97,0&zoom=30"],
  ["fuel_channel_pin_wall_layers_zoom", "shape=fuel-channel&colour=layer&axis=z&at=52,0&zoom=20"],
  ["fuel_channel_outer_wall_layers_zoom", "shape=fuel-channel&colour=layer&axis=z&at=97,0&zoom=20"],
  ["fuel_channel_axial_layers", "shape=fuel-channel&colour=layer&axis=x"],
  ["fuel_channel_growth_1_of_3", "shape=fuel-channel&colour=layer&axis=z&layers=1"],
  ["fuel_channel_growth_1_of_3_zoom", "shape=fuel-channel&colour=layer&axis=z&layers=1&at=52,0&zoom=20"],
  ["fuel_channel_stage_1_carve", "shape=fuel-channel&tets=carved&colour=nonorth&axis=z&stage=carve"],
  ["fuel_channel_stage_3_tets", "shape=fuel-channel&tets=carved&colour=nonorth&axis=z&stage=tetrahedralise"],
  ["fuel_channel_stage_5_dual", "shape=fuel-channel&tets=carved&colour=nonorth&axis=z&stage=polyhedral-dual"],
  ["pebble_axial_nonorth", "shape=pebble&colour=nonorth&axis=x"],
  ["pebble_cross_skew", "shape=pebble&colour=skew&axis=z"],
  ["box_cross_layers", "shape=box&colour=layer&axis=z"],
  // gh:#1796: the four mesh types on TetGen's tets, per reactor shape.
  ["pipe_tetgen_tets", `shape=pipe&${T}&mesh=tet&n=0&colour=nonorth&axis=z&at=0,0&zoom=3`],
  ["pipe_tetgen_tets_layers_zoom", `shape=pipe&${T}&mesh=tet&n=3&strategy=tetmesh&colour=layer&axis=z&at=97,0&zoom=30`],
  ["pipe_tetgen_dual", `shape=pipe&${T}&mesh=poly&n=0&colour=nonorth&axis=z&at=0,0&zoom=3`],
  ["pipe_tetgen_dual_layers_zoom", `shape=pipe&${T}&mesh=poly&n=3&colour=layer&axis=z&at=97,0&zoom=30`],
  ["pipe_tetgen_dual_20_layers_zoom", `shape=pipe&${T}&mesh=poly&n=20&first=0.002&ratio=1.1&strategy=cfmesh&colour=layer&axis=z&at=94,0&zoom=20`],
  ["fuel_channel_tetgen_tets", `shape=fuel-channel&${T}&mesh=tet&n=0&colour=nonorth&axis=z&at=0,0&zoom=1.5`],
  ["fuel_channel_tetgen_tets_layers_zoom", `shape=fuel-channel&${T}&mesh=tet&n=3&strategy=tetmesh&colour=layer&axis=z&at=52,0&zoom=20`],
  ["fuel_channel_tetgen_dual", `shape=fuel-channel&${T}&mesh=poly&n=0&colour=nonorth&axis=z&at=0,0&zoom=1.5`],
  ["fuel_channel_tetgen_dual_layers_zoom", `shape=fuel-channel&${T}&mesh=poly&n=3&colour=layer&axis=z&at=52,0&zoom=20`],
  ["pebble_tetgen_tets", `shape=pebble&${T}&mesh=tet&n=0&colour=nonorth&axis=z&at=0,0&zoom=1.5`],
  ["pebble_tetgen_tets_layers_zoom", `shape=pebble&${T}&mesh=tet&n=3&strategy=tetmesh&colour=layer&axis=z&at=59,0&zoom=20`],
  ["pebble_tetgen_dual", `shape=pebble&${T}&mesh=poly&n=0&colour=nonorth&axis=z&at=0,0&zoom=1.5`],
  ["pebble_tetgen_dual_layers_zoom", `shape=pebble&${T}&mesh=poly&n=3&strategy=cfmesh&colour=layer&axis=z&at=59,0&zoom=20`],
  // gh:#1796: the structured-vs-TetGen pair for the dual, same view.
  ["pair_pipe_dual_carved", "shape=pipe&tets=carved&mesh=poly&n=0&colour=nonorth&axis=z&at=0,0&zoom=3"],
  ["pair_pipe_dual_tetgen", `shape=pipe&${T}&mesh=poly&n=0&colour=nonorth&axis=z&at=0,0&zoom=3`],
  // Phone width, controls folded (the mobile-first rule).
  ["phone_pebble_tetgen", `shape=pebble&${T}&colour=layer&axis=z`, 390, 844],
  // gh:#1825: TetGen's tets graded toward the walls (`grade=on`: walls half
  // the cell size, growth 1.2, twice the cell size inside) and their dual,
  // z- and x-cuts, and a zoom across the walls; the ungraded pair beside.
  ["pipe_graded_tets_z", `shape=pipe&${T}&grade=on&mesh=tet&n=0&colour=nonorth&axis=z&at=0,0&zoom=3`],
  ["pipe_graded_tets_x", `shape=pipe&${T}&grade=on&mesh=tet&n=0&colour=nonorth&axis=x`],
  ["pipe_graded_dual_z", `shape=pipe&${T}&grade=on&mesh=poly&n=0&colour=nonorth&axis=z&at=0,0&zoom=3`],
  ["pipe_graded_dual_x", `shape=pipe&${T}&grade=on&mesh=poly&n=0&colour=nonorth&axis=x`],
  ["pipe_graded_dual_wall_zoom", `shape=pipe&${T}&grade=on&mesh=poly&n=0&colour=nonorth&axis=z&at=80,0&zoom=6`],
  ["pipe_ungraded_dual_wall_zoom", `shape=pipe&${T}&mesh=poly&n=0&colour=nonorth&axis=z&at=80,0&zoom=6`],
  ["fuel_channel_graded_tets_z", `shape=fuel-channel&${T}&grade=on&mesh=tet&n=0&colour=nonorth&axis=z&at=0,0&zoom=1.5`],
  ["fuel_channel_graded_tets_x", `shape=fuel-channel&${T}&grade=on&mesh=tet&n=0&colour=nonorth&axis=x`],
  ["fuel_channel_graded_dual_z", `shape=fuel-channel&${T}&grade=on&mesh=poly&n=0&colour=nonorth&axis=z&at=0,0&zoom=1.5`],
  ["fuel_channel_graded_dual_x", `shape=fuel-channel&${T}&grade=on&mesh=poly&n=0&colour=nonorth&axis=x`],
  ["fuel_channel_graded_dual_wall_zoom", `shape=fuel-channel&${T}&grade=on&mesh=poly&n=0&colour=nonorth&axis=z&at=75,0&zoom=6`],
  ["pebble_graded_tets_z", `shape=pebble&${T}&grade=on&mesh=tet&n=0&colour=nonorth&axis=z&at=0,0&zoom=1.5`],
  ["pebble_graded_tets_x", `shape=pebble&${T}&grade=on&mesh=tet&n=0&colour=nonorth&axis=x`],
  ["pebble_graded_dual_z", `shape=pebble&${T}&grade=on&mesh=poly&n=0&colour=nonorth&axis=z&at=0,0&zoom=1.5`],
  ["pebble_graded_dual_x", `shape=pebble&${T}&grade=on&mesh=poly&n=0&colour=nonorth&axis=x`],
  ["pebble_graded_dual_wall_zoom", `shape=pebble&${T}&grade=on&mesh=poly&n=0&colour=nonorth&axis=z&at=80,0&zoom=6`],
];
const browser = await chromium.launch({});
for (const [name, query, w = 1100, h = 760] of views) {
  if (pick && !pick.test(name)) continue;
  const page = await browser.newPage({ viewport: { width: w, height: h } });
  await page.goto(`${base}/?${query}`);
  // cfMesh's layer optimiser takes minutes in a browser: wait up to 20.
  for (let i = 0; i < 4800 && !/cells/.test(await page.title()); i++) await page.waitForTimeout(250);
  await page.waitForTimeout(1500);
  await page.screenshot({ path: `${out}/${name}.png` });
  console.log(`${name}\t${await page.title()}`);
  await page.close();
}
await browser.close();
