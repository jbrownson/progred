//! Depth-buffered fallback for the browser and headless captures.

use super::{Geometry, Vector3, View};

pub(super) fn render(geometry: &Geometry, view: &View) -> Option<Vec<u8>> {
    let count = (view.width as usize).checked_mul(view.height as usize)?;
    let mut rgba = vec![0; count.checked_mul(4)?];
    let mut depth = vec![1.0; count];
    let vertices: Vec<_> = geometry
        .vertices
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
            )
        })
        .collect();
    let edge = |a: Vector3<f32>, b: Vector3<f32>, x: f32, y: f32| {
        (b.x - a.x) * (y - a.y) - (b.y - a.y) * (x - a.x)
    };
    let light = Vector3::new(0.35, -0.45, 1.0).normalize();
    for triangle in geometry.indices.chunks_exact(3) {
        let [i, j, k] = [
            triangle[0] as usize,
            triangle[1] as usize,
            triangle[2] as usize,
        ];
        let [(pa, a), (pb, b), (pc, c)] = [vertices[i], vertices[j], vertices[k]];
        let area = edge(a, b, c.x, c.y);
        if !area.is_finite() || area == 0.0 {
            continue;
        }
        let normal = (pb - pa).cross(&(pc - pa));
        let normal = if normal.z < 0.0 { -normal } else { normal }.normalize();
        let brightness = 0.22 + 0.78 * normal.dot(&light).max(0.0);
        let color = geometry.vertices[i]
            .color
            .map(|v| (v * brightness * 255.0).round() as u8);
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
                    rgba[4 * index..4 * index + 4]
                        .copy_from_slice(&[color[0], color[1], color[2], 255]);
                }
            }
        }
    }
    Some(rgba)
}
