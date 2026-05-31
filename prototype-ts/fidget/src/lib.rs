use fidget::{
    context::Tree,
    mesh::{Octree, Settings},
    vm::VmShape,
};
use serde::Deserialize;
use serde::Serialize;
use wasm_bindgen::prelude::*;

#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "camelCase")]
enum Implicit {
    X,
    Y,
    Z,
    Constant { value: f32 },
    Add { a: Box<Implicit>, b: Box<Implicit> },
    Subtract { a: Box<Implicit>, b: Box<Implicit> },
    Multiply { a: Box<Implicit>, b: Box<Implicit> },
    Divide { a: Box<Implicit>, b: Box<Implicit> },
    Minimum { a: Box<Implicit>, b: Box<Implicit> },
    Maximum { a: Box<Implicit>, b: Box<Implicit> },
}

#[derive(Serialize)]
struct MeshJSON {
    positions: Vec<f32>,
    indices: Vec<u32>,
}

#[wasm_bindgen]
pub fn mesh_implicit_json(json: &str, depth: u8, scale: f32) -> Result<String, JsValue> {
    let implicit: Implicit =
        serde_json::from_str(json).map_err(|err| JsValue::from_str(&err.to_string()))?;
    mesh_json(implicit.tree(), depth, scale)
}

impl Implicit {
    fn tree(self) -> Tree {
        match self {
            Implicit::X => Tree::x(),
            Implicit::Y => Tree::y(),
            Implicit::Z => Tree::z(),
            Implicit::Constant { value } => Tree::from(value),
            Implicit::Add { a, b } => a.tree() + b.tree(),
            Implicit::Subtract { a, b } => a.tree() - b.tree(),
            Implicit::Multiply { a, b } => a.tree() * b.tree(),
            Implicit::Divide { a, b } => a.tree() / b.tree(),
            Implicit::Minimum { a, b } => a.tree().min(b.tree()),
            Implicit::Maximum { a, b } => a.tree().max(b.tree()),
        }
    }
}

fn mesh_json(tree: Tree, depth: u8, scale: f32) -> Result<String, JsValue> {
    if !scale.is_finite() || scale <= 0.0 {
        return Err(JsValue::from_str("Fidget meshing scale must be finite and positive"));
    }
    let shape = VmShape::from(tree);
    let settings = Settings {
        depth,
        world_to_model: nalgebra::Matrix4::new_nonuniform_scaling(&nalgebra::Vector3::new(
            scale, scale, scale,
        )),
        threads: None,
        ..Default::default()
    };
    let mesh = Octree::build(&shape, &settings)
        .ok_or_else(|| JsValue::from_str("Fidget meshing failed"))?
        .walk_dual();

    let positions: Vec<_> = mesh
        .vertices
        .iter()
        .flat_map(|vertex| [vertex.x, vertex.y, vertex.z])
        .collect();
    serde_json::to_string(&MeshJSON {
        positions,
        indices: mesh
            .triangles
            .iter()
            .flat_map(|triangle| [triangle.x as u32, triangle.y as u32, triangle.z as u32])
            .collect(),
    })
    .map_err(|err| JsValue::from_str(&err.to_string()))
}
