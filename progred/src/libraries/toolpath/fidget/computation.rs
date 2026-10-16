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
    pub program: Input<RuntimeValue>,
    pub fuel: Input<usize>,
    pub settings: Input<Settings>,
    pub image: Memo<Outcome<ViewImage>>,
    pub paths: Memo<Outcome<fidget::mesh::Mesh>>,
}

impl Computation {
    pub fn new(
        computations: &Computations,
        program: RuntimeValue,
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
        program: RuntimeValue,
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
mod tests;
