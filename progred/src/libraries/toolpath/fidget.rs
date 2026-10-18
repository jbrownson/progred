//! Async implicit surfaces combined with mesh paths and cutters.

#[cfg(test)]
use super::cutter::{SectionKind, Tool};
use super::playback;
#[cfg(test)]
use super::playback::Draw;
use super::{Error, argument, invalid, number, paths::*, runtime_result, vocabulary::*};
use crate::computations::memo;
use crate::display::host::{Host, Layout, ProjectionInput};
use crate::libraries::{absent, color, f64, fidget, layout, presentation};
use ::grap::{Context, Environment, Expression, Halt, RuntimeValue};
use cam::field::read_radius;
#[cfg(test)]
use cam::field::{capsule, coordinate};
#[cfg(test)]
use fidget_engine::context::Tree;
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
                track: puri::Color::from_rgba8(196, 204, 214, 220),
                fill: puri::Color::from_rgb8(48, 126, 210),
            };
            Box::new(move |output, placement| {
                if !placement.clipped_out() {
                    let rect = puri::Rect::new(
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

// The all-implicit path field: the benchmark reference, and the subject of a
// regression test that it compiles without GPU memory operations.
#[cfg(test)]
pub(super) struct Tubes {
    radius: f32,
    previous: Option<[f32; 3]>,
    path: Option<Tree>,
    paths: Vec<Tree>,
    color: [u8; 3],
    objects: Vec<fidget::SceneObject>,
}

#[cfg(test)]
impl Tubes {
    pub(super) fn new(radius: f64) -> Option<Self> {
        Some(Self {
            radius: read_radius(radius)?,
            previous: None,
            path: None,
            paths: Vec::new(),
            color: [255; 3],
            objects: Vec::new(),
        })
    }

    fn flush(&mut self) {
        self.paths.extend(self.path.take());
        self.objects
            .extend(self.paths.drain(..).map(|tree| fidget::SceneObject {
                tree,
                color: self.color,
            }));
    }

    pub(super) fn scene(mut self) -> Vec<fidget::SceneObject> {
        self.flush();
        self.objects
    }
}

#[cfg(test)]
impl Draw for Tubes {
    fn style(&mut self, radius: f64, color: [u8; 3]) -> Result<(), InvalidPath> {
        let radius = read_radius(radius).ok_or(InvalidPath::CoordinateRange)?;
        if self.radius != radius || self.color != color {
            self.flush();
            self.radius = radius;
            self.color = color;
        }
        Ok(())
    }

    fn tool(
        &mut self,
        tool: &Tool,
        pose: Pose,
        color: [u8; 3],
        tolerance: f64,
    ) -> Result<(), InvalidPath> {
        self.flush();
        for section in &tool.sections {
            let field = section.sweep(tolerance, pose.tip, pose.tip, pose.axis)?;
            let color = match section.kind {
                SectionKind::Cutting => color,
                SectionKind::NonCutting => [122, 138, 153],
            };
            self.objects
                .push(fidget::SceneObject { tree: field, color });
        }
        Ok(())
    }
}

#[cfg(test)]
impl Sink for Tubes {
    type Error = InvalidPath;

    fn end_path(&mut self) {
        self.paths.extend(self.path.take());
        self.previous = None;
    }

    fn start_at(&mut self, point: Point3, _: Axis) -> Result<(), Self::Error> {
        let point = coordinate(point)?;
        self.paths.extend(self.path.take());
        self.previous = Some(point);
        Ok(())
    }

    fn line_to(&mut self, point: Point3) -> Result<(), Self::Error> {
        let previous = self.previous.ok_or(InvalidPath::MissingStart)?;
        let point = coordinate(point)?;
        let tube = capsule(previous, point, self.radius)?;
        self.path = Some(match self.path.take() {
            Some(field) => field.min(tube),
            None => tube,
        });
        self.previous = Some(point);
        Ok(())
    }
}

pub(super) fn read_color(value: &Value) -> Option<[u8; 3]> {
    let rgba = color::read(value)?.to_rgba8();
    (rgba.a == 255).then_some([rgba.r, rgba.g, rgba.b])
}

pub(super) fn preview(
    context: &mut Context,
    call: &Expression,
    environment: &Environment,
) -> Result<RuntimeValue, Halt> {
    preview_with(
        context,
        call,
        environment,
        PREVIEW_3D,
        fidget::preview_3d_function,
    )
}

pub(super) fn preview_with(
    context: &mut Context,
    call: &Expression,
    environment: &Environment,
    marker: gid::CellId,
    model: impl FnOnce(&mut Context, &Expression, &Environment) -> Result<Value, Halt>,
) -> Result<RuntimeValue, Halt> {
    runtime_result((|| {
        let playback = match context.field(call, PLAYBACK) {
            Some(expression) => {
                let value = context.eval_to_value(expression, environment)?;
                if absent::is_absent(&value) {
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
        read_color(&color).ok_or_else(|| {
            Error::Invalid(absent::with_reason(fidget::vocabulary::INVALID_COLOR).into())
        })?;
        let fuel = super::fuel(context, call, environment)?;
        let model = model(context, call, environment)?;
        if absent::is_absent(&model) {
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
    })())
}

pub(super) fn display<W: Host>(input: &ProjectionInput<'_, W, RuntimeValue>) -> Option<Layout<W>> {
    let fields = input.value?.field(PREVIEW_3D)?;
    let model = fidget::volume_preview(fields.field(presentation::vocabulary::VALUE)?.as_value())?;
    let program = fields.field(PROGRAM)?;
    let radius = fields.field(LINE_RADIUS)?.as_f64()?;
    read_radius(radius)?;
    let color = read_color(fields.field(fidget::vocabulary::COLOR)?.as_value())?;
    let fuel = fields.field(layout::vocabulary::FUEL)?.as_f64()?;
    let fuel = super::read_fuel(fuel)?;
    let request = fidget::raster::Request::new(model.clone(), input.state, input.scale_factor)?;
    let settings = computation::Settings {
        request,
        radius,
        color,
        playback: match fields.field(PLAYBACK) {
            Some(value) => Some(playback::Settings::read(value.as_value())?),
            None => None,
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
                    fidget::mesh::Surface {
                        frame: frame.clone(),
                        mesh_start: paths.indices.len(),
                    }
                });
                let drawing =
                    fidget::mesh::drawing_surface(paths, surface, &model, state.as_ref(), scale)
                        .ok_or_else(|| absent::with_reason(fidget::vocabulary::INVALID_FIELD))?;
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
mod tests;
