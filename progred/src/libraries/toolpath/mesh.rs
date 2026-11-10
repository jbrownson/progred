//! Stream path segments into triangles alongside a meshed Fidget model.

use super::{computation::outcome, playback, vocabulary::*};
use crate::display::host::{Host, Layout, ProjectionInput};
use crate::libraries::{f64, fidget, presentation};
use ::grap::RuntimeValue;
use cam::field::coordinate;
use cam::paths::*;
use nalgebra::Vector3;
use puri::mesh::{Geometry, Mesh, Normal, Vertex};

pub(super) mod computation;
#[cfg(test)]
#[path = "tests/mesh.rs"]
mod tests;
mod tubes;

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
        shape: model.shape.clone(),
        style,
    };
    Some(super::path_preview_layout(
        input,
        model,
        move |context, tasks, model| {
            let computations = context.viewing().computations();
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
            let geometry = outcome(&geometry)?;
            super::drawn(fidget::mesh::drawing(
                &geometry.geometry,
                model,
                state.as_ref(),
                scale,
            ))
            .map(|drawing| {
                if geometry.awaiting_first_surface {
                    crate::display::overlay([drawing, crate::display::dim("…")])
                } else {
                    drawing
                }
            })
        },
    ))
}
