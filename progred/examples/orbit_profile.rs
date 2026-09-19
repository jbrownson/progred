#[cfg(not(target_arch = "wasm32"))]
fn main() {
    use std::time::{Duration, Instant};
    let args: Vec<_> = std::env::args().skip(1).collect();
    let refined = args.iter().any(|arg| arg == "--refined");
    let collapsed = args.iter().any(|arg| arg == "--collapsed");
    let finish = args.iter().any(|arg| arg == "--finish");
    let position: f64 = args
        .windows(2)
        .find(|a| a[0] == "--position")
        .map_or(0.5, |a| a[1].parse().expect("numeric playback position"));
    assert!((0.0..=1.0).contains(&position));
    let setup = Instant::now();
    let mut orbit =
        progred::orbit_profile::OrbitProfile::new(2400, 1800, 2.0, position, refined, collapsed);
    while !orbit.ready() {
        assert!(
            setup.elapsed() < Duration::from_secs(240),
            "initial rendering timed out"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    let setup_ms = setup.elapsed().as_secs_f64() * 1000.0;
    let mut samples = Vec::new();
    let mut next = Instant::now();
    // Pace input like a 60 Hz display; warm eight frames before measuring 80.
    for index in 0..88 {
        std::thread::sleep(next.saturating_duration_since(Instant::now()));
        let start = Instant::now();
        next = start + Duration::from_secs_f64(1.0 / 60.0);
        let mut sample = orbit.frame(&mut puri_vello::compositor::SplitCanvas::default());
        sample["frame_ms"] = (start.elapsed().as_secs_f64() * 1000.0).into();
        if index >= 8 {
            samples.push(sample);
        }
    }
    let release = Instant::now();
    orbit.release();
    let release_pending = !orbit.ready();
    let release_to_ready_ms = finish.then(|| {
        while !orbit.ready() {
            assert!(
                release.elapsed() < Duration::from_secs(240),
                "rendering after release timed out"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        release.elapsed().as_secs_f64() * 1000.0
    });
    println!(
        "{}",
        serde_json::json!({
            "position":position, "refined":refined, "collapsed":collapsed,
            "setup_ms":setup_ms, "samples":samples,
            "release_pending":release_pending, "release_to_ready_ms":release_to_ready_ms,
        })
    );
}

#[cfg(target_arch = "wasm32")]
fn main() {}

#[cfg(target_arch = "wasm32")]
mod web {
    use progred::{orbit_profile::OrbitProfile, web_render::Renderer};
    use std::cell::RefCell;
    use wasm_bindgen::prelude::*;
    thread_local! { static PROFILE: RefCell<Option<(OrbitProfile, Renderer)>> = const { RefCell::new(None) }; }
    #[wasm_bindgen]
    pub async fn start_orbit_profile(
        canvas: web_sys::HtmlCanvasElement,
        scale: f64,
        progress: f64,
        refined: bool,
        collapsed: bool,
    ) -> Result<String, JsValue> {
        progred::web_worker::initialize();
        let renderer = Renderer::new(canvas.clone()).await?;
        let name = renderer.name().to_owned();
        let orbit = OrbitProfile::new(
            canvas.width(),
            canvas.height(),
            scale,
            progress,
            refined,
            collapsed,
        );
        PROFILE.with(|p| *p.borrow_mut() = Some((orbit, renderer)));
        Ok(name)
    }
    #[wasm_bindgen]
    pub fn orbit_ready() -> bool {
        PROFILE.with(|p| p.borrow_mut().as_mut().unwrap().0.ready())
    }
    #[wasm_bindgen]
    pub fn orbit_frame(width: u32, height: u32) -> Result<String, JsValue> {
        PROFILE.with(|p| {
            let mut p = p.borrow_mut();
            let (orbit, renderer) = p.as_mut().unwrap();
            let start = web_time::Instant::now();
            let mut result = serde_json::Value::Null;
            renderer
                .render(width, height, peniko::Color::WHITE, |canvas| {
                    result = orbit.frame(canvas);
                })
                .map_err(|e| JsValue::from_str(&e))?;
            result["submit_ms"] = (start.elapsed().as_secs_f64() * 1000.0).into();
            Ok(result.to_string())
        })
    }
}
