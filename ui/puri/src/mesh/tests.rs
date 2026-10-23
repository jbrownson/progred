use super::*;
use crate::draw::{CanvasSink, DrawCmd, DrawList};
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
    list.draw_mesh(scene, inner);
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
fn invalid_geometry_declines_and_a_malformed_surface_is_left_out() {
    let mut broken = scene();
    std::sync::Arc::make_mut(&mut broken.geometry).indices[0] = 100;
    assert!(broken.rasterize().is_none());

    let plain = scene().rasterize().unwrap();
    let surface = Surface {
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
        mesh_start: 0,
    };
    let unsized_depth = DepthImage {
        depth: vec![].into(),
        ..surface.frame.clone()
    };
    for malformed in [
        Surface {
            mesh_start: 6,
            ..surface.clone()
        },
        Surface {
            mesh_start: 1,
            ..surface.clone()
        },
        Surface {
            frame: unsized_depth,
            ..surface.clone()
        },
    ] {
        let mut scene = scene();
        scene.surface = Some(malformed);
        assert_eq!(scene.rasterize().unwrap().data.data(), plain.data.data());
    }
    let mut scene = scene();
    scene.surface = Some(surface);
    assert!(scene.rasterize().is_some());
}
