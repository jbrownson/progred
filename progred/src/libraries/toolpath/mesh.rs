//! Stream path segments into triangles alongside a meshed Fidget model.

use super::{fidget::coordinate, paths::*, run, vocabulary::*};
use crate::display::{Layout, ProjectionInput};
use crate::libraries::{absent, f64, fidget, layout, presentation};
use fidget::mesh::{Geometry, Vertex};
use gid::Value;
use nalgebra::Vector3;
use std::{cell::RefCell, rc::Rc};

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
    let drawing = Layout::program(Rc::new(move |context, build| {
        let mut tubes = tubes::Tubes::new(radius, color).unwrap();
        let mut recording = Recording::default();
        let sink: &mut dyn Sink<Error = InvalidPath> = if playback.is_some() {
            &mut recording
        } else {
            &mut tubes
        };
        let evaluation = run(sink, |scope| {
            ::grap::apply_scoped(&program, [], &context.inputs.sources, scope, fuel)
        });
        if !evaluation.completed || absent::is_absent(&evaluation.result) {
            return context.project.transient(
                context.text,
                build,
                evaluation.result,
                evaluation.remaining_fuel,
            );
        }
        let mut scene = model.clone();
        if let Some(settings) = &playback {
            match settings.draw(&recording, &mut tubes, radius, color) {
                Ok(Some(stock)) => scene.objects = vec![stock],
                Ok(None) => {}
                Err(_) => {
                    return context.project.transient(
                        context.text,
                        build,
                        absent::with_reason(INVALID_INPUT),
                        evaluation.remaining_fuel,
                    );
                }
            }
        }
        let drawing = (|| {
            fidget::mesh::append(&mut tubes.geometry, &scene, depth)?;
            fidget::mesh::image(
                &tubes.geometry,
                &model,
                state.as_ref(),
                scale,
                &mut renderer.borrow_mut(),
            )
        })();
        match drawing {
            Some(drawing) => drawing.measure(context, build),
            None => context.project.transient(
                context.text,
                build,
                absent::with_reason(fidget::vocabulary::INVALID_FIELD),
                evaluation.remaining_fuel,
            ),
        }
    }));
    Some(fidget::interactive_volume(drawing, input))
}
