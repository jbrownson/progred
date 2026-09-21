use super::super::computation::{Outcome, Recorded, recording};
use super::*;
use crate::computations::Computations;
use fidget::raster::{Frame, Passes, SoftwareScene, ViewRequest};
use incremental::background::{Availability, Progress};
use incremental::{Input, Memo};

#[derive(Clone, PartialEq)]
pub(crate) struct Settings {
    pub request: fidget::raster::Request,
    pub radius: f64,
    pub color: [u8; 3],
    pub playback: Option<playback::Settings>,
}

#[derive(Clone, PartialEq)]
enum SceneRequest {
    Model(Vec<fidget::SceneObject>),
    Stock(Arc<Recording>, playback::Settings),
}

impl SceneRequest {
    fn new(path: Arc<Recording>, settings: &Settings) -> Self {
        match settings
            .playback
            .as_ref()
            .filter(|p| p.stock_color().is_some())
        {
            Some(playback) => Self::Stock(path, playback.clone()),
            None => Self::Model(settings.request.preview.objects.clone()),
        }
    }
}

pub(crate) struct ViewImage {
    pub image: Option<Frame>,
    pub pending: bool,
    pub stale: bool,
    pub progress: Option<Progress>,
}

pub(super) struct Computation {
    pub program: Input<Value>,
    pub fuel: Input<usize>,
    pub settings: Input<Settings>,
    pub image: Memo<Outcome<ViewImage>>,
    pub paths: Memo<Outcome<fidget::mesh::Mesh>>,
}

impl Computation {
    pub fn new(
        computations: &Computations,
        program: Value,
        fuel: usize,
        settings: Settings,
        permitted: Memo<bool>,
    ) -> Self {
        let runtime = &computations.runtime;
        let program = runtime.input(program);
        let fuel = runtime.input(fuel);
        let settings = runtime.input(settings);
        let recording = recording(computations, program.clone(), fuel.clone());
        let paths = paths(computations, recording.clone(), settings.clone());
        let image = image(
            computations,
            recording,
            settings.clone(),
            Passes::Progressive {
                first_max_edge: 128,
            },
            None,
            Some(permitted),
        );
        Self {
            program,
            fuel,
            settings,
            image,
            paths,
        }
    }

    #[cfg(test)]
    fn with_render(
        computations: &Computations,
        program: Value,
        fuel: usize,
        settings: Settings,
        render: impl Fn(
            Arc<SceneRequest>,
            ViewRequest,
            &incremental::Cancellation,
            &mut (dyn FnMut(Outcome<Frame>) -> Result<(), incremental::Error> + Send),
            &(dyn Fn(Progress) + Sync),
        ) -> Result<Outcome<Frame>, incremental::Error>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        let runtime = &computations.runtime;
        let program = runtime.input(program);
        let fuel = runtime.input(fuel);
        let settings = runtime.input(settings);
        let recording = recording(computations, program.clone(), fuel.clone());
        let paths = paths(computations, recording.clone(), settings.clone());
        let image = image_with(
            computations,
            recording,
            settings.clone(),
            None,
            None,
            |request, _| Ok(Ok(Arc::new(request))),
            move |scene, view, cancel, publish, progress| {
                render(scene.clone(), view, cancel, publish, progress)
            },
        );
        Self {
            program,
            fuel,
            settings,
            image,
            paths,
        }
    }
}

fn paths(
    computations: &Computations,
    recording: Memo<Recorded>,
    settings: Input<Settings>,
) -> Memo<Outcome<fidget::mesh::Mesh>> {
    let geometry_settings = computations.runtime.memo(move |read| {
        let settings = settings.read(read);
        Ok((settings.radius, settings.color, settings.playback.clone()))
    });
    computations.runtime.memo_by(
        move |read| {
            let record = recording.read(read)?;
            let settings = geometry_settings.read(read)?;
            Ok(super::super::mesh::computation::paths(
                &record,
                settings.0,
                settings.1,
                settings.2.as_ref(),
            ))
        },
        |_, _| false,
    )
}

