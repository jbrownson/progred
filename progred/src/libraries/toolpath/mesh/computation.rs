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
    pub geometry: Mesh,
    pub surface_start: usize,
    pub awaiting_first_surface: bool,
    /// True even when `geometry` includes a retained, outdated stock surface.
    pub surface_pending: bool,
}

#[derive(Clone, PartialEq)]
struct Surface {
    model: fidget::mesh::Shape,
    removal: Option<(Arc<Recording>, playback::Settings)>,
    depth: u8,
}

pub(super) struct Computation {
    pub program: Input<RuntimeValue>,
    pub fuel: Input<usize>,
    pub settings: Input<Settings>,
    pub depth: Input<u8>,
    pub geometry: Memo<Outcome<ViewGeometry>>,
}

impl Computation {
    pub fn new(
        computations: &Computations,
        program: RuntimeValue,
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
    paths: Memo<Outcome<Mesh>>,
    surface: AsyncMemo<Outcome<Surface>, Outcome<Mesh>>,
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
                    model: settings.shape.clone(),
                    removal: settings
                        .playback
                        .as_ref()
                        .filter(|playback| playback.stock_color().is_some())
                        .map(|playback| (record.path.clone(), playback.clone())),
                    depth,
                }))
            }
        });
        // Paths and cutter redraw immediately. Let each admitted surface finish,
        // then catch up to the latest inputs, without a queue of drag samples.
        let surface = tasks.memo_conflated(prepared, |prepared, cancel| {
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
                .map(|()| geometry.into())
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

fn combine(paths: &Outcome<Mesh>, surface: &Availability<Outcome<Mesh>>) -> Outcome<ViewGeometry> {
    let paths = paths.as_ref().map_err(Clone::clone)?;
    let mut geometry = paths.as_ref().clone();
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
        surface_start: paths.indices.len(),
        geometry: geometry.into(),
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
    if let Some((path, playback)) = &request.removal
        && let Some(stock) = playback
            .remaining_stock(path)
            .map_err(|_| absent::with_reason(INVALID_INPUT))?
    {
        shape.objects = vec![stock];
    }
    Ok(shape)
}

fn path_geometry(record: &Recorded, settings: &Settings) -> Outcome<Mesh> {
    paths(
        record,
        settings.radius,
        settings.color,
        settings.playback.as_ref(),
    )
}

pub(crate) fn paths(
    record: &Recorded,
    radius: f64,
    color: [u8; 3],
    playback: Option<&playback::Settings>,
) -> Outcome<Mesh> {
    let path = record.path()?;
    let invalid = || absent::with_reason(INVALID_INPUT);
    let mut tubes = tubes::Tubes::new(radius, color).ok_or_else(invalid)?;
    if let Some(playback) = playback {
        playback
            .draw(path, &mut tubes, radius, color)
            .map_err(|_| invalid())?;
    } else {
        path.replay(&mut tubes).map_err(|_| invalid())?;
    }
    Ok(tubes.geometry.into())
}

#[cfg(test)]
mod tests;
