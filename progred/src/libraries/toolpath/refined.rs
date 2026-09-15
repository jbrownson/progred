//! Independent mesh and implicit interpretations over one observed program.
//! Current implicit results refine the mesh; camera changes don't remesh it.

use super::{
    computation::{Outcome, recording},
    fidget as implicit, mesh, playback,
    vocabulary::*,
};
use crate::libraries::{absent, f64, fidget, layout, presentation};
use crate::{
    computations::Computations,
    display::{Layout, ProjectionInput},
};
use gid::Value;
use incremental::{Input, Memo};
use std::{cell::RefCell, rc::Rc};

#[cfg(test)]
mod tests;

pub(super) fn preview(
    context: &mut ::grap::Context,
    call: ::grap::Expression,
    environment: &::grap::Environment,
) -> Result<Value, ::grap::Halt> {
    implicit::preview_with(
        context,
        call,
        environment,
        PREVIEW_REFINED,
        fidget::mesh::preview,
    )
}

struct Computation {
    program: Input<Value>,
    fuel: Input<usize>,
    settings: Input<implicit::computation::Settings>,
    depth: Input<u8>,
    view: Memo<View>,
}

enum View {
    Mesh(Rc<Outcome<mesh::computation::ViewGeometry>>),
    Implicit(Rc<Outcome<implicit::computation::ViewImage>>),
}

impl Computation {
    fn new(
        computations: &Computations,
        program: Value,
        fuel: usize,
        settings: implicit::computation::Settings,
        depth: u8,
    ) -> Self {
        let runtime = &computations.runtime;
        let program = runtime.input(program);
        let fuel = runtime.input(fuel);
        let settings = runtime.input(settings);
        let depth = runtime.input(depth);
        let recording = recording(computations, program.clone(), fuel.clone());
        // A derived node drops the camera and image dimensions. Equality cuts
        // propagation here when only the view changes; no event classification.
        let mesh_settings = runtime.memo({
            let settings = settings.clone();
            move |read| {
                let settings = settings.read(read);
                Ok(mesh::computation::Settings {
                    shape: (&settings.request.preview).into(),
                    radius: settings.radius,
                    color: settings.color,
                    playback: settings.playback.clone(),
                })
            }
        });
        let geometry = mesh::computation::geometry(
            computations,
            recording.clone(),
            mesh_settings,
            depth.clone(),
        );
        // Skip the standalone renderer's two coarsest implicit levels: the mesh is
        // already a useful draft. Keep the remaining XY and depth refinements.
        let image = implicit::computation::image(computations, recording, settings.clone(), 512);
        let view = runtime.memo_by(
            move |read| {
                // Demand both independently. Mesh work is queued first, but an image
                // can become usable without a mesh having completed (and vice versa).
                let geometry = geometry.read(read)?;
                let image = image.read(read)?;
                Ok(match image.as_ref() {
                    Ok((image, _)) if image.stale || image.image.is_none() => View::Mesh(geometry),
                    // A current error is a result too; don't hide it behind old geometry.
                    _ => View::Implicit(image),
                })
            },
            |_, _| false,
        );
        Self {
            program,
            fuel,
            settings,
            depth,
            view,
        }
    }
}

pub(super) fn display(
    input: &ProjectionInput<'_, crate::Editor, crate::frame::Hovered>,
    renderer: &Rc<RefCell<fidget::mesh::Renderer>>,
) -> Option<Layout<crate::Editor, crate::frame::Hovered>> {
    let fields = input
        .value?
        .as_record()?
        .get(&PREVIEW_REFINED)?
        .as_record()?;
    let (model, depth) = fidget::mesh::read(fields.get(&presentation::vocabulary::VALUE)?)?;
    let program = fields.get(&PROGRAM)?.clone();
    let radius = f64::read(fields.get(&LINE_RADIUS)?)?;
    implicit::read_radius(radius)?;
    let color = implicit::read_color(fields.get(&fidget::vocabulary::COLOR)?)?;
    let fuel = super::read_fuel(f64::read(fields.get(&layout::vocabulary::FUEL)?)?)?;
    let playback = match fields.get(&PLAYBACK) {
        Some(value) => Some(playback::Settings::read(value)?),
        None => None,
    };
    let image_settings = implicit::computation::Settings {
        request: fidget::raster::Request::new(model.clone(), input.state, input.scale_factor)?,
        radius,
        color,
        playback,
    };
    let size = image_settings.request.size();
    let state = input.state.cloned();
    let scale = input.scale_factor;
    let renderer = renderer.clone();
    let drawing = Layout::program(Rc::new(move |context, build| {
        let local;
        let computations = match context.inputs.computations {
            Some(computations) => computations,
            None => {
                local = Computations::from_sources(context.inputs.sources);
                &local
            }
        };
        let computation = computations.at(context.inputs.view, context.path, || {
            Computation::new(
                computations,
                program.clone(),
                fuel,
                image_settings.clone(),
                depth,
            )
        });
        computation.program.set(program.clone());
        computation.fuel.set(fuel);
        computation.settings.set(image_settings.clone());
        computation.depth.set(depth);
        let view = computations.runtime.read(&computation.view);
        let drawing = view
            .as_ref()
            .map_err(|error| (::grap::memo::failure(*error), fuel))
            .and_then(|view| match view.as_ref() {
                View::Mesh(geometry) => {
                    let (geometry, fuel) = geometry.as_ref().as_ref().map_err(Clone::clone)?;
                    let image = fidget::mesh::image(
                        &geometry.geometry,
                        &model,
                        state.as_ref(),
                        scale,
                        &mut renderer.borrow_mut(),
                    )
                    .ok_or_else(|| {
                        (
                            absent::with_reason(fidget::vocabulary::INVALID_FIELD),
                            *fuel,
                        )
                    })?;
                    // Implicit refinement is pending, even when the fallback is current.
                    Ok(crate::display::overlay([image, crate::display::dim("…")]))
                }
                View::Implicit(image) => {
                    let (image, _) = image.as_ref().as_ref().map_err(Clone::clone)?;
                    let data = image
                        .image
                        .as_ref()
                        .expect("only current images refine the mesh");
                    let drawing = fidget::image_from_data(size, data.clone(), false);
                    Ok(if image.pending {
                        crate::display::overlay([drawing, crate::display::dim("…")])
                    } else {
                        drawing
                    })
                }
            });
        match drawing {
            Ok(drawing) => drawing.measure(context, build),
            Err((value, fuel)) => context.project.transient(context.text, build, value, fuel),
        }
    }));
    Some(fidget::interactive_volume(drawing, input))
}
