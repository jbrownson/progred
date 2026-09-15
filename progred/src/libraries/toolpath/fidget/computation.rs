use super::super::computation::{Outcome, Recorded, recording};
use super::*;
use crate::computations::Computations;
use incremental::background::Availability;
use incremental::{Input, Memo};

#[derive(Clone, PartialEq)]
pub(crate) struct Settings {
    pub request: fidget::raster::Request,
    pub radius: f64,
    pub color: [u8; 3],
    pub playback: Option<playback::Settings>,
}

#[derive(Clone, PartialEq)]
struct Request {
    path: Arc<Recording>,
    settings: Settings,
}

pub(crate) struct ViewImage {
    pub image: Option<puri::ImageData>,
    pub pending: bool,
    pub stale: bool,
}

pub(super) struct Computation {
    pub program: Input<Value>,
    pub fuel: Input<usize>,
    pub settings: Input<Settings>,
    pub image: Memo<Outcome<ViewImage>>,
}

impl Computation {
    pub fn new(
        computations: &Computations,
        program: Value,
        fuel: usize,
        settings: Settings,
    ) -> Self {
        let runtime = &computations.runtime;
        let program = runtime.input(program);
        let fuel = runtime.input(fuel);
        let settings = runtime.input(settings);
        let recording = recording(computations, program.clone(), fuel.clone());
        let image = image(computations, recording, settings.clone(), 128);
        Self {
            program,
            fuel,
            settings,
            image,
        }
    }

    #[cfg(test)]
    fn with_render(
        computations: &Computations,
        program: Value,
        fuel: usize,
        settings: Settings,
        render: impl Fn(
            Request,
            usize,
            &incremental::Cancellation,
            &mut dyn FnMut(Outcome<puri::ImageData>) -> Result<(), incremental::Error>,
        ) -> Result<Outcome<puri::ImageData>, incremental::Error>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        let runtime = &computations.runtime;
        let program = runtime.input(program);
        let fuel = runtime.input(fuel);
        let settings = runtime.input(settings);
        let recording = recording(computations, program.clone(), fuel.clone());
        let image = image_with_render(computations, recording, settings.clone(), render);
        Self {
            program,
            fuel,
            settings,
            image,
        }
    }
}

/// Use the same observed program as other interpretations. The caller chooses
/// the first image resolution independently of its fallback and scheduling.
pub(crate) fn image(
    computations: &Computations,
    recording: Memo<Recorded>,
    settings: Input<Settings>,
    first_max_edge: u32,
) -> Memo<Outcome<ViewImage>> {
    image_with_render(
        computations,
        recording,
        settings,
        move |request, fuel, cancel, publish| {
            let scene = scene(request, fuel);
            cancel.check()?;
            match scene {
                Ok((scene, fuel)) => Ok(scene
                    .render_software_progressive(first_max_edge, 4, cancel, &mut |image| {
                        publish(Ok((image, fuel)))
                    })?
                    .map(|image| (image, fuel))
                    .ok_or_else(|| (absent::with_reason(fidget::vocabulary::INVALID_FIELD), fuel))),
                Err(failure) => Ok(Err(failure)),
            }
        },
    )
}

