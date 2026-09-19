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

impl DepthImage {
    pub fn is_partial(&self) -> bool {
        self.partial
    }
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

/// Select the required triangle prefix, checking the optional image/split contract.
pub fn visible_indices<'a>(geometry: &'a Geometry, surface: Option<&Surface>) -> Option<&'a [u32]> {
    if let Some(surface) = surface {
        let image = &surface.frame.image;
        let pixels = (image.width as usize).checked_mul(image.height as usize)?;
        if image.width == 0
            || image.height == 0
            || image.format != ImageFormat::Rgba8
            || image.alpha_type != ImageAlphaType::Alpha
            || image.data.data().len() < pixels.checked_mul(4)?
            || surface.frame.depth.len() != pixels
            || surface.mesh_start % 3 != 0
            || surface.mesh_start > geometry.indices.len()
        {
            return None;
        }
        if !surface.frame.is_partial() {
            return geometry.indices.get(..surface.mesh_start);
        }
    }
    Some(&geometry.indices)
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
mod tests {
    use super::*;
    use crate::draw::{Canvas, DrawCmd, DrawList};
    use kurbo::Affine;

    fn scene() -> Scene {
        Scene {
            geometry: Mesh::new(Geometry {
                vertices: [[-0.9, -0.8, 0.0], [0.9, -0.8, 0.0], [0.0, 0.9, 0.0]]
                    .map(|p| Vertex {
                        position: Vector3::from(p),
                        color: [1.0, 0.0, 0.0],
                        normal: Normal::default(),
                    })
                    .into(),
                indices: vec![0, 1, 2],
            }),
            view: View {
                model_to_view: Matrix4::identity(),
                projection: [1.0, 1.0, -0.5, 0.5],
                width: 16,
                height: 16,
            },
            surface: None,
        }
    }

    #[test]
    fn recording_and_replay_retain_geometry_without_rasterizing() {
        let scene = scene();
        let mesh = scene.geometry.clone();
        let inner = Affine::scale(0.5);
        let outer = Affine::translate((10.0, 20.0));
        let mut list = DrawList::new();
        list.mesh(scene, inner);
        let mut replay = DrawList::new();
        crate::draw::replay_at(&list, &mut replay, outer);
        let [DrawCmd::Mesh { scene, transform }] = replay.0.as_slice() else {
            panic!("mesh stays a mesh");
        };
        assert!(std::sync::Arc::ptr_eq(&mesh, &scene.geometry));
        assert_eq!(*transform, outer * inner);
        let image = scene.rasterize().unwrap();
        assert_eq!((image.width, image.height), (16, 16));
        assert!(
            image
                .data
                .data()
                .chunks_exact(4)
                .any(|p| p[0] > 0 && p[3] == 255)
        );
    }

    #[test]
    fn invalid_geometry_and_surface_decline_without_panicking() {
        let mut scene = scene();
        std::sync::Arc::make_mut(&mut scene.geometry).indices[0] = 100;
        assert!(scene.rasterize().is_none());
        scene.geometry = Mesh::default();
        scene.surface = Some(Surface {
            frame: DepthImage {
                image: ImageData {
                    data: vec![0; 4].into(),
                    width: 1,
                    height: 1,
                    format: ImageFormat::Rgba8,
                    alpha_type: ImageAlphaType::Alpha,
                },
                depth: vec![1.0].into(),
                partial: false,
            },
            mesh_start: 3,
        });
        assert!(scene.rasterize().is_none());
        scene.surface.as_mut().unwrap().mesh_start = 0;
        assert!(scene.rasterize().is_some());
        scene.surface.as_mut().unwrap().frame.depth = vec![].into();
        assert!(scene.rasterize().is_none());
    }
}
