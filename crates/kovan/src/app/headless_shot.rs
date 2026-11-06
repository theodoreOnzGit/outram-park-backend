//! **Headless screenshots for tests** (GitHub #1019): run an egui frame
//! with no window or GPU, tessellate it, and rasterise the triangles in
//! software to a PNG, so a review can see a view before reading its code
//! (`docs/claude-md/visual-review-first.md`). Test-only.
//!
//! The rasteriser is deliberately small: nearest-texel sampling, vertex
//! colours interpolated barycentrically, premultiplied-alpha "over"
//! blending in sRGB space (as egui's own renderers blend), the clip
//! rectangle honoured, one pixel per point. It is a faithful picture of the
//! layout and the text, not a pixel match of the wgpu backend.

use std::collections::HashMap;
use std::path::Path;

use egui::epaint::{ClippedPrimitive, Primitive};
use egui::{Color32, ColorImage, Context, FullOutput, ImageData, Pos2, RawInput, Rect, TextureId, Vec2};

/// The textures egui has handed out, kept across frames.
#[derive(Default)]
pub(crate) struct Shooter {
    textures: HashMap<TextureId, ColorImage>,
}

impl Shooter {
    /// One frame of `size` points: run `f`, keep the texture updates.
    pub(crate) fn frame(
        &mut self,
        ctx: &Context,
        size: Vec2,
        f: impl FnMut(&mut egui::Ui),
    ) -> FullOutput {
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
            ..RawInput::default()
        };
        ctx.set_pixels_per_point(1.0);
        let out = ctx.run_ui(input, f);
        for (id, deltas) in &out.textures_delta.set {
            for delta in deltas {
                let ImageData::Color(img) = &delta.image;
                match delta.pos {
                    None => {
                        self.textures.insert(*id, (**img).clone());
                    }
                    Some([x0, y0]) => {
                        if let Some(t) = self.textures.get_mut(id) {
                            for y in 0..img.size[1] {
                                for x in 0..img.size[0] {
                                    t.pixels[(y0 + y) * t.size[0] + x0 + x] =
                                        img.pixels[y * img.size[0] + x];
                                }
                            }
                        }
                    }
                }
            }
        }
        out
    }

    /// Rasterise `out` (a frame of `size` points) on `bg` and write a PNG
    /// to `path` (its folder is created).
    pub(crate) fn save(
        &self,
        ctx: &Context,
        out: FullOutput,
        size: Vec2,
        bg: Color32,
        path: &Path,
    ) {
        let (w, h) = (size.x as usize, size.y as usize);
        let mut px = vec![bg; w * h];
        let prims: Vec<ClippedPrimitive> = ctx.tessellate(out.shapes, 1.0);
        for p in prims {
            let Primitive::Mesh(mesh) = p.primitive else {
                continue;
            };
            let tex = self.textures.get(&mesh.texture_id);
            let clip = p.clip_rect.intersect(Rect::from_min_size(Pos2::ZERO, size));
            for tri in mesh.indices.chunks_exact(3) {
                let v = [tri[0], tri[1], tri[2]].map(|i| mesh.vertices[i as usize]);
                let (a, b, c) = (v[0].pos, v[1].pos, v[2].pos);
                let area = (b - a).x * (c - a).y - (b - a).y * (c - a).x;
                if area.abs() < 1e-6 {
                    continue;
                }
                let x0 = a.x.min(b.x).min(c.x).max(clip.min.x).floor().max(0.0) as usize;
                let x1 = a.x.max(b.x).max(c.x).min(clip.max.x).ceil().min(w as f32) as usize;
                let y0 = a.y.min(b.y).min(c.y).max(clip.min.y).floor().max(0.0) as usize;
                let y1 = a.y.max(b.y).max(c.y).min(clip.max.y).ceil().min(h as f32) as usize;
                for y in y0..y1 {
                    for x in x0..x1 {
                        let q = Pos2::new(x as f32 + 0.5, y as f32 + 0.5);
                        let w0 = ((b - q).x * (c - q).y - (b - q).y * (c - q).x) / area;
                        let w1 = ((c - q).x * (a - q).y - (c - q).y * (a - q).x) / area;
                        let w2 = 1.0 - w0 - w1;
                        if w0 < -1e-4 || w1 < -1e-4 || w2 < -1e-4 {
                            continue;
                        }
                        let lerp = |f: fn(Color32) -> u8| {
                            w0 * f(v[0].color) as f32
                                + w1 * f(v[1].color) as f32
                                + w2 * f(v[2].color) as f32
                        };
                        let mut col = [
                            lerp(|c| c.r()),
                            lerp(|c| c.g()),
                            lerp(|c| c.b()),
                            lerp(|c| c.a()),
                        ];
                        if let Some(t) = tex {
                            let u = w0 * v[0].uv.x + w1 * v[1].uv.x + w2 * v[2].uv.x;
                            let vv = w0 * v[0].uv.y + w1 * v[1].uv.y + w2 * v[2].uv.y;
                            let tx = ((u * t.size[0] as f32) as usize).min(t.size[0] - 1);
                            let ty = ((vv * t.size[1] as f32) as usize).min(t.size[1] - 1);
                            let s = t.pixels[ty * t.size[0] + tx];
                            let m = [s.r(), s.g(), s.b(), s.a()];
                            for k in 0..4 {
                                col[k] *= m[k] as f32 / 255.0;
                            }
                        }
                        let d = &mut px[y * w + x];
                        let inv = 1.0 - col[3] / 255.0;
                        let mix = |s: f32, dst: u8| {
                            (s + dst as f32 * inv).round().clamp(0.0, 255.0) as u8
                        };
                        *d = Color32::from_rgba_premultiplied(
                            mix(col[0], d.r()),
                            mix(col[1], d.g()),
                            mix(col[2], d.b()),
                            255,
                        );
                    }
                }
            }
        }
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).unwrap();
        }
        let raw: Vec<u8> = px.iter().flat_map(|c| [c.r(), c.g(), c.b(), 255]).collect();
        image::RgbaImage::from_raw(w as u32, h as u32, raw)
            .unwrap()
            .save(path)
            .unwrap();
    }
}
