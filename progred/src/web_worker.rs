//! Browser transport for the ordinary `Send` computation jobs.
//!
//! The host starts a coordinator and its Rayon workers with this module and
//! shared linear memory before the editor. No editor state crosses threads.
use incremental::background::{Executor, Job};
use std::sync::OnceLock;
use wasm_bindgen::prelude::*;
pub use wasm_bindgen_rayon::init_thread_pool;

static CHANNEL: OnceLock<String> = OnceLock::new();

#[wasm_bindgen]
pub fn set_worker_channel(name: String) {
    CHANNEL.set(name).expect("worker channel initialized once");
}

#[wasm_bindgen]
pub fn worker_threads() -> usize {
    fidget_engine::render::ThreadPool::Global.thread_count()
}

pub fn initialize() {
    console_error_panic_hook::set_once();
}

#[wasm_bindgen(raw_module = "../worker-host.js")]
extern "C" {
    #[wasm_bindgen(js_name = submitJob)]
    fn submit_job(pointer: usize, channel: &str);
    #[wasm_bindgen(js_name = workerWake)]
    fn worker_wake(channel: &str);
}

pub fn wake() {
    worker_wake(CHANNEL.get().expect("worker channel initialized"));
}

pub fn executor() -> Executor {
    // A second box gives the trait object a thin pointer for postMessage.
    Executor::new(|job| {
        submit_job(
            Box::into_raw(Box::new(job)) as usize,
            CHANNEL.get().expect("worker channel initialized"),
        )
    })
}

/// # Safety
/// The host must deliver each submitted pointer exactly once to an instance
/// of this same WASM module sharing its memory. Never replay a job pointer.
#[wasm_bindgen]
pub unsafe fn run_worker_job(pointer: usize) {
    let job = unsafe { Box::from_raw(pointer as *mut Job) };
    job();
}

#[wasm_bindgen]
pub fn worker_memory() -> JsValue {
    wasm_bindgen::memory()
}

#[wasm_bindgen]
pub fn worker_module() -> JsValue {
    wasm_bindgen::module()
}
