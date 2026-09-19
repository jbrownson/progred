use super::*;

fn raster(preview: &VolumePreview, state: Option<&Value>) -> raster::Frame {
    raster::Request::new(preview.clone(), state, 1.0)
        .unwrap()
        .render_software_tiles(
            raster::Passes::Final,
            4,
            &Default::default(),
            &mut |_| Ok(()),
            None,
        )
        .unwrap()
        .unwrap()
}

#[test]
fn implicit_depth_matches_mesh_camera_and_pixel_centers() {
    let center = Vector3::new(0.18, -0.12, 0.15);
    let mut preview = sphere_preview();
    preview.objects[0].tree = (Tree::x() - center.x).square()
        + (Tree::y() - center.y).square()
        + (Tree::z() - center.z).square()
        - 0.25;
    for (width, height, yaw, pitch, zoom) in [
        (81, 53, 0.0, 0.0, 1.0),
        (53, 81, 35.0, 25.0, 1.6),
        (67, 45, -65.0, 70.0, 0.8),
    ] {
        preview.size = Size::new(width as f64, height as f64);
        let state = Value::record([(
            vocabulary::CAMERA,
            Value::record([
                (vocabulary::YAW, f32::value(yaw)),
                (vocabulary::PITCH, f32::value(pitch)),
                (vocabulary::ZOOM, f32::value(zoom)),
            ]),
        )]);
        let frame = raster(&preview, Some(&state));
        let view = view(
            &preview,
            camera(Some(&state)),
            PixelRenderSize::new(width, height),
        )
        .unwrap();
        let c = view
            .model_to_view
            .transform_point(&nalgebra::Point3::from(center));
        let [sx, sy, sz, dz] = view.projection;
        let depth_count = super::super::super::volume_view(
            &preview,
            camera(Some(&state)),
            PixelRenderSize::new(width, height),
        )
        .size
        .depth()
            * 4;
        let mut checked = 0;
        for y in 0..height {
            for x in 0..width {
                let px = (2.0 * (x as f32 + 0.5) / width as f32 - 1.0) / sx;
                let py = (1.0 - 2.0 * (y as f32 + 0.5) / height as f32) / sy;
                let r2 = 0.25 - (px - c.x).powi(2) - (py - c.y).powi(2);
                if r2 > 0.01 {
                    let expected = (c.z + r2.sqrt()) * sz + dz;
                    let actual = frame.depth[(y * width + x) as usize];
                    assert!(
                        (actual - expected).abs() <= 1.01 / depth_count as f32,
                        "{width}x{height} ({x},{y}): {actual} != {expected}"
                    );
                    checked += 1;
                }
            }
        }
        assert!(checked > 20);
    }
}

fn quad(geometry: &mut Geometry, x: std::ops::Range<f32>, z: f32, color: [f32; 3]) {
    let offset = geometry.vertices.len() as u32;
    geometry.vertices.extend(
        [
            [x.start, -1.0, z],
            [x.end, -1.0, z],
            [x.end, 1.0, z],
            [x.start, 1.0, z],
        ]
        .map(|p| Vertex {
            position: Vector3::from(p),
            color,
            normal: Normal::default(),
        }),
    );
    geometry
        .indices
        .extend([0, 1, 2, 0, 2, 3].map(|i| i + offset));
}

fn composition_fixture() -> (Geometry, View, raster::Frame, usize) {
    let mut geometry = Geometry::default();
    quad(&mut geometry, -1.0..0.0, -0.2, [1.0, 0.0, 0.0]); // behind implicit plane
    quad(&mut geometry, 0.0..1.0, 0.4, [0.0, 1.0, 0.0]); // in front
    let surface_start = geometry.indices.len();
    quad(&mut geometry, -1.0..1.0, 0.9, [0.0, 0.0, 1.0]); // draft must be replaced
    let view = View {
        model_to_view: Matrix4::identity(),
        projection: [1.0, 1.0, -0.5, 0.5],
        width: 8,
        height: 4,
    };
    let frame = raster::Frame {
        image: ImageData {
            data: [180, 120, 60, 255].repeat(32).into(),
            format: ImageFormat::Rgba8,
            alpha_type: ImageAlphaType::Alpha,
            width: 8,
            height: 4,
        },
        depth: vec![0.5; 32].into(),
        partial: false,
    };
    (geometry, view, frame, surface_start)
}

fn check_composition(mut render: impl FnMut(&Mesh, &View, Surface) -> Vec<u8>) {
    let (geometry, view, mut frame, mesh_start) = composition_fixture();
    let geometry = Mesh::new(geometry);
    let plain = render(
        &Mesh::default(),
        &view,
        Surface {
            frame: frame.clone(),
            mesh_start: 0,
        },
    );
    assert_eq!(plain, frame.image.data.data());
    let pixel =
        |bytes: &[u8], x: usize| <[u8; 4]>::try_from(&bytes[(8 + x) * 4..(8 + x + 1) * 4]).unwrap();
    let image = render(
        &geometry,
        &view,
        Surface {
            frame: frame.clone(),
            mesh_start,
        },
    );
    assert_eq!(
        pixel(&image, 1),
        [180, 120, 60, 255],
        "behind mesh is hidden; draft replaced"
    );
    let front = pixel(&image, 6);
    assert!(
        front[1] > 180 && front[0] == 0 && front[2] == 0,
        "front mesh visible"
    );

    frame.partial = true;
    let depths = std::sync::Arc::make_mut(&mut frame.depth);
    depths[9] = -1.0; // unfinished: retain draft
    depths[10] = 1.0; // finished empty: erase draft, reveal rear mesh
    let mut colors = frame.image.data.data().to_vec();
    colors[40..44].fill(0);
    frame.image.data = colors.into();
    let image = render(
        &geometry,
        &view,
        Surface {
            frame: frame.clone(),
            mesh_start,
        },
    );
    let unknown = pixel(&image, 1);
    assert!(unknown[2] > 180 && unknown[0] == 0);
    let cleared = pixel(&image, 2);
    assert!(
        cleared[0] > 180 && cleared[2] == 0,
        "completed empty pixel reveals mesh behind"
    );

    // A different raster resolution uses nearest samples, never interpolates depth.
    let small = raster::Frame {
        image: ImageData {
            data: [180, 120, 60, 255].repeat(8).into(),
            width: 4,
            height: 2,
            ..frame.image.clone()
        },
        depth: vec![0.5; 8].into(),
        partial: false,
    };
    let image = render(
        &geometry,
        &view,
        Surface {
            frame: small,
            mesh_start,
        },
    );
    assert_eq!(pixel(&image, 1), [180, 120, 60, 255]);
    assert_eq!(pixel(&image, 6), front);
}

#[test]
fn cpu_composes_mesh_and_partial_raster_with_one_depth_buffer() {
    check_composition(|g, v, s| cpu::render_surface(g, v, Some(s)).unwrap());
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
#[ignore = "headless GPU depth/color composition; no application launch"]
fn gpu_composes_mesh_and_partial_raster_with_one_depth_buffer() {
    let gpu = pollster::block_on(fidget_engine::wgpu::Gpu::init_basic()).unwrap();
    let mut renderer = gpu::Renderer::new(gpu);
    check_composition(|g, v, s| renderer.render(g, v, Some(s)).unwrap());
}
