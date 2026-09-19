use super::*;
use crate::libraries::{Definitions, control, f32};
use gid::{Cells, Document, new_cell_id};
use incremental::background::{Executor, Job};
use std::{
    cell::Cell,
    collections::VecDeque,
    sync::{Arc, Mutex},
};

fn settings(yaw: f32, progress: f64) -> implicit::computation::Settings {
    settings_at(yaw, progress, 32.0, 24.0, 1.0)
}

fn settings_at(
    yaw: f32,
    progress: f64,
    width: f64,
    height: f64,
    scale: f64,
) -> implicit::computation::Settings {
    use fidget::vocabulary as f;
    let model = fidget::volume_preview(&Value::record([(
        f::PREVIEW_3D,
        Value::record([
            (f::FIELD, f32::value(1.0)),
            (layout::vocabulary::WIDTH, f64::value(width)),
            (layout::vocabulary::HEIGHT, f64::value(height)),
            (f::MIN_X, f32::value(-0.6)),
            (f::MAX_X, f32::value(0.6)),
            (f::MIN_Y, f32::value(-0.6)),
            (f::MAX_Y, f32::value(0.6)),
            (f::MIN_Z, f32::value(-0.6)),
            (f::MAX_Z, f32::value(0.6)),
        ]),
    )]))
    .unwrap();
    let playback = playback::Settings::read(&Value::record([
        (PROGRESS, f64::value(progress)),
        (PROFILE_TOLERANCE, f64::value(0.001)),
        (
            STOCK_MIN,
            Value::record([X, Y, Z].map(|k| (k, f64::value(-0.5)))),
        ),
        (
            STOCK_MAX,
            Value::record([X, Y, Z].map(|k| (k, f64::value(0.5)))),
        ),
        (
            STOCK,
            Value::record([(
                fidget::vocabulary::COLOR,
                crate::libraries::color::value(peniko::Color::from_rgb8(100, 180, 230)),
            )]),
        ),
    ]))
    .unwrap();
    let camera = Value::record([(
        fidget::vocabulary::CAMERA,
        Value::record([(fidget::vocabulary::YAW, f32::value(yaw))]),
    )]);
    implicit::computation::Settings {
        request: fidget::raster::Request::new(model, Some(&camera), scale).unwrap(),
        radius: 0.01,
        color: [200, 150, 20],
        playback: Some(playback),
    }
}

struct Fixture {
    computations: Computations,
    graph: Computation,
    queue: Arc<Mutex<VecDeque<Job>>>,
    runs: Rc<Cell<usize>>,
}

impl Fixture {
    fn new() -> Self {
        let queue = Arc::new(Mutex::new(VecDeque::new()));
        let computations = Computations::new(
            Executor::new({
                let queue = queue.clone();
                move |job| queue.lock().unwrap().push_back(job)
            }),
            || {},
        );
        let mut libraries = crate::stack::load().libraries;
        let counter = new_cell_id();
        let runs = Rc::new(Cell::new(0));
        let mut definitions = Definitions::default();
        definitions.insert(
            counter,
            ::grap::Definition::foreign(
                Value::record([]),
                ::grap::ForeignFunction::new({
                    let runs = runs.clone();
                    move |_, _, _| {
                        runs.set(runs.get() + 1);
                        Ok(Value::record([]))
                    }
                })
                .tracked(),
            ),
        );
        libraries.insert(new_cell_id(), definitions);
        computations.begin(
            Rc::new(Document {
                root: None,
                cells: Cells::new(),
            }),
            libraries,
        );
        let program = ::grap::lambda(
            [],
            ::grap::call(
                control::vocabulary::DO.into(),
                [(
                    control::vocabulary::EXPRESSIONS,
                    Value::list([
                        ::grap::call(counter.into(), []),
                        ::grap::call(
                            START_AT.into(),
                            [X, Y, Z]
                                .into_iter()
                                .zip([-0.25, 0.0, 0.45].map(f64::value)),
                        ),
                        ::grap::call(
                            LINE_TO.into(),
                            [X, Y, Z].into_iter().zip([0.25, 0.0, 0.45].map(f64::value)),
                        ),
                    ]),
                )],
            ),
        );
        let program = crate::libraries::toolpath::tests::tool_program(program);
        let graph = Computation::new(&computations, program, 10000, settings(0.0, 0.25), 3);
        Self {
            computations,
            graph,
            queue,
            runs,
        }
    }