fn image_with_render(
    computations: &Computations,
    recording: Memo<Recorded>,
    settings: Input<Settings>,
    render: impl Fn(
        Request,
        usize,
        &incremental::Cancellation,
        &mut dyn FnMut(Outcome<puri::ImageData>) -> Result<(), incremental::Error>,
    ) -> Result<Outcome<puri::ImageData>, incremental::Error>
    + Send
    + Sync
    + 'static,
) -> Memo<Outcome<ViewImage>> {
    let runtime = &computations.runtime;
    let prepared = runtime.memo({
        let settings = settings.clone();
        move |read| {
            let record = recording.read(read)?;
            let settings = settings.read(read);
            Ok(record.path().map(|_| {
                (
                    Request {
                        path: record.path.clone(),
                        settings: (*settings).clone(),
                    },
                    record.evaluation.remaining_fuel,
                )
            }))
        }
    });
    let worker =
        computations
            .tasks
            .memo_progressive(prepared.clone(), move |request, cancel, publish| {
                cancel.check()?;
                match request {
                    Ok((request, fuel)) => render(request, fuel, cancel, publish),
                    Err(failure) => Ok(Err(failure)),
                }
            });
    runtime.memo_by(
        move |read| {
            // Invalid current programs replace old images immediately, without a worker round-trip.
            let prepared = prepared.read(read)?;
            let availability = worker.read(read)?;
            let fuel = match &*prepared {
                Ok((_, fuel)) => *fuel,
                Err(failure) => return Ok(Err(failure.clone())),
            };
            Ok(match &*availability {
                Availability::Ready(image) | Availability::Refining(image) => image
                    .as_ref()
                    .as_ref()
                    .map(|(image, fuel)| {
                        (
                            ViewImage {
                                image: Some(image.clone()),
                                pending: matches!(&*availability, Availability::Refining(_)),
                                stale: false,
                            },
                            *fuel,
                        )
                    })
                    .map_err(Clone::clone),
                Availability::Pending { previous } => Ok((
                    ViewImage {
                        image: previous
                            .as_deref()
                            .and_then(|result| result.as_ref().ok())
                            .map(|(image, _)| image.clone()),
                        pending: true,
                        stale: previous.is_some(),
                    },
                    fuel,
                )),
            })
        },
        |_, _| false,
    )
}

