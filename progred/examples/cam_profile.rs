//! Same headless CAM computation on native and the browser worker.
#[cfg(not(target_arch = "wasm32"))]
fn main() {
    let args: Vec<_> = std::env::args().collect();
    let progress = args.get(1).map_or(0.5, |s| s.parse().unwrap());
    let size = args.get(2).map_or(512, |s| s.parse().unwrap());
    let trials = args.get(3).map_or(4, |s| s.parse().unwrap());
    let controls = args.get(4).is_some_and(|s| s == "controls");
    for trial in 0..trials {
        println!(
            "{}",
            serde_json::json!({"trial":trial, "result":serde_json::from_str::<serde_json::Value>(
            &progred::profile_cam(progress, size, size, controls)).unwrap()})
        );
    }
}

#[cfg(target_arch = "wasm32")]
fn main() {}

#[cfg(target_arch = "wasm32")]
mod web {
    use incremental::{
        Runtime,
        background::{Availability, Tasks},
    };
    use std::cell::RefCell;
    use wasm_bindgen::prelude::*;
    thread_local! { static POLL: RefCell<Option<Box<dyn Fn() -> Option<String>>>> = const { RefCell::new(None) }; }
    #[wasm_bindgen]
    pub fn start_profile(progress: f64, size: u32, controls: bool) {
        start_profile_rect(progress, size, size, controls);
    }
    #[wasm_bindgen]
    pub fn start_profile_rect(progress: f64, width: u32, height: u32, controls: bool) {
        progred::web_worker::initialize();
        let runtime = Runtime::default();
        let tasks = Tasks::new(
            &runtime,
            progred::web_worker::executor(),
            progred::web_worker::wake,
        );
        let input = runtime.memo(move |_| Ok((progress, width, height, controls)));
        let result = tasks.memo(input, |(progress, width, height, controls), _| {
            Ok(progred::profile_cam(progress, width, height, controls))
        });
        let result = runtime.memo(move |read| {
            Ok(match &*result.read(read)? {
                Availability::Ready(result) => Some((**result).clone()),
                _ => None,
            })
        });
        assert!(runtime.read(&result).unwrap().is_none());
        POLL.with(|slot| {
            *slot.borrow_mut() = Some(Box::new(move || {
                tasks.poll();
                (*runtime.read(&result).unwrap()).clone()
            }))
        });
    }
    #[wasm_bindgen]
    pub fn poll_profile() -> Option<String> {
        POLL.with(|slot| slot.borrow().as_ref().unwrap()())
    }

    #[wasm_bindgen]
    pub async fn draw_profile_gpu() -> Result<String, JsValue> {
        let (mesh, views) = progred::take_profile_mesh().ok_or("no profile geometry")?;
        let mut context = vello::util::RenderContext::new();
        let id = context.device(None).await.ok_or("WebGPU unavailable")?;
        let gpu = &context.devices[id];
        let mut renderer = puri_vello::mesh::Renderer::new(&gpu.device, &gpu.queue);
        let mut times = Vec::new();
        for view in &views {
            let start = web_time::Instant::now();
            let texture = renderer
                .render(&mesh, view, None)
                .ok_or("mesh render failed")?;
            let (send, receive) = futures_intrusive::channel::shared::oneshot_channel();
            gpu.queue.on_submitted_work_done(move || {
                let _ = send.send(());
            });
            receive.receive().await.ok_or("GPU completion lost")?;
            times.push(start.elapsed().as_secs_f64() * 1000.0);
            std::hint::black_box(texture);
        }
        Ok(serde_json::json!({"gpu_draw_ms": times,
            "adapter": format!("{:?}", gpu.adapter().get_info()),
            "triangles": mesh.indices.len()/3})
        .to_string())
    }
}
