//! Stream path segments into triangles alongside a meshed Fidget model.

use super::{fidget::coordinate, paths::*, playback, vocabulary::*};
use crate::display::{Layout, ProjectionInput};
use crate::libraries::{absent, f64, fidget, layout, presentation};
use fidget::mesh::{Geometry, Vertex};
use gid::Value;
use nalgebra::Vector3;
use std::{cell::RefCell, rc::Rc};

pub(super) mod computation;
#[cfg(test)]
mod tests;
mod tubes;

pub(super) fn preview(
    context: &mut ::grap::Context,
    call: ::grap::Expression,
    environment: &::grap::Environment,
) -> Result<Value, ::grap::Halt> {
    super::fidget::preview_with(
        context,
        call,
        environment,
        PREVIEW_MESH,
        fidget::mesh::preview,
    )
}

pub(super) fn display(
    input: &ProjectionInput<'_, crate::Editor, crate::frame::Hovered>,
    renderer: &Rc<RefCell<fidget::mesh::Renderer>>,
) -> Option<Layout<crate::Editor, crate::frame::Hovered>> {
    let fields = input.value?.as_record()?.get(&PREVIEW_MESH)?.as_record()?;
    let (model, depth) = fidget::mesh::read(fields.get(&presentation::vocabulary::VALUE)?)?;
    let program = fields.get(&PROGRAM)?.clone();
    let radius = f64::read(fields.get(&LINE_RADIUS)?)?;
    let color = super::fidget::read_color(fields.get(&fidget::vocabulary::COLOR)?)?;
    super::fidget::read_radius(radius)?;
    let fuel = super::read_fuel(f64::read(fields.get(&layout::vocabulary::FUEL)?)?)?;
    let playback = match fields.get(&PLAYBACK) {
        Some(value) => Some(playback::Settings::read(value)?),
        None => None,
    };
    let state = input.state.cloned();
    let scale = input.scale_factor;
    let renderer = renderer.clone();
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
        computation.program.set(program.clone());
        computation.fuel.set(fuel);
        computation.settings.set(settings.clone());
        computation.depth.set(depth);
        let geometry = computations.runtime.read(&computation.geometry);
        let result = geometry
            .as_ref()
            .map_err(|error| (::grap::memo::failure(*error), fuel))
            .and_then(|geometry| geometry.as_ref().as_ref().map_err(Clone::clone));
        let drawing = result.and_then(|(geometry, fuel)| {
            fidget::mesh::image(
                &geometry.geometry,
                &model,
                state.as_ref(),
                scale,
                &mut renderer.borrow_mut(),
            )
            .map(|drawing| {
                if geometry.awaiting_first_surface {
                    crate::display::overlay([drawing, crate::display::dim("…")])
                } else {
                    drawing
                }
            })
            .ok_or_else(|| {
                (
                    absent::with_reason(fidget::vocabulary::INVALID_FIELD),
                    *fuel,
                )
            })
        });
        match drawing {
            Ok(drawing) => drawing.measure(context, build),
            Err((value, fuel)) => context.project.transient(context.text, build, value, fuel),
        }
    }));
    Some(fidget::interactive_volume(drawing, input))
}
