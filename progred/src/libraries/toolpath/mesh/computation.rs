use super::super::computation::{Outcome, Recorded, recording};
use super::*;
use crate::computations::Computations;
use incremental::background::{AsyncMemo, Availability, Tasks};
use incremental::{Input, Memo, Runtime};
use std::sync::Arc;

#[derive(Clone, PartialEq)]
pub(crate) struct Settings {
    pub shape: fidget::mesh::Shape,
    pub radius: f64,
    pub color: [u8; 3],
    pub playback: Option<playback::Settings>,
}

pub(crate) struct ViewGeometry {
    pub geometry: Geometry,
    pub awaiting_first_surface: bool,
    /// True even when `geometry` includes a retained, outdated stock surface.
    pub surface_pending: bool,
}

#[derive(Clone, PartialEq)]
struct Surface {
    path: Arc<Recording>,
    model: fidget::mesh::Shape,
    playback: Option<playback::Settings>,
    depth: u8,
}

pub(super) struct Computation {
    pub program: Input<Value>,
    pub fuel: Input<usize>,
    pub settings: Input<Settings>,
    pub depth: Input<u8>,
    pub geometry: Memo<Outcome<ViewGeometry>>,
}

impl Computation {
    pub fn new(
        computations: &Computations,
        program: Value,
        fuel: usize,
        settings: Settings,
        depth: u8,
    ) -> Self {
        let runtime = &computations.runtime;
        let program = runtime.input(program);
        let fuel = runtime.input(fuel);
        let settings = runtime.input(settings);
        let depth = runtime.input(depth);
        let recording = recording(computations, program.clone(), fuel.clone());
        let geometry = geometry(
            computations,
            recording,
            runtime.memo({
                let settings = settings.clone();
                move |read| Ok((*settings.read(read)).clone())
            }),
            depth.clone(),
        );
        Self {
            program,
            fuel,
            settings,
            depth,
            geometry,
        }
    }
}

/// Compose mesh layers over a caller's recording, independently of image rendering.
pub(crate) fn geometry(
    computations: &Computations,
    recording: Memo<Recorded>,
    settings: Memo<Settings>,
    depth: Input<u8>,
) -> Memo<Outcome<ViewGeometry>> {
    Layers::new(
        &computations.runtime,
        &computations.tasks,
        recording,
        settings,
        depth,
    )
    .combined(&computations.runtime)
}

struct Layers {
    paths: Memo<Outcome<Geometry>>,
    surface: AsyncMemo<Outcome<Surface>, Outcome<Geometry>>,
}

impl Layers {
    fn new(
        runtime: &Runtime,
        tasks: &Tasks,
        recording: Memo<Recorded>,
        settings: Memo<Settings>,
        depth: Input<u8>,
    ) -> Self {
        let prepared = runtime.memo({
            let recording = recording.clone();
            let settings = settings.clone();
            move |read| {
                let record = recording.read(read)?;
                let settings = settings.read(read)?;
                let depth = *depth.read(read);
                Ok(record.path().map(|_| Surface {
                    path: record.path.clone(),
                    model: settings.shape.clone(),
                    playback: settings.playback.clone(),
                    depth,
                }))
            }
        });
        let surface = tasks.memo(prepared, |prepared, cancel| {
            let request = match prepared {
                Ok(request) => request,
                Err(failure) => return Ok(Err(failure)),
            };
            cancel.check()?;
            let shape = remaining_shape(&request);
            cancel.check()?;
            let shape = match shape {
                Ok(shape) => shape,
                Err(failure) => return Ok(Err(failure)),
            };
            let mut geometry = Geometry::default();
            Ok(shape
                .append_cancellable(&mut geometry, request.depth, cancel)?
                .map(|()| geometry)
                .ok_or_else(|| absent::with_reason(fidget::vocabulary::INVALID_FIELD)))
        });
        let paths = runtime.memo_by(
            move |read| {
                let record = recording.read(read)?;
                let settings = settings.read(read)?;
                Ok(path_geometry(&record, &settings))
            },
            |_, _| false,
        );
        Self { paths, surface }
    }

    fn combined(self, runtime: &Runtime) -> Memo<Outcome<ViewGeometry>> {
        runtime.memo_by(
            move |read| {
                let paths = self.paths.read(read)?;
                let surface = self.surface.read(read)?;
                Ok(combine(&paths, &surface))
            },
            |_, _| false,
        )
    }
}

