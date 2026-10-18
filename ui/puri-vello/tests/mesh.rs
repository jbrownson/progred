//! Headless checks of the mesh renderer and its composition; never launches
//! the app. Run with `--features capture -- --ignored`.
#![cfg(feature = "capture")]

use puri::draw::{Canvas, DrawList};
use puri::mesh;
use puri_vello::capture::{self as gpu, Readback, composite, mesh_images};
use puri_vello::compositor::{Compositor, Resources};
use std::sync::Arc;
use vello::kurbo::{self, Affine, Rect};
use vello::peniko::{self, Color, ImageData};

fn triangle(color: [f32; 3]) -> mesh::Scene {
    mesh::Scene {
        geometry: Arc::new(mesh::Geometry {
            vertices: [[-0.9, -0.8, 0.0], [0.9, -0.8, 0.0], [0.0, 0.9, 0.0]]
                .map(|p| mesh::Vertex {
                    position: nalgebra::Vector3::from(p),
                    color,
                    normal: Default::default(),
                })
                .into(),
            indices: vec![0, 1, 2],
        }),
        view: mesh::View {
            model_to_view: nalgebra::Matrix4::identity(),
            projection: [1.0, 1.0, -0.5, 0.5],
            width: 64,
            height: 64,
        },
        surface: None,
    }
}

#[test]
#[ignore = "headless GPU mesh ordering, clipping, and upload regression"]
fn mesh_layers_match_roundtrip_with_clips_and_reused_target() {
    let gpu = gpu::Gpu::new();
    let mut renderer = puri_vello::mesh::Renderer::new(&gpu.device, &gpu.queue);
    let mut readback = Readback::default();
    let target = gpu.texture(180, 90);
    let mut compositor = Compositor::new(&gpu.device, &gpu.queue).unwrap();
    let mut resources = Resources::default();
    let mut red = triangle([1.0, 0.0, 0.0]);
    for edited in [false, true] {
        if edited {
            for v in &mut Arc::make_mut(&mut red.geometry).vertices {
                v.color = [0.0, 1.0, 0.0];
            }
        }
        let mut list = DrawList::new();
        list.fill(
            Rect::new(0.0, 0.0, 180.0, 90.0),
            Color::WHITE,
            Affine::IDENTITY,
        );
        list.clip(
            Rect::new(8.0, 8.0, 55.0, 65.0),
            Affine::IDENTITY,
            |canvas| {
                canvas.draw_mesh(red.clone(), Affine::translate((5.0, 5.0)));
                canvas.fill(
                    Rect::new(25.0, 30.0, 35.0, 40.0),
                    Color::BLACK,
                    Affine::IDENTITY,
                );
            },
        );
        list.clip(
            kurbo::Circle::new((115.0, 40.0), 26.3),
            Affine::IDENTITY,
            |canvas| {
                canvas.draw_mesh(triangle([0.0, 0.0, 1.0]), Affine::translate((80.0, 8.0)));
            },
        );
        let reference = mesh_images(&gpu, &mut renderer, &mut readback, &list);
        let expected = gpu.read(&composite(
            &gpu,
            &mut compositor,
            &mut resources,
            &target,
            &reference,
        ));
        let actual = gpu.read(&composite(
            &gpu,
            &mut compositor,
            &mut resources,
            &target,
            &list,
        ));
        assert_eq!(
            actual, expected,
            "same-sized scratch targets must composite before reuse"
        );
        assert_eq!(compositor.counts.uploads, 0);
        let pixel = |x: usize, y: usize| &actual[(y * 180 + x) * 4..(y * 180 + x + 1) * 4];
        assert_eq!(
            pixel(30, 35),
            [0, 0, 0, 255],
            "vector overlay paints after mesh"
        );
        assert_eq!(pixel(60, 50), [255; 4], "rectangular clip");
        assert!(pixel(35, 45)[usize::from(edited)] > 180);
        assert!(pixel(112, 45)[2] > 180 && pixel(112, 45)[0] == 0);
        assert_eq!(pixel(85, 10), [255; 4], "rounded clip");
    }
}

#[test]
#[ignore = "headless GPU depth upload identity and premultiplied-alpha contract"]
fn depth_only_changes_refresh_the_surface_upload() {
    let gpu = gpu::Gpu::new();
    let mut renderer = puri_vello::mesh::Renderer::new(&gpu.device, &gpu.queue);
    let mut scene = triangle([1.0, 0.0, 0.0]);
    scene.surface = Some(mesh::Surface {
        frame: mesh::DepthImage {
            image: ImageData {
                data: vec![0, 0, 255, 255].into(),
                width: 1,
                height: 1,
                format: peniko::ImageFormat::Rgba8,
                alpha_type: peniko::ImageAlphaType::Alpha,
            },
            depth: vec![0.8].into(),
            partial: false,
        },
        mesh_start: 3,
    });
    let render = |renderer: &mut puri_vello::mesh::Renderer, scene: &mesh::Scene| {
        gpu.read(
            &renderer
                .render(&scene.geometry, &scene.view, scene.surface.as_ref())
                .unwrap(),
        )
    };
    let center = (32 * 64 + 32) * 4;
    assert!(render(&mut renderer, &scene)[center] > 180);
    // Preserve image identity but change only depth. A retained weak identity
    // must observe Arc::make_mut, just as it does for geometry edits.
    Arc::make_mut(&mut scene.surface.as_mut().unwrap().frame.depth)[0] = 0.2;
    assert_eq!(
        &render(&mut renderer, &scene)[center..center + 4],
        &[0, 0, 255, 255]
    );
    scene.surface.as_mut().unwrap().frame.image.data = vec![200, 100, 50, 128].into();
    assert_eq!(
        &render(&mut renderer, &scene)[center..center + 4],
        &[100, 50, 25, 128]
    );
}