/// Use the same observed program as other interpretations. The caller chooses
/// the quality passes independently of its fallback and scheduling.
/// A readiness dependency gates background work; becoming unready cancels it.
pub(crate) fn image(
    computations: &Computations,
    recording: Memo<Recorded>,
    settings: Input<Settings>,
    passes: Passes,
    ready: Option<Memo<bool>>,
    permitted: Option<Memo<bool>>,
) -> Memo<Outcome<ViewImage>> {
    image_with(
        computations,
        recording,
        settings,
        ready,
        permitted,
        |request, cancel| {
            let objects = match scene(request) {
                Ok(objects) => objects,
                Err(failure) => return Ok(Err(failure)),
            };
            cancel.check()?;
            let compiled = SoftwareScene::new(&objects, cancel);
            cancel.check()?;
            Ok(compiled.ok_or_else(|| absent::with_reason(fidget::vocabulary::INVALID_FIELD)))
        },
        move |scene, view, cancel, publish, progress| {
            Ok(view
                .render(
                    scene,
                    passes,
                    4,
                    cancel,
                    &mut |image| publish(Ok(image)),
                    Some(progress),
                )?
                .ok_or_else(|| absent::with_reason(fidget::vocabulary::INVALID_FIELD)))
        },
    )
}

/// The two workers have independent generations: a new camera cancels pixels,
/// while scene preparation survives until its geometry inputs actually change.
fn image_with<S: Send + Sync + 'static>(
    computations: &Computations,
    recording: Memo<Recorded>,
    settings: Input<Settings>,
    ready: Option<Memo<bool>>,
    permitted: Option<Memo<bool>>,
    prepare: impl Fn(SceneRequest, &incremental::Cancellation) -> Result<Outcome<S>, incremental::Error>
    + Send
    + Sync
    + 'static,
    render: impl Fn(
        &S,
        ViewRequest,
        &incremental::Cancellation,
        &mut (dyn FnMut(Outcome<Frame>) -> Result<(), incremental::Error> + Send),
        &(dyn Fn(Progress) + Sync),
    ) -> Result<Outcome<Frame>, incremental::Error>
    + Send
    + Sync
    + 'static,
) -> Memo<Outcome<ViewImage>> {
    let runtime = &computations.runtime;
    let requested_scene = runtime.memo({
        let settings = settings.clone();
        move |read| {
            let record = recording.read(read)?;
            let settings = settings.read(read);
            Ok(record
                .path()
                .map(|_| SceneRequest::new(record.path.clone(), &settings)))
        }
    });
    let view = runtime.memo(move |read| Ok(settings.read(read).request.view()));
    let scene_input = runtime.memo({
        let requested_scene = requested_scene.clone();
        move |read| {
            if let Some(ready) = &ready
                && !*ready.read(read)?
            {
                return Ok(None);
            }
            Ok(Some((*requested_scene.read(read)?).clone()))
        }
    });
    let compiled =
        computations
            .tasks
            .memo_reporting_when_ready(scene_input, move |request, cancel, _, _| {
                cancel.check()?;
                match request {
                    Ok(request) => prepare(request, cancel),
                    Err(failure) => Ok(Err(failure)),
                }
            });
    let worker_input = runtime.memo_by(
        {
            let compiled = compiled.clone();
            move |read| {
                Ok(match &*compiled.read(read)? {
                    Availability::Ready(scene) | Availability::Refining(scene) => {
                        Some((scene.clone(), (*view.read(read)?).clone()))
                    }
                    // Never render a new camera using a previous geometry generation.
                    Availability::Pending { .. } => None,
                })
            }
        },
        |a, b| match (a, b) {
            (Some((a_scene, a_view)), Some((b_scene, b_view))) => {
                // The shared async result names one completed scene generation;
                // compiled programs have no structural equality operation.
                Arc::ptr_eq(a_scene, b_scene) && a_view == b_view
            }
            (None, None) => true,
            _ => false,
        },
    );
    let worker = computations.tasks.memo_reporting_with_start_condition(
        worker_input,
        permitted,
        move |(scene, view), cancel, publish, progress| {
            cancel.check()?;
            match scene.as_ref() {
                Ok(scene) => render(scene, view, cancel, publish, progress),
                Err(failure) => Ok(Err(failure.clone())),
            }
        },
    );
    runtime.memo_by(
        move |read| {
            // Invalid current programs replace old images immediately, without a worker round-trip.
            let requested_scene = requested_scene.read(read)?;
            let availability = worker.read(read)?;
            let progress = worker.progress(read)?;
            if let Err(failure) = &*requested_scene {
                return Ok(Err(failure.clone()));
            }
            if let Availability::Ready(scene) = &*compiled.read(read)?
                && let Err(failure) = scene.as_ref()
            {
                return Ok(Err(failure.clone()));
            }
            Ok(match &*availability {
                Availability::Ready(image) | Availability::Refining(image) => image
                    .as_ref()
                    .as_ref()
                    .map(|image| ViewImage {
                        image: Some(image.clone()),
                        pending: matches!(&*availability, Availability::Refining(_)),
                        stale: false,
                        progress,
                    })
                    .map_err(Clone::clone),
                Availability::Pending { previous } => Ok(ViewImage {
                    image: previous
                        .as_deref()
                        .and_then(|result| result.as_ref().ok())
                        .cloned(),
                    pending: true,
                    stale: previous.is_some(),
                    progress,
                }),
            })
        },
        |_, _| false,
    )
}

