use super::*;
use nalgebra::Vector3;

#[test]
fn upload_reuse_requires_the_same_unchanged_mesh_and_an_uploaded_prefix() {
    let mut geometry = Mesh::default();
    let uploaded = UploadedMesh {
        source: Arc::downgrade(&geometry),
        index_count: 6,
    };
    assert!(uploaded.covers(&geometry.clone(), 6));
    assert!(uploaded.covers(&geometry, 3));
    assert!(!uploaded.covers(&geometry, 9));
    assert!(!uploaded.covers(&Mesh::default(), 6));
    assert!(Arc::get_mut(&mut geometry).is_none());
    Arc::make_mut(&mut geometry).indices.push(0);
    assert!(!uploaded.covers(&geometry, 1));
    assert!(uploaded.source.upgrade().is_none());

    let uploaded = UploadedMesh {
        source: Arc::downgrade(&geometry),
        index_count: 1,
    };
    drop(geometry);
    assert!(
        uploaded.source.upgrade().is_none(),
        "uploads don't retain CPU geometry"
    );
    assert!(!uploaded.covers(&Mesh::default(), 0));
}

fn iterator_vertices(vertices: &[Vertex]) -> Vec<u8> {
    vertices
        .iter()
        .flat_map(|v| {
            v.position
                .iter()
                .copied()
                .chain(v.color)
                .flat_map(f32::to_ne_bytes)
                .chain(v.normal.0.map(|n| n as u8))
        })
        .collect()
}

#[test]
fn packed_mesh_data_preserves_shader_layout_and_all_bits() {
    let vertices = [
        Vertex {
            position: Vector3::new(1.0, -2.0, 3.0),
            color: [0.25, 0.5, 1.0],
            normal: puri::mesh::Normal::new(-Vector3::z()),
        },
        Vertex {
            position: Vector3::new(-0.0, f32::INFINITY, f32::from_bits(0x7fc01234)),
            color: [1.0, 0.0, 0.0],
            normal: puri::mesh::Normal::default(),
        },
    ];
    let bytes = vertex_bytes(&vertices);
    assert_eq!(bytes.len(), vertices.len() * 28);
    assert_eq!(bytes, iterator_vertices(&vertices));
    assert_eq!(&bytes[..4], &1.0_f32.to_ne_bytes());
    assert_eq!(&bytes[12..16], &0.25_f32.to_ne_bytes());
    assert_eq!(&bytes[24..28], &[0, 0, 129, 0]);
    assert!(vertex_bytes(&[]).is_empty());
}
