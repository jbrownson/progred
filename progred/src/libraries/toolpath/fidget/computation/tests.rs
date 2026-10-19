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
    let graph = Computation::with_render(&computations, program.into(), 10000, settings(0.0), {
        let rendered = rendered.clone();
        move |scene, view, cancel, _publish, _progress| {
            cancel.check()?;
            let mut rendered = rendered.lock().unwrap();
            rendered.push((scene, view));
            Ok(Ok(fidget::raster::Frame {
                image: peniko::ImageData {
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
    let program = runtime.input(::grap::lambda([], Value::record([])).into());
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
                Ok(Err(::grap::absent::value(
                    fidget::vocabulary::INVALID_FIELD,
                )))
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
        ::grap::lambda([], Value::record([])).into(),
        10000,
        settings(0.0),
        move |_, _, _, publish, progress| {
            let image = peniko::ImageData {
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