fn scene(request: SceneRequest) -> Outcome<Vec<fidget::SceneObject>> {
    let invalid = || absent::with_reason(INVALID_INPUT);
    let objects = match request {
        SceneRequest::Model(objects) => objects,
        SceneRequest::Stock(path, playback) => vec![
            playback
                .remaining_stock(&path)
                .map_err(|_| invalid())?
                .ok_or_else(invalid)?,
        ],
    };
    if objects.len() > usize::from(u16::MAX) + 1 {
        Err(absent::with_reason(fidget::vocabulary::INVALID_SCENE))
    } else {
        Ok(objects)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::libraries::{control, f32};
    use gid::{Cells, Document, new_cell_id};
    use incremental::background::{Executor, Job};
    use std::{collections::VecDeque, sync::Mutex};

    fn settings(yaw: f32) -> Settings {
        use fidget::vocabulary as f;
        let preview = fidget::volume_preview(&Value::record([(
            f::PREVIEW_3D,
            Value::record([
                (f::FIELD, f32::value(1.0)),
                (layout::vocabulary::WIDTH, f64::value(32.0)),
                (layout::vocabulary::HEIGHT, f64::value(24.0)),
                (f::MIN_X, f32::value(-1.0)),
                (f::MAX_X, f32::value(1.0)),
                (f::MIN_Y, f32::value(-1.0)),
                (f::MAX_Y, f32::value(1.0)),
                (f::MIN_Z, f32::value(-1.0)),
                (f::MAX_Z, f32::value(1.0)),
            ]),
        )]))
        .unwrap();
        let camera = Value::record([(
            fidget::vocabulary::CAMERA,
            Value::record([(fidget::vocabulary::YAW, f32::value(yaw))]),
        )]);
        Settings {
            request: fidget::raster::Request::new(preview, Some(&camera), 1.0).unwrap(),
            radius: 0.01,
            color: [20, 150, 230],
            playback: None,
        }
    }

    fn playback(progress: f64) -> playback::Settings {
        playback::Settings::read(&Value::record([
            (PROGRESS, f64::value(progress)),
            (PROFILE_TOLERANCE, f64::value(0.001)),
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
                    color::value(peniko::Color::from_rgb8(100, 180, 230)),
                )]),
            ),
        ]))
        .unwrap()
    }

    #[test]
    fn async_images_reuse_paths_replace_pending_requests_and_expose_current_failures() {
        let settings = |yaw| Settings {
            playback: Some(playback(0.5)),
            ..settings(yaw)
        };
        let stack = crate::stack::load();
        let endpoint = new_cell_id();
        let mut doc = Document {
            root: None,
            cells: Cells::new(),
        };
        doc.cells.set_value(endpoint, f64::value(0.5));
        let queue = Arc::new(Mutex::new(VecDeque::<Job>::new()));
        let computations = Computations::new(
            Executor::new({
                let queue = queue.clone();
                move |job| queue.lock().unwrap().push_back(job)
            }),
            || {},
        );
        computations.begin(Rc::new(doc.clone()), stack.libraries.clone());
        let program = ::grap::lambda(
            [],
            ::grap::call(
                control::vocabulary::DO.into(),
                [(
                    control::vocabulary::EXPRESSIONS,
                    Value::list([
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
        let rendered = Arc::new(Mutex::new(Vec::new()));
        let graph = Computation::with_render(&computations, program, 10000, settings(0.0), {
            let rendered = rendered.clone();
            move |scene, view, cancel, _publish, _progress| {
                cancel.check()?;
                let mut rendered = rendered.lock().unwrap();
                rendered.push((scene, view));
                Ok(Ok(fidget::raster::Frame {
                    image: puri::ImageData {
                        data: vec![rendered.len() as u8, 0, 0, 255].into(),
                        format: peniko::ImageFormat::Rgba8,
                        alpha_type: peniko::ImageAlphaType::Alpha,
                        width: 1,
                        height: 1,
                    },
                    depth: vec![1.0].into(),
                    partial: false,
                }))
            }
        });
        let read = || computations.runtime.read(&graph.image).unwrap();
        let finish = || {
            let job = queue.lock().unwrap().pop_front().unwrap();
            std::thread::spawn(job).join().unwrap();
            assert!(computations.tasks.poll());
        };
        let first = read();
        let first = &first.as_ref().as_ref().unwrap();
        assert!(first.pending && first.image.is_none());
        finish();
        assert!(read().as_ref().as_ref().unwrap().pending);
        finish();
        let ready = read();
        assert!(!ready.as_ref().as_ref().unwrap().pending);
        assert!(Rc::ptr_eq(&ready, &read()));

        doc.cells.set_value(new_cell_id(), f64::value(9.0));
        computations.begin(Rc::new(doc.clone()), stack.libraries.clone());
        assert!(
            Rc::ptr_eq(&ready, &read()),
            "unrelated edits retain the image"
        );
        graph.settings.set(settings(30.0));
        let pending = read();
        let pending = &pending.as_ref().as_ref().unwrap();
        assert!(pending.pending);
        assert_eq!(pending.image.as_ref().unwrap().image.data.data()[0], 1);
        graph.settings.set(settings(60.0));
        assert!(read().as_ref().as_ref().unwrap().pending);
        assert_eq!(queue.lock().unwrap().len(), 1);
        finish();
        assert!(!read().as_ref().as_ref().unwrap().pending);
        {
            let rendered = rendered.lock().unwrap();
            assert_eq!(
                rendered.len(),
                2,
                "superseded camera request wasn't rendered"
            );
            assert!(Arc::ptr_eq(&rendered[0].0, &rendered[1].0));
            assert!(rendered[1].1 == settings(60.0).request.view());
        }

        doc.cells.set_value(endpoint, f64::value(0.75));
        computations.begin(Rc::new(doc.clone()), stack.libraries.clone());
        assert!(read().as_ref().as_ref().unwrap().pending);
        finish();
        read();
        finish();
        read();
        {
            let rendered = rendered.lock().unwrap();
            assert!(!Arc::ptr_eq(&rendered[1].0, &rendered[2].0));
        }
        // A current failure must not leave the previously successful image on screen.
        doc.cells.set_value(endpoint, Value::record([]));
        computations.begin(Rc::new(doc), stack.libraries.clone());
        assert!(read().as_ref().is_err());
        finish();
        assert!(read().as_ref().is_err());
        assert_eq!(rendered.lock().unwrap().len(), 3);
    }

    #[test]
    fn camera_changes_do_not_cancel_running_preparation_but_geometry_changes_do() {
        use std::sync::mpsc;
        use std::time::Duration;

        let queue = Arc::new(Mutex::new(VecDeque::<Job>::new()));
        let computations = Computations::new(
            Executor::new({
                let queue = queue.clone();
                move |job| queue.lock().unwrap().push_back(job)
            }),
            || {},
        );
        let runtime = &computations.runtime;
        let program = runtime.input(::grap::lambda([], Value::record([])));
        let fuel = runtime.input(10000);
        let settings_input = runtime.input(settings(0.0));
        let (started, receive) = mpsc::channel();
        let (resume, resumed) = mpsc::channel();
        let resumed = Mutex::new(resumed);
        let rendered = Arc::new(Mutex::new(Vec::new()));
        let image = image_with(
            &computations,
            recording(&computations, program, fuel),
            settings_input.clone(),
            None,
            None,
            move |request, cancel| {
                started.send(cancel.clone()).unwrap();
                resumed.lock().unwrap().recv().unwrap();
                cancel.check()?;
                Ok(Ok(request))
            },
            {
                let rendered = rendered.clone();
                move |_, view, _, _, _| {
                    rendered.lock().unwrap().push(view);
                    Ok(Err(absent::with_reason(fidget::vocabulary::INVALID_FIELD)))
                }
            },
        );
        let read = || runtime.read(&image).unwrap();
        read();
        let job = queue.lock().unwrap().pop_front().unwrap();
        let worker = std::thread::spawn(job);
        let cancel = receive.recv_timeout(Duration::from_secs(5)).unwrap();
        let mut next = settings(60.0);
        settings_input.set(next.clone());
        read();
        assert!(
            cancel.check().is_ok(),
            "camera changes retain running preparation"
        );
        assert!(queue.lock().unwrap().is_empty());
        resume.send(()).unwrap();
        worker.join().unwrap();
        assert!(computations.tasks.poll());
        read();
        let job = queue.lock().unwrap().pop_front().unwrap();
        std::thread::spawn(job).join().unwrap();
        assert!(computations.tasks.poll());
        assert!(
            rendered.lock().unwrap()[0] == next.request.view(),
            "render uses the latest camera"
        );

        next.request.preview.objects[0].color = [1, 2, 3];
        settings_input.set(next.clone());
        read();
        let job = queue.lock().unwrap().pop_front().unwrap();
        let worker = std::thread::spawn(job);
        let cancel = receive.recv_timeout(Duration::from_secs(5)).unwrap();
        next.request.preview.objects[0].color = [4, 5, 6];
        settings_input.set(next);
        read();
        assert!(
            cancel.check().is_err(),
            "scene-input changes cancel preparation"
        );
        resume.send(()).unwrap();
        worker.join().unwrap();
        assert!(
            !computations.tasks.poll(),
            "cancelled preparation cannot publish"
        );
    }

    #[test]
    fn current_refinements_are_not_dimmed_like_stale_images() {
        use std::sync::mpsc;
        use std::time::Duration;

        let queue = Arc::new(Mutex::new(VecDeque::<Job>::new()));
        let computations = Computations::new(
            Executor::new({
                let queue = queue.clone();
                move |job| queue.lock().unwrap().push_back(job)
            }),
            || {},
        );
        let (published, receive) = mpsc::channel();
        let (resume, resumed) = mpsc::channel();
        let resumed = Mutex::new(resumed);
        let graph = Computation::with_render(
            &computations,
            ::grap::lambda([], Value::record([])),
            10000,
            settings(0.0),
            move |_, _, _, publish, progress| {
                let image = puri::ImageData {
                    data: vec![10, 20, 30, 255].into(),
                    format: peniko::ImageFormat::Rgba8,
                    alpha_type: peniko::ImageAlphaType::Alpha,
                    width: 1,
                    height: 1,
                };
                let image = fidget::raster::Frame {
                    image,
                    depth: vec![1.0].into(),
                    partial: false,
                };
                publish(Ok(image.clone()))?;
                progress(Progress {
                    completed: 3,
                    total: 10,
                });
                published.send(()).unwrap();
                resumed.lock().unwrap().recv().unwrap();
                Ok(Ok(image))
            },
        );
        let read = || computations.runtime.read(&graph.image).unwrap();
        let initial = read();
        assert!(initial.as_ref().as_ref().unwrap().image.is_none());
        let prepare = queue.lock().unwrap().pop_front().unwrap();
        std::thread::spawn(prepare).join().unwrap();
        assert!(computations.tasks.poll());
        read();
        let job = queue.lock().unwrap().pop_front().unwrap();
        let worker = std::thread::spawn(job);
        receive.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(computations.tasks.poll());
        let partial = read();
        let partial = &partial.as_ref().as_ref().unwrap();
        assert!(partial.pending && !partial.stale && partial.image.is_some());
        assert_eq!(
            partial.progress,
            Some(Progress {
                completed: 3,
                total: 10
            })
        );
        resume.send(()).unwrap();
        worker.join().unwrap();
        assert!(computations.tasks.poll());
        let final_image = read();
        let final_image = &final_image.as_ref().as_ref().unwrap();
        assert!(!final_image.pending && !final_image.stale);
        assert_eq!(final_image.progress, None);

        graph.settings.set(settings(30.0));
        let pending = read();
        let pending = &pending.as_ref().as_ref().unwrap();
        assert!(pending.pending && pending.stale && pending.image.is_some());
        assert_eq!(pending.progress, None);
    }

    #[test]
    fn implicit_playback_contains_only_stock_not_paths_or_visible_cutter() {
        let mut path = Recording::default();
        path.enter_tool(&crate::libraries::toolpath::cutter::Tool::ball(0.2, 0.4).unwrap());
        path.start_at(
            [-0.25, 0.0, 0.45],
            crate::libraries::toolpath::paths::Axis::Z,
        )
        .unwrap();
        for x in [-0.125, 0.0, 0.125, 0.25] {
            path.line_to([x, 0.0, 0.45]).unwrap();
        }
        path.leave_tool();
        let path = Arc::new(path);
        let mut settings = settings(0.0);
        let playback = playback(0.5);
        let expected = playback.remaining_stock(&path).unwrap().unwrap();
        settings.playback = Some(playback);
        let rendered = scene(SceneRequest::new(path, &settings)).unwrap();
        assert_eq!(
            rendered.len(),
            1,
            "only stock; displayed paths and cutter are meshes"
        );
        let stock = &rendered[0];
        assert_eq!(stock.color, expected.color);
        assert!(stock.tree == expected.tree);
    }
}