    fn read(&self) -> Rc<View> {
        self.computations.runtime.read(&self.graph.view).unwrap()
    }

    fn finish(&self, index: usize) {
        let job = self
            .queue
            .lock()
            .unwrap()
            .remove(index)
            .expect("requested work");
        std::thread::spawn(job).join().unwrap();
        assert!(self.computations.tasks.poll());
    }

    fn mesh(&self) -> Rc<Outcome<mesh::computation::ViewGeometry>> {
        let view = self.read();
        let View::Mesh(mesh, _) = &*view else {
            panic!("expected immediate mesh fallback")
        };
        mesh.clone()
    }
}

#[test]
fn orbit_reuses_mesh_and_conflates_only_implicit_requests() {
    let f = Fixture::new();
    assert!(f.mesh().as_ref().as_ref().unwrap().awaiting_first_surface);
    assert_eq!(f.runs.get(), 1, "both interpretations share one evaluation");
    assert_eq!(f.queue.lock().unwrap().len(), 1, "only mesh work is ready");
    f.finish(0);
    let mesh = f.mesh();
    assert!(!mesh.as_ref().as_ref().unwrap().awaiting_first_surface);
    f.finish(0);
    assert!(matches!(&*f.read(), View::Implicit(_, _)));

    f.graph.settings.set(settings(30.0, 0.25));
    assert!(
        Rc::ptr_eq(&mesh, &f.mesh()),
        "camera changes reuse the same geometry immediately"
    );
    f.graph.settings.set(settings(60.0, 0.25));
    assert!(Rc::ptr_eq(&mesh, &f.mesh()));
    f.graph
        .settings
        .set(settings_at(60.0, 0.25, 24.0, 32.0, 2.0));
    assert!(
        Rc::ptr_eq(&mesh, &f.mesh()),
        "size and display scale also retain geometry"
    );
    assert_eq!(
        f.queue.lock().unwrap().len(),
        1,
        "no remesh and no queued camera history"
    );
    assert_eq!(f.runs.get(), 1);
    f.finish(0);
    let view = f.read();
    let View::Implicit(image, _) = &*view else {
        panic!("replacement image")
    };
    let image = &image.as_ref().as_ref().unwrap();
    assert!(!image.stale && !image.pending);
    let image = &image.image.as_ref().unwrap().image;
    assert_eq!((image.width, image.height), (48, 64));
    assert!(f.queue.lock().unwrap().is_empty());
}

#[test]
fn playback_moves_tool_immediately_but_implicit_waits_for_the_current_mesh() {
    let f = Fixture::new();
    f.read();
    f.finish(0);
    let old = f.mesh();
    f.finish(0);
    assert!(matches!(&*f.read(), View::Implicit(_, _)));
    f.graph.settings.set(settings(0.0, 0.75));
    let new = f.mesh();
    assert!(!new.as_ref().as_ref().unwrap().awaiting_first_surface);
    assert!(new.as_ref().as_ref().unwrap().surface_pending);
    let tool_center = |view: &Outcome<mesh::computation::ViewGeometry>| {
        let vertices = &view.as_ref().unwrap().geometry.vertices;
        let xs: Vec<_> = vertices
            .iter()
            .filter(|v| v.color == [225, 94, 58].map(|v| v as f32 / 255.0))
            .map(|v| v.position.x)
            .collect();
        (xs.iter().copied().fold(f32::INFINITY, f32::min)
            + xs.iter().copied().fold(f32::NEG_INFINITY, f32::max))
            / 2.0
    };
    assert!(tool_center(&new) > tool_center(&old));
    assert_eq!(f.runs.get(), 1);
    assert_eq!(f.queue.lock().unwrap().len(), 1, "no implicit job yet");
    f.graph.settings.set(settings(15.0, 0.75));
    assert!(
        Rc::ptr_eq(&new, &f.mesh()),
        "orbit retains the pending mesh"
    );
    f.graph.settings.set(settings(15.0, 0.9));
    assert!(f.mesh().as_ref().as_ref().unwrap().surface_pending);
    assert_eq!(
        f.queue.lock().unwrap().len(),
        1,
        "only the latest mesh is queued"
    );
    f.finish(0);
    let current = f.mesh();
    assert!(!current.as_ref().as_ref().unwrap().surface_pending);
    assert_eq!(
        f.queue.lock().unwrap().len(),
        1,
        "now implicit work is ready"
    );
    f.finish(0);
    assert!(matches!(&*f.read(), View::Implicit(_, _)));
    f.graph.settings.set(settings(30.0, 0.9));
    assert!(
        Rc::ptr_eq(&current, &f.mesh()),
        "orbit after implicit cannot regress the stock"
    );
    assert_eq!(f.queue.lock().unwrap().len(), 1);
}

