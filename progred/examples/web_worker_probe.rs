#[cfg(target_arch = "wasm32")]
mod probe {
    use fidget_engine::render::CancelToken;
    use incremental::background::{Availability, Tasks};
    use incremental::{Cancellation, Runtime};
    use progred::web_worker;
    use std::cell::RefCell;
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    use wasm_bindgen::prelude::*;

    thread_local! {
        static CANCEL: RefCell<Option<Cancellation>> = const { RefCell::new(None) };
        static POLL: RefCell<Option<Box<dyn Fn() -> bool>>> = const { RefCell::new(None) };
        static ADVANCE: RefCell<Option<Box<dyn Fn(u32)>>> = const { RefCell::new(None) };
        static STARTED: RefCell<Arc<AtomicBool>> = RefCell::new(Arc::new(AtomicBool::new(false)));
        static SUMMARY: RefCell<String> = const { RefCell::new(String::new()) };
        static PROGRESS_SEEN: RefCell<bool> = const { RefCell::new(false) };
    }

    #[wasm_bindgen]
    pub fn probe_summary() -> String {
        SUMMARY.with(|slot| slot.borrow().clone())
    }

    #[wasm_bindgen]
    pub fn probe_progress_seen() -> bool {
        PROGRESS_SEEN.with(|slot| *slot.borrow())
    }

    #[wasm_bindgen]
    pub fn probe_started() -> bool {
        STARTED.with(|slot| slot.borrow().load(Ordering::Relaxed))
    }

    #[wasm_bindgen]
    pub fn start_probe() {
        console_error_panic_hook::set_once();
        let runtime = Runtime::default();
        let tasks = Tasks::new(&runtime, web_worker::executor(), web_worker::wake);
        let input = runtime.memo(|_| Ok(7_u32));
        let cancel = Cancellation::default();
        CANCEL.with(|slot| *slot.borrow_mut() = Some(cancel.clone()));
        let started = STARTED.with(|slot| slot.borrow().clone());
        let result = tasks.memo(input, move |value, _| {
            let fidget_cancel = CancelToken::from_shared_flag(cancel.shared_flag().clone());
            started.store(true, Ordering::Relaxed);
            web_worker::wake();
            while !fidget_cancel.is_cancelled() {
                std::hint::spin_loop();
            }
            assert_eq!(cancel.check(), Err(incremental::Error::Cancelled));
            Ok(value * 6)
        });
        let result = runtime.memo(move |read| {
            Ok(matches!(&*result.read(read)?, Availability::Ready(value) if **value == 42))
        });
        assert!(!*runtime.read(&result).unwrap());
        POLL.with(|slot| {
            *slot.borrow_mut() = Some(Box::new(move || {
                tasks.poll();
                *runtime.read(&result).unwrap()
            }))
        });
    }

    #[wasm_bindgen]
    pub fn cancel_probe() {
        CANCEL.with(|slot| slot.borrow().as_ref().unwrap().cancel());
    }

    #[wasm_bindgen]
    pub fn poll_probe() -> bool {
        POLL.with(|slot| slot.borrow().as_ref().unwrap()())
    }

    #[wasm_bindgen]
    pub fn start_replacement_probe() {
        let runtime = Runtime::default();
        let tasks = Tasks::new(&runtime, web_worker::executor(), || {});
        let input = runtime.input(0_u32);
        let prepared = runtime.memo({
            let input = input.clone();
            move |read| Ok(*input.read(read))
        });
        let result = tasks.memo(prepared, move |value, cancel| {
            let fidget_cancel = CancelToken::from_shared_flag(cancel.shared_flag().clone());
            let until = web_time::Instant::now() + std::time::Duration::from_micros(500);
            while web_time::Instant::now() < until {
                if fidget_cancel.is_cancelled() {
                    return Err(incremental::Error::Cancelled);
                }
            }
            Ok(value)
        });
        let result = runtime.memo(move |read| {
            Ok(matches!(&*result.read(read)?, Availability::Ready(value) if **value == 50_000))
        });
        ADVANCE.with(|slot| {
            *slot.borrow_mut() = Some(Box::new({
                let runtime = runtime.clone();
                let result = result.clone();
                move |value| {
                    input.set(value);
                    runtime.read(&result).unwrap();
                }
            }))
        });
        POLL.with(|slot| {
            *slot.borrow_mut() = Some(Box::new(move || {
                tasks.poll();
                *runtime.read(&result).unwrap()
            }))
        });
    }

