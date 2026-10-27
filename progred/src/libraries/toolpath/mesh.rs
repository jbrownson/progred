//! Stream path segments into triangles alongside a meshed Fidget model.

use super::{playback, vocabulary::*};
use crate::computations::memo;
use crate::display::host::{Host, Layout, ProjectionInput};
use crate::libraries::{f64, fidget, presentation};
use ::grap::RuntimeValue;
use cam::field::coordinate;
use cam::paths::*;
use nalgebra::Vector3;
use puri::mesh::{Geometry, Mesh, Normal, Vertex};
use std::rc::Rc;

pub(super) mod computation;
#[cfg(test)]
#[path = "tests/mesh.rs"]
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
        kind::PATHS_MESH_PREVIEW,
        fidget::mesh::preview,
    ))
}

pub(super) fn display<W: Host>(input: &ProjectionInput<'_, W, RuntimeValue>) -> Option<Layout<W>> {
    let fields = input.value?;
    let (model, depth) =
        fidget::mesh::read(fields.field(presentation::vocabulary::VALUE)?.as_value())?;
    let super::fidget::PathPreview {
        program,
        fuel,
        style,
    } = super::fidget::PathPreview::read(&fields)?;
    let state = input.state.cloned();
    let scale = input.scale_factor;
    let settings = computation::Settings {
        shape: (&model).into(),
        style,
    };
    let drawing = Layout::program(Rc::new(move |context, build| {
        let computations = context.viewing().computations();
        match &computations.tasks {
            None => super::background_unavailable(&model),
            Some(tasks) => {
                let computation = context.site().memo(computations, || {
                    computation::Computation::new(
                        computations,
                        tasks,
                        program.clone(),
                        fuel,
                        settings.clone(),
                        depth,
                    )
                });
                computation.program.update(program.clone(), fuel);
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
                drawing.unwrap_or_else(presentation::result_layout)
            }
        }
        .measure(context, build)
    }));
    Some(fidget::interactive_volume(drawing, input))
}