fn combine(
    paths: &Outcome<Geometry>,
    surface: &Availability<Outcome<Geometry>>,
) -> Outcome<ViewGeometry> {
    let paths = paths.as_ref().map_err(Clone::clone)?;
    let mut geometry = paths.clone();
    let (surface, pending) = match surface {
        Availability::Ready(value) | Availability::Refining(value) => (Some(value.as_ref()), false),
        Availability::Pending { previous } => (previous.as_deref(), true),
    };
    // A previous failure has no geometry to retain while a replacement is pending.
    let surface = match surface {
        Some(Ok(surface)) => Some(surface),
        Some(Err(failure)) if !pending => return Err(failure.clone()),
        _ => None,
    };
    if let Some(surface) = surface {
        geometry
            .append_colored(surface, |color| {
                if pending {
                    updating_color(color)
                } else {
                    color
                }
            })
            .ok_or_else(|| absent::with_reason(INVALID_INPUT))?;
    }
    Ok(ViewGeometry {
        geometry,
        awaiting_first_surface: pending && surface.is_none(),
        surface_pending: pending,
    })
}

fn updating_color(color: [f32; 3]) -> [f32; 3] {
    let gray = (color[0] + color[1] + color[2]) / 3.0;
    color.map(|channel| 0.25 * channel + 0.75 * gray)
}

fn remaining_shape(request: &Surface) -> Outcome<fidget::mesh::Shape> {
    let mut shape = request.model.clone();
    if let Some(playback) = &request.playback {
        if let Some(stock) = playback
            .remaining_stock(&request.path)
            .map_err(|_| absent::with_reason(INVALID_INPUT))?
        {
            shape.objects = vec![stock];
        }
    }
    Ok(shape)
}

