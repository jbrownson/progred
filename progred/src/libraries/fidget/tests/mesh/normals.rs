use super::*;

fn fixture() -> (Geometry, View) {
    let geometry = Geometry {
        vertices: [
            ([-0.9, -0.9, 0.0], [-0.8, 0.0, 0.6]),
            ([0.9, -0.9, 0.0], [0.8, 0.0, 0.6]),
            ([0.0, 0.9, 0.0], [0.0, 0.0, 1.0]),
        ]
        .map(|(p, n)| Vertex {
            position: Vector3::from(p),
            color: [1.0; 3],
            normal: Normal::new(Vector3::from(n)),
        })
        .into(),
        indices: vec![0, 1, 2],
    };
    let view = View {
        model_to_view: Rotation3::from_axis_angle(&Vector3::z_axis(), 0.3).to_homogeneous(),
        projection: [1.0, 1.0, -0.5, 0.5],
        width: 80,
        height: 80,
    };
    (geometry, view)
}

#[test]
fn normals_remain_attached_when_geometry_is_recolored() {
    let (geometry, _) = fixture();
    let mut colored = Geometry::default();
    colored.append_colored(&geometry, |_| [0.5; 3]).unwrap();
    for (a, b) in colored.vertices.iter().zip(&geometry.vertices) {
        assert_eq!(a.normal, b.normal);
        assert_eq!(a.position, b.position);
        assert_eq!(a.color, [0.5; 3]);
        assert!((a.normal.vector().norm() - 1.0).abs() < 0.01);
    }
}

fn check_shading(geometry: &Geometry, view: &View, rgba: &[u8]) {
    let light = Vector3::new(0.35, -0.45, 1.0).normalize();
    for (x, y) in [(25, 55), (45, 55), (40, 35)] {
        let p = view
            .model_to_view
            .try_inverse()
            .unwrap()
            .transform_point(&nalgebra::Point3::new(
                2.0 * (x as f32 + 0.5) / view.width as f32 - 1.0,
                1.0 - 2.0 * (y as f32 + 0.5) / view.height as f32,
                0.0,
            ));
        let wc = (p.y + 0.9) / 1.8;
        let wb = ((1.0 - wc) + p.x / 0.9) / 2.0;
        let wa = 1.0 - wb - wc;
        assert!([wa, wb, wc].iter().all(|w| *w > 0.05));
        let n = geometry.vertices[0].normal.vector() * wa
            + geometry.vertices[1].normal.vector() * wb
            + geometry.vertices[2].normal.vector() * wc;
        let n = view.model_to_view.transform_vector(&n).normalize();
        let expected = ((0.22 + 0.78 * n.dot(&light).max(0.0)) * 255.0).round() as u8;
        let pixel = &rgba[(y * view.width as usize + x) * 4..][..4];
        assert_eq!(pixel[3], 255);
        assert!(pixel[0].abs_diff(expected) <= 1, "{pixel:?} != {expected}");
    }
}

#[test]
fn cpu_interpolates_normals_then_computes_lighting() {
    let (geometry, view) = fixture();
    check_shading(&geometry, &view, &cpu::render(&geometry, &view).unwrap());
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
#[ignore = "requires a GPU; does not use the CPU fallback"]
fn gpu_interpolates_normals_then_computes_lighting() {
    let (geometry, view) = fixture();
    let geometry = Mesh::new(geometry);
    let mut renderer = gpu::Renderer::new(pollster::block_on(Gpu::init_basic()).unwrap());
    check_shading(
        &geometry,
        &view,
        &renderer.render(&geometry, &view, None).unwrap(),
    );
}
