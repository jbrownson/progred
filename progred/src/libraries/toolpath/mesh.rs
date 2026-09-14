//! Stream path segments into triangles alongside a meshed Fidget model.

use super::{fidget::coordinate, paths::*, run, vocabulary::*};
use crate::display::{Layout, ProjectionInput};
use crate::libraries::{absent, f64, fidget, layout, presentation};
use fidget::mesh::{Geometry, Vertex};
use gid::Value;
use nalgebra::Vector3;
use std::{cell::RefCell, rc::Rc};

mod computation;
mod playback;
#[cfg(test)]
mod tests;
mod tubes;

pub(super) fn preview(
    context: &mut ::grap::Context,
    call: ::grap::Expression,
    environment: &::grap::Environment,
) -> Result<Value, ::grap::Halt> {
    let playback = match context.field(call, PLAYBACK) {
        Some(expression) => {
            let value = context.eval(expression, environment)?;
            if absent::is_absent(&value) {
                return Ok(value);
            }
            if playback::Settings::read(&value).is_none() {
                return Ok(absent::with_reason(INVALID_INPUT));
            }
            Some(value)
        }
        None => None,
    };
    let value = super::fidget::preview_with(
        context,
        call,
        environment,
        PREVIEW_MESH,
        fidget::mesh::preview,
    )?;
    Ok(
        match (
            playback,
            value
                .as_record()
                .and_then(|r| r.get(&PREVIEW_MESH))
                .and_then(Value::as_record),
        ) {
            (Some(playback), Some(fields)) => Value::record([(
                PREVIEW_MESH,
                Value::record(
                    fields
                        .iter()
                        .map(|(k, v)| (*k, v.clone()))
                        .chain([(PLAYBACK, playback)]),
                ),
            )]),
            _ => value,
        },
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
                geometry,
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
            })
        });
        match drawing {
            Ok(drawing) => drawing.measure(context, build),
            Err((value, fuel)) => context.project.transient(context.text, build, value, fuel),
        }
    }));
    Some(fidget::interactive_volume(drawing, input))
}