fn path_geometry(record: &Recorded, settings: &Settings) -> Outcome<Geometry> {
    let path = record.path()?;
    let invalid = || absent::with_reason(INVALID_INPUT);
    let mut tubes = tubes::Tubes::new(settings.radius, settings.color).ok_or_else(invalid)?;
    if let Some(playback) = &settings.playback {
        playback
            .draw(path, &mut tubes, settings.radius, settings.color)
            .map_err(|_| invalid())?;
    } else {
        path.replay(&mut tubes).map_err(|_| invalid())?;
    }
    Ok(tubes.geometry)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::libraries::{Definitions, control};
    use gid::{Cells, Document, new_cell_id};
    use std::cell::Cell;

    fn preview() -> fidget::VolumePreview {
        // Use the ordinary preview decoder, including its volume defaults.
        let stack = crate::stack::load();
        let doc = Document {
            root: None,
            cells: Cells::new(),
        };
        let sources = crate::sources::Sources {
            doc: &doc,
            libraries: &stack.libraries,
        };
        let value = ::grap::evaluate(
            &::grap::call(
                fidget::vocabulary::PREVIEW_MESH.into(),
                [(
                    presentation::vocabulary::VALUE,
                    crate::libraries::f32::value(1.0),
                )],
            ),
            &sources,
            1000,
        )
        .result;
        fidget::mesh::read(&value).unwrap().0
    }

    #[test]
    fn playback_moves_immediately_and_only_the_surface_uses_pending_colors() {
        use incremental::background::{Executor, Job};
        use std::collections::VecDeque;
        use std::sync::Mutex;

        let runtime = Runtime::default();
        let queue = Arc::new(Mutex::new(VecDeque::<Job>::new()));
        let tasks = Tasks::new(
            &runtime,
            Executor::new({
                let queue = queue.clone();
                move |job| queue.lock().unwrap().push_back(job)
            }),
            || {},
        );
        let next = || queue.lock().unwrap().pop_front().unwrap();
        let mut path = Recording::default();
        path.enter_tool(&crate::libraries::toolpath::cutter::Tool::ball(0.2, 0.4).unwrap());
        path.start_at(
            [-0.25, 0.0, 0.5],
            crate::libraries::toolpath::paths::Axis::Z,
        )
        .unwrap();
        path.line_to([0.25, 0.0, 0.5]).unwrap();
        path.leave_tool();
        let path = Arc::new(path);
        let recording = runtime.memo(move |_| {
            Ok(Recorded {
                path: path.clone(),
                evaluation: ::grap::Evaluation {
                    result: Value::record([]),
                    completed: true,
                    remaining_fuel: 100,
                },
            })
        });
        let playback = |progress, tolerance| {
            playback::Settings::read(&Value::record([
                (PROGRESS, f64::value(progress)),
                (PROFILE_TOLERANCE, f64::value(tolerance)),
                (
                    STOCK_MIN,
                    Value::record([X, Y, Z].map(|key| (key, f64::value(-0.5)))),
                ),
                (
                    STOCK_MAX,
                    Value::record([X, Y, Z].map(|key| (key, f64::value(0.5)))),
                ),
                (
                    STOCK,
                    Value::record([(
                        fidget::vocabulary::COLOR,
                        crate::libraries::color::value(peniko::Color::from_rgb8(100, 180, 230)),
                    )]),
                ),
            ]))
            .unwrap()
        };
        let mut props = Settings {
            shape: (&preview()).into(),
            radius: 0.01,
            color: [20, 150, 230],
            playback: Some(playback(0.25, 0.001)),
        };
        let settings = runtime.input(props.clone());
        let layers = Layers::new(
            &runtime,
            &tasks,
            recording,
            runtime.memo({
                let settings = settings.clone();
                move |read| Ok((*settings.read(read)).clone())
            }),
            runtime.input(4),
        );
        let paths = layers.paths.clone();
        let surface = runtime.memo_by(
            {
                let surface = layers.surface.clone();
                move |read| surface.read(read)
            },
            Rc::ptr_eq,
        );
        let combined = layers.combined(&runtime);
        let first = runtime.read(&combined).unwrap();
        assert!(first.as_ref().as_ref().unwrap().awaiting_first_surface);
        assert!(!first.as_ref().as_ref().unwrap().geometry.indices.is_empty());
        next()();
        assert!(tasks.poll());
        let ready = runtime.read(&combined).unwrap();
        assert!(!ready.as_ref().as_ref().unwrap().awaiting_first_surface);
        let old_surface = runtime.read(&surface).unwrap();
        let Availability::Ready(old_surface) = &**old_surface else {
            panic!()
        };
        let old_geometry = &old_surface.as_ref().as_ref().unwrap();
        assert!(!old_geometry.vertices.is_empty());

        props.playback = Some(playback(0.75, 0.001));
        settings.set(props);
        let waiting = runtime.read(&combined).unwrap();
        let waiting = &waiting.as_ref().as_ref().unwrap();
        assert!(!waiting.awaiting_first_surface);
        let paths = runtime.read(&paths).unwrap();
        let paths = &paths.as_ref().as_ref().unwrap();
        let (new_paths, stale_stock) = waiting.geometry.vertices.split_at(paths.vertices.len());
        assert_eq!(new_paths.len(), paths.vertices.len());
        assert!(
            new_paths
                .iter()
                .zip(&paths.vertices)
                .all(|(a, b)| { a.position == b.position && a.color == b.color })
        );
        let tool_color = [225, 94, 58].map(|n| n as f32 / 255.0);
        let tool_center = |geometry: &Geometry| {
            let xs: Vec<_> = geometry
                .vertices
                .iter()
                .filter(|v| v.color == tool_color)
                .map(|v| v.position.x)
                .collect();
            (xs.iter().copied().fold(f32::INFINITY, f32::min)
                + xs.iter().copied().fold(f32::NEG_INFINITY, f32::max))
                / 2.0
        };
        assert!(
            tool_center(&waiting.geometry)
                > tool_center(&ready.as_ref().as_ref().unwrap().geometry)
        );
        assert_eq!(stale_stock.len(), old_geometry.vertices.len());
        assert!(
            stale_stock
                .iter()
                .zip(&old_geometry.vertices)
                .all(|(a, b)| { a.position == b.position && a.color == updating_color(b.color) })
        );
        assert_eq!(queue.lock().unwrap().len(), 1);
        next()();
        tasks.poll();
        let complete = runtime.read(&combined).unwrap();
        let complete = &complete.as_ref().as_ref().unwrap().geometry;
        let stock_color = [100, 180, 230].map(|n| n as f32 / 255.0);
        assert!(
            complete
                .vertices
                .iter()
                .skip(paths.vertices.len())
                .all(|v| v.color == stock_color)
        );
    }

    #[test]
    fn a_pending_replacement_does_not_retain_a_previous_absent_as_an_error() {
        let paths = Ok(Geometry::default());
        let failure = Err(absent::with_reason(INVALID_INPUT));
        let pending = Availability::Pending {
            previous: Some(Arc::new(failure.clone())),
        };
        assert!(combine(&paths, &pending).unwrap().awaiting_first_surface);
        let ready = Availability::Ready(Arc::new(failure));
        assert!(combine(&paths, &ready).is_err());
    }

    #[test]
    fn path_appearance_changes_retain_the_stock_mesh() {
        let runtime = Runtime::default();
        let tasks = Tasks::new(&runtime, incremental::background::Executor::inline(), || {});
        let recording = runtime.memo(move |_| {
            let mut path = Recording::default();
            path.enter_tool(&crate::libraries::toolpath::cutter::Tool::ball(0.2, 0.4).unwrap());
            path.start_at(
                [-0.25, 0.0, 0.5],
                crate::libraries::toolpath::paths::Axis::Z,
            )
            .unwrap();
            path.line_to([0.25, 0.0, 0.5]).unwrap();
            path.leave_tool();
            Ok(Recorded {
                path: Arc::new(path),
                evaluation: ::grap::Evaluation {
                    result: Value::record([]),
                    completed: true,
                    remaining_fuel: 100,
                },
            })
        });
        let playback = |progress, tolerance| {
            playback::Settings::read(&Value::record([
                (PROGRESS, f64::value(progress)),
                (PROFILE_TOLERANCE, f64::value(tolerance)),
                (
                    STOCK_MIN,
                    Value::record([X, Y, Z].map(|key| (key, f64::value(-0.5)))),
                ),
                (
                    STOCK_MAX,
                    Value::record([X, Y, Z].map(|key| (key, f64::value(0.5)))),
                ),
                (
                    STOCK,
                    Value::record([(
                        fidget::vocabulary::COLOR,
                        crate::libraries::color::value(peniko::Color::from_rgb8(100, 180, 230)),
                    )]),
                ),
            ]))
            .unwrap()
        };
        let mut props = Settings {
            shape: (&preview()).into(),
            radius: 0.01,
            color: [20, 150, 230],
            playback: Some(playback(0.25, 0.001)),
        };
        let settings = runtime.input(props.clone());
        let depth = runtime.input(3);
        let layers = Layers::new(
            &runtime,
            &tasks,
            recording,
            runtime.memo({
                let settings = settings.clone();
                move |read| Ok((*settings.read(read)).clone())
            }),
            depth.clone(),
        );
        let surface_node = runtime.memo_by(
            {
                let surface = layers.surface.clone();
                move |read| surface.read(read)
            },
            Rc::ptr_eq,
        );
        let surface = runtime.read(&surface_node).unwrap();
        let Availability::Ready(value) = &**surface else {
            panic!("inline result is ready")
        };
        assert!(!value.as_ref().as_ref().unwrap().indices.is_empty());
        let paths = runtime.read(&layers.paths).unwrap();
        props.color = [250, 100, 20];
        settings.set(props.clone());
        assert!(Rc::ptr_eq(&surface, &runtime.read(&surface_node).unwrap()));
        assert!(!Rc::ptr_eq(&paths, &runtime.read(&layers.paths).unwrap()));
        props.radius = 0.02;
        settings.set(props.clone());
        assert!(Rc::ptr_eq(&surface, &runtime.read(&surface_node).unwrap()));
        props.playback = Some(playback(0.75, 0.001));
        settings.set(props.clone());
        let moved = runtime.read(&surface_node).unwrap();
        assert!(!Rc::ptr_eq(&surface, &moved));
        props.playback = Some(playback(0.75, 0.01));
        settings.set(props);
        let accuracy_changed = runtime.read(&surface_node).unwrap();
        assert!(
            !Rc::ptr_eq(&moved, &accuracy_changed),
            "accuracy is a stock computation input"
        );
        depth.set(4);
        assert!(!Rc::ptr_eq(
            &accuracy_changed,
            &runtime.read(&surface_node).unwrap()
        ));
    }

    #[test]
    fn cam_dependencies_reuse_geometry_and_invalidate_program_and_settings() {
        let stack = crate::stack::load();
        let mut libraries = stack.libraries.clone();
        let counter = new_cell_id();
        let endpoint = new_cell_id();
        let unrelated = new_cell_id();
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
        let mut doc = Document {
            root: None,
            cells: Cells::new(),
        };
        doc.cells.set_value(endpoint, f64::value(0.5));
        let program = ::grap::lambda(
            [],
            ::grap::call(
                control::vocabulary::DO.into(),
                [(
                    control::vocabulary::EXPRESSIONS,
                    Value::list([
                        ::grap::call(counter.into(), []),
                        ::grap::call(START_AT.into(), [X, Y, Z].map(|key| (key, f64::value(0.0)))),
                        ::grap::call(
                            LINE_TO.into(),
                            [
                                (X, endpoint.into()),
                                (Y, f64::value(0.0)),
                                (Z, f64::value(0.0)),
                            ],
                        ),
                    ]),
                )],
            ),
        );
        let program = crate::libraries::toolpath::tests::tool_program(program);
        let model = preview();
        let settings = Settings {
            shape: (&model).into(),
            radius: 0.01,
            color: [255, 120, 20],
            playback: None,
        };
        let computations = Computations::from_sources(crate::sources::Sources {
            doc: &doc,
            libraries: &libraries,
        });
        let graph = Computation::new(&computations, program.clone(), 10000, settings.clone(), 3);
        let first = computations.runtime.read(&graph.geometry).unwrap();
        assert!(!first.as_ref().as_ref().unwrap().geometry.indices.is_empty());
        assert!(Rc::ptr_eq(
            &first,
            &computations.runtime.read(&graph.geometry).unwrap()
        ));
        assert_eq!(runs.get(), 1);
        doc.cells.set_value(unrelated, f64::value(9.0));
        computations.begin(Rc::new(doc.clone()), libraries.clone());
        assert!(Rc::ptr_eq(
            &first,
            &computations.runtime.read(&graph.geometry).unwrap()
        ));
        assert_eq!(runs.get(), 1);

        graph.depth.set(4);
        let deeper = computations.runtime.read(&graph.geometry).unwrap();
        assert!(!Rc::ptr_eq(&first, &deeper));
        assert_eq!(runs.get(), 1, "meshing quality does not re-run the path");
        let mut recolored = settings.clone();
        recolored.color = [50, 100, 255];
        graph.settings.set(recolored);
        let colored = computations.runtime.read(&graph.geometry).unwrap();
        assert!(!Rc::ptr_eq(&deeper, &colored));
        assert_eq!(runs.get(), 1, "view settings do not re-run the path");

        doc.cells.set_value(endpoint, f64::value(0.75));
        computations.begin(Rc::new(doc.clone()), libraries.clone());
        let edited = computations.runtime.read(&graph.geometry).unwrap();
        assert!(!Rc::ptr_eq(&colored, &edited));
        assert_eq!(runs.get(), 2);

        graph.settings.set(settings.clone());
        graph.depth.set(3);
        let reused = computations.runtime.read(&graph.geometry).unwrap();
        let fresh = Computation::new(&computations, program, 10000, settings, 3);
        let recomputed = computations.runtime.read(&fresh.geometry).unwrap();
        let a = reused.as_ref().as_ref().unwrap();
        let b = recomputed.as_ref().as_ref().unwrap();
        let (a, b) = (&a.geometry, &b.geometry);
        assert_eq!(a.indices, b.indices);
        assert!(
            a.vertices
                .iter()
                .zip(&b.vertices)
                .all(|(a, b)| a.position == b.position && a.color == b.color)
        );
        assert_eq!(a.vertices.len(), b.vertices.len());

        let mut playback = Value::record([
            (PROGRESS, f64::value(0.25)),
            (PROFILE_TOLERANCE, f64::value(0.001)),
            (
                STOCK_MIN,
                Value::record([X, Y, Z].map(|key| (key, f64::value(-1.0)))),
            ),
            (
                STOCK_MAX,
                Value::record([X, Y, Z].map(|key| (key, f64::value(1.0)))),
            ),
            (
                STOCK,
                Value::record([(
                    fidget::vocabulary::COLOR,
                    crate::libraries::color::value(peniko::Color::from_rgb8(100, 180, 230)),
                )]),
            ),
        ]);
        let set_playback = |value: &Value| {
            let mut settings = (*graph.settings.observed().0).clone();
            settings.playback = Some(playback::Settings::read(value).unwrap());
            graph.settings.set(settings);
            computations.runtime.read(&graph.geometry).unwrap()
        };
        let runs_before = runs.get();
        let quarter = set_playback(&playback);
        assert!(quarter.is_ok());
        playback = Value::record(playback.as_record().unwrap().iter().map(|(k, v)| {
            (
                *k,
                if *k == PROGRESS {
                    f64::value(0.75)
                } else {
                    v.clone()
                },
            )
        }));
        let three_quarters = set_playback(&playback);
        assert!(three_quarters.is_ok());
        assert!(!Rc::ptr_eq(&quarter, &three_quarters));
        assert_eq!(
            runs.get(),
            runs_before,
            "stock playback reuses the path recording"
        );

        doc.cells.set_value(endpoint, Value::record([]));
        computations.begin(Rc::new(doc), libraries);
        assert!(
            computations.runtime.read(&graph.geometry).unwrap().is_err(),
            "invalid program replaces the old image with an absence"
        );
    }
}
