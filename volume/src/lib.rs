//! Fidget shapes as volumes for display, meshed into triangles off the editor
//! thread. Nothing here knows the editor. It builds optimized like the other
//! dependencies, so previews and their tests don't run Fidget's generic
//! meshing at the editor's debug level.

pub mod mesh;

use fidget_engine::context::Tree;

/// One shape of a scene, in the color it shows in.
#[derive(Clone, PartialEq)]
pub struct SceneObject {
    pub tree: Tree,
    pub color: [u8; 3],
}
