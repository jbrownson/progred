//! Async implicit surfaces combined with mesh paths and cutters.

#[cfg(test)]
use super::cutter::{SectionKind, Tool};
use super::playback;
#[cfg(test)]
use super::playback::Draw;
use super::{Error, argument, invalid, number, paths::*, result, vocabulary::*};
use crate::display::{Layout, ProjectionInput};
use crate::libraries::{absent, color, f64, fidget, layout, presentation};
use ::grap::{Context, Environment, Expression, Halt};
use fidget_engine::context::Tree;
use gid::Value;
use std::rc::Rc;
use std::sync::Arc;

pub(super) mod computation;

/// Overlay only: progress never changes the viewport's size or input handling.
pub(super) fn progress_bar(
    drawing: Layout<crate::Editor, crate::frame::Hovered>,
    progress: Option<incremental::background::Progress>,
) -> Layout<crate::Editor, crate::frame::Hovered> {
    crate::display::widget::after(
        drawing,
        Rc::new(move |context| {
            let height = 3.0 * context.inputs.styles.scale;
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

// Retained only as the all-implicit benchmark reference.
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

pub(super) fn read_radius(radius: f64) -> Option<f32> {
    let radius = radius as f32;
    (radius > 0.0 && (radius * radius).is_finite() && radius * radius > 0.0).then_some(radius)
}

pub(super) fn coordinate(point: Point3) -> Result<[f32; 3], InvalidPath> {
    if !point.into_iter().all(f64::is_finite) {
        return Err(InvalidPath::NonFinitePoint);
    }
    let point = point.map(|n| n as f32);
    point
        .into_iter()
        .all(f32::is_finite)
        .then_some(point)
        .ok_or(InvalidPath::CoordinateRange)
}

pub(super) fn capsule(a: [f32; 3], b: [f32; 3], radius: f32) -> Result<Tree, InvalidPath> {
    let direction = std::array::from_fn::<_, 3, _>(|i| b[i] - a[i]);
    let length_squared: f32 = direction.iter().map(|n| n * n).sum();
    if !length_squared.is_finite() {
        return Err(InvalidPath::CoordinateRange);
    }
    let p = [Tree::x() - a[0], Tree::y() - a[1], Tree::z() - a[2]];
    let t = if length_squared == 0.0 {
        Tree::constant(0.0)
    } else {
        ((p[0].clone() * direction[0] + p[1].clone() * direction[1] + p[2].clone() * direction[2])
            / length_squared)
            .max(0.0)
            .min(1.0)
    };
    let q = std::array::from_fn::<_, 3, _>(|i| p[i].clone() - t.clone() * direction[i]);
    Ok(q[0].square() + q[1].square() + q[2].square() - radius * radius)
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
    call: Expression,
    environment: &Environment,
) -> Result<Value, Halt> {
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
    call: Expression,
    environment: &Environment,
    marker: gid::CellId,
    model: impl FnOnce(&mut Context, Expression, &Environment) -> Result<Value, Halt>,
) -> Result<Value, Halt> {
    result((|| {
        let playback = match context.field(call, PLAYBACK) {
            Some(expression) => {
                let value = context.eval(expression, environment)?;
                if absent::is_absent(&value) {
                    return Ok(value);
                }
                playback::Settings::read(&value).ok_or_else(invalid)?;
                Some((PLAYBACK, value))
            }
            None => None,
        };
        let program = argument(context, call, PROGRAM)?;
        let program = context.eval(program, environment)?;
        if absent::is_absent(&program) {
            return Ok(program);
        }
        let radius = number(context, call, environment, LINE_RADIUS)?;
        read_radius(radius).ok_or_else(invalid)?;
        let color = argument(context, call, fidget::vocabulary::COLOR)?;
        let color = context.eval(color, environment)?;
        read_color(&color).ok_or_else(|| {
            Error::Invalid(absent::with_reason(fidget::vocabulary::INVALID_COLOR))
        })?;
        let fuel = super::fuel(context, call, environment)?;
        let model = model(context, call, environment)?;
        if absent::is_absent(&model) {
            return Ok(model);
        }
        Ok(Value::record([(
            marker,
            Value::record(
                [
                    (presentation::vocabulary::VALUE, model),
                    (PROGRAM, program),
                    (LINE_RADIUS, f64::value(radius)),
                    (fidget::vocabulary::COLOR, color),
                    (layout::vocabulary::FUEL, f64::value(fuel as f64)),
                ]
                .into_iter()
                .chain(playback),
            ),
        )]))
    })())
}

pub(super) fn display(
    input: &ProjectionInput<'_, crate::Editor, crate::frame::Hovered>,
    renderer: &Rc<std::cell::RefCell<fidget::mesh::Renderer>>,
) -> Option<Layout<crate::Editor, crate::frame::Hovered>> {
    let fields = input.value?.as_record()?.get(&PREVIEW_3D)?.as_record()?;
    let model = fidget::volume_preview(fields.get(&presentation::vocabulary::VALUE)?)?;
    let program = fields.get(&PROGRAM)?.clone();
    let radius = f64::read(fields.get(&LINE_RADIUS)?)?;
    read_radius(radius)?;
    let color = read_color(fields.get(&fidget::vocabulary::COLOR)?)?;
    let fuel = f64::read(fields.get(&layout::vocabulary::FUEL)?)?;
    let fuel = super::read_fuel(fuel)?;
    let request = fidget::raster::Request::new(model.clone(), input.state, input.scale_factor)?;
    let size = request.size();
    let settings = computation::Settings {
        request,
        radius,
        color,
        playback: match fields.get(&PLAYBACK) {
            Some(value) => Some(playback::Settings::read(value)?),
            None => None,
        },
    };
    let renderer = renderer.clone();
    let state = input.state.cloned();
    let scale = input.scale_factor;
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
            computation::Computation::new(computations, program.clone(), fuel, settings.clone())
        });
        computation.program.set(program.clone());
        computation.fuel.set(fuel);
        computation.settings.set(settings.clone());
        let image = computations.runtime.read(&computation.image);
        let paths = computations.runtime.read(&computation.paths);
        let result = image
            .as_ref()
            .map_err(|error| ::grap::memo::failure(*error))
            .and_then(|image| image.as_ref().as_ref().map_err(Clone::clone))
            .and_then(|image| {
                let paths = paths
                    .as_ref()
                    .map_err(|e| ::grap::memo::failure(*e))?
                    .as_ref()
                    .as_ref()
                    .map_err(Clone::clone)?;
                let surface = image.image.as_ref().filter(|_| !image.stale).map(|frame| {
                    fidget::mesh::Surface {
                        frame,
                        mesh_start: paths.indices.len(),
                    }
                });
                let pixels = fidget::mesh::raster_surface(
                    paths,
                    surface,
                    &model,
                    state.as_ref(),
                    scale,
                    &mut renderer.borrow_mut(),
                )
                .ok_or_else(|| absent::with_reason(fidget::vocabulary::INVALID_FIELD))?;
                Ok((image, pixels))
            });
        match result {
            Ok((image, pixels)) => {
                let drawing = fidget::image_from_data(size, pixels, false);
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
mod tests {
    use super::*;
    use fidget_engine::{shape::EzShape, vm::VmShape};

    fn sample(tree: Tree, point: [f32; 3]) -> f32 {
        let shape = VmShape::from(tree);
        let mut evaluator = VmShape::new_float_slice_eval();
        let tape = shape.ez_float_slice_tape();
        evaluator
            .eval(&tape, &[point[0]], &[point[1]], &[point[2]])
            .unwrap()[0]
    }

    #[test]
    fn capsule_has_round_ends_and_the_requested_radius() {
        let tree = capsule([0.0; 3], [1.0, 1.0, 1.0], 0.1).unwrap();
        assert!(sample(tree.clone(), [0.5; 3]) < 0.0);
        assert!(sample(tree.clone(), [0.5, 0.7, 0.5]) > 0.0);
        let end = 1.0 + 0.1 / 3.0_f32.sqrt();
        assert!(sample(tree, [end; 3]).abs() < 1e-6);
        let tree = capsule([0.0; 3], [1.0, 0.0, 0.0], 0.1).unwrap();
        assert!(sample(tree.clone(), [0.5, 0.1, 0.0]).abs() < 1e-6);
        assert!(sample(tree, [-0.1, 0.0, 0.0]).abs() < 1e-6);
    }

    #[test]
    fn tubes_preserve_breaks_and_do_not_draw_isolated_starts() {
        let mut tubes = Tubes::new(0.1).unwrap();
        tubes.start_at([100.0; 3], Axis::Z).unwrap();
        assert!(tubes.path.is_none());
        assert!(tubes.paths.is_empty());
        tubes.start_at([0.0; 3], Axis::Z).unwrap();
        tubes.line_to([1.0, 0.0, 0.0]).unwrap();
        tubes.line_to([2.0, 0.0, 0.0]).unwrap();
        tubes.start_at([0.0, 2.0, 0.0], Axis::Z).unwrap();
        tubes.line_to([1.0, 2.0, 0.0]).unwrap();
        tubes.start_at([100.0; 3], Axis::Z).unwrap();
        let paths = tubes
            .scene()
            .into_iter()
            .map(|object| object.tree)
            .collect::<Vec<_>>();
        assert_eq!(paths.len(), 2);
        assert!(sample(paths[0].clone(), [0.5, 0.0, 0.0]) < 0.0);
        assert!(sample(paths[0].clone(), [1.5, 0.0, 0.0]) < 0.0);
        assert!(sample(paths[0].clone(), [0.5, 2.0, 0.0]) > 0.0);
        assert!(sample(paths[1].clone(), [0.5, 2.0, 0.0]) < 0.0);
        for path in paths {
            assert!(sample(path, [0.5, 1.0, 0.0]) > 0.0, "no connecting move");
        }
    }

    #[test]
    fn duplicate_points_are_spheres_and_invalid_emissions_leave_the_sink_unchanged() {
        let mut tubes = Tubes::new(0.1).unwrap();
        assert_eq!(tubes.line_to([0.0; 3]), Err(InvalidPath::MissingStart));
        tubes.start_at([0.0; 3], Axis::Z).unwrap();
        tubes.line_to([0.0; 3]).unwrap();
        assert_eq!(
            tubes.start_at([f64::NAN; 3], Axis::Z),
            Err(InvalidPath::NonFinitePoint)
        );
        assert_eq!(
            tubes.line_to([f64::MAX; 3]),
            Err(InvalidPath::CoordinateRange)
        );
        tubes.line_to([0.0; 3]).unwrap();
        let paths = tubes
            .scene()
            .into_iter()
            .map(|object| object.tree)
            .collect::<Vec<_>>();
        assert_eq!(paths.len(), 1);
        assert!(sample(paths[0].clone(), [0.1, 0.0, 0.0]).abs() < 1e-6);
        for radius in [
            0.0,
            -1.0,
            f64::INFINITY,
            f64::NAN,
            f64::MAX,
            f64::MIN_POSITIVE,
        ] {
            assert!(Tubes::new(radius).is_none());
        }
    }

    #[test]
    fn direct_and_recorded_paths_produce_the_same_fields() {
        fn draw(sink: &mut impl Sink<Error = InvalidPath>) {
            sink.start_at([0.0; 3], Axis::Z).unwrap();
            sink.line_to([1.0, 0.5, 0.1]).unwrap();
            sink.start_at([-1.0, 0.0, 0.5], Axis::Z).unwrap();
            sink.line_to([0.0, 0.5, 0.2]).unwrap();
        }
        let mut direct = Tubes::new(0.1).unwrap();
        draw(&mut direct);
        let mut recording = Recording::default();
        draw(&mut recording);
        let mut replayed = Tubes::new(0.1).unwrap();
        recording.replay(&mut replayed).unwrap();
        assert!(direct.scene() == replayed.scene());
    }
}
