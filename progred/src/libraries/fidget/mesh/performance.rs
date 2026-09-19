//! Headless presentation cost with retained geometry; no editor is launched.
use super::*;
use serde_json::{Value as Json, json};
use web_time::Instant;

#[cfg(target_arch = "wasm32")]
static PROFILE_MESH: std::sync::Mutex<Option<(Mesh, Vec<puri::mesh::View>)>> =
    std::sync::Mutex::new(None);

/// Move the diagnostic geometry from the compute worker to the browser's GPU
/// benchmark, through shared memory, without encoding it as JavaScript data.
#[cfg(target_arch = "wasm32")]
pub fn take_profile_mesh() -> Option<(Mesh, Vec<puri::mesh::View>)> {
    PROFILE_MESH.lock().unwrap().take()
}

pub(crate) fn draw(preview: &VolumePreview, geometry: Mesh) -> Json {
    let pixels = raster_size(preview.size, 1.0).unwrap();
    let views: Vec<_> = (0..8)
        .map(|i| {
            view(
                preview,
                Camera {
                    yaw: 30.0 + i as f32,
                    ..Camera::default()
                },
                pixels,
            )
            .unwrap()
        })
        .collect();
    let mut cpu_ms = Vec::new();
    for view in &views {
        let scene = puri::mesh::Scene {
            geometry: geometry.clone(),
            view: view.clone(),
            surface: None,
        };
        let start = Instant::now();
        let image = scene.rasterize().unwrap();
        cpu_ms.push(start.elapsed().as_secs_f64() * 1000.0);
        std::hint::black_box(image);
    }
    #[cfg(target_arch = "wasm32")]
    {
        *PROFILE_MESH.lock().unwrap() = Some((geometry.clone(), views.clone()));
    }
    let gpu_ms: Vec<f64> = {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let gpu = pollster::block_on(fidget_engine::wgpu::Gpu::init_basic()).unwrap();
            let mut renderer = puri_vello::mesh::Renderer::new(&gpu.device, &gpu.queue);
            views
                .iter()
                .map(|view| {
                    let start = Instant::now();
                    let texture = renderer.render(&geometry, view, None).unwrap();
                    gpu.device
                        .poll(vello::wgpu::PollType::wait_indefinitely())
                        .unwrap();
                    std::hint::black_box(texture);
                    start.elapsed().as_secs_f64() * 1000.0
                })
                .collect()
        }
        #[cfg(target_arch = "wasm32")]
        {
            Vec::new()
        }
    };
    json!({"cpu_draw_ms":cpu_ms, "gpu_draw_ms":gpu_ms, "triangles":geometry.indices.len()/3})
}