fn scene(request: Request, fuel: usize) -> Outcome<fidget::raster::Request> {
    let Request { path, settings } = request;
    let mut request = settings.request;
    let invalid = || (absent::with_reason(INVALID_INPUT), fuel);
    let mut tubes = Tubes::new(settings.radius).ok_or_else(invalid)?;
    tubes
        .style(settings.radius, settings.color)
        .map_err(|_| invalid())?;
    if let Some(playback) = settings.playback {
        playback
            .draw(&path, &mut tubes, settings.radius, settings.color)
            .map_err(|_| invalid())?;
        if let Some(stock) = playback.remaining_stock(&path).map_err(|_| invalid())? {
            request.preview.objects = vec![stock];
        }
    } else {
        path.replay(&mut tubes).map_err(|_| invalid())?;
    }
    request.preview.objects.splice(0..0, tubes.scene());
    if request.preview.objects.len() > usize::from(u16::MAX) + 1 {
        Err((absent::with_reason(fidget::vocabulary::INVALID_SCENE), fuel))
    } else {
        Ok((request, fuel))
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
                crate::libraries::toolpath::cutter::vocabulary::TOOL,
                crate::libraries::toolpath::cutter::Tool::ball(0.2, 0.4)
                    .unwrap()
                    .value(),
            ),
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
        let rendered = Arc::new(Mutex::new(Vec::new()));
        let graph = Computation::with_render(&computations, program, 10000, settings(0.0), {
            let rendered = rendered.clone();
            move |request, fuel, cancel, _publish| {
                cancel.check()?;
                let mut rendered = rendered.lock().unwrap();
                rendered.push(request);
                Ok(Ok((
                    puri::ImageData {
                        data: vec![rendered.len() as u8, 0, 0, 255].into(),
                        format: peniko::ImageFormat::Rgba8,
                        alpha_type: peniko::ImageAlphaType::Alpha,
                        width: 1,
                        height: 1,
                    },
                    fuel,
                )))
            }
        });
        let read = || computations.runtime.read(&graph.image).unwrap();
        let finish = || {
            let job = queue.lock().unwrap().pop_front().unwrap();
            std::thread::spawn(job).join().unwrap();
            assert!(computations.tasks.poll());
        };
        let first = read();
        let first = &first.as_ref().as_ref().unwrap().0;
        assert!(first.pending && first.image.is_none());
        finish();
        let ready = read();
        assert!(!ready.as_ref().as_ref().unwrap().0.pending);
        assert!(Rc::ptr_eq(&ready, &read()));

        doc.cells.set_value(new_cell_id(), f64::value(9.0));
        computations.begin(Rc::new(doc.clone()), stack.libraries.clone());
        assert!(
            Rc::ptr_eq(&ready, &read()),
            "unrelated edits retain the image"
        );
        graph.settings.set(settings(30.0));
        let pending = read();
        let pending = &pending.as_ref().as_ref().unwrap().0;
        assert!(pending.pending);
        assert_eq!(pending.image.as_ref().unwrap().data.data()[0], 1);
        graph.settings.set(settings(60.0));
        assert!(read().as_ref().as_ref().unwrap().0.pending);
        assert_eq!(queue.lock().unwrap().len(), 1);
        finish();
        assert!(!read().as_ref().as_ref().unwrap().0.pending);
        {
            let rendered = rendered.lock().unwrap();
            assert_eq!(
                rendered.len(),
                2,
                "superseded camera request wasn't rendered"
            );
            assert!(Arc::ptr_eq(&rendered[0].path, &rendered[1].path));
            assert!(rendered[1].settings == settings(60.0));
        }

        doc.cells.set_value(endpoint, f64::value(0.75));
        computations.begin(Rc::new(doc.clone()), stack.libraries.clone());
        assert!(read().as_ref().as_ref().unwrap().0.pending);
        finish();
        read();
        {
            let rendered = rendered.lock().unwrap();
            assert!(!Arc::ptr_eq(&rendered[1].path, &rendered[2].path));
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
            move |_, fuel, _, publish| {
                let image = puri::ImageData {
                    data: vec![10, 20, 30, 255].into(),
                    format: peniko::ImageFormat::Rgba8,
                    alpha_type: peniko::ImageAlphaType::Alpha,
                    width: 1,
                    height: 1,
                };
                publish(Ok((image.clone(), fuel)))?;
                published.send(()).unwrap();
                resumed.lock().unwrap().recv().unwrap();
                Ok(Ok((image, fuel)))
            },
        );
        let read = || computations.runtime.read(&graph.image).unwrap();
        let initial = read();
        assert!(initial.as_ref().as_ref().unwrap().0.image.is_none());
        let job = queue.lock().unwrap().pop_front().unwrap();
        let worker = std::thread::spawn(job);
        receive.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(computations.tasks.poll());
        let partial = read();
        let partial = &partial.as_ref().as_ref().unwrap().0;
        assert!(partial.pending && !partial.stale && partial.image.is_some());
        resume.send(()).unwrap();
        worker.join().unwrap();
        assert!(computations.tasks.poll());
        let final_image = read();
        let final_image = &final_image.as_ref().as_ref().unwrap().0;
        assert!(!final_image.pending && !final_image.stale);

        graph.settings.set(settings(30.0));
        let pending = read();
        let pending = &pending.as_ref().as_ref().unwrap().0;
        assert!(pending.pending && pending.stale && pending.image.is_some());
    }

    #[test]
    fn implicit_playback_uses_shared_stock_sweeps_and_groups_connected_segments() {
        let mut path = Recording::default();
        path.start_at(
            [-0.25, 0.0, 0.45],
            crate::libraries::toolpath::paths::Axis::Z,
        )
        .unwrap();
        for x in [-0.125, 0.0, 0.125, 0.25] {
            path.line_to([x, 0.0, 0.45]).unwrap();
        }
        let path = Arc::new(path);
        let mut settings = settings(0.0);
        let playback = playback(0.5);
        let expected = playback.remaining_stock(&path).unwrap().unwrap();
        settings.playback = Some(playback);
        let rendered = scene(Request { path, settings }, 100).unwrap().0;
        assert_eq!(
            rendered.preview.objects.len(),
            3,
            "one connected path, cutter, stock"
        );
        assert_eq!(rendered.preview.objects[0].color, [20, 150, 230]);
        assert_eq!(rendered.preview.objects[1].color, [225, 94, 58]);
        let stock = &rendered.preview.objects[2];
        assert_eq!(stock.color, expected.color);
        assert!(stock.tree == expected.tree);
    }
}
