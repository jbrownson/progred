//! Stream path segments into triangles alongside a meshed Fidget model.

use super::{playback, vocabulary::*};
use crate::computations::memo;
use crate::display::host::{Host, Layout, ProjectionInput};
use crate::libraries::{f64, fidget, layout, presentation};
use ::grap::RuntimeValue;
use cam::field::{coordinate, read_radius};
use cam::paths::*;
use nalgebra::Vector3;
use puri::mesh::{Geometry, Mesh, Normal, Vertex};
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
    super::runtime_result(super::fidget::preview_with(
        context,
        call,
        environment,
        PREVIEW_MESH,
        fidget::mesh::preview,
    ))
}

pub(super) fn display<W: Host>(input: &ProjectionInput<'_, W, RuntimeValue>) -> Option<Layout<W>> {
    let fields = input.value?.field(PREVIEW_MESH)?;
    let (model, depth) =
        fidget::mesh::read(fields.field(presentation::vocabulary::VALUE)?.as_value())?;
    let program = fields.field(PROGRAM)?;
    let radius = fields.field(LINE_RADIUS)?.as_f64()?;
    let color =
        crate::libraries::color::read_opaque(fields.field(fidget::vocabulary::COLOR)?.as_value())?;
    read_radius(radius)?;
    let fuel = ::grap::f64::to_usize(fields.field(layout::vocabulary::FUEL)?.as_f64()?)?;
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
        let computations = context.viewing().computations();
        let computation = context.site().memo(computations, || {
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
            .map_err(|error| memo::failure(*error))
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
                .ok_or_else(|| ::grap::absent::value(fidget::vocabulary::INVALID_FIELD))
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
