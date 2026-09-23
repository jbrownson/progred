use super::*;

mod hybrid;
mod normals;

fn sphere_preview() -> VolumePreview {
    VolumePreview {
        objects: vec![SceneObject {
            tree: Tree::x().square() + Tree::y().square() + Tree::z().square() - 0.25,
            color: [64, 180, 240],
        }],
        size: Size::new(100.0, 100.0),
        min: Vector3::repeat(-1.0),
        max: Vector3::repeat(1.0),
    }
}

#[test]
fn depth_is_bounded_and_defaults_to_six() {
    assert_eq!(depth(None), Some(6));
    for n in 1..=8 {
        assert_eq!(depth(Some(&crate::libraries::u64::value(n))), Some(n as u8));
    }
    for n in [0, 9, u64::MAX] {
        assert_eq!(depth(Some(&crate::libraries::u64::value(n))), None);
    }
    assert_eq!(depth(Some(&f32::value(6.0))), None);
}

#[test]
fn cancellation_does_not_publish_a_partial_mesh() {
    let cancel = incremental::Cancellation::default();
    cancel.cancel();
    let mut geometry = Geometry::default();
    assert_eq!(
        Shape::from(&sphere_preview()).append_cancellable(&mut geometry, 4, &cancel),
        Err(incremental::Error::Cancelled)
    );
    assert!(geometry.vertices.is_empty());
}

#[test]
fn mesh_function_returns_an_ordinary_declaration() {
    let stack = crate::stack::load();
    let doc = gid::Document {
        root: None,
        cells: Cells::new(),
    };
    let sources = crate::sources::Sources {
        doc: &doc,
        libraries: &stack.libraries,
    };
    let call = |arguments| {
        ::grap::evaluate_value(
            &::grap::call(vocabulary::PREVIEW_MESH.into(), arguments),
            &sources,
            10_000,
        )
        .result
    };
    let result = call(vec![(presentation::vocabulary::VALUE, f32::value(1.0))]);
    let fields = result
        .as_record()
        .unwrap()
        .get(&vocabulary::PREVIEW_MESH)
        .unwrap()
        .as_record()
        .unwrap();
    assert_eq!(depth(fields.get(&vocabulary::MESH_DEPTH)), Some(6));
    assert_eq!(volume_preview_fields(fields).unwrap().objects.len(), 1);
    let invalid = call(vec![
        (presentation::vocabulary::VALUE, f32::value(1.0)),
        (vocabulary::MESH_DEPTH, crate::libraries::u64::value(0)),
    ]);
    assert_eq!(invalid, absent::with_reason(vocabulary::INVALID_MESH_DEPTH));
}

#[test]
fn meshing_handles_empty_and_constant_fields() {
    let mut preview = sphere_preview();
    preview.objects.clear();
    assert!(generate(&preview, 3).unwrap().indices.is_empty());
    for constant in [0.0, 1.0, -1.0] {
        preview.objects = vec![SceneObject {
            tree: Tree::constant(constant),
            color: [255; 3],
        }];
        assert!(generate(&preview, 3).unwrap().indices.is_empty());
    }
}

#[test]
fn model_changes_and_colors_are_used_on_every_generation() {
    let mut preview = sphere_preview();
    let first = generate(&preview, 4).unwrap();
    assert!(!first.indices.is_empty());
    assert!(
        first
            .indices
            .iter()
            .all(|i| (*i as usize) < first.vertices.len())
    );
    preview.objects[0].tree = Tree::x().square() + Tree::y().square() + Tree::z().square() - 0.0625;
    preview.objects[0].color = [200, 30, 20];
    let second = generate(&preview, 4).unwrap();
    let radius = |geometry: &Geometry| {
        geometry
            .vertices
            .iter()
            .map(|v| v.position.norm())
            .fold(0.0, f32::max)
    };
    assert!(radius(&second) < radius(&first) * 0.6);
    assert_eq!(
        second.vertices[0].color,
        [200.0 / 255.0, 30.0 / 255.0, 20.0 / 255.0]
    );
}

#[test]
fn camera_preserves_aspect_and_changes_pixels_without_affecting_model_space() {
    let preview = sphere_preview();
    let geometry = generate(&preview, 4).unwrap();
    let camera = Camera::default();
    let tall = view(&preview, camera, PixelRenderSize::new(80, 160)).unwrap();
    assert!((tall.projection[0] * 80.0 - tall.projection[1] * 160.0).abs() < 0.001);
    let normal = cpu::render(&geometry, &tall).unwrap();
    let zoomed = cpu::render(
        &geometry,
        &view(
            &preview,
            Camera {
                zoom: camera.zoom * 2.0,
                ..camera
            },
            PixelRenderSize::new(80, 160),
        )
        .unwrap(),
    )
    .unwrap();
    let painted = |image: &[u8]| image.chunks_exact(4).filter(|p| p[3] != 0).count();
    assert!(painted(&normal) > 0);
    assert!(painted(&zoomed) > painted(&normal) * 2);
}

