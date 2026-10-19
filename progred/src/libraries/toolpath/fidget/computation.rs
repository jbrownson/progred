use super::super::computation::{Outcome, Program, Recorded};
use super::super::mesh::computation::{PathStyle, path_tubes};
use super::*;
use crate::computations::Computations;
use fidget::raster::{Passes, SoftwareScene, ViewRequest};
use incremental::background::{Availability, Progress, Tasks};
use incremental::{Input, Memo};
use puri::mesh::{DepthImage, Mesh};

#[derive(Clone, PartialEq)]
pub(crate) struct Settings {
    pub request: fidget::raster::Request,
    pub style: PathStyle,
}

#[derive(Clone, PartialEq)]
enum SceneRequest {
    Model(Vec<volume::SceneObject>),
    Stock(Arc<Recording>, playback::Removal),
}

impl SceneRequest {
    fn new(path: Arc<Recording>, settings: &Settings) -> Self {
        match settings
            .style
            .playback
            .as_ref()
            .and_then(playback::Settings::removal)
        {
            Some(removal) => Self::Stock(path, removal),
            None => Self::Model(settings.request.preview.objects.clone()),
        }
    }
}

pub(crate) struct ViewImage {
    pub image: Option<DepthImage>,
    pub pending: bool,
    pub stale: bool,
    pub progress: Option<Progress>,
}

pub(super) struct Computation {
    pub program: Program,
    pub settings: Input<Settings>,
    pub image: Memo<Outcome<ViewImage>>,
    pub paths: Memo<Outcome<Mesh>>,
}

impl Computation {
    pub fn new(
        computations: &Computations,
        tasks: &Tasks,
        program: RuntimeValue,
        fuel: usize,
        settings: Settings,
        permitted: Memo<bool>,
    ) -> Self {
        Self::with_image(
            computations,
            program,
            fuel,
            settings,
            |recording, settings| {
                image(
                    computations,
                    tasks,
                    recording,
                    settings,
                    Passes::Progressive {
                        first_max_edge: 128,
                    },
                    None,
                    permitted,
                )
            },
        )
    }

    /// The paths' tubes beside the image that `image` makes of the recording
    /// in the settings.
    fn with_image(
        computations: &Computations,
        program: RuntimeValue,
        fuel: usize,
        settings: Settings,
        image: impl FnOnce(Memo<Recorded>, Input<Settings>) -> Memo<Outcome<ViewImage>>,
    ) -> Self {
        let program = Program::new(computations, program, fuel);
        let settings = computations.runtime.input(settings);
        let style = computations.runtime.memo({
            let settings = settings.clone();
            move |read| Ok(settings.read(read).style.clone())
        });
        Self {
            paths: path_tubes(&computations.runtime, program.recording.clone(), style),
            image: image(program.recording.clone(), settings.clone()),
            program,
            settings,
        }
    }
}

/// Use the same observed program as other interpretations. The caller chooses
/// the quality passes independently of its fallback and scheduling.
/// A readiness dependency gates background work; becoming unready cancels it.
pub(crate) fn image(
    computations: &Computations,
    tasks: &Tasks,
    recording: Memo<Recorded>,
    settings: Input<Settings>,
    passes: Passes,
    ready: Option<Memo<bool>>,
    permitted: Memo<bool>,
) -> Memo<Outcome<ViewImage>> {
    image_with(
        computations,
        tasks,
        recording,
        settings,
        ready,
        permitted,
        |request, cancel| {
            Ok(match scene(request) {
                Ok(objects) => SoftwareScene::new(&objects, cancel)?
                    .ok_or_else(|| ::grap::absent::value(fidget::vocabulary::INVALID_FIELD)),
                Err(failure) => Err(failure),
            })
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
                .ok_or_else(|| ::grap::absent::value(fidget::vocabulary::INVALID_FIELD)))
        },
    )
}

/// The two workers have independent generations: a new camera cancels pixels,
/// while scene preparation survives until its geometry inputs actually change.
fn image_with<S: Send + Sync + 'static>(
    computations: &Computations,
    tasks: &Tasks,
    recording: Memo<Recorded>,
    settings: Input<Settings>,
    ready: Option<Memo<bool>>,
    permitted: Memo<bool>,
    prepare: impl Fn(SceneRequest, &incremental::Cancellation) -> Result<Outcome<S>, incremental::Error>
    + Send
    + Sync
    + 'static,
    render: impl Fn(
        &S,
        ViewRequest,
        &incremental::Cancellation,
        &mut (dyn FnMut(Outcome<DepthImage>) -> Result<(), incremental::Error> + Send),
        &(dyn Fn(Progress) + Sync),
    ) -> Result<Outcome<DepthImage>, incremental::Error>
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
            Ok(match &ready {
                Some(ready) if !*ready.read(read)? => None,
                _ => Some((*requested_scene.read(read)?).clone()),
            })
        }
    });
    let compiled = tasks.memo_reporting_when_ready(scene_input, move |request, cancel, _, _| {
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
    let worker = tasks.memo_reporting_with_start_condition(
        worker_input,
        Some(permitted),
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
            let failure = match &*requested_scene {
                Err(failure) => Some(failure.clone()),
                Ok(_) => match &*compiled.read(read)? {
                    Availability::Ready(scene) => scene.as_ref().as_ref().err().cloned(),
                    _ => None,
                },
            };
            Ok(match failure {
                Some(failure) => Err(failure),
                None => match &*availability {
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
                },
            })
        },
        |_, _| false,
    )
}

fn scene(request: SceneRequest) -> Outcome<Vec<volume::SceneObject>> {
    match request {
        SceneRequest::Model(objects) => Ok(objects),
        SceneRequest::Stock(path, removal) => removal
            .remaining(&path)
            .map(|stock| vec![stock])
            .map_err(|_| ::grap::absent::value(INVALID_INPUT)),
    }
}

#[cfg(test)]
mod tests;
