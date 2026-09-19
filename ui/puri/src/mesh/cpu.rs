//! Depth-buffered fallback for the browser and headless captures.

use super::{Geometry, Surface, Vector3, View};

pub(super) fn render_surface(
    geometry: &Geometry,
    view: &View,
    surface: Option<&Surface>,
) -> Option<Vec<u8>> {
    if view.width == 0 || view.height == 0 {
        return None;
    }
    let count = (view.width as usize).checked_mul(view.height as usize)?;
    let mut rgba = vec![0; count.checked_mul(4)?];
    let mut depth = vec![1.0; count];
    let visible = super::visible_indices(geometry, surface)?;
    let vertex_count = visible.iter().max().map_or(0, |i| *i as usize + 1);
    let vertices: Vec<_> = geometry
        .vertices
        .get(..vertex_count)?
        .iter()
        .map(|vertex| {
            let p = view
                .model_to_view
                .transform_point(&nalgebra::Point3::from(vertex.position))
                .coords;
            let [sx, sy, sz, dz] = view.projection;
            (
                p,
                Vector3::new(
                    (p.x * sx + 1.0) * view.width as f32 / 2.0,
                    (1.0 - p.y * sy) * view.height as f32 / 2.0,
                    p.z * sz + dz,
                ),
                view.model_to_view.transform_vector(&vertex.normal.vector()),
            )
        })
        .collect();
    let edge = |a: Vector3<f32>, b: Vector3<f32>, x: f32, y: f32| {
        (b.x - a.x) * (y - a.y) - (b.y - a.y) * (x - a.x)
    };
    let light = Vector3::new(0.35, -0.45, 1.0).normalize();
    let draw = |indices: &[u32], rgba: &mut [u8], depth: &mut [f32]| {
        for triangle in indices.chunks_exact(3) {
            let [i, j, k] = [
                triangle[0] as usize,
                triangle[1] as usize,
                triangle[2] as usize,
            ];
            let [(pa, a, na), (pb, b, nb), (pc, c, nc)] = [vertices[i], vertices[j], vertices[k]];
            let area = edge(a, b, c.x, c.y);
            if !area.is_finite() || area == 0.0 {
                continue;
            }
            let face = (pb - pa).cross(&(pc - pa));
            let shade = |normal: Vector3<f32>| {
                let normal = if face.z < 0.0 { -normal } else { normal }.normalize();
                let brightness = 0.22 + 0.78 * normal.dot(&light).max(0.0);
                geometry.vertices[i]
                    .color
                    .map(|v| (v * brightness * 255.0).round() as u8)
            };
            let flat_color = shade(face);
            let smooth = na != Vector3::zeros() || nb != Vector3::zeros() || nc != Vector3::zeros();
            let min_x = a.x.min(b.x).min(c.x).floor().max(0.0) as u32;
            let max_x = a.x.max(b.x).max(c.x).ceil().min(view.width as f32) as u32;
            let min_y = a.y.min(b.y).min(c.y).floor().max(0.0) as u32;
            let max_y = a.y.max(b.y).max(c.y).ceil().min(view.height as f32) as u32;
            for y in min_y..max_y {
                for x in min_x..max_x {
                    let [wa, wb, wc] = [
                        edge(b, c, x as f32 + 0.5, y as f32 + 0.5),
                        edge(c, a, x as f32 + 0.5, y as f32 + 0.5),
                        edge(a, b, x as f32 + 0.5, y as f32 + 0.5),
                    ]
                    .map(|w| w / area);
                    if wa < 0.0 || wb < 0.0 || wc < 0.0 {
                        continue;
                    }
                    let z = wa * a.z + wb * b.z + wc * c.z;
                    let index = (y * view.width + x) as usize;
                    if z >= 0.0 && z < depth[index] {
                        depth[index] = z;
                        let color = if smooth {
                            let interpolated = na * wa + nb * wb + nc * wc;
                            if interpolated.norm_squared() > 0.0 {
                                shade(interpolated)
                            } else {
                                flat_color
                            }
                        } else {
                            flat_color
                        };
                        rgba[4 * index..4 * index + 4]
                            .copy_from_slice(&[color[0], color[1], color[2], 255]);
                    }
                }
            }
        }
    };
    if let Some(surface) = surface {
        if surface.frame.is_partial() {
            draw(
                &geometry.indices[surface.mesh_start..],
                &mut rgba,
                &mut depth,
            );
        }
        let image = &surface.frame.image;
        for y in 0..view.height as usize {
            for x in 0..view.width as usize {
                let src = (y * image.height as usize / view.height as usize) * image.width as usize
                    + x * image.width as usize / view.width as usize;
                let dst = y * view.width as usize + x;
                if surface.frame.depth[src] >= 0.0 {
                    depth[dst] = surface.frame.depth[src];
                    rgba[dst * 4..dst * 4 + 4]
                        .copy_from_slice(&image.data.data()[src * 4..src * 4 + 4]);
                }
            }
        }
        draw(
            &geometry.indices[..surface.mesh_start],
            &mut rgba,
            &mut depth,
        );
    } else {
        draw(&geometry.indices, &mut rgba, &mut depth);
    }
    Some(rgba)
}
