use super::super::computation::{Outcome, Program, Recorded};
use super::*;
use crate::computations::Computations;
use incremental::background::{AsyncMemo, Availability, Tasks};
use incremental::{Input, Memo, Runtime};
use std::sync::Arc;

/// How a preview draws its recorded paths: their radius and color, and the
/// playback, if any, which hides what's cut, places the cutter, and cuts the
/// stock.
#[derive(Clone, PartialEq)]
pub(crate) struct PathStyle {
    pub radius: f64,
    pub color: [u8; 3],
    pub playback: Option<playback::Settings>,
}

#[derive(Clone, PartialEq)]
pub(crate) struct Settings {
    pub shape: fidget::mesh::Shape,
    pub style: PathStyle,
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
    removal: Option<(Arc<Recording>, playback::Removal)>,
    depth: u8,
}

pub(super) struct Computation {
    pub program: Program,
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
        let program = Program::new(computations, program, fuel);
        let settings = runtime.input(settings);
        let depth = runtime.input(depth);
        let geometry = geometry(
            computations,
            program.recording.clone(),
            runtime.memo({
                let settings = settings.clone();
                move |read| Ok((*settings.read(read)).clone())
            }),
            depth.clone(),
        );
        Self {
            program,
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
    let runtime = &computations.runtime;
    let style = runtime.memo({
        let settings = settings.clone();
        move |read| Ok(settings.read(read)?.style.clone())
    });
    combined(
        runtime,
        path_tubes(runtime, recording.clone(), style),
        surface(runtime, &computations.tasks, recording, settings, depth),
    )
}

/// The model's surface, or the stock's once playback cuts it, meshed off the
/// editor thread.
fn surface(
    runtime: &Runtime,
    tasks: &Tasks,
    recording: Memo<Recorded>,
    settings: Memo<Settings>,
    depth: Input<u8>,
) -> AsyncMemo<Outcome<Surface>, Outcome<Mesh>> {
    let prepared = runtime.memo(move |read| {
        let record = recording.read(read)?;
        let settings = settings.read(read)?;
        let depth = *depth.read(read);
        Ok(record.path().map(|_| Surface {
            model: settings.shape.clone(),
            removal: settings
                .style
                .playback
                .as_ref()
                .and_then(playback::Settings::removal)
                .map(|removal| (record.path.clone(), removal)),
            depth,
        }))
    });
    // Paths and cutter redraw immediately. Let each admitted surface finish,
    // then catch up to the latest inputs, without a queue of drag samples.
    tasks.memo_conflated(prepared, |prepared, cancel| match prepared {
        Ok(request) => {
            cancel.check()?;
            let shape = remaining_shape(&request);
            cancel.check()?;
            match shape {
                Ok(shape) => {
                    let mut geometry = Geometry::default();
                    Ok(shape
                        .append(&mut geometry, request.depth, cancel)?
                        .map(|()| geometry.into())
                        .ok_or_else(|| ::grap::absent::value(fidget::vocabulary::INVALID_FIELD)))
                }
                Err(failure) => Ok(Err(failure)),
            }
        }
        Err(failure) => Ok(Err(failure)),
    })
}

/// The paths' tubes with the surface after them, which shows as updating
/// while it's remeshed.
fn combined(
    runtime: &Runtime,
    paths: Memo<Outcome<Mesh>>,
    surface: AsyncMemo<Outcome<Surface>, Outcome<Mesh>>,
) -> Memo<Outcome<ViewGeometry>> {
    runtime.memo_by(
        move |read| {
            let paths = paths.read(read)?;
            let surface = surface.read(read)?;
            Ok(combine(&paths, &surface))
        },
        |_, _| false,
    )
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
            .ok_or_else(|| ::grap::absent::value(INVALID_INPUT))?;
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
    if let Some((path, removal)) = &request.removal {
        shape.objects = vec![
            removal
                .remaining(path)
                .map_err(|_| ::grap::absent::value(INVALID_INPUT))?,
        ];
    }
    Ok(shape)
}

/// The tubes of the recorded paths and the cutter, drawn again only when the
/// recording or `style` changes.
pub(crate) fn path_tubes(
    runtime: &Runtime,
    recording: Memo<Recorded>,
    style: Memo<PathStyle>,
) -> Memo<Outcome<Mesh>> {
    runtime.memo_by(
        move |read| {
            let record = recording.read(read)?;
            let style = style.read(read)?;
            Ok(paths(&record, &style))
        },
        |_, _| false,
    )
}

pub(crate) fn paths(record: &Recorded, style: &PathStyle) -> Outcome<Mesh> {
    let path = record.path()?;
    let invalid = || ::grap::absent::value(INVALID_INPUT);
    let mut tubes = tubes::Tubes::new(style.radius, style.color).ok_or_else(invalid)?;
    if let Some(playback) = &style.playback {
        playback
            .draw(path, &mut tubes, style.radius, style.color)
            .map_err(|_| invalid())?;
    } else {
        path.replay(&mut tubes).map_err(|_| invalid())?;
    }
    Ok(tubes.geometry.into())
}

#[cfg(test)]
mod tests;
