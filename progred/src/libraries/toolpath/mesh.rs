//! Stream path segments into triangles alongside a meshed Fidget model.

use super::{fidget::coordinate, paths::*, playback, vocabulary::*};
use crate::display::{Layout, ProjectionInput};
use crate::libraries::{absent, f64, fidget, layout, presentation};
use ::grap::RuntimeValue;
use fidget::mesh::{Geometry, Mesh, Normal, Vertex};
#[cfg(test)]
use gid::Value;
use nalgebra::Vector3;
use std::rc::Rc;

pub(super) mod computation;
#[cfg(test)]
mod tests;
mod tubes;

pub(super) fn preview(
    context: &mut ::grap::Context,
    call: &::grap::Expression,
    environment: &::grap::Environment,
) -> Result<RuntimeValue, ::grap::Halt> {
    super::fidget::preview_with(
        context,
        call,
        environment,
        PREVIEW_MESH,
        fidget::mesh::preview,
    )
}

pub(super) fn display(
    input: &ProjectionInput<'_, crate::Editor, crate::frame::Hovered, RuntimeValue>,
) -> Option<Layout<crate::Editor, crate::frame::Hovered>> {
    let fields = input.value?.field(PREVIEW_MESH)?;
    let (model, depth) =
        fidget::mesh::read(fields.field(presentation::vocabulary::VALUE)?.as_value())?;
    let program = fields.field(PROGRAM)?;
    let radius = fields.field(LINE_RADIUS)?.as_f64()?;
    let color = super::fidget::read_color(fields.field(fidget::vocabulary::COLOR)?.as_value())?;
    super::fidget::read_radius(radius)?;
    let fuel = super::read_fuel(fields.field(layout::vocabulary::FUEL)?.as_f64()?)?;
    let playback = match fields.field(PLAYBACK) {
        Some(value) => Some(playback::Settings::read(value.as_value())?),
        None => None,
    };
    let state = input.state.cloned();
    let scale = input.scale_factor;
    let settings = computation::Settings {
        shape: (&model).into(),
        radius,
        color,
        playback,
    };
    let drawing = Layout::program(Rc::new(move |context, build| {
        let local;
        let computations = match context.inputs.computations {
            Some(computations) => computations,
            None => {
                local = crate::computations::Computations::from_sources(context.inputs.sources);
                &local
            }
        };
        let computation = computations.at(context.inputs.view, context.path, || {
            computation::Computation::new(
                computations,
                program.clone(),
                fuel,
                settings.clone(),
                depth,
            )
        });
        computation
            .program
            .set_by(program.clone(), RuntimeValue::same_result);
        computation.fuel.set(fuel);
        computation.settings.set(settings.clone());
        computation.depth.set(depth);
        let geometry = computations.runtime.read(&computation.geometry);
        let result = geometry
            .as_ref()
            .map_err(|error| ::grap::memo::failure(*error))
            .and_then(|geometry| geometry.as_ref().as_ref().map_err(Clone::clone));
        let drawing = result.and_then(|geometry| {
            fidget::mesh::drawing(&geometry.geometry, &model, state.as_ref(), scale)
                .map(|drawing| {
                    if geometry.awaiting_first_surface {
                        crate::display::overlay([drawing, crate::display::dim("…")])
                    } else {
                        drawing
                    }
                })
                .ok_or_else(|| absent::with_reason(fidget::vocabulary::INVALID_FIELD))
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
