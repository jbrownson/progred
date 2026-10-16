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
