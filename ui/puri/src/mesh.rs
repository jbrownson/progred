//! Backend-neutral orthographic triangle drawing, with optional depth-image replacement.

use nalgebra::{Matrix4, Vector3};
use peniko::{ImageAlphaType, ImageData, ImageFormat};

mod cpu;

/// Orthographic camera with a rigid model-to-view transform. `projection` contains x/y scale,
/// depth scale and depth offset; depth is 0 at the near plane and 1 at the far plane.
#[derive(Clone, Debug)]
pub struct View {
    pub model_to_view: Matrix4<f32>,
    pub projection: [f32; 4],
    pub width: u32,
    pub height: u32,
}

/// Straight-alpha RGBA8 and depth, published together. Negative depth denotes
/// unfinished pixels; depth 1 denotes empty/far pixels (including transparent ones).
#[derive(Clone, Debug)]
pub struct DepthImage {
    pub image: ImageData,
    pub depth: std::sync::Arc<[f32]>,
    pub partial: bool,
}

/// Replace the draft triangles starting at `mesh_start` with completed raster
/// pixels, then depth-test the preceding triangles against that surface.
#[derive(Clone, Debug)]
pub struct Surface {
    pub frame: DepthImage,
    pub mesh_start: usize,
}

#[derive(Clone, Debug)]
pub struct Scene {
    pub geometry: Mesh,
    pub view: View,
    pub surface: Option<Surface>,
}

impl Scene {
    /// CPU interpretation for backends without a triangle renderer.
    pub fn rasterize(&self) -> Option<ImageData> {
        Some(ImageData {
            data: cpu::render_surface(&self.geometry, &self.view, self.surface.as_ref())?.into(),
            format: ImageFormat::Rgba8,
            alpha_type: ImageAlphaType::Alpha,
            width: self.view.width,
            height: self.view.height,
        })
    }
}

/// `surface` when it keeps the image/split contract with `geometry`. A
/// malformed surface is left out, so the whole mesh draws without it.
pub fn usable_surface<'a>(
    geometry: &Geometry,
    surface: Option<&'a Surface>,
) -> Option<&'a Surface> {
    surface.filter(|surface| {
        let image = &surface.frame.image;
        (image.width as usize)
            .checked_mul(image.height as usize)
            .and_then(|pixels| Some((pixels, pixels.checked_mul(4)?)))
            .is_some_and(|(pixels, bytes)| {
                pixels > 0
                    && image.format == ImageFormat::Rgba8
                    && image.alpha_type == ImageAlphaType::Alpha
                    && image.data.data().len() >= bytes
                    && surface.frame.depth.len() == pixels
                    && surface.mesh_start % 3 == 0
                    && surface.mesh_start <= geometry.indices.len()
            })
    })
}

/// The triangles to rasterize beside a usable `surface`: all of them, or,
/// when the surface is complete, the prefix before the ones it replaces.
pub fn visible_indices<'a>(geometry: &'a Geometry, surface: Option<&Surface>) -> &'a [u32] {
    surface
        .filter(|surface| !surface.frame.partial)
        .and_then(|surface| geometry.indices.get(..surface.mesh_start))
        .unwrap_or(&geometry.indices)
}

#[derive(Clone, Copy, Debug)]
pub struct Vertex {
    pub position: Vector3<f32>,
    pub color: [f32; 3],
    pub normal: Normal,
}

/// GPU-native signed normalized bytes; zero requests the triangle's normal.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Normal(pub [i8; 4]);

impl Normal {
    pub fn new(normal: Vector3<f32>) -> Self {
        Self([
            (normal.x.clamp(-1.0, 1.0) * 127.0).round() as i8,
            (normal.y.clamp(-1.0, 1.0) * 127.0).round() as i8,
            (normal.z.clamp(-1.0, 1.0) * 127.0).round() as i8,
            0,
        ])
    }

    pub fn vector(self) -> Vector3<f32> {
        Vector3::new(self.0[0] as f32, self.0[1] as f32, self.0[2] as f32) / 127.0
    }
}

#[derive(Clone, Default, Debug)]
pub struct Geometry {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}

/// Shared render input. Edits through `Arc::make_mut` detach any retained upload identity.
pub type Mesh = std::sync::Arc<Geometry>;

impl Geometry {
    pub fn append_colored(
        &mut self,
        other: &Self,
        color: impl Fn([f32; 3]) -> [f32; 3],
    ) -> Option<()> {
        let offset = u32::try_from(self.vertices.len()).ok()?;
        u32::try_from(self.vertices.len().checked_add(other.vertices.len())?).ok()?;
        self.vertices
            .extend(other.vertices.iter().map(|vertex| Vertex {
                position: vertex.position,
                color: color(vertex.color),
                normal: vertex.normal,
            }));
        self.indices
            .extend(other.indices.iter().map(|index| offset + index));
        Some(())
    }
}

#[cfg(test)]
mod tests;