    #[wasm_bindgen]
    pub fn replace_probe(value: u32) {
        ADVANCE.with(|slot| slot.borrow().as_ref().unwrap()(value));
    }

    // Real Fidget work, using the same VM mesher/rasterizer as the browser CAM
    // recipes. This deliberately starts no editor or window.
    #[wasm_bindgen]
    pub fn start_fidget_probe() {
        use fidget_engine::raster::voxel::{EvalConfig, RenderConfig, Scene};
        use fidget_engine::{
            context::Tree,
            mesh::{Octree, Settings},
            vm::VmShape,
        };
        let runtime = Runtime::default();
        let tasks = Tasks::new(&runtime, web_worker::executor(), web_worker::wake);
        let input = runtime.input(36_u32);
        let prepare = runtime.memo({
            let input = input.clone();
            move |read| Ok(*input.read(read))
        });
        let started = STARTED.with(|slot| slot.borrow().clone());
        let node = tasks.memo_reporting(prepare, move |cuts, cancel, publish, progress| {
            started.store(true, Ordering::Relaxed);
            web_worker::wake();
            let token = CancelToken::from_shared_flag(cancel.shared_flag().clone());
            let mut stock = Tree::x().abs().max(Tree::y().abs()).max(Tree::z().abs()) - 0.7;
            for i in 0..cuts {
                cancel.check()?;
                let x = (i % 6) as f32 * 0.22 - 0.55;
                let y = (i / 6) as f32 * 0.22 - 0.55;
                let ball = (Tree::x() - x).square()
                    + (Tree::y() - y).square()
                    + (Tree::z() - 0.68).square()
                    - 0.01;
                stock = stock.max(-ball);
            }
            let shape = VmShape::from(stock).try_into().unwrap();
            let octree = Octree::build(
                &shape,
                &Settings {
                    depth: 7,
                    cancel: token.clone(),
                    ..Default::default()
                },
            );
            cancel.check()?;
            let mesh = octree.unwrap().walk_dual();
            publish((cuts, mesh.vertices.len(), 0))?;
            let scene = Scene::new(vec![shape], &token).unwrap();
            let config =
                RenderConfig::from_size(fidget_engine::render::VoxelSize::new(320, 240, 960));
            let image = scene.render(
                &config,
                &EvalConfig {
                    cancel: token,
                    progress: Some(&|completed, total| {
                        progress(incremental::background::Progress { completed, total })
                    }),
                    ..Default::default()
                },
                None,
            );
            cancel.check()?;
            let image = image.unwrap();
            let occupied = image.iter().filter(|p| p.geometry.depth != 0).count();
            assert!(!mesh.vertices.is_empty() && occupied > 0);
            Ok((cuts, mesh.vertices.len(), occupied))
        });
        let reports = runtime.memo({
            let node = node.clone();
            move |read| node.progress(read)
        });
        let ready = runtime.memo(move |read| {
            Ok(match &*node.read(read)? {
                Availability::Ready(value) => Some(**value),
                _ => None,
            })
        });
        ADVANCE.with(|slot| {
            *slot.borrow_mut() = Some(Box::new({
                let runtime = runtime.clone();
                let ready = ready.clone();
                move |cuts| {
                    input.set(cuts);
                    runtime.read(&ready).unwrap();
                }
            }))
        });
        assert!(runtime.read(&ready).unwrap().is_none());
        POLL.with(|slot| {
            *slot.borrow_mut() = Some(Box::new(move || {
                tasks.poll();
                let value = runtime.read(&ready).unwrap();
                if value.is_none() && runtime.read(&reports).unwrap().is_some() {
                    PROGRESS_SEEN.with(|slot| *slot.borrow_mut() = true);
                }
                if let Some((cuts, vertices, pixels)) = *value {
                    SUMMARY.with(|slot| {
                        *slot.borrow_mut() = format!(
                            "{cuts} cuts, {vertices} mesh vertices, {pixels} occupied pixels"
                        )
                    });
                    true
                } else {
                    false
                }
            }))
        });
    }
}

fn main() {}