#[test]
fn playback_cancels_queued_implicit_work_while_waiting_for_the_new_mesh() {
    let f = Fixture::new();
    f.read();
    f.finish(0);
    f.mesh(); // Current mesh starts implicit work, but don't execute it yet.
    f.graph.settings.set(settings(0.0, 0.75));
    assert!(f.mesh().as_ref().as_ref().unwrap().surface_pending);
    let cancelled = f.queue.lock().unwrap().pop_front().unwrap();
    std::thread::spawn(cancelled).join().unwrap();
    assert!(
        !f.computations.tasks.poll(),
        "the old implicit slot was cleared"
    );
    assert_eq!(
        f.queue.lock().unwrap().len(),
        1,
        "only replacement mesh remains"
    );
    f.finish(0);
    assert!(!f.mesh().as_ref().as_ref().unwrap().surface_pending);
    f.finish(0);
    assert!(matches!(&*f.read(), View::Implicit(_, _)));
}

#[test]
fn current_failure_is_not_hidden_by_a_previous_successful_render() {
    let f = Fixture::new();
    f.read();
    f.finish(0);
    f.read();
    f.finish(0);
    assert!(matches!(&*f.read(), View::Implicit(_, _)));
    f.graph.fuel.set(0);
    let view = f.read();
    let View::Mesh(result, _) = &*view else {
        panic!("current error must be exposed")
    };
    assert!(result.as_ref().is_err());
}

#[test]
fn model_implicit_image_survives_tool_motion_and_path_style_changes() {
    let f = Fixture::new();
    let model_settings = |progress| {
        let mut settings = settings(0.0, progress);
        settings.playback = Some(
            playback::Settings::read(&Value::record([
                (PROGRESS, f64::value(progress)),
                (PROFILE_TOLERANCE, f64::value(0.001)),
                (
                    STOCK_MIN,
                    Value::record([X, Y, Z].map(|k| (k, f64::value(-0.5)))),
                ),
                (
                    STOCK_MAX,
                    Value::record([X, Y, Z].map(|k| (k, f64::value(0.5)))),
                ),
            ]))
            .unwrap(),
        );
        settings
    };
    f.graph.settings.set(model_settings(0.25));
    f.read();
    f.finish(0);
    f.read();
    f.finish(0);
    let initial = f.read();
    let View::Implicit(image, geometry) = &*initial else {
        panic!("finished model")
    };
    f.graph.settings.set(model_settings(0.75));
    let moved = f.read();
    let View::Implicit(next_image, next_geometry) = &*moved else {
        panic!("model should stay ready")
    };
    assert!(Rc::ptr_eq(image, next_image));
    assert!(!Rc::ptr_eq(geometry, next_geometry));
    assert!(
        f.queue.lock().unwrap().is_empty(),
        "tool motion schedules neither meshing nor implicit rendering"
    );
    let mut restyled = model_settings(0.75);
    restyled.color = [100, 50, 200];
    restyled.radius *= 2.0;
    f.graph.settings.set(restyled);
    let styled = f.read();
    let View::Implicit(styled_image, _) = &*styled else {
        panic!("model should stay ready")
    };
    assert!(Rc::ptr_eq(image, styled_image));
    assert!(f.queue.lock().unwrap().is_empty());
    assert_eq!(f.runs.get(), 1);
}
