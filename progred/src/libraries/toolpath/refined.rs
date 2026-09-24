//! Mesh-first implicit refinement over one observed program.
//! Current implicit results refine the mesh; camera changes don't remesh it.

use super::{
    computation::{Outcome, recording},
    fidget as implicit, mesh, playback,
    vocabulary::*,
};
#[cfg(test)]
use crate::libraries::f64;
use crate::libraries::{absent, fidget, layout, presentation};
use crate::{
    computations::Computations,
    display::{Layout, ProjectionInput},
};
use ::grap::RuntimeValue;
#[cfg(test)]
use gid::Value;
use incremental::{Input, Memo};
use std::rc::Rc;

#[cfg(test)]
mod tests;

pub(super) fn preview(
    context: &mut ::grap::Context,
    call: &::grap::Expression,
    environment: &::grap::Environment,
) -> Result<RuntimeValue, ::grap::Halt> {
    implicit::preview_with(
        context,
        call,
        environment,
        PREVIEW_REFINED,
        fidget::mesh::preview,
    )
}

struct Computation {
    program: Input<RuntimeValue>,
    fuel: Input<usize>,
    settings: Input<implicit::computation::Settings>,
    depth: Input<u8>,
    view: Memo<View>,
}

enum View {
    Mesh(
        Rc<Outcome<mesh::computation::ViewGeometry>>,
        Option<incremental::background::Progress>,
    ),
    Implicit(
        Rc<Outcome<implicit::computation::ViewImage>>,
        Rc<Outcome<mesh::computation::ViewGeometry>>,
    ),
}

impl Computation {
    fn new(
        computations: &Computations,
        program: RuntimeValue,
        fuel: usize,
        settings: implicit::computation::Settings,
        depth: u8,
        permitted: Memo<bool>,
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
        let mesh_ready = runtime.memo({
            let geometry = geometry.clone();
            move |read| {
                Ok(geometry
                    .read(read)?
                    .as_ref()
                    .as_ref()
                    .is_ok_and(|geometry| !geometry.surface_pending))
            }
        });
        // The mesh supplies the draft; completed final-quality tiles replace it.
        let image = implicit::computation::image(
            computations,
            recording,
            settings.clone(),
            fidget::raster::Passes::Final,
            Some(mesh_ready),
            Some(permitted),
        );
        let view = runtime.memo_by(
            move |read| {
                // Read both even while meshing: waiting preparation cancels any
                // old implicit job without scheduling a new one ahead of the mesh.
                let geometry = geometry.read(read)?;
                let image = image.read(read)?;
                if geometry.is_err() {
                    return Ok(View::Mesh(geometry, None));
                }
                Ok(match image.as_ref() {
                    Ok(image) if image.stale || image.image.is_none() => {
                        View::Mesh(geometry, image.progress)
                    }
                    // A current error is a result too; don't hide it behind old geometry.
                    _ => View::Implicit(image, geometry),
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
    input: &ProjectionInput<'_, crate::Editor, crate::frame::Hovered, RuntimeValue>,
) -> Option<Layout<crate::Editor, crate::frame::Hovered>> {
    let fields = input.value?.field(PREVIEW_REFINED)?;
    let (model, depth) =
        fidget::mesh::read(fields.field(presentation::vocabulary::VALUE)?.as_value())?;
    let program = fields.field(PROGRAM)?;
    let radius = fields.field(LINE_RADIUS)?.as_f64()?;
    implicit::read_radius(radius)?;
    let color = implicit::read_color(fields.field(fidget::vocabulary::COLOR)?.as_value())?;
    let fuel = super::read_fuel(fields.field(layout::vocabulary::FUEL)?.as_f64()?)?;
    let playback = match fields.field(PLAYBACK) {
        Some(value) => Some(playback::Settings::read(value.as_value())?),
        None => None,
    };
    let image_settings = implicit::computation::Settings {
        request: fidget::raster::Request::new(model.clone(), input.state, input.scale_factor)?,
        radius,
        color,
        playback,
    };
    let state = input.state.cloned();
    let scale = input.scale_factor;
    let drawing = Layout::program(Rc::new(move |context, build| {
        let local;
        let computations = match context.inputs.computations {
            Some(computations) => computations,
            None => {
                local = Computations::from_sources(context.inputs.sources);
                &local
            }
        };
        let interaction =
            fidget::interaction::Interaction::at(computations, context.inputs.view, context.path);
        let computation = computations.at(context.inputs.view, context.path, || {
            Computation::new(
                computations,
                program.clone(),
                fuel,
                image_settings.clone(),
                depth,
                interaction.permitted.clone(),
            )
        });
        computation
            .program
            .set_by(program.clone(), RuntimeValue::same_result);
        computation.fuel.set(fuel);
        computation.settings.set(image_settings.clone());
        computation.depth.set(depth);
        let view = computations.runtime.read(&computation.view);
        let drawing = view
            .as_ref()
            .map_err(|error| ::grap::memo::failure(*error))
            .and_then(|view| match view.as_ref() {
                View::Mesh(geometry, progress) => {
                    let geometry = geometry.as_ref().as_ref().map_err(Clone::clone)?;
                    let image =
                        fidget::mesh::drawing(&geometry.geometry, &model, state.as_ref(), scale)
                            .ok_or_else(|| {
                                absent::with_reason(fidget::vocabulary::INVALID_FIELD)
                            })?;
                    // Implicit refinement is pending, even when the fallback is current.
                    Ok(implicit::progress_bar(image, *progress))
                }
                View::Implicit(image, geometry) => {
                    let image = image.as_ref().as_ref().map_err(Clone::clone)?;
                    let data = image
                        .image
                        .as_ref()
                        .expect("only current images refine the mesh");
                    let geometry = geometry.as_ref().as_ref().map_err(Clone::clone)?;
                    let drawing = fidget::mesh::drawing_surface(
                        &geometry.geometry,
                        Some(fidget::mesh::Surface {
                            frame: data.clone(),
                            mesh_start: geometry.surface_start,
                        }),
                        &model,
                        state.as_ref(),
                        scale,
                    )
                    .ok_or_else(|| absent::with_reason(fidget::vocabulary::INVALID_FIELD))?;
                    Ok(if image.pending {
                        implicit::progress_bar(drawing, image.progress)
                    } else {
                        drawing
                    })
                }
            });
        match drawing {
            Ok(drawing) => drawing.measure(context, build),
            Err(value) => {
                crate::display::at([gid::Step::Key(presentation::vocabulary::RESULT)], &value)
                    .measure(context, build)
            }
        }
    }));
    Some(fidget::interactive_volume(drawing, input))
}
