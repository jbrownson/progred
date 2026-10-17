//! Triangle meshes of a scene's surfaces.

use crate::SceneObject;
use fidget_engine::mesh::{Octree, Settings};
use fidget_engine::vm::VmShape;
use nalgebra::{Scale3, Translation3, Vector3};
use puri::mesh::{Geometry, Normal, Vertex};

/// Append the surfaces of `objects` to `out`, meshed `depth` octree levels
/// deep within the box from `min` to `max`, each vertex in its object's
/// color. None when an object doesn't compile or meshes to a non-finite
/// vertex; an error once `cancellation` is cancelled.
pub fn append(
    out: &mut Geometry,
    objects: &[SceneObject],
    min: Vector3<f32>,
    max: Vector3<f32>,
    depth: u8,
    cancellation: &incremental::Cancellation,
) -> Result<Option<()>, incremental::Error> {
    cancellation.check()?;
    let settings = Settings {
        depth,
        cancel: fidget_engine::render::CancelToken::from_shared_flag(
            cancellation.shared_flag().clone(),
        ),
        world_to_model: Translation3::from((min + max) / 2.0).to_homogeneous()
            * Scale3::from((max - min) / 2.0).to_homogeneous(),
        ..Default::default()
    };
    let result = objects.iter().try_for_each(|object| {
        if cancellation.check().is_err() {
            return None;
        }
        let shape = VmShape::from(object.tree.clone()).try_into().ok()?;
        let octree = Octree::build(&shape, &settings)?;
        if cancellation.check().is_err() {
            return None;
        }
        let mesh = octree.walk_dual();
        if cancellation.check().is_err() {
            return None;
        }
        if !mesh
            .vertices
            .iter()
            .all(|v| v.iter().all(|x| x.is_finite()))
        {
            return None;
        }
        let offset = u32::try_from(out.vertices.len()).ok()?;
        u32::try_from(out.vertices.len().checked_add(mesh.vertices.len())?).ok()?;
        out.vertices
            .extend(mesh.vertices.into_iter().map(|position| Vertex {
                position,
                color: object.color.map(|n| f32::from(n) / 255.0),
                normal: Normal::default(),
            }));
        out.indices.extend(
            mesh.triangles
                .into_iter()
                .flat_map(|t| [t.x, t.y, t.z].map(|i| offset + i as u32)),
        );
        Some(())
    });
    cancellation.check()?;
    Ok(result)
}
