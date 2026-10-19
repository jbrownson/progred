//! Async implicit surfaces combined with mesh paths and cutters.

use super::playback;
use super::{Error, argument, invalid, number, runtime_result, vocabulary::*};
use crate::computations::memo;
use crate::display::host::{Host, Layout, ProjectionInput};
use crate::libraries::{color, f64, fidget, layout, presentation};
use ::grap::{Context, Environment, Expression, Halt, RuntimeValue};
use cam::field::read_radius;
use cam::paths::*;
use gid::Value;
use std::rc::Rc;
use std::sync::Arc;

pub(super) mod computation;

/// Overlay only: progress never changes the viewport's size or input handling.
pub(super) fn progress_bar<W: Host>(
    drawing: Layout<W>,
    progress: Option<incremental::background::Progress>,
) -> Layout<W> {
    crate::display::widget::after(
        drawing,
        Rc::new(move |context| {
            let height = 3.0 * context.viewing().scale();
            let bar = puri_widgets::progress::ProgressBar {
                fraction: progress.map_or(0.0, |p| p.fraction()),
                track: peniko::Color::from_rgba8(196, 204, 214, 220),
                fill: peniko::Color::from_rgb8(48, 126, 210),
            };
            Box::new(move |output, placement| {
                if !placement.clipped_out() {
                    let rect = kurbo::Rect::new(
                        placement.rect.x0,
                        placement.rect.y0,
                        placement.rect.x1,
                        (placement.rect.y0 + height).min(placement.rect.y1),
                    );
                    output.render(move |canvas, _| bar.draw(canvas, rect));
                }
            })
        }),
    )
}

pub(super) fn preview(
    context: &mut Context,
    call: &Expression,
    environment: &Environment,
) -> Result<RuntimeValue, Halt> {
    runtime_result(preview_with(
        context,
        call,
        environment,
        PREVIEW_3D,
        fidget::preview_3d_function,
    ))
}

pub(super) fn preview_with(
    context: &mut Context,
    call: &Expression,
    environment: &Environment,
    marker: gid::CellId,
    model: impl FnOnce(&mut Context, &Expression, &Environment) -> Result<Value, Halt>,
) -> Result<RuntimeValue, Error> {
    let playback = match context.field(call, PLAYBACK) {
        Some(expression) => {
            let value = context.eval_to_value(expression, environment)?;
            if ::grap::absent::is_absent(&value) {
                return Ok(value.into());
            }
            playback::Settings::read(&value).ok_or_else(invalid)?;
            Some((PLAYBACK, value.into()))
        }
        None => None,
    };
    let program = argument(context, call, PROGRAM)?;
    let program = context.eval(program, environment)?;
    if program.is_absent() {
        return Ok(program);
    }
    let radius = number(context, call, environment, LINE_RADIUS)?;
    read_radius(radius).ok_or_else(invalid)?;
    let color = argument(context, call, fidget::vocabulary::COLOR)?;
    let color = context.eval_to_value(color, environment)?;
    color::read_opaque(&color).ok_or_else(|| {
        Error::Invalid(::grap::absent::value(fidget::vocabulary::INVALID_COLOR).into())
    })?;
    let fuel = super::fuel(context, call, environment)?;
    let model = model(context, call, environment)?;
    if ::grap::absent::is_absent(&model) {
        return Ok(model.into());
    }
    Ok(RuntimeValue::record([(
        marker,
        RuntimeValue::record(
            [
                (presentation::vocabulary::VALUE, model.into()),
                (PROGRAM, program),
                (LINE_RADIUS, RuntimeValue::f64(radius)),
                (fidget::vocabulary::COLOR, color.into()),
                (layout::vocabulary::FUEL, RuntimeValue::f64(fuel as f64)),
            ]
            .into_iter()
            .chain(playback),
        ),
    )]))
}

pub(super) fn display<W: Host>(input: &ProjectionInput<'_, W, RuntimeValue>) -> Option<Layout<W>> {
    let fields = input.value?.field(PREVIEW_3D)?;
    let model = fidget::volume_preview(fields.field(presentation::vocabulary::VALUE)?.as_value())?;
    let program = fields.field(PROGRAM)?;
    let radius = fields.field(LINE_RADIUS)?.as_f64()?;
    read_radius(radius)?;
    let color = color::read_opaque(fields.field(fidget::vocabulary::COLOR)?.as_value())?;
    let fuel = fields.field(layout::vocabulary::FUEL)?.as_f64()?;
    let fuel = ::grap::f64::to_usize(fuel)?;
    let request = fidget::raster::Request::new(model.clone(), input.state, input.scale_factor)?;
    let settings = computation::Settings {
        request,
        style: super::mesh::computation::PathStyle {
            radius,
            color,
            playback: match fields.field(PLAYBACK) {
                Some(value) => Some(playback::Settings::read(value.as_value())?),
                None => None,
            },
        },
    };
    let state = input.state.cloned();
    let scale = input.scale_factor;
    let drawing = Layout::program(Rc::new(move |context, build| {
        let computations = context.viewing().computations();
        let site = context.site();
        let interaction = fidget::interaction::Interaction::at(computations, &site);
        let computation = site.memo(computations, || {
            computation::Computation::new(
                computations,
                program.clone(),
                fuel,
                settings.clone(),
                interaction.permitted.clone(),
            )
        });
        computation
            .program
            .set_by(program.clone(), RuntimeValue::same_result);
        computation.fuel.set(fuel);
        computation.settings.set(settings.clone());
        let image = computations.runtime.read(&computation.image);
        let paths = computations.runtime.read(&computation.paths);
        let result = image
            .as_ref()
            .map_err(|error| memo::failure(*error))
            .and_then(|image| image.as_ref().as_ref().map_err(Clone::clone))
            .and_then(|image| {
                let paths = paths
                    .as_ref()
                    .map_err(|e| memo::failure(*e))?
                    .as_ref()
                    .as_ref()
                    .map_err(Clone::clone)?;
                let surface = image.image.as_ref().filter(|_| !image.stale).map(|frame| {
                    puri::mesh::Surface {
                        frame: frame.clone(),
                        mesh_start: paths.indices.len(),
                    }
                });
                let drawing =
                    fidget::mesh::drawing_surface(paths, surface, &model, state.as_ref(), scale)
                        .ok_or_else(|| ::grap::absent::value(fidget::vocabulary::INVALID_FIELD))?;
                Ok((image, drawing))
            });
        match result {
            Ok((image, drawing)) => {
                let drawing = if image.pending {
                    progress_bar(drawing, image.progress)
                } else {
                    drawing
                };
                drawing.measure(context, build)
            }
            Err(value) => {
                crate::display::at([gid::Step::Key(presentation::vocabulary::RESULT)], &value)
                    .measure(context, build)
            }
        }
    }));
    Some(fidget::interactive_volume(drawing, input))
}

#[cfg(test)]
pub(super) mod tests;
