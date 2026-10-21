// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Step 1's DEM pour drawn as sphere impostors (gh:#587): one screen-aligned
// quad per pebble (instanced, no vertex buffer for the quad), and per
// fragment the exact ray-sphere hit of an orthographic view ray, with the
// sphere's own depth written so pebbles hide each other correctly. The
// projection is DemView::project's (dem.rs): screen right, screen up and
// depth toward the viewer, from yaw and pitch about the vessel axis.

struct U {
    // cos yaw, sin yaw, cos pitch, sin pitch
    rot: vec4<f32>,
    // NDC per metre (x, y), z of the view centre [m], pebble radius [m]
    view: vec4<f32>,
    // 1 / (depth range) [1/m], half cutaway (1 = only y >= 0), 0, 0
    misc: vec4<f32>,
};

@group(0) @binding(0) var<uniform> u: U;

struct VOut {
    @builtin(position) pos: vec4<f32>,
    // corner of the quad in units of the radius
    @location(0) corner: vec2<f32>,
    // depth of the centre (toward the viewer) [m], world z [m]
    @location(1) info: vec2<f32>,
};

fn project(p: vec3<f32>) -> vec3<f32> {
    let cy = u.rot.x;
    let sy = u.rot.y;
    let cp = u.rot.z;
    let sp = u.rot.w;
    let h = cy * p.x + sy * p.y;
    return vec3<f32>(-sy * p.x + cy * p.y, -sp * h + cp * p.z, cp * h + sp * p.z);
}

@vertex
fn vs(@builtin(vertex_index) vi: u32, @location(0) centre: vec3<f32>) -> VOut {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, -1.0), vec2<f32>(1.0, 1.0),
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, 1.0), vec2<f32>(-1.0, 1.0));
    let c = corners[vi];
    var o: VOut;
    o.corner = c;
    let s = project(centre);
    o.info = vec2<f32>(s.z, centre.z);
    if (u.misc.y > 0.5 && centre.y < 0.0) {
        // The near half is cut away: a degenerate quad outside the view.
        o.pos = vec4<f32>(2.0, 2.0, 2.0, 1.0);
        return o;
    }
    let r = u.view.w;
    let xy = vec2<f32>(s.x + c.x * r, s.y - u.view.z + c.y * r) * u.view.xy;
    o.pos = vec4<f32>(xy, 0.5, 1.0);
    return o;
}

struct FOut {
    @location(0) colour: vec4<f32>,
    @builtin(frag_depth) depth: f32,
};

@fragment
fn fs(i: VOut) -> FOut {
    let q = dot(i.corner, i.corner);
    if (q > 1.0) { discard; }
    let nz = sqrt(1.0 - q);
    // View-space normal: screen right, screen up, toward the viewer.
    let n = vec3<f32>(i.corner.x, i.corner.y, nz);
    let light = normalize(vec3<f32>(-0.45, 0.6, 0.66));
    let lambert = max(dot(n, light), 0.0);
    // Conus and tube pebbles darker, as the CPU painter draws them.
    var base = vec3<f32>(175.0, 175.0, 182.0) / 255.0;
    if (i.info.y < 0.0) { base = vec3<f32>(120.0, 120.0, 128.0) / 255.0; }
    var c = base * (0.3 + 0.75 * lambert);
    // A thin dark rim, the CPU painter's outline.
    if (q > 0.88) { c = c * 0.45; }
    var o: FOut;
    o.colour = vec4<f32>(c, 1.0);
    // Nearer (larger depth toward the viewer) is smaller in [0, 1].
    let d = i.info.x + nz * u.view.w;
    o.depth = clamp(0.5 - d * u.misc.x, 0.0, 1.0);
    return o;
}