#[test]
fn changing_pane_width_preserves_mesh_and_implicit_camera_scale() {
    let preview = sphere_preview();
    let height = 160;
    for zoom in [0.5, 1.0, 4.0] {
        let camera = Camera {
            zoom,
            ..Camera::default()
        };
        let square = PixelRenderSize::from(height);
        let mesh = view(&preview, camera, square).unwrap();
        let implicit = volume_view(&preview, camera, square);
        let screen_to_model = implicit.world_to_model * implicit.size.screen_to_world();
        // Cross the square aspect ratio: neither narrowing nor widening a pane
        // should change pixel spacing or the depth volume, only its X extent.
        for width in [80, 160, 320] {
            let pixels = PixelRenderSize::new(width, height);
            let resized_mesh = view(&preview, camera, pixels).unwrap();
            assert!(
                (resized_mesh.projection[0] * width as f32 - mesh.projection[0] * height as f32)
                    .abs()
                    < 0.001
            );
            assert_eq!(resized_mesh.projection[1], mesh.projection[1]);
            let resized_implicit = volume_view(&preview, camera, pixels);
            assert_eq!(resized_implicit.size.depth(), implicit.size.depth());
            let resized_screen_to_model =
                resized_implicit.world_to_model * resized_implicit.size.screen_to_world();
            for axis in 0..3 {
                assert!(
                    (resized_screen_to_model.fixed_view::<3, 1>(0, axis)
                        - screen_to_model.fixed_view::<3, 1>(0, axis))
                    .norm()
                        < 0.0001
                );
            }
        }
    }
}

#[test]
fn depth_buffer_keeps_the_front_object_independent_of_order() {
    let plane = |z, color| Geometry {
        vertices: [[-0.8, -0.8, z], [0.8, -0.8, z], [0.0, 0.8, z]]
            .map(|p| Vertex {
                position: Vector3::from(p),
                color,
                normal: Normal::default(),
            })
            .into(),
        indices: vec![0, 1, 2],
    };
    let near = plane(0.2, [1.0, 0.0, 0.0]);
    let far = plane(-0.2, [0.0, 0.0, 1.0]);
    let view = View {
        model_to_view: Matrix4::identity(),
        projection: [1.0, 1.0, -0.5, 0.5],
        width: 31,
        height: 23,
    };
    let combine = |a: &Geometry, b: &Geometry| Geometry {
        vertices: a.vertices.iter().chain(&b.vertices).copied().collect(),
        indices: a
            .indices
            .iter()
            .copied()
            .chain(b.indices.iter().map(|i| i + 3))
            .collect(),
    };
    let first = cpu::render(&combine(&near, &far), &view).unwrap();
    let second = cpu::render(&combine(&far, &near), &view).unwrap();
    assert_eq!(first, second);
    assert!(first.chunks_exact(4).any(|p| p[0] > 0));
    assert!(first.chunks_exact(4).all(|p| p[2] == 0));
}

#[test]
fn mesh_and_implicit_views_agree_on_framing_and_flat_surface_lighting() {
    let mut preview = sphere_preview();
    preview.objects[0].tree = (Tree::z() - 0.1)
        .max(-Tree::z() - 0.7)
        .max(Tree::x().abs() - 0.75)
        .max(Tree::y().abs() - 0.75);
    let geometry = generate(&preview, 4).unwrap();
    for (width, height) in [(96, 64), (64, 96)] {
        for camera in [
            Camera {
                yaw: 0.0,
                pitch: 0.0,
                zoom: 1.0,
            },
            Camera {
                yaw: 35.0,
                pitch: 40.0,
                zoom: 2.0,
            },
        ] {
            let pixels = PixelRenderSize::new(width, height);
            let mesh_view = view(&preview, camera, pixels).unwrap();
            let voxel_view = volume_view(&preview, camera, pixels);
            let to_model = voxel_view.world_to_model * voxel_view.size.screen_to_world();
            for (x, y) in [(0.0, 0.0), (width as f32 * 0.5, height as f32 * 0.5)] {
                let model = to_model.transform_point(&nalgebra::Point3::new(x, y, 1.0));
                let view = mesh_view.model_to_view.transform_point(&model);
                let screen_x = (view.x * mesh_view.projection[0] + 1.0) * width as f32 / 2.0;
                let screen_y = (1.0 - view.y * mesh_view.projection[1]) * height as f32 / 2.0;
                assert!((screen_x - (x + 0.5)).abs() < 0.001);
                assert!((screen_y - (y + 0.5)).abs() < 0.001);
            }
            let mesh = cpu::render(&geometry, &mesh_view).unwrap();
            let implicit = cpu_volume(
                &preview.objects,
                &voxel_view,
                &incremental::Cancellation::default(),
            )
            .unwrap();
            let index = (height / 2 * width + width / 2) as usize * 4;
            assert_eq!(mesh[index + 3], 255);
            assert_eq!(implicit[index + 3], 255);
            for channel in 0..3 {
                assert!(
                    mesh[index + channel].abs_diff(implicit[index + channel]) <= 1,
                    "mesh {:?}, implicit {:?}",
                    &mesh[index..index + 4],
                    &implicit[index..index + 4]
                );
            }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
#[ignore = "requires a GPU; does not use the CPU fallback"]
fn mesh_gpu_renders_and_reuses_buffers() {
    let mut renderer = gpu::Renderer::new(pollster::block_on(Gpu::init_basic()).unwrap());
    let preview = sphere_preview();
    let geometry = generate(&preview, 4).unwrap();
    for (width, height) in [(73, 91), (73, 91), (101, 59)] {
        let view = view(
            &preview,
            Camera::default(),
            PixelRenderSize::new(width, height),
        )
        .unwrap();
        let image = renderer.render(&geometry, &view, None).unwrap();
        assert_eq!(image.len(), (width * height * 4) as usize);
        assert!(image.chunks_exact(4).any(|p| p[3] == 255 && p[2] > p[0]));
        assert!(image.chunks_exact(4).any(|p| p[3] == 0));
        let empty = renderer.render(&Mesh::default(), &view, None).unwrap();
        assert!(empty.iter().all(|v| *v == 0));
    }
}
