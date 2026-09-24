//! Opt-in full-frame orbit replay. No window or application event loop.
//!
//! Uses the real pointer handler, editor frame builder, and paint output. During
//! the replay worker publications are intentionally not polled: refined mode
//! measures contention from background rendering, not progress-triggered frames.
use crate::{EditorRunner, command::Example, libraries, workspace};
use gid::{Step, Value};
use incremental::background::Executor;
use kurbo::Size;
use puri::draw::CanvasSink;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use web_time::Instant;
use winit::{
    dpi::PhysicalPosition,
    event::{DeviceId, ElementState, MouseButton, WindowEvent},
};

pub struct OrbitProfile {
    runner: EditorRunner,
    outstanding: Arc<AtomicUsize>,
    size: Size,
    scale: f64,
    step: usize,
}

impl OrbitProfile {
    pub fn new(
        width: u32,
        height: u32,
        scale: f64,
        progress: f64,
        refined: bool,
        collapsed: bool,
    ) -> Self {
        use libraries::{
            controls::{tree_range, vocabulary::STATE},
            toolpath::vocabulary as t,
        };
        let source = Example::Toolpaths.source().replace(
            &t::PREVIEW_REFINED.simple().to_string(),
            &if refined {
                t::PREVIEW_REFINED
            } else {
                t::PREVIEW_MESH
            }
            .simple()
            .to_string(),
        );
        let (doc, names) = crate::gid_text::parse(&source).unwrap();
        let stack = crate::stack::load();
        let tree = libraries::tree::build(
            &names["program_tree"].into(),
            &crate::sources::Sources {
                doc: &doc,
                libraries: &stack.libraries,
            },
            3_000_000,
        )
        .unwrap();
        let selection = tree_range::Selection::new(&tree.items, None);
        let declarations = workspace::declarations(doc.root.as_ref());
        let controls: Vec<_> = declarations[0]
            .path
            .iter()
            .cloned()
            .chain([Step::Key(libraries::presentation::vocabulary::RESULT)])
            .collect();
        let mut editor = crate::new_editor(
            crate::styles::Theme::Light.palette(),
            crate::modifiers::native(),
            true,
            stack,
            crate::font_context(),
            doc,
            None,
            Default::default(),
            None,
        );
        editor.model.workspace.sync_declared(&declarations);
        editor.model.workspace.left_width = 0.5;
        if collapsed {
            let outline = libraries::presentation::vocabulary::OUTLINE;
            let sections: Vec<_> = editor
                .model
                .doc
                .root
                .as_ref()
                .unwrap()
                .as_record()
                .unwrap()
                .get(&outline)
                .unwrap()
                .as_list()
                .unwrap()
                .iter()
                .map(|(position, value)| (position.clone(), value.as_cell().unwrap()))
                .collect();
            let root = editor.model.workspace.document_root().clone();
            for (position, key) in sections {
                editor.set_collapsed(
                    &root,
                    &[Step::Key(outline), Step::Element(position), Step::Key(key)],
                    false,
                    Some(true),
                );
            }
        }
        editor.model.workspace.left.panes[0]
            .view
            .annotations
            .set_field(
                &controls,
                STATE,
                Some(Value::record([
                    (
                        names["focus"],
                        tree_range::cursor_state(
                            &selection,
                            progress * selection.leaves.end as f64,
                        ),
                    ),
                    (names["preview_mode"], names["stock"].into()),
                ])),
            );
        #[cfg(target_arch = "wasm32")]
        let executor = crate::web_worker::executor();
        #[cfg(not(target_arch = "wasm32"))]
        let executor = Executor::threaded(std::num::NonZeroUsize::MIN).unwrap();
        let outstanding = Arc::new(AtomicUsize::new(0));
        editor.computations = crate::computations::Computations::new(
            Executor::new({
                let outstanding = outstanding.clone();
                move |job| {
                    outstanding.fetch_add(1, Ordering::SeqCst);
                    let outstanding = outstanding.clone();
                    executor.submit(Box::new(move || {
                        job();
                        outstanding.fetch_sub(1, Ordering::SeqCst);
                    }));
                }
            }),
            || {},
        );
        let size = Size::new(width.into(), height.into());
        let mut runner = EditorRunner::new(editor);
        runner.refresh_frame(scale, size);
        Self {
            runner,
            outstanding,
            size,
            scale,
            step: 0,
        }
    }

    pub fn ready(&mut self) -> bool {
        if self.runner.editor.computations.tasks.poll() {
            self.runner.refresh_frame(self.scale, self.size);
        }
        self.outstanding.load(Ordering::SeqCst) == 0
    }

    fn event(&mut self, event: WindowEvent) {
        self.runner.flush_before_window_event(&event);
        if let Some(crate::WindowEventTranslation::Pointer(pointer)) =
            crate::translate_window_event(&mut self.runner.editor.reducer, self.scale, &event)
        {
            self.runner.pointer_event(&pointer, self.scale, self.size);
        }
    }

    pub fn release(&mut self) {
        self.event(WindowEvent::MouseInput {
            device_id: DeviceId::dummy(),
            state: ElementState::Released,
            button: MouseButton::Left,
        });
        assert!(self.runner.editor.gesture.is_none());
    }

    pub fn frame(&mut self, canvas: &mut dyn CanvasSink) -> serde_json::Value {
        let start = Instant::now();
        if self.step == 0 {
            self.event(WindowEvent::CursorMoved {
                device_id: DeviceId::dummy(),
                position: PhysicalPosition::new(self.size.width * 0.25, self.size.height * 0.4),
            });
            self.runner.flush_pending_continuous();
            self.event(WindowEvent::MouseInput {
                device_id: DeviceId::dummy(),
                state: ElementState::Pressed,
                button: MouseButton::Left,
            });
            assert!(
                self.runner.editor.gesture.is_some(),
                "orbit must use the installed handler"
            );
        }
        let angle = self.step as f64 * 0.1;
        self.event(WindowEvent::CursorMoved {
            device_id: DeviceId::dummy(),
            position: PhysicalPosition::new(
                self.size.width * (0.25 + 0.05 * angle.sin()),
                self.size.height * (0.4 + 0.03 * angle.sin()),
            ),
        });
        self.runner.flush_pending_continuous();
        let update_ms = start.elapsed().as_secs_f64() * 1000.0;
        let start = Instant::now();
        puri::frame::render(
            self.runner.prepare_paint(self.scale, self.size).renders,
            canvas,
        );
        self.runner.frame_presented();
        self.step += 1;
        serde_json::json!({
            "update_ms":update_ms,
            "paint_ms":start.elapsed().as_secs_f64()*1000.0,
            "outstanding_jobs":self.outstanding.load(Ordering::SeqCst),
        })
    }
}
